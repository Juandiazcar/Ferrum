//! Every `unsafe` call of the Unix backend (`docs/05` §4: one audited module).

#![allow(unsafe_code, reason = "docs/05 §4: the Unix PTY calls, each with SAFETY")]

use std::os::fd::{BorrowedFd, OwnedFd};
use std::os::unix::process::CommandExt;

/// Duplicates the master descriptor of `master` (close-on-exec) into one this crate owns.
pub(super) fn duplicate_master(master: &dyn portable_pty::MasterPty) -> std::io::Result<OwnedFd> {
    let fd = master.as_raw_fd().ok_or(std::io::ErrorKind::NotFound)?;
    // SAFETY: `fd` is owned by `master`, which is borrowed for this whole call, so it stays open
    // while the borrowed descriptor exists; the borrow ends before this function returns.
    let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
    Ok(rustix::io::fcntl_dupfd_cloexec(borrowed, 0)?)
}

/// Makes the child a session leader whose controlling terminal is its standard input (the
/// slave), as a terminal emulator must (`docs/11` §2).
pub(super) fn controlling_terminal(command: &mut std::process::Command) {
    // SAFETY: the closure runs in the forked child before `exec` and calls only `setsid` and
    // `ioctl(TIOCSCTTY)`, both async-signal-safe system calls; it allocates nothing and touches
    // no lock. Descriptor 0 is the slave, installed by `Command` before the closure runs.
    unsafe {
        command.pre_exec(|| {
            rustix::process::setsid()?;
            rustix::process::ioctl_tiocsctty(rustix::stdio::stdin())?;
            Ok(())
        });
    }
}
