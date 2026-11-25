//! The single I/O thread of the `ConPTY` backend (maintainer decision Q-31, `docs/11` §4): one
//! thread, `pty-io`, waits on an I/O completion port (through `mio`) for the overlapped reads of
//! every session's output pipe and tells each session's reader when output is ready. There is no
//! thread per session.
//!
//! A reader marks itself not ready, tries a non-blocking read, and only then waits; the I/O thread
//! marks it ready and calls its notification. So no readiness is ever lost between the two.

use std::collections::HashMap;
use std::io::{ErrorKind, Read};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::thread::{JoinHandle, ThreadId};

use mio::windows::NamedPipe;
use mio::{Events, Interest, Poll, Registry, Token, Waker};

use crate::{Notify, PtyError, PtyReader, Readiness};

/// The token of the waker (mio allows one per poller): it stops the thread, and delivers the
/// notifications [`Driver::renotify`] asked for; session tokens start after it.
const STOP: Token = Token(0);
/// Name of the I/O thread.
const THREAD_NAME: &str = "pty-io";
/// Stack of the I/O thread: it only waits and dispatches events (no buffers on its stack).
const THREAD_STACK_BYTES: usize = 64 * 1024;
/// Events taken from the completion port per wait; more simply wait for the next turn.
const EVENTS_PER_WAIT: usize = 64;

/// What the I/O thread wakes: a session's output (readable) or input (writable) pipe.
pub(super) trait Endpoint: Send + Sync {
    /// The pipe is ready for the next non-blocking attempt.
    fn mark_ready(&self);
}

type Sessions = Arc<Mutex<HashMap<Token, Weak<dyn Endpoint>>>>;

pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The I/O thread and what sessions need to register with it.
pub(super) struct Driver {
    pub(super) registry: Registry,
    stop: Waker,
    pub(super) sessions: Sessions,
    /// Pipes whose notification must be called again on the I/O thread.
    kicked: Arc<Mutex<Vec<Token>>>,
    next_token: AtomicUsize,
    stopping: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
    thread_id: ThreadId,
}

impl Driver {
    /// Starts the I/O thread.
    pub(super) fn start() -> std::io::Result<Arc<Self>> {
        let poll = Poll::new()?;
        let registry = poll.registry().try_clone()?;
        let stop = Waker::new(poll.registry(), STOP)?;
        let sessions: Sessions = Arc::default();
        let stopping = Arc::new(AtomicBool::new(false));
        let kicked: Arc<Mutex<Vec<Token>>> = Arc::default();
        let shared = Shared {
            sessions: Arc::clone(&sessions),
            stopping: Arc::clone(&stopping),
            kicked: Arc::clone(&kicked),
        };
        let thread = std::thread::Builder::new()
            .name(THREAD_NAME.into())
            .stack_size(THREAD_STACK_BYTES)
            .spawn(move || serve(poll, &shared))?;
        let thread_id = thread.thread().id();
        Ok(Arc::new(Self {
            registry,
            stop,
            sessions,
            kicked,
            next_token: AtomicUsize::new(STOP.0 + 1),
            stopping,
            thread: Mutex::new(Some(thread)),
            thread_id,
        }))
    }

    /// Registers the output pipe of a new session and returns its reader.
    pub(super) fn reader(self: &Arc<Self>, mut pipe: NamedPipe) -> std::io::Result<ConptyReader> {
        let token = self.token();
        self.registry.register(&mut pipe, token, Interest::READABLE)?;
        let output = Arc::new(Output {
            pipe,
            token,
            state: Mutex::new(State { ready: true, ended: false, notify: None }),
            ready: Condvar::new(),
            driver: Arc::clone(self),
        });
        let endpoint: Arc<dyn Endpoint> = output.clone();
        lock(&self.sessions).insert(token, Arc::downgrade(&endpoint));
        Ok(ConptyReader(output))
    }

    /// Calls the notification of pipe `token` again, from the I/O thread (notifications always
    /// come from `pty-io`, never from the caller's thread).
    pub(super) fn renotify(&self, token: Token) {
        lock(&self.kicked).push(token);
        // If waking fails the thread is gone and nothing waits for the notification any more.
        let _woken = self.stop.wake();
    }

