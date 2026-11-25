//! The input side of a `ConPTY` session: an overlapped named pipe written through `mio` and
//! served by the single I/O thread (`docs/11` §4: "writes use a per-session writer thread or
//! overlapped I/O"; ADR 0026 W4). A write never blocks the caller's thread: `mio` copies the
//! bytes into its buffer and starts the write; the next one waits until the I/O thread reports the
//! pipe writable, which wakes a blocked [`write_all`](crate::PtyWriter::write_all) and calls the
//! notification of [`set_notify`](crate::PtyWriter::set_notify).

use std::io::{ErrorKind, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use mio::windows::NamedPipe;
use mio::{Interest, Token};

use super::driver::{Driver, Endpoint, lock};
use crate::{Notify, PtyError, PtyWriter, Readiness};

/// What the I/O thread and the writer share.
struct State {
    ready: bool,
    notify: Option<Arc<dyn Fn() + Send + Sync>>,
}

/// One session's input pipe, registered with the I/O thread.
struct Input {
    pipe: NamedPipe,
    token: Token,
    state: Mutex<State>,
    ready: Condvar,
    driver: Arc<Driver>,
}

impl Endpoint for Input {
    fn mark_ready(&self) {
        let mut state = lock(&self.state);
        state.ready = true;
        let notify = state.notify.clone();
        drop(state);
        self.ready.notify_all();
        if let Some(notify) = notify {
            notify();
        }
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        lock(&self.driver.sessions).remove(&self.token);
        // The pipe closes right after; a failed deregistration only leaves a dead entry.
        let _deregistered = self.driver.registry.deregister(&mut self.pipe);
    }
}

/// The writer of a `ConPTY` session ([`Readiness::Notify`]).
pub(super) struct ConptyWriter {
    input: Arc<Input>,
    closed: Arc<AtomicBool>,
}

impl ConptyWriter {
    /// Registers the server end of the input pipe with the I/O thread.
    pub(super) fn new(
        driver: &Arc<Driver>,
        mut pipe: NamedPipe,
        closed: Arc<AtomicBool>,
    ) -> std::io::Result<Self> {
        let token = driver.token();
        driver.registry.register(&mut pipe, token, Interest::WRITABLE)?;
        let state = Mutex::new(State { ready: true, notify: None });
        let input = Arc::new(Input {
            pipe,
            token,
            state,
            ready: Condvar::new(),
            driver: Arc::clone(driver),
        });
        let endpoint: Arc<dyn Endpoint> = input.clone();
        lock(&driver.sessions).insert(token, Arc::downgrade(&endpoint));
        Ok(Self { input, closed })
    }

    fn error(&self, error: std::io::Error) -> PtyError {
        if self.closed.load(Ordering::Acquire) {
            PtyError::Closed
        } else {
            PtyError::io("write the pseudoconsole", error)
        }
    }
}

impl PtyWriter for ConptyWriter {
    fn write_all(&mut self, mut bytes: &[u8]) -> Result<(), PtyError> {
        while !bytes.is_empty() {
            if let Some(written) = self.try_write(bytes)? {
                bytes = bytes.get(written..).unwrap_or_default();
                continue;
            }
            let state = lock(&self.input.state);
            let _ready = self
                .input
                .ready
                .wait_while(state, |state| !state.ready)
                .unwrap_or_else(PoisonError::into_inner);
        }
        Ok(())
    }

    fn try_write(&mut self, bytes: &[u8]) -> Result<Option<usize>, PtyError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(PtyError::Closed);
        }
        if bytes.is_empty() {
            return Ok(Some(0));
        }
        lock(&self.input.state).ready = false;
        match (&self.input.pipe).write(bytes) {
            Ok(0) => Err(PtyError::Closed),
            Ok(written) => Ok(Some(written)),
            Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(self.error(error)),
        }
    }

    fn readiness(&self) -> Readiness {
        Readiness::Notify
    }

    fn set_notify(&mut self, notify: Notify) {
        let mut state = lock(&self.input.state);
        state.notify = Some(Arc::from(notify));
        let ready = state.ready;
        drop(state);
        // Room that freed up before the notification was set is reported from the I/O thread.
        if ready {
            self.input.driver.renotify(self.input.token);
        }
    }
}
