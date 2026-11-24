//! The processes attached to a terminal (its controlling terminal), so `close` can end all of
//! them as closing a pseudoconsole does on Windows (ADR 0026, W3), whatever process group or job
//! they are in.
//!
//! Both systems are asked for the device number of each process's controlling terminal, read
//! from the kernel without starting any program: Linux from `/proc/<pid>/stat` (`tty_nr`), macOS
//! from `proc_pidinfo` (`e_tdev`, [`sys`](super::sys)).

use std::os::fd::OwnedFd;

use rustix::process::{Pid, Signal};

/// The device number of one terminal, as the kernel reports it for its processes.
pub(super) struct Tty {
    #[cfg(target_os = "linux")]
    device: u64,
    #[cfg(target_os = "macos")]
    device: u32,
}

impl Tty {
    /// Identifies the terminal whose slave is `slave`.
    pub(super) fn new(slave: &OwnedFd) -> std::io::Result<Self> {
        let device = rustix::fs::fstat(slave)?.st_rdev;
        #[cfg(target_os = "linux")]
        let device = proc_tty_nr(device);
        #[cfg(target_os = "macos")]
        let device = u32::try_from(device).map_err(|_| std::io::ErrorKind::InvalidData)?;
        Ok(Self { device })
    }

    /// The processes attached to the terminal, except this one. List them while the session
    /// leader is alive: when it exits, the system detaches the terminal from the session.
    pub(super) fn attached_processes(&self) -> Vec<u32> {
        let own = std::process::id();
        self.attached().into_iter().filter(|&pid| pid != own).collect()
    }

    #[cfg(target_os = "linux")]
    fn attached(&self) -> Vec<u32> {
        let Ok(entries) = std::fs::read_dir("/proc") else { return Vec::new() };
        entries
            .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<u32>().ok())
            .filter(|pid| stat_tty(*pid) == Some(self.device))
            .collect()
    }

    #[cfg(target_os = "macos")]
    fn attached(&self) -> Vec<u32> {
        super::sys::processes_on_terminal(self.device)
    }
}

/// Sends `SIGKILL` to each of `pids`. A process that exited since it was listed is simply gone
/// (its pid could only be reused by a process that is not attached to the terminal, a window of
/// microseconds this accepts).
pub(super) fn kill_all(pids: &[u32]) {
    for &pid in pids {
        if let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) {
            // The process may have exited since it was listed; nothing is left to end then.
            let _gone = rustix::process::kill_process(pid, Signal::KILL);
        }
    }
}

/// The `tty_nr` field of `/proc/<pid>/stat` (the 7th; the 2nd, the command, may hold spaces).
#[cfg(target_os = "linux")]
fn stat_tty(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_command = stat.get(stat.rfind(')')? + 1..)?;
    // Fields after the command: state, ppid, pgrp, session, tty_nr.
    let field = after_command.split_whitespace().nth(4)?;
    field.parse::<i64>().ok().and_then(|nr| u64::try_from(nr).ok())
}

/// Bits of the minor number kept in the low byte of `tty_nr` (`new_encode_dev` in Linux).
#[cfg(target_os = "linux")]
const MINOR_LOW: u64 = 0xff;
/// Where `new_encode_dev` puts the major number and the rest of the minor number.
#[cfg(target_os = "linux")]
const MAJOR_SHIFT: u32 = 8;
#[cfg(target_os = "linux")]
const MINOR_HIGH_SHIFT: u32 = 12;

/// Encodes a device number the way the kernel writes `tty_nr` (`new_encode_dev`).
#[cfg(target_os = "linux")]
fn proc_tty_nr(device: u64) -> u64 {
    let major = u64::from(rustix::fs::major(device));
    let minor = u64::from(rustix::fs::minor(device));
    (minor & MINOR_LOW) | (major << MAJOR_SHIFT) | ((minor & !MINOR_LOW) << MINOR_HIGH_SHIFT)
}
