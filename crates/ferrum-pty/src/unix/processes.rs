//! The processes attached to a terminal (its controlling terminal), so `close` can end all of
//! them as closing a pseudoconsole does on Windows (ADR 0026, W3), whatever process group or job
//! they are in.
//!
//! Linux reads `tty_nr` from `/proc/<pid>/stat`; macOS asks `/bin/ps -t <tty>` (the system's own
//! process table reader; no other unprivileged interface exists without a C library binding).

use std::os::fd::OwnedFd;

use rustix::process::{Pid, Signal};

/// How the processes of one terminal are found.
pub(super) struct Tty {
    #[cfg(target_os = "linux")]
    device: u64,
    #[cfg(not(target_os = "linux"))]
    name: std::ffi::OsString,
}

impl Tty {
    /// Identifies the terminal whose slave is `slave`, opened from `path`.
    pub(super) fn new(slave: &OwnedFd, path: &std::path::Path) -> std::io::Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let _ = path;
            let stat = rustix::fs::fstat(slave)?;
            Ok(Self { device: proc_tty_nr(stat.st_rdev) })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = slave;
            let name = path.file_name().ok_or(std::io::ErrorKind::InvalidInput)?;
            Ok(Self { name: name.to_owned() })
        }
    }

    /// The processes attached to the terminal, except this one. List them while the session
    /// leader is alive: when it exits, the system detaches the terminal from the session.
    pub(super) fn attached_processes(&self) -> Vec<u32> {
        let own = std::process::id();
        self.attached().into_iter().filter(|&pid| pid != own).collect()
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

impl Tty {
    #[cfg(target_os = "linux")]
    fn attached(&self) -> Vec<u32> {
        let Ok(entries) = std::fs::read_dir("/proc") else { return Vec::new() };
        entries
            .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<u32>().ok())
            .filter(|pid| stat_tty(*pid) == Some(self.device))
            .collect()
    }

    #[cfg(not(target_os = "linux"))]
    fn attached(&self) -> Vec<u32> {
        let output = std::process::Command::new("/bin/ps")
            .args([std::ffi::OsStr::new("-o"), "pid=".as_ref(), "-t".as_ref(), &self.name])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();
        let Ok(output) = output else { return Vec::new() };
        let text = String::from_utf8_lossy(&output.stdout);
        text.split_whitespace().filter_map(|pid| pid.parse().ok()).collect()
    }
}

/// The `tty_nr` field of `/proc/<pid>/stat` (the 7th; the 2nd, the command, may hold spaces).
#[cfg(target_os = "linux")]
fn stat_tty(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_command = stat.get(stat.rfind(')')? + 1..)?;
    // Fields after the command: state, ppid, pgrp, session, tty_nr.
    after_command
        .split_whitespace()
        .nth(4)?
        .parse::<i64>()
        .ok()
        .and_then(|nr| u64::try_from(nr).ok())
}

/// Encodes a device number the way the kernel writes `tty_nr` (`new_encode_dev`).
#[cfg(target_os = "linux")]
fn proc_tty_nr(device: u64) -> u64 {
    let (major, minor) =
        (u64::from(rustix::fs::major(device)), u64::from(rustix::fs::minor(device)));
    (minor & 0xff) | (major << 8) | ((minor & !0xff) << 12)
}
