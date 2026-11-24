//! Learning that children exited without a thread per child and without polling (`docs/02` §5,
//! NFR-PERF-06: no timers when idle; ADR 0026 W2: exit is observed on the process, not the
//! reader).
//!
//! One [`ExitWatch`] serves every session. On Linux it is an `epoll` set of `pidfd`s, on macOS a
//! `kqueue` with an `EVFILT_PROC`/`NOTE_EXIT` filter per child; both report
//! [`Readiness::Fd`](crate::Readiness::Fd), one descriptor the caller's poller watches. On
//! Windows a named `pty-exit` thread waits on the process handles with `WaitForMultipleObjects`,
//! one thread per 63 children (maintainer decision Q-33), and calls the notification of
//! [`ExitWatch::set_notify`] ([`Readiness::Notify`](crate::Readiness::Notify)). After a wake-up,
//! [`ExitWatch::take_exited`] lists the keys of the children that exited; the caller then reads
//! their status with [`PtyChild::try_wait`](crate::PtyChild::try_wait).

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use crate::{ExitSource, Notify, PtyError, Readiness};

/// Watches children for their exit (see the module documentation).
pub trait ExitWatch: Send {
    /// How the caller learns that [`take_exited`](Self::take_exited) has something.
    fn readiness(&self) -> Readiness;

    /// For [`Readiness::Notify`] watchers: `notify` is called, from a `pty-exit` thread (never
    /// the caller's), when a watched child exits. It must return quickly and never block. Others ignore it.
    fn set_notify(&mut self, notify: Notify) {
        drop(notify);
    }

    /// Starts watching the child behind `source` under `key`. A child that already exited is
    /// reported by the next [`take_exited`](Self::take_exited). `source` must stay valid until
    /// [`unwatch`](Self::unwatch) (keep the [`PtyChild`](crate::PtyChild) alive).
    ///
    /// # Errors
    /// [`PtyError::InvalidSpec`] for a source this watcher cannot wait on
    /// ([`ExitSource::Polling`]), or the OS error.
    fn watch(&mut self, key: u64, source: ExitSource) -> Result<(), PtyError>;

    /// Stops watching `key`; unknown keys are ignored. Returns once no notification for it can
    /// still run.
    fn unwatch(&mut self, key: u64);

    /// Appends the keys of the watched children that exited since the last call, and stops
    /// watching them. Never blocks.
    ///
    /// # Errors
    /// The OS error of querying the watcher.
    fn take_exited(&mut self, out: &mut Vec<u64>) -> Result<(), PtyError>;
}

