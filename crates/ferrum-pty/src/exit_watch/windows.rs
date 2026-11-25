//! Windows exit watcher (maintainer decision Q-33): named `pty-exit` threads wait with
//! `WaitForMultipleObjects`, one thread per [`HANDLES_PER_THREAD`] children. Slot 0 of every
//! wait set is a manual-reset wake event: `watch` and `unwatch` send the change on the thread's
//! bounded channel and signal the event, so the thread rebuilds its set. An exit is recorded and
//! the notification of [`ExitWatch::set_notify`] is called from the `pty-exit` thread, never from
//! the caller's. No OS thread pool.

#![allow(unsafe_code, reason = "docs/05 §4: the handle and wait calls, each with SAFETY")]

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

use windows_sys::Win32::Foundation::{
    CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE, WAIT_OBJECT_0,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, INFINITE, ResetEvent, SetEvent, WaitForMultipleObjects,
};

use super::ExitWatch;
use crate::{ExitSource, Notify, PtyError, Readiness};

/// `MAXIMUM_WAIT_OBJECTS` (64) minus slot 0, the wake event.
const HANDLES_PER_THREAD: usize = 63;
/// Changes waiting for one thread: a burst of watches and unwatches of its 63 children.
const CHANGES_QUEUE: usize = 128;
/// Stack of a `pty-exit` thread: it only waits and copies 64 handles.
const STACK_BYTES: usize = 64 * 1_024;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Exits recorded, and the watch generation of each watched key: an exit is recorded only for
/// the generation still watched, so a child that was unwatched (or replaced under the same key)
/// is never reported.
#[derive(Default)]
struct Exits {
    keys: Vec<u64>,
    current: HashMap<u64, u64>,
}

/// What the threads share with the watcher.
#[derive(Default)]
struct Shared {
    exits: Mutex<Exits>,
    notify: Mutex<Option<Notify>>,
}

/// An owned kernel handle (a duplicated process handle or the wake event).
struct Owned(HANDLE);

// SAFETY: a kernel handle is a process-wide value; it may be used and closed from any thread.
unsafe impl Send for Owned {}

// SAFETY: the shared value is only passed to thread-safe OS calls (`SetEvent`, waits); closing
// happens once, in `drop`, when no other reference is left.
unsafe impl Sync for Owned {}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the handle is owned by this value and closed once, here.
        unsafe { CloseHandle(self.0) };
    }
}

/// A change for a `pty-exit` thread.
enum Change {
    Watch(Watched),
    Unwatch(u64),
    Stop,
}

/// A watched child: its key, watch generation and process handle.
struct Watched {
    key: u64,
    generation: u64,
    handle: Owned,
}

/// One `pty-exit` thread and its channel.
struct Group {
    changes: SyncSender<Change>,
    wake: Arc<Owned>,
    count: usize,
    thread: Option<JoinHandle<()>>,
}

impl Group {
    fn start(index: usize, shared: &Arc<Shared>) -> Result<Self, PtyError> {
        // SAFETY: default security, manual reset, not signaled, no name; checked below.
        let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if event.is_null() {
            return Err(PtyError::io(
                "create the exit wake event",
                std::io::Error::last_os_error(),
            ));
        }
        let wake = Arc::new(Owned(event));
        let (changes, inbox) = sync_channel(CHANGES_QUEUE);
        let name = if index == 0 { "pty-exit".to_owned() } else { format!("pty-exit-{index}") };
        let (thread_wake, thread_shared) = (Arc::clone(&wake), Arc::clone(shared));
        let thread = std::thread::Builder::new()
            .name(name)
            .stack_size(STACK_BYTES)
            .spawn(move || serve(&thread_wake, &inbox, &thread_shared))
            .map_err(|error| PtyError::io("start the exit watcher thread", error))?;
        Ok(Self { changes, wake, count: 0, thread: Some(thread) })
    }

    /// Sends `change` and wakes the thread to rebuild its wait set.
    fn send(&self, change: Change) {
        // A closed channel means the thread is gone; nothing is left to watch then.
        let _sent = self.changes.send(change);
        // SAFETY: the event is alive (owned by `wake`).
        unsafe { SetEvent(self.wake.0) };
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        self.send(Change::Stop);
        if let Some(thread) = self.thread.take() {
            let _joined = thread.join(); // the thread only waits; it ends at `Stop`
        }
    }
}

