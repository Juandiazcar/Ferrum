//! Process memory of sessions: PT-23, and the probe PT-18 uses.
//!
//! The probe reads the metric each OS exposes without `unsafe` (`docs/11`: Linux and macOS
//! resident set, Windows private commit, as in ADR 0026 item 6): Linux `VmRSS` from
//! `/proc/self/status`, macOS `ps -o rss=`, Windows `PrivateMemorySize64` through `PowerShell`.
//! It is a test probe; the product metric of `docs/03` §5 belongs to `xtask memcheck`.

use std::time::Duration;

use ferrum_pty::{PtySession, Readiness};

use super::Harness;

/// PT-23 allowance: `docs/03` §2 "per session misc" (style table, parser, PTY buffers, indices),
/// 0.11 MB, times ten sessions.
const TEN_SESSIONS_BYTES: u64 = SESSIONS * PER_SESSION_BYTES;
/// `docs/03` §2 "per session misc": 0.11 MB.
const PER_SESSION_BYTES: u64 = 110_000;
const SESSIONS: u64 = 10;
const START: Duration = Duration::from_secs(10);

/// Memory of the current process in bytes (see the module documentation for the metric).
///
/// # Errors
/// A message when the OS reports nothing usable.
pub fn process_memory_bytes() -> Result<u64, String> {
    let text = probe()?;
    let number: u64 =
        text.trim().parse().map_err(|_| format!("unexpected probe output {text:?}"))?;
    Ok(if cfg!(windows) { number } else { number * 1024 })
}

#[cfg(target_os = "linux")]
fn probe() -> Result<String, String> {
    let status = std::fs::read_to_string("/proc/self/status").map_err(|error| error.to_string())?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:")).ok_or("no VmRSS")?;
    Ok(line.trim_start_matches("VmRSS:").trim_end_matches("kB").trim().to_owned())
}

#[cfg(target_os = "macos")]
fn probe() -> Result<String, String> {
    command("ps", &["-o", "rss=", "-p", &std::process::id().to_string()])
}

#[cfg(windows)]
fn probe() -> Result<String, String> {
    let script = format!("(Get-Process -Id {}).PrivateMemorySize64", std::process::id());
    command("powershell", &["-NoProfile", "-NonInteractive", "-Command", &script])
}

#[cfg(any(target_os = "macos", windows))]
fn command(program: &str, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(program).args(args).output();
    let output = output.map_err(|error| error.to_string())?;
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}

/// PT-23: ten sessions, read the way the daemon reads them (a polled descriptor on Unix, the
/// backend's single I/O thread on Windows; no thread per session, Q-31), grow the process by less
/// than the `docs/03` §2 allowance each.
pub fn ten_sessions_memory(h: &Harness<'_>) {
    let before = process_memory_bytes().unwrap();
    let (ready_tx, ready) = std::sync::mpsc::channel();
    let mut sessions = Vec::new();
    let mut notified = 0;
    for _ in 0..SESSIONS {
        let mut session: PtySession = h.spawn(&["repl", "1"]);
        if session.reader.readiness() == Readiness::Notify {
            let tx = ready_tx.clone();
            // The test may have stopped listening; the notification is then simply dropped.
            session.reader.set_notify(Box::new(move || {
                let _dropped = tx.send(());
            }));
            notified += 1;
        }
        sessions.push(session);
    }
    // Every notified session has produced output, so its read path has been exercised.
    for _ in 0..notified {
        ready.recv_timeout(START).expect("a session was never notified");
    }
    let after = process_memory_bytes().unwrap();
    for session in sessions {
        session.child.close().unwrap();
    }
    let delta = after.saturating_sub(before);
    report_memory(before, after);
    assert!(delta < TEN_SESSIONS_BYTES, "ten sessions grew the process by {delta} bytes");
}

/// PT-23 records the measured numbers in the test output.
#[allow(clippy::print_stderr, reason = "PT-23 records the measured memory in the test log")]
fn report_memory(before: u64, after: u64) {
    let delta = after.saturating_sub(before);
    eprintln!(
        "PT-23 ten_sessions_memory: {before} -> {after} bytes, delta {delta} ({} per session)",
        delta / SESSIONS
    );
}