/// The exit watcher of the running OS.
///
/// # Errors
/// The OS error of creating it.
pub fn native_exit_watch() -> Result<Box<dyn ExitWatch>, PtyError> {
    #[cfg(target_os = "linux")]
    let watch = linux::PidfdWatch::new()?;
    #[cfg(target_os = "macos")]
    let watch = macos::KqueueWatch::new()?;
    #[cfg(windows)]
    let watch = windows::WaitWatch::new();
    Ok(Box::new(watch))
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::{Receiver, sync_channel};
    use std::time::{Duration, Instant};

    use super::*;

    /// How long a test waits for exits to be reported.
    const DEADLINE: Duration = Duration::from_secs(30);

    /// A watcher whose notification (Windows) lands on a channel.
    fn notified_watch() -> (Box<dyn ExitWatch>, Receiver<()>) {
        let mut watch = native_exit_watch().unwrap();
        let (tx, rx) = sync_channel(1);
        watch.set_notify(Box::new(move || {
            let _full = tx.try_send(()); // one pending wake-up is enough
        }));
        (watch, rx)
    }

    /// Blocks until the watcher reports something or `deadline` passes: on its descriptor
    /// (Unix) or its notification (Windows), never by spinning.
    fn wait_news(watch: &dyn ExitWatch, notified: &Receiver<()>, deadline: Instant) {
        let left = deadline.saturating_duration_since(Instant::now());
        match watch.readiness() {
            #[cfg(unix)]
            Readiness::Fd(fd) => {
                let mut poll = mio::Poll::new().unwrap();
                let mut source = mio::unix::SourceFd(&fd);
                poll.registry()
                    .register(&mut source, mio::Token(0), mio::Interest::READABLE)
                    .unwrap();
                let mut events = mio::Events::with_capacity(1);
                let _woken = poll.poll(&mut events, Some(left));
            }
            _ => {
                let _woken = notified.recv_timeout(left);
            }
        }
    }

    /// Collects exits until `count` keys arrived, failing after [`DEADLINE`].
    fn exits(watch: &mut dyn ExitWatch, notified: &Receiver<()>, count: usize) -> Vec<u64> {
        let deadline = Instant::now() + DEADLINE;
        let mut keys = Vec::new();
        watch.take_exited(&mut keys).unwrap();
        while keys.len() < count {
            assert!(Instant::now() < deadline, "only {} of {count} exits reported", keys.len());
            wait_news(watch, notified, deadline);
            watch.take_exited(&mut keys).unwrap();
        }
        keys
    }

    fn spawn(script_unix: &str, script_windows: &str) -> std::process::Child {
        let (program, flag, script) =
            if cfg!(windows) { ("cmd", "/c", script_windows) } else { ("sh", "-c", script_unix) };
        std::process::Command::new(program).args([flag, script]).spawn().unwrap()
    }

    #[test]
    fn an_exit_of_an_unwatched_child_is_never_reported_for_a_new_child_of_the_same_key() {
        let mut watch = native_exit_watch().unwrap();
        watch.set_notify(Box::new(|| {}));
        let mut first = spawn("exit 0", "exit 0");
        watch.watch(9, source(&first)).unwrap();
        first.wait().unwrap(); // its exit is queued in the watcher now (or soon)
        watch.unwatch(9);
        let mut second = spawn("sleep 60", "ping -n 60 127.0.0.1 > nul");
        watch.watch(9, source(&second)).unwrap();
        let mut keys = Vec::new();
        watch.take_exited(&mut keys).unwrap();
        assert!(keys.is_empty(), "the first child's exit was reported for the second: {keys:?}");
        second.kill().unwrap();
        second.wait().unwrap();
        watch.unwatch(9);
    }

    fn spawn_exiting() -> std::process::Child {
        let mut command = if cfg!(windows) {
            std::process::Command::new("cmd")
        } else {
            std::process::Command::new("sh")
        };
        let args: &[&str] = if cfg!(windows) { &["/c", "exit 0"] } else { &["-c", "exit 0"] };
        command.args(args).spawn().unwrap()
    }

    #[cfg(unix)]
    fn source(child: &std::process::Child) -> ExitSource {
        ExitSource::Pid(child.id())
    }

    #[cfg(windows)]
    fn source(child: &std::process::Child) -> ExitSource {
        use std::os::windows::io::AsRawHandle;
        ExitSource::Handle(child.as_raw_handle())
    }

    #[test]
    fn reports_an_exited_child_once_and_refuses_polling_sources() {
        let (mut watch, rx) = notified_watch();
        let mut child = spawn_exiting();
        watch.watch(7, source(&child)).unwrap();
        assert!(matches!(watch.watch(8, ExitSource::Polling), Err(PtyError::InvalidSpec(_))));
        let mut keys = exits(&mut *watch, &rx, 1);
        assert_eq!(keys, [7]);
        keys.clear();
        watch.take_exited(&mut keys).unwrap();
        assert!(keys.is_empty(), "an exit is reported once");
        child.wait().unwrap();
        watch.unwatch(7);
    }

    /// Children beyond one wait set (63 on Windows) are watched too.
    const MANY: usize = 70;

    #[test]
    fn the_exits_of_more_children_than_one_wait_set_are_all_reported() {
        let (mut watch, rx) = notified_watch();
        let mut children: Vec<_> = (0..MANY).map(|_| spawn_exiting()).collect();
        for (key, child) in (0_u64..).zip(&children) {
            watch.watch(key, source(child)).unwrap();
        }
        let mut keys = exits(&mut *watch, &rx, MANY);
        keys.sort_unstable();
        assert_eq!(keys, (0..u64::try_from(MANY).unwrap()).collect::<Vec<_>>());
        for child in &mut children {
            child.wait().unwrap();
        }
    }
}
