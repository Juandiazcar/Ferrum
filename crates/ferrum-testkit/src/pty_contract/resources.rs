//! Process memory of sessions: PT-23, and the probe PT-18 uses.
//!
//! The probe reads the metric each OS exposes without `unsafe` (`docs/11`: Linux and macOS
//! resident set, Windows private commit, as in ADR 0026 item 6): Linux `VmRSS` from
//! `/proc/self/status`, macOS `ps -o rss=`, Windows `PrivateMemorySize64` through `PowerShell`.
//! It is a test probe; the product metric of `docs/03` §5 belongs to `xtask memcheck`.

use std::ops::ControlFlow;
use std::time::Duration;

use ferrum_pty::{ChunkSink, PtyError, PtySession, Readiness, spawn_reader_thread};

use super::Harness;

/// PT-23 allowance: `docs/03` §2 "per session misc" (style table, parser, PTY buffers, indices),
/// 0.11 MB, times ten sessions.
const TEN_SESSIONS_BYTES: u64 = 10 * 110_000;
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

/// Drains a session on its reader thread and reports its first chunk once.
struct DrainSink(Option<std::sync::mpsc::Sender<()>>);

impl ChunkSink for DrainSink {
    fn chunk(&mut self, _bytes: &[u8]) -> ControlFlow<()> {
        if let Some(started) = self.0.take() {
            // The test may have stopped waiting; draining goes on regardless.
            let _ignored = started.send(());
        }
        ControlFlow::Continue(())
    }

    fn end(&mut self, _result: Result<(), PtyError>) {}
}

/// PT-23: ten sessions, with the reader threads the daemon would run for blocking readers, grow
/// the process by less than 0.20 MB each.
pub fn ten_sessions_memory(h: &Harness<'_>) {
    let before = process_memory_bytes().unwrap();
    let (started_tx, started) = std::sync::mpsc::channel();
    let mut sessions = Vec::new();
    let mut threads = Vec::new();
    for id in 0..SESSIONS {
        let PtySession { reader, writer, resize, child } = h.spawn(&["repl", "1"]);
        if reader.readiness() == Readiness::Blocking {
            let sink = DrainSink(Some(started_tx.clone()));
            threads.push(spawn_reader_thread(id, reader, sink).unwrap());
            sessions.push((child, writer, resize, None));
        } else {
            // Polled by the daemon's I/O thread: no thread of its own.
            sessions.push((child, writer, resize, Some(reader)));
        }
    }
    for _ in 0..threads.len() {
        started.recv_timeout(START).expect("a reader thread received nothing");
    }
    let after = process_memory_bytes().unwrap();
    for (child, ..) in sessions {
        child.close().unwrap();
    }
    for thread in threads {
        thread.join().unwrap();
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
