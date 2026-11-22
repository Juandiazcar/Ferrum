//! Experiment 6: blocking reader and writer, thread cost, cost of ten real sessions.

use crate::out::say;
use crate::session::{Msg, Sess, spawn};
use crate::sysinfo::{Mem, helper, mem};
use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

const SESSIONS: usize = 10;

pub(crate) fn run() -> Result<()> {
    say!("\n=== [6] I/O model and thread cost ===");
    blocking_read()?;
    blocked_writer()?;
    bare_threads()?;
    real_sessions()
}

fn describe_msg(msg: &Msg, t0: Instant) -> String {
    match msg {
        Msg::Data { at, bytes, blocked } => {
            format!(
                "data {}B at {:?} (read call blocked {blocked:?})",
                bytes.len(),
                at.saturating_duration_since(t0)
            )
        }
        Msg::End { at, why } => format!("end({why}) at {:?}", at.saturating_duration_since(t0)),
    }
}

/// A child silent for 3 s: when does the reader return, and does it see EOF at exit?
fn blocking_read() -> Result<()> {
    let mut s = spawn(helper(&["sleep", "3"])?, 80, 24)?;
    let t0 = Instant::now();
    let mut events = Vec::new();
    while t0.elapsed() < Duration::from_millis(4500) {
        if let Ok(msg) = s.rx.recv_timeout(Duration::from_millis(50)) {
            events.push(describe_msg(&msg, t0));
            s.absorb(msg);
        }
    }
    say!("[6a] child sleeps 3 s printing nothing; reader events: {events:?}");
    say!("[6a] child exited: {:?}", s.child.try_wait().map(|o| o.map(|x| x.exit_code())).ok());
    Ok(())
}

/// 4 MiB written to a child that never reads: does the writer block?
fn blocked_writer() -> Result<()> {
    let mut s = spawn(helper(&["sleep", "6"])?, 80, 24)?;
    s.pump(Duration::from_millis(300));
    let mut writer = s.writer.take().context("writer")?;
    let written = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let (w2, d2) = (Arc::clone(&written), Arc::clone(&done));
    // Detached on purpose: this thread may stay blocked in `write_all`, which is the finding.
    std::thread::spawn(move || {
        let chunk = vec![b'a'; 4096];
        for _ in 0..1024 {
            if writer.write_all(&chunk).is_err() {
                break;
            }
            w2.fetch_add(chunk.len(), Ordering::Relaxed);
        }
        d2.store(true, Ordering::Relaxed);
    });
    for sec in 1..=4 {
        s.pump(Duration::from_secs(1)); // keep draining the echo so only the input side can block
        say!(
            "[6b] writing 4 MiB to a child that never reads: t={sec}s written={} KiB completed={}",
            written.load(Ordering::Relaxed) / 1024,
            done.load(Ordering::Relaxed)
        );
    }
    s.child.kill().ok(); // cleanup only
    Ok(())
}

fn delta(before: usize, after: usize) -> i64 {
    i64::try_from(after).unwrap_or(0) - i64::try_from(before).unwrap_or(0)
}

fn report_delta(label: &str, before: Mem, after: Mem) {
    say!(
        "{label}: process KiB (ws,peak,private) base={before:?} with={after:?} delta_ws={} delta_private={}",
        delta(before.0, after.0),
        delta(before.2, after.2)
    );
}

/// Ten bare threads with a 256 KiB stack blocked on a channel: the pure thread cost.
fn bare_threads() -> Result<()> {
    let base = mem();
    let mut handles = Vec::new();
    for i in 0..SESSIONS {
        let (tx, rx) = mpsc::channel::<()>();
        let handle = std::thread::Builder::new()
            .name(format!("pty-read-{i}"))
            .stack_size(256 * 1024)
            .spawn(move || rx.recv().ok())
            .context("spawn bare thread")?;
        handles.push((tx, handle));
    }
    std::thread::sleep(Duration::from_millis(300));
    report_delta("[6c] 10 bare blocked threads @256 KiB stack", base, mem());
    for (tx, handle) in handles {
        drop(tx); // unblocks `recv`
        handle.join().ok(); // a panicked thread cannot happen here; nothing to report
    }
    Ok(())
}

#[cfg(windows)]
fn conhost() -> (usize, usize) {
    use std::process::Command;
    let filter = "IMAGENAME eq conhost.exe";
    let out =
        Command::new("tasklist").args([concat!("/F", "O"), "CSV", "/NH", "/FI", filter]).output();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let rows: Vec<&str> = text.lines().filter(|l| l.contains("conhost")).collect();
    let kib = rows
        .iter()
        .filter_map(|l| l.rsplit(',').next())
        .filter_map(|m| {
            m.chars().filter(char::is_ascii_digit).collect::<String>().parse::<usize>().ok()
        })
        .sum();
    (rows.len(), kib)
}

/// Ten real sessions: one `pty-read` thread, 64 KiB buffer and bounded channel each.
fn real_sessions() -> Result<()> {
    #[cfg(windows)]
    let hosts_before = conhost();
    let base = mem();
    let sessions = (0..SESSIONS)
        .map(|_| spawn(helper(&["sleep", "6"])?, 80, 24))
        .collect::<Result<Vec<Sess>>>()?;
    std::thread::sleep(Duration::from_millis(1500));
    report_delta(
        "[6d] 10 real sessions (1 pty-read thread each, 64 KiB buffer, bounded channel 32)",
        base,
        mem(),
    );
    #[cfg(windows)]
    say!(
        "[6d] conhost.exe (count, KiB) before={hosts_before:?} during={:?} (ConPTY host processes live outside the Ferrum process)",
        conhost()
    );
    for mut s in sessions {
        s.child.kill().ok(); // cleanup only
    }
    Ok(())
}
