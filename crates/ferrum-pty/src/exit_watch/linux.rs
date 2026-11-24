//! Linux exit watcher: an `epoll` set of `pidfd`s (safe `rustix` calls, no `unsafe`).

use std::collections::HashMap;
use std::os::fd::{AsRawFd, OwnedFd};

use rustix::event::epoll;
use rustix::io::Errno;
use rustix::process::{Pid, PidfdFlags, pidfd_open};

use super::ExitWatch;
use crate::{ExitSource, PtyError, Readiness};

/// Exits read from the set per call; more stay ready for the next call.
const EVENTS_PER_CALL: usize = 32;

pub(super) struct PidfdWatch {
    set: OwnedFd,
    pidfds: HashMap<u64, OwnedFd>,
    /// Children that had already been reaped when watched.
    gone: Vec<u64>,
}

fn os(op: &'static str) -> impl Fn(Errno) -> PtyError {
    move |errno| PtyError::io(op, errno.into())
}

impl PidfdWatch {
    pub(super) fn new() -> Result<Self, PtyError> {
        let set =
            epoll::create(epoll::CreateFlags::CLOEXEC).map_err(os("create the exit watch"))?;
        Ok(Self { set, pidfds: HashMap::new(), gone: Vec::new() })
    }
}

impl ExitWatch for PidfdWatch {
    fn readiness(&self) -> Readiness {
        Readiness::Fd(self.set.as_raw_fd())
    }

    fn watch(&mut self, key: u64, source: ExitSource) -> Result<(), PtyError> {
        let ExitSource::Pid(pid) = source else {
            return Err(PtyError::InvalidSpec("this exit source cannot be watched"));
        };
        let pid = i32::try_from(pid).ok().and_then(Pid::from_raw);
        let pid = pid.ok_or(PtyError::InvalidSpec("invalid process id"))?;
        let pidfd = match pidfd_open(pid, PidfdFlags::empty()) {
            Ok(pidfd) => pidfd,
            // Already reaped: it has exited.
            Err(Errno::SRCH) => {
                self.gone.push(key);
                return Ok(());
            }
            Err(errno) => return Err(os("watch a child")(errno)),
        };
        let tag = epoll::EventData::new_u64(key);
        epoll::add(&self.set, &pidfd, tag, epoll::EventFlags::IN).map_err(os("watch a child"))?;
        self.pidfds.insert(key, pidfd);
        Ok(())
    }

    fn unwatch(&mut self, key: u64) {
        self.gone.retain(|&gone| gone != key);
        if let Some(pidfd) = self.pidfds.remove(&key) {
            // Closing the pidfd also removes it from the set; the explicit delete keeps the set
            // exact even if the descriptor was duplicated.
            let _removed = epoll::delete(&self.set, &pidfd);
        }
    }

    fn take_exited(&mut self, out: &mut Vec<u64>) -> Result<(), PtyError> {
        out.append(&mut self.gone);
        let mut events = Vec::with_capacity(EVENTS_PER_CALL);
        let zero = rustix::event::Timespec { tv_sec: 0, tv_nsec: 0 };
        epoll::wait(&self.set, rustix::buffer::spare_capacity(&mut events), Some(&zero))
            .map_err(os("query the exit watch"))?;
        let keys: Vec<u64> = events.iter().map(|event| event.data.u64()).collect();
        for key in keys {
            // A pidfd stays readable after the exit: stop watching so it is reported once.
            self.unwatch(key);
            out.push(key);
        }
        Ok(())
    }
}