/// The loop of a `pty-exit` thread.
fn serve(wake: &Owned, inbox: &Receiver<Change>, shared: &Shared) {
    let mut watched: Vec<Watched> = Vec::new();
    let mut slots: Vec<HANDLE> = Vec::with_capacity(HANDLES_PER_THREAD + 1);
    loop {
        // Reset before reading the changes: a change sent after this signals again.
        // SAFETY: the event is alive for the thread's life.
        unsafe { ResetEvent(wake.0) };
        if !apply_changes(inbox, &mut watched) {
            return;
        }
        slots.clear();
        slots.push(wake.0);
        slots.extend(watched.iter().map(|child| child.handle.0));
        let count = u32::try_from(slots.len()).unwrap_or(u32::MAX);
        // SAFETY: `slots` holds `count` valid handles (the event and owned process handles, none
        // closed while waiting: only this thread closes them, between waits).
        let result = unsafe { WaitForMultipleObjects(count, slots.as_ptr(), 0, INFINITE) };
        let index = usize::try_from(result.wrapping_sub(WAIT_OBJECT_0)).unwrap_or(usize::MAX);
        match index.checked_sub(1) {
            Some(exited) if exited < watched.len() => {
                let child = watched.swap_remove(exited);
                report(shared, child.key, child.generation);
            }
            Some(_) => return, // a failed wait: the handles are owned and valid, so the OS failed
            None => {}         // the wake event: rebuild
        }
    }
}

/// Applies the queued changes; false at `Stop`.
fn apply_changes(inbox: &Receiver<Change>, watched: &mut Vec<Watched>) -> bool {
    loop {
        match inbox.try_recv() {
            Ok(Change::Watch(child)) => {
                watched.retain(|known| known.key != child.key);
                watched.push(child);
            }
            Ok(Change::Unwatch(key)) => watched.retain(|known| known.key != key),
            Ok(Change::Stop) | Err(TryRecvError::Disconnected) => return false,
            Err(TryRecvError::Empty) => return true,
        }
    }
}

/// Records an exit of the watch still current and notifies, on the `pty-exit` thread.
fn report(shared: &Shared, key: u64, generation: u64) {
    let mut exits = lock(&shared.exits);
    if exits.current.get(&key) != Some(&generation) {
        return; // unwatched meanwhile
    }
    exits.current.remove(&key);
    exits.keys.push(key);
    drop(exits);
    if let Some(notify) = lock(&shared.notify).as_ref() {
        notify();
    }
}

/// The watcher: its threads and which thread watches which key.
pub(super) struct WaitWatch {
    shared: Arc<Shared>,
    groups: Vec<Group>,
    owner: HashMap<u64, usize>,
    next_generation: u64,
}

impl WaitWatch {
    pub(super) fn new() -> Self {
        Self {
            shared: Arc::new(Shared::default()),
            groups: Vec::new(),
            owner: HashMap::new(),
            next_generation: 0,
        }
    }

    /// The first thread with room, started when every thread is full.
    fn group_with_room(&mut self) -> Result<usize, PtyError> {
        if let Some(index) = self.groups.iter().position(|g| g.count < HANDLES_PER_THREAD) {
            return Ok(index);
        }
        self.groups.push(Group::start(self.groups.len(), &self.shared)?);
        Ok(self.groups.len() - 1)
    }
}

/// A handle of the process behind `process` that the watcher owns (synchronize access).
fn duplicate(process: HANDLE) -> Result<Owned, PtyError> {
    let mut copy: HANDLE = std::ptr::null_mut();
    // SAFETY: `process` is a valid process handle (the caller's contract); `copy` receives a new
    // handle in this process, checked below.
    let done = unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            process,
            GetCurrentProcess(),
            &raw mut copy,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    };
    if done == 0 {
        return Err(PtyError::io("watch a child", std::io::Error::last_os_error()));
    }
    Ok(Owned(copy))
}

impl ExitWatch for WaitWatch {
    fn readiness(&self) -> Readiness {
        Readiness::Notify
    }

    fn set_notify(&mut self, notify: Notify) {
        *lock(&self.shared.notify) = Some(notify);
    }

    fn watch(&mut self, key: u64, source: ExitSource) -> Result<(), PtyError> {
        let ExitSource::Handle(process) = source else {
            return Err(PtyError::InvalidSpec("this exit source cannot be watched"));
        };
        self.unwatch(key);
        let handle = duplicate(process)?;
        let index = self.group_with_room()?;
        let Some(group) = self.groups.get_mut(index) else {
            return Err(PtyError::InvalidSpec("no exit watcher thread"));
        };
        let generation = self.next_generation;
        self.next_generation += 1;
        lock(&self.shared.exits).current.insert(key, generation);
        group.count += 1;
        group.send(Change::Watch(Watched { key, generation, handle }));
        self.owner.insert(key, index);
        Ok(())
    }

    fn unwatch(&mut self, key: u64) {
        if let Some(group) = self.owner.remove(&key).and_then(|index| self.groups.get_mut(index)) {
            group.count -= 1;
            group.send(Change::Unwatch(key));
        }
        // No exit of this watch is reported from now on, recorded or not.
        let mut exits = lock(&self.shared.exits);
        exits.current.remove(&key);
        exits.keys.retain(|&exited| exited != key);
    }

    fn take_exited(&mut self, out: &mut Vec<u64>) -> Result<(), PtyError> {
        let exited = std::mem::take(&mut lock(&self.shared.exits).keys);
        for key in exited {
            if let Some(group) =
                self.owner.remove(&key).and_then(|index| self.groups.get_mut(index))
            {
                group.count -= 1; // the thread already dropped the handle
            }
            out.push(key);
        }
        Ok(())
    }
}
