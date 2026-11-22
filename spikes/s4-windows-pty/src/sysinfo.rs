//! Process helpers: own executable, helper child commands, memory probe and pid liveness.
//! Safe-only: the probes shell out (`PowerShell`, `tasklist`, `ps`, `kill`) instead of calling the OS API.

use anyhow::{Context, Result};
use portable_pty::CommandBuilder;
use std::process::{Command, Stdio};

/// (working set or RSS KiB, peak KiB, private commit KiB); 0 when the OS does not give it.
pub(crate) type Mem = (usize, usize, usize);

pub(crate) fn exe() -> Result<String> {
    let path = std::env::current_exe().context("current_exe")?;
    Ok(path.to_string_lossy().into_owned())
}

/// A command that re-runs this binary as a helper child (see `helpers`).
pub(crate) fn helper(args: &[&str]) -> Result<CommandBuilder> {
    let mut cmd = CommandBuilder::new(exe()?);
    cmd.args(args);
    Ok(cmd)
}

#[cfg(windows)]
fn three_numbers(text: &str) -> Mem {
    let mut it = text.split_whitespace().filter_map(|x| x.parse().ok());
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

#[cfg(any(windows, target_os = "macos"))]
fn stdout_of(cmd: &mut Command) -> String {
    let out = cmd.output();
    out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

#[cfg(windows)]
pub(crate) fn mem() -> Mem {
    let script = format!(
        "$p=Get-Process -Id {}; '{{0}} {{1}} {{2}}' -f [long]($p.WorkingSet64/1KB),[long]($p.PeakWorkingSet64/1KB),[long]($p.PrivateMemorySize64/1KB)",
        std::process::id()
    );
    three_numbers(&stdout_of(Command::new("powershell.exe").args([
        "-NoProfile",
        "-Command",
        &script,
    ])))
}

#[cfg(target_os = "linux")]
pub(crate) fn mem() -> Mem {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |key: &str| {
        let line = status.lines().find(|l| l.starts_with(key));
        line.and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse().ok()).unwrap_or(0)
    };
    (field("VmRSS"), field("VmHWM"), field("VmData"))
}

#[cfg(target_os = "macos")]
pub(crate) fn mem() -> Mem {
    let pid = std::process::id().to_string();
    let rss = stdout_of(Command::new("ps").args(["-o", "rss=", "-p", &pid]));
    (rss.trim().parse().unwrap_or(0), 0, 0)
}

pub(crate) fn pid_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let filter = format!("PID eq {pid}");
        stdout_of(Command::new("tasklist").args(["/FI", &filter, "/NH"])).contains(&pid.to_string())
    }
    #[cfg(unix)]
    {
        let status =
            Command::new("kill").args(["-0", &pid.to_string()]).stderr(Stdio::null()).status();
        status.is_ok_and(|s| s.success())
    }
}

/// Best-effort cleanup of a leftover process.
pub(crate) fn kill_pid(pid: u32) {
    #[cfg(windows)]
    Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .status()
        .ok();
    #[cfg(unix)]
    Command::new("kill").arg(pid.to_string()).stderr(Stdio::null()).status().ok();
}