    /// A fresh token for a pipe.
    pub(super) fn token(&self) -> Token {
        Token(self.next_token.fetch_add(1, Ordering::Relaxed))
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        // If waking fails the thread is already gone; there is nothing left to stop.
        let _woken = self.stop.wake();
        // The thread can hold the last reference for a moment; it then simply returns.
        let on_io_thread = std::thread::current().id() == self.thread_id;
        if let Some(thread) = lock(&self.thread).take().filter(|_| !on_io_thread) {
            // A panic on the I/O thread already ended it; nothing more to clean up here.
            let _joined = thread.join();
        }
    }
}

/// What the I/O thread shares with the driver.
struct Shared {
    sessions: Sessions,
    stopping: Arc<AtomicBool>,
    kicked: Arc<Mutex<Vec<Token>>>,
}

/// The loop of the I/O thread: wait, then mark each ready session and notify it.
fn serve(mut poll: Poll, shared: &Shared) {
    let mut events = Events::with_capacity(EVENTS_PER_WAIT);
    let mut kicked = Vec::new();
    while !shared.stopping.load(Ordering::Acquire) {
        match poll.poll(&mut events, None) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            // The port itself failed: sessions end through their own pipes' errors.
            Err(_) => return,
        }
        for event in &events {
            if event.token() == STOP {
                std::mem::swap(&mut kicked, &mut *lock(&shared.kicked));
            } else {
                kicked.push(event.token());
            }
            for token in kicked.drain(..) {
                let endpoint = lock(&shared.sessions).get(&token).and_then(Weak::upgrade);
                if let Some(endpoint) = endpoint {
                    endpoint.mark_ready();
                }
            }
        }
    }
}

/// What the I/O thread and the reader share.
struct State {
    ready: bool,
    ended: bool,
    notify: Option<Arc<dyn Fn() + Send + Sync>>,
}

/// One session's output pipe, registered with the I/O thread.
struct Output {
    pipe: NamedPipe,
    token: Token,
    state: Mutex<State>,
    ready: Condvar,
    driver: Arc<Driver>,
}

impl Endpoint for Output {
    fn mark_ready(&self) {
        let mut state = lock(&self.state);
        state.ready = true;
        let notify = state.notify.clone();
        drop(state);
        self.ready.notify_all();
        // Called without the lock held, so the notification may even call back into the reader.
        if let Some(notify) = notify {
            notify();
        }
    }
}

impl Output {
    /// One non-blocking attempt; `None` when no output is ready.
    fn try_read(&self, buf: &mut [u8]) -> Result<Option<usize>, PtyError> {
        let mut state = lock(&self.state);
        if state.ended {
            return Ok(Some(0));
        }
        state.ready = false;
        drop(state);
        match (&self.pipe).read(buf) {
            Ok(read) => {
                if read == 0 {
                    lock(&self.state).ended = true;
                }
                Ok(Some(read))
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(PtyError::io("read the pseudoconsole", error)),
        }
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, PtyError> {
        loop {
            if let Some(read) = self.try_read(buf)? {
                return Ok(read);
            }
            let state = lock(&self.state);
            let _ready = self
                .ready
                .wait_while(state, |state| !state.ready)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        lock(&self.driver.sessions).remove(&self.token);
        // The pipe closes right after; a failed deregistration only leaves a dead entry in the port.
        let _deregistered = self.driver.registry.deregister(&mut self.pipe);
    }
}

/// The reader of a `ConPTY` session ([`Readiness::Notify`]).
pub(super) struct ConptyReader(Arc<Output>);

impl PtyReader for ConptyReader {
    fn readiness(&self) -> Readiness {
        Readiness::Notify
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PtyError> {
        self.0.read(buf)
    }

    fn try_read(&mut self, buf: &mut [u8]) -> Result<Option<usize>, PtyError> {
        self.0.try_read(buf)
    }

    fn set_notify(&mut self, notify: Notify) {
        let mut state = lock(&self.0.state);
        state.notify = Some(Arc::from(notify));
        let ready = state.ready;
        drop(state);
        // Output that became ready before the notification was set is not lost; it is reported
        // from the I/O thread like every notification.
        if ready {
            self.0.driver.renotify(self.0.token);
        }
    }
}
