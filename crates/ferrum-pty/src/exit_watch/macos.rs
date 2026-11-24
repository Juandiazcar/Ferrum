//! macOS exit watcher: one `kqueue` with an `EVFILT_PROC`/`NOTE_EXIT` filter per child.

#![allow(unsafe_code, reason = "docs/05 §4: rustix's kevent is unsafe; SAFETY at each call")]

use std::collections::HashMap;
use std::os::fd::{AsRawFd, OwnedFd};
use std::time::Duration;

use rustix::event::kqueue::{self, Event, EventFilter, EventFlags, ProcessEvents};
use rustix::io::Errno;
use rustix::process::Pid;

use super::ExitWatch;
use crate::{ExitSource, PtyError, Readiness};

/// Exits read from the queue per call; more stay queued for the next call.
const EVENTS_PER_CALL: usize = 32;

pub(super) struct KqueueWatch {
    queue: OwnedFd,
    /// Live filters by tag: the key and the process. Every watch gets a new tag, so an exit queued
    /// for a key that was unwatched and watched again (a restarted session) is never reported
    /// for the new child.
    watched: HashMap<u64, (u64, Pid)>,
    /// The tag of each watched key.
    tags: HashMap<u64, u64>,
    next_tag: u64,
    /// Children that had already exited when watched.
    gone: Vec<u64>,
}

fn os(op: &'static str) -> impl Fn(Errno) -> PtyError {
    move |errno| PtyError::io(op, errno.into())
}

/// The tag travels in the event's user data as an integer, never as a pointer.
#[allow(clippy::cast_possible_truncation, reason = "tags count watches, far below usize::MAX")]
fn udata(tag: u64) -> *mut std::ffi::c_void {
    std::ptr::without_provenance_mut(tag as usize)
}

impl KqueueWatch {
    pub(super) fn new() -> Result<Self, PtyError> {
        let queue = kqueue::kqueue().map_err(os("create the exit watch"))?;
        Ok(Self {
            queue,
            watched: HashMap::new(),
            tags: HashMap::new(),
            next_tag: 0,
            gone: Vec::new(),
        })
    }

    fn change(&self, event: &Event) -> Result<(), Errno> {
        let mut none: [Event; 0] = [];
        // SAFETY: the change refers to a process, not to a descriptor, and its user data is an
        // integer that is never dereferenced, so nothing has to outlive the call.
        unsafe { kqueue::kevent(&self.queue, std::slice::from_ref(event), &mut none, None) }
            .map(drop)
    }
}

impl ExitWatch for KqueueWatch {
    fn readiness(&self) -> Readiness {
        Readiness::Fd(self.queue.as_raw_fd())
    }

    fn watch(&mut self, key: u64, source: ExitSource) -> Result<(), PtyError> {
        let ExitSource::Pid(pid) = source else {
            return Err(PtyError::InvalidSpec("this exit source cannot be watched"));
        };
        let pid = i32::try_from(pid).ok().and_then(Pid::from_raw);
        let pid = pid.ok_or(PtyError::InvalidSpec("invalid process id"))?;
        self.unwatch(key);
        let tag = self.next_tag;
        self.next_tag += 1;
        let filter = EventFilter::Proc { pid, flags: ProcessEvents::EXIT };
        let event = Event::new(filter, EventFlags::ADD | EventFlags::ONESHOT, udata(tag));
        match self.change(&event) {
            Ok(()) => {
                self.watched.insert(tag, (key, pid));
                self.tags.insert(key, tag);
                Ok(())
            }
            // Already exited (a zombie or reaped): report it.
            Err(Errno::SRCH) => {
                self.gone.push(key);
                Ok(())
            }
            Err(errno) => Err(os("watch a child")(errno)),
        }
    }

    fn unwatch(&mut self, key: u64) {
        self.gone.retain(|&gone| gone != key);
        let Some(tag) = self.tags.remove(&key) else { return };
        let Some((_, pid)) = self.watched.remove(&tag) else { return };
        let filter = EventFilter::Proc { pid, flags: ProcessEvents::EXIT };
        // The filter is gone already when the process exited (one-shot): ENOENT is expected; an
        // exit queued before this is dropped by `take_exited` (its tag is no longer watched).
        let _deleted = self.change(&Event::new(filter, EventFlags::DELETE, udata(tag)));
    }

    fn take_exited(&mut self, out: &mut Vec<u64>) -> Result<(), PtyError> {
        out.append(&mut self.gone);
        let mut events = Vec::with_capacity(EVENTS_PER_CALL);
        let spare = rustix::buffer::spare_capacity(&mut events);
        // SAFETY: every registered event refers to a process and carries an integer as user
        // data, so no descriptor or pointer has to stay valid.
        unsafe { kqueue::kevent(&self.queue, &[], spare, Some(Duration::ZERO)) }
            .map_err(os("query the exit watch"))?;
        for event in &events {
            let tag = event.udata().addr() as u64;
            if let Some((key, _)) = self.watched.remove(&tag) {
                self.tags.remove(&key);
                out.push(key);
            }
        }
        Ok(())
    }
}
