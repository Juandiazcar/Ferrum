//! Experiment 8: teardown. Does closing the master hang with an unread flooding child?

use crate::out::say;
use crate::session::spawn;
use crate::sysinfo::helper;
use anyhow::Result;
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub(crate) fn run() -> Result<()> {
    say!("\n=== [8] teardown semantics ===");
    let mut s = spawn(helper(&["flood", "64"])?, 80, 24)?;
    std::thread::sleep(Duration::from_millis(500)); // the reader is blocked on the full channel
    let (tx, rx) = mpsc::channel::<String>();
    // Detached on purpose: if the close hangs, only this thread hangs.
    std::thread::spawn(move || {
        let t = Instant::now();
        s.close();
        tx.send(format!("{:?}", t.elapsed())).ok(); // receiver gone = main already timed out
        let exit = s.wait_exit(Duration::from_secs(5));
        let text = exit.map_or_else(
            || "still running".to_string(),
            |(d, st)| format!("exit_code={:#x} ({}) after {d:?}", st.exit_code(), st.exit_code()),
        );
        tx.send(text).ok(); // same
    });
    match rx.recv_timeout(Duration::from_secs(10)) {
        Ok(d) => say!(
            "[8a] drop(master) with a flooding child and an unread output pipe returned in {d:?}"
        ),
        Err(_) => say!(
            "[8a] drop(master) HUNG > 10 s with unread output (ClosePseudoConsole blocks until output is drained)"
        ),
    }
    match rx.recv_timeout(Duration::from_secs(8)) {
        Ok(c) => say!("[8a] child after master drop: {c}"),
        Err(_) => say!("[8a] child state unknown 8 s after master drop"),
    }
    Ok(())
}
