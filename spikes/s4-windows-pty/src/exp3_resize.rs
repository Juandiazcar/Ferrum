//! Experiment 3: resize during output, repaint bytes, scrolled content, child-visible size.

use crate::bytes::{describe, hex};
use crate::out::say;
use crate::session::{Sess, size, spawn};
use crate::shells::idle_shell;
use crate::stats::consume;
use crate::sysinfo::helper;
use anyhow::{Context, Result};
use portable_pty::MasterPty;
use std::time::{Duration, Instant};

const STORM_MIB: usize = 16;
const STORM_INTERVAL: Duration = Duration::from_millis(40);

pub(crate) fn run() -> Result<()> {
    say!("\n=== [3] resize ===");
    storm()?;
    let mut s = spawn(idle_shell(), 80, 24)?;
    s.pump(Duration::from_millis(1500));
    let master = s.master.take().context("master")?;
    idle_resizes(&mut s, master.as_ref());
    scrolled_resize(&mut s, master.as_ref());
    child_size(&mut s);
    s.send(b"exit\r");
    Ok(())
}

/// 16 MiB flood while resizing 80x24 <-> 120x40 every 40 ms.
fn storm() -> Result<()> {
    let mut s = spawn(helper(&["flood", &STORM_MIB.to_string()])?, 80, 24)?;
    let master = s.master.take().context("master")?;
    let (mut ok, mut errors, mut flip) = (0usize, 0usize, false);
    let mut last = Instant::now();
    let mut tick = || {
        if last.elapsed() < STORM_INTERVAL {
            return;
        }
        last = Instant::now();
        flip = !flip;
        let (cols, rows) = if flip { (120, 40) } else { (80, 24) };
        match master.resize(size(cols, rows)) {
            Ok(()) => ok += 1,
            Err(_) => errors += 1,
        }
    };
    let stats = consume(&mut s, Duration::from_secs(120), Some(&mut tick));
    stats.print("3a flood 16 MB with resize every 40 ms", STORM_MIB << 20);
    say!("[3a] resizes_ok={ok} resize_errors={errors}");
    Ok(())
}

/// Resizes the idle shell and prints what the pty emits in the next 700 ms.
fn idle_resizes(s: &mut Sess, master: &dyn MasterPty) {
    let steps = [
        ("same size 80x24", 80, 24),
        ("grow 100x30", 100, 30),
        ("shrink 60x20", 60, 20),
        ("back 80x24", 80, 24),
    ];
    for (label, cols, rows) in steps {
        s.buf.clear();
        let t = Instant::now();
        let result = master.resize(size(cols, rows));
        let call = t.elapsed();
        s.pump(Duration::from_millis(700));
        say!(
            "[3b] resize {label}: result={result:?} call={call:?} bytes_after_700ms={} hex={}",
            s.buf.len(),
            hex(&s.buf, 96)
        );
        for d in describe(&s.buf).iter().take(12) {
            say!("[3b]     {d}");
        }
    }
    say!("[3b] get_size after = {:?}", master.get_size().map(|z| (z.cols, z.rows)).ok());
}

#[cfg(windows)]
const PRINT_60_LINES: &str = "for /L %i in (1,1,60) do @echo LINE%i\r";
#[cfg(unix)]
const PRINT_60_LINES: &str = "i=1; while [ $i -le 60 ]; do echo LINE$i; i=$((i+1)); done\r";

/// 60 lines in a 24-row pty, then a resize: how much is re-emitted?
fn scrolled_resize(s: &mut Sess, master: &dyn MasterPty) {
    s.buf.clear();
    s.send(PRINT_60_LINES.as_bytes());
    s.pump_until("LINE60", Duration::from_secs(5));
    s.pump(Duration::from_millis(500));
    let printed = String::from_utf8_lossy(&s.buf).matches("LINE").count();
    s.buf.clear();
    let result = master.resize(size(100, 30));
    s.pump(Duration::from_millis(800));
    let text = String::from_utf8_lossy(&s.buf).into_owned();
    let first = text
        .find("LINE")
        .map(|i| text.get(i..).unwrap_or_default().chars().take(8).collect::<String>());
    say!(
        "[3c] 60 lines printed (LINE occurrences while printing={printed}); resize 80x24 -> 100x30: result={result:?} bytes_emitted={} LINE_occurrences_in_repaint={} first_line_in_repaint={first:?}",
        s.buf.len(),
        text.matches("LINE").count()
    );
    for d in describe(&s.buf).iter().take(10) {
        say!("[3c]     {d}");
    }
}

#[cfg(windows)]
const ASK_SIZE: (&str, &str) = ("mode con\r", "Columns:");
#[cfg(unix)]
const ASK_SIZE: (&str, &str) = ("stty size\r", "30 100");

/// Does the child see the size set by the last resize (100x30)?
fn child_size(s: &mut Sess) {
    let (ask, want) = ASK_SIZE;
    s.buf.clear();
    s.send(ask.as_bytes());
    let seen = s.pump_until(want, Duration::from_secs(5));
    let text = String::from_utf8_lossy(&s.buf).into_owned();
    let sizes: Vec<&str> = text
        .lines()
        .filter(|l| l.contains("Columns") || l.contains("Lines") || l.contains("30 100"))
        .map(str::trim)
        .collect();
    say!(
        "[3d] child-visible size after resize to 100x30: reply_seen={} lines={sizes:?}",
        seen.is_some()
    );
}
