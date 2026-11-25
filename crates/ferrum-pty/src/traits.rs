//! The `PtyBackend` traits (ADR 0026, ISP per `docs/05` §2): one small trait per capability, so the
//! daemon hands each part of a session to the code that needs it.
//!
//! Every implementation (Unix, `ConPTY`, the fake in `ferrum-testkit`) passes the same contract
//! suite, `ferrum_testkit::pty_contract` (PT-01 to PT-25).

use crate::{ExitSource, ExitStatus, PtyError, PtySize, Readiness, SpawnSpec};

/// Starts children in new pseudo terminals.
pub trait PtyBackend: Send + Sync {
    /// Starts `spec.program` in a new pseudo terminal of `spec.size`.
    ///
    /// # Errors
    /// Any [`PtyError`]: a bad spec, a missing program or an OS failure. No child is left running
    /// when this fails.
    fn spawn(&self, spec: &SpawnSpec) -> Result<PtySession, PtyError>;
}

/// The parts of one running session, each owned by whoever needs it.
pub struct PtySession {
    /// Output of the child.
    pub reader: Box<dyn PtyReader>,
    /// Input to the child.
    pub writer: Box<dyn PtyWriter>,
    /// Terminal size.
    pub resize: Box<dyn PtyResize>,
    /// Lifecycle of the child.
    pub child: Box<dyn PtyChild>,
}

/// Output side of a session.
pub trait PtyReader: Send {
    /// How the caller learns that `read` has data.
    fn readiness(&self) -> Readiness;

    /// Reads at most `buf.len()` bytes, blocking until some output arrives or the session is
    /// closed. Returns `Ok(0)` only at the end of the output, which comes after `close` once the
    /// output written before it has been read, on every OS. The exit of the child does not end
    /// the output: use [`PtyChild::try_wait`].
    ///
    /// # Errors
    /// [`PtyError::Io`] when the OS read fails.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PtyError>;

    /// Like [`read`](Self::read), but returns `Ok(None)` instead of blocking when no output is
    /// ready yet.
    ///
    /// # Errors
    /// [`PtyError::Io`] when the OS read fails.
    fn try_read(&mut self, buf: &mut [u8]) -> Result<Option<usize>, PtyError>;

    /// For [`Readiness::Notify`] readers: `notify` is called, from the backend's I/O thread and
    /// without any lock of the reader held, every time output (or its end) becomes ready, and
    /// once at once if some already is. Notifications are edge-triggered: after one, call
    /// [`try_read`](Self::try_read) until it returns `Ok(None)`, because output left over after a
    /// short buffer raises no new notification. A notification may also find nothing, which is
    /// harmless. `notify` must return quickly and never block. Other readers ignore it.
    fn set_notify(&mut self, notify: Notify) {
        drop(notify);
    }
}

/// What a [`Readiness::Notify`] reader calls when output is ready (the daemon passes a closure
/// that wakes its loop, e.g. a `mio::Waker`). It must return quickly and never block; see
/// [`PtyReader::set_notify`] for the edge-triggered contract.
pub type Notify = Box<dyn Fn() + Send + Sync>;

/// Input side of a session.
pub trait PtyWriter: Send {
    /// Writes all of `bytes`. May block while the child does not read: the caller owns the thread
    /// (ADR 0026, W4). Dropping the writer sends nothing to the child.
    ///
    /// # Errors
    /// [`PtyError::Closed`] after `close` (also for a write blocked when `close` happens), or
    /// [`PtyError::Io`] when the OS write fails.
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), PtyError>;

    /// Writes as much of `bytes` as the terminal takes now, never blocking: `Ok(Some(n))` with
    /// `n` bytes written (at least one for a non-empty `bytes`), `Ok(None)` when nothing fits;
    /// [`readiness`](Self::readiness) says how to learn that it would make progress again. The
    /// default writes everything with [`write_all`](Self::write_all) (writers that never block).
    ///
    /// # Errors
    /// As [`write_all`](Self::write_all).
    fn try_write(&mut self, bytes: &[u8]) -> Result<Option<usize>, PtyError> {
        self.write_all(bytes).map(|()| Some(bytes.len()))
    }

    /// How the caller learns that [`try_write`](Self::try_write) would take bytes again:
    /// [`Readiness::Fd`] (the descriptor becomes writable; on Unix it is the reader's
    /// descriptor), [`Readiness::Notify`] (the function of [`set_notify`](Self::set_notify) is
    /// called), or [`Readiness::Blocking`] for a writer whose `try_write` always takes everything.
    fn readiness(&self) -> Readiness {
        Readiness::Blocking
    }

    /// For [`Readiness::Notify`] writers: `notify` is called, from a thread of the backend, when
    /// room frees up after a `try_write` returned `Ok(None)`. Other writers ignore it.
    fn set_notify(&mut self, notify: Notify) {
        drop(notify);
    }
}

/// Terminal size of a session.
pub trait PtyResize: Send + Sync {
    /// Sets the terminal size. Idempotent; on `ConPTY` every call, even with the same size, is
    /// followed by a repaint of the visible rows (ADR 0026, W6).
    ///
    /// # Errors
    /// [`PtyError::Closed`] after `close`, or the OS error.
    fn resize(&self, size: PtySize) -> Result<(), PtyError>;

    /// The size set last.
    fn size(&self) -> PtySize;
}

/// Lifecycle of the child.
pub trait PtyChild: Send {
    /// Process id of the child, when it has one.
    fn pid(&self) -> Option<u32>;

    /// Returns the exit status once the child has exited, without blocking.
    ///
    /// # Errors
    /// The OS error when the status cannot be queried.
    fn try_wait(&mut self) -> Result<Option<ExitStatus>, PtyError>;

    /// What the caller can wait on to learn that the child exited.
    fn exit_source(&self) -> ExitSource;

    /// Ends the child at once (Unix `SIGKILL`, Windows `TerminateProcess`) without blocking. `Ok`
    /// means the child is dead or about to die; calling it again, or after the child exited, also
    /// returns `Ok`. Descendants may survive: use `close` to end them.
    ///
    /// # Errors
    /// The OS error when the child could not be ended.
    fn kill(&mut self) -> Result<(), PtyError>;

    /// A handle that can kill the child from another thread, with the same contract as `kill`.
    fn killer(&self) -> Box<dyn PtyKiller>;

    /// Ends the session: the child and every process attached to the terminal are ended at once,
    /// a reader returns `Ok(0)` once the output written before `close` is consumed, and writes
    /// fail with [`PtyError::Closed`].
    ///
    /// # Errors
    /// The OS error when the child could not be ended; the terminal is released anyway.
    fn close(self: Box<Self>) -> Result<(), PtyError>;
}

/// Kills a child from any thread (see [`PtyChild::kill`]).
pub trait PtyKiller: Send + Sync {
    /// Same contract as [`PtyChild::kill`].
    ///
    /// # Errors
    /// The OS error when the child could not be ended.
    fn kill(&self) -> Result<(), PtyError>;
}
