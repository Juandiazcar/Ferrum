//! Experiment 5: the exact bytes the pty emits at startup, with and without answering the cursor query.

use crate::bytes::{contains, describe, hex};
use crate::helpers::MARKER;
use crate::out::say;
use crate::session::spawn;
use crate::shells::{Shell, shells};
use crate::sysinfo::helper;
use anyhow::Result;
use std::time::Duration;

const CURSOR_QUERY: &[u8] = b"\x1b[6n";
const CURSOR_REPLY: &[u8] = b"\x1b[1;1R";

pub(crate) fn run() -> Result<()> {
    say!("\n=== [5] initial bytes at attach ===");
    for sh in shells() {
        shell_startup(sh);
    }
    plain_child_startup()
}

/// Never answers the query at first: does the shell start? Then answers and shows what follows.
fn shell_startup(sh: Shell) {
    let name = sh.name;
    let mut s = match spawn(sh.cmd, 80, 24) {
        Ok(s) => s,
        Err(e) => {
            say!("[5] {name}: SPAWN FAILED {e:#}");
            return;
        }
    };
    s.auto_dsr = false;
    s.pump(Duration::from_millis(2000));
    let query = contains(&s.buf, CURSOR_QUERY);
    say!(
        "[5] {name}: no-reply run: bytes_in_2s={} contains_DSR_query(ESC[6n)={query}",
        s.buf.len()
    );
    say!("[5] {name}: hex(first 192)={}", hex(&s.buf, 192));
    for d in describe(&s.buf).iter().take(40) {
        say!("[5]     {d}");
    }
    if query {
        let before = s.buf.len();
        s.send(CURSOR_REPLY);
        s.pump(Duration::from_millis(1500));
        let after = s.buf.get(before..).unwrap_or_default().to_vec();
        say!("[5] {name}: after replying ESC[1;1R: +{} bytes", after.len());
        for d in describe(&after).iter().take(25) {
            say!("[5]     {d}");
        }
    }
    s.send(b"exit\r");
    s.wait_exit(Duration::from_secs(3));
}

/// A plain child that prints one line and exits: is its output held back until the query is answered?
fn plain_child_startup() -> Result<()> {
    let mut s = spawn(helper(&["flood", "0"])?, 80, 24)?;
    s.auto_dsr = false;
    s.pump(Duration::from_millis(1500));
    let held = s.buf.len();
    let exited = s.child.try_wait().map(|o| o.is_some()).ok();
    s.send(CURSOR_REPLY);
    s.pump(Duration::from_millis(1000));
    say!(
        "[5b] plain child (prints one line then exits), DSR unanswered for 1.5 s: bytes_seen={held} child_exited={exited:?}; after reply: total_bytes={} marker_seen={}",
        s.buf.len(),
        contains(&s.buf, MARKER.as_bytes())
    );
    for d in describe(&s.buf).iter().take(20) {
        say!("[5b]     {d}");
    }
    Ok(())
}
