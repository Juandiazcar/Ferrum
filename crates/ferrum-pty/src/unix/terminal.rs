//! The terminal of one Unix session, shared by its reader, writer, resizer and child.
//!
//! The master is non-blocking; reads and writes wait in `poll` on the master and on a wake pipe
//! that `close` signals, so a blocked `read` or `write_all` always returns after `close`. The
//! parent keeps its own slave descriptor until the reader has drained the output after `close`:
//! the terminal is never "last closed" by a child's exit, so output a child wrote just before
//! exiting is never discarded (macOS drops unread output on the last slave close), and the
//! output ends at `close` on every OS, as with `ConPTY`.

use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use rustix::event::{PollFd, PollFlags};

use crate::PtyError;

/// One terminal: master, the parent's slave descriptor and the `close` wake pipe.
pub(super) struct Terminal {
    master: File,
    slave: Mutex<Option<OwnedFd>>,
    wake_read: std::io::PipeReader,
    wake_write: std::io::PipeWriter,
    closed: AtomicBool,
}

/// What a wait in `poll` found.
enum Ready {
    Master,
    Closed,
    Nothing,
}

/// How long [`Terminal::read`] may wait for output.
#[derive(Clone, Copy)]
pub(super) enum Wait {
    /// Until output arrives or the terminal is closed.
    Forever,
    /// Not at all (`try_read`).
    Never,
}

/// A zero timeout for `poll`.
const NO_WAIT: rustix::event::Timespec = rustix::event::Timespec { tv_sec: 0, tv_nsec: 0 };

impl Terminal {
    /// Wraps a master (made non-blocking here) and the parent's slave descriptor.
    pub(super) fn new(master: OwnedFd, slave: OwnedFd) -> Result<Self, PtyError> {
        let error = |source: rustix::io::Errno| PtyError::io("prepare the terminal", source.into());
        rustix::fs::fcntl_setfl(&master, rustix::fs::OFlags::NONBLOCK).map_err(error)?;
        // `std` pipes are close-on-exec, so no child inherits them.
        let (wake_read, wake_write) =
            std::io::pipe().map_err(|source| PtyError::io("prepare the terminal", source))?;
        Ok(Self {
            master: File::from(master),
            slave: Mutex::new(Some(slave)),
            wake_read,
            wake_write,
            closed: AtomicBool::new(false),
        })
    }

    pub(super) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    /// Marks the terminal closed and wakes every blocked read and write.
    pub(super) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        // The pipe only has to become readable; a full pipe already is, so a failure is moot.
        let _woken = rustix::io::write(&self.wake_write, &[1]);
    }

    /// Waits until the master has `events` or the terminal is closed (master first).
    fn wait(&self, events: PollFlags, wait: Wait) -> Result<Ready, PtyError> {
        let timeout = match wait {
            Wait::Forever => None,
            Wait::Never => Some(&NO_WAIT),
        };
        loop {
            let mut fds =
                [PollFd::new(&self.master, events), PollFd::new(&self.wake_read, PollFlags::IN)];
            match rustix::event::poll(&mut fds, timeout) {
                Ok(_) => {}
                Err(rustix::io::Errno::INTR) => continue,
                Err(errno) => return Err(PtyError::io("wait for the terminal", errno.into())),
            }
            let [master, wake] = fds;
            if !master.revents().is_empty() {
                return Ok(Ready::Master);
            }
            if !wake.revents().is_empty() || self.is_closed() {
                return Ok(Ready::Closed);
            }
            if matches!(wait, Wait::Never) {
                return Ok(Ready::Nothing);
            }
        }
    }

    /// Reads output; `Some(0)` once the terminal is closed and drained (see the module docs);
    /// `None` only with [`Wait::Never`] when nothing is ready.
    pub(super) fn read(&self, buf: &mut [u8], wait: Wait) -> Result<Option<usize>, PtyError> {
        loop {
            let ready = self.wait(PollFlags::IN, wait)?;
            if matches!(ready, Ready::Nothing) {
                return Ok(None);
            }
            match (&self.master).read(buf) {
                Ok(0) => return Ok(Some(self.end())),
                Ok(read) => return Ok(Some(read)),
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) if error.kind() == ErrorKind::WouldBlock => match (ready, wait) {
                    (Ready::Closed, _) => return Ok(Some(self.end())),
                    (_, Wait::Never) => return Ok(None),
                    _ => {}
                },
                // EIO: every slave descriptor is gone, so nothing more can arrive.
                Err(error)
                    if error.raw_os_error() == Some(rustix::io::Errno::IO.raw_os_error()) =>
                {
                    return Ok(Some(self.end()));
                }
                Err(error) => return Err(PtyError::io("read the terminal", error)),
            }
        }
    }

    /// The output is over: release the parent's slave descriptor.
    fn end(&self) -> usize {
        drop(self.slave.lock().unwrap_or_else(PoisonError::into_inner).take());
        0
    }

    /// Writes all of `bytes`, or fails with [`PtyError::Closed`] once the terminal is closed.
    pub(super) fn write_all(&self, mut bytes: &[u8]) -> Result<(), PtyError> {
        while !bytes.is_empty() {
            if self.is_closed()
                || matches!(self.wait(PollFlags::OUT, Wait::Forever)?, Ready::Closed)
            {
                return Err(PtyError::Closed);
            }
            match (&self.master).write(bytes) {
                Ok(written) => bytes = bytes.get(written..).unwrap_or_default(),
                Err(error)
                    if matches!(error.kind(), ErrorKind::Interrupted | ErrorKind::WouldBlock) => {}
                Err(error) => return Err(PtyError::io("write the terminal", error)),
            }
        }
        Ok(())
    }

    /// The parent's slave descriptor, while the output is not over.
    pub(super) fn with_slave<T>(&self, action: impl FnOnce(&OwnedFd) -> T) -> Option<T> {
        self.slave.lock().unwrap_or_else(PoisonError::into_inner).as_ref().map(action)
    }

    pub(super) fn raw_master(&self) -> std::os::fd::RawFd {
        std::os::fd::AsRawFd::as_raw_fd(&self.master)
    }

    pub(super) fn master_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.master.as_fd()
    }
}
