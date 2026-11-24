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

/// The processes whose controlling terminal is device `terminal`, from the kernel's process
/// table (`proc_listallpids` and `proc_pidinfo`, the interface macOS offers for it).
#[cfg(target_os = "macos")]
pub(super) fn processes_on_terminal(terminal: u32) -> Vec<u32> {
    use std::ffi::c_int;
    /// Room for processes started between counting and listing.
    const SLACK: usize = 64;
    // SAFETY: a null buffer with size 0 is the documented way to ask for the number of pids.
    let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    let Ok(count) = usize::try_from(count) else { return Vec::new() };
    let mut pids = vec![0 as libc::pid_t; count + SLACK];
    let bytes = c_int::try_from(pids.len() * size_of::<libc::pid_t>()).unwrap_or(c_int::MAX);
    // SAFETY: `pids` is writable for `bytes` bytes, which is what the call is told it may fill.
    let listed = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
    pids.truncate(usize::try_from(listed).unwrap_or(0));
    pids.into_iter()
        .filter(|&pid| terminal_of(pid) == Some(terminal))
        .filter_map(|pid| u32::try_from(pid).ok())
        .collect()
}

#[cfg(target_os = "macos")]
fn terminal_of(pid: libc::pid_t) -> Option<u32> {
    let size = std::ffi::c_int::try_from(size_of::<libc::proc_bsdinfo>()).ok()?;
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    // SAFETY: `info` is writable for `size` bytes, the size of `proc_bsdinfo` that
    // `PROC_PIDTBSDINFO` fills; a process that is gone makes the call fail, checked below.
    let written = unsafe {
        libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, info.as_mut_ptr().cast(), size)
    };
    // SAFETY: the call filled the whole structure (`written == size`); a zeroed `proc_bsdinfo`
    // (plain integers and byte arrays) is valid in any case.
    (written == size).then(|| unsafe { info.assume_init() }.e_tdev)
}
