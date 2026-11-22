//! Experiment 1: spawn each shell, write input, verify the echo and the result.

use crate::bytes::contains;
use crate::out::say;
use crate::session::spawn;
use crate::shells::{Shell, shells};
use anyhow::Result;
use std::time::Duration;

// Uniform experiment signature (`main::EXPERIMENTS`); spawn failures are observations, not errors.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn run() -> Result<()> {
    say!("\n=== [1] spawn shells, write input, verify echo ===");
    for sh in shells() {
        check_shell(sh);
    }
    Ok(())
}

fn check_shell(sh: Shell) {
    let name = sh.name;
    let mut s = match spawn(sh.cmd, 80, 24) {
        Ok(s) => s,
        Err(e) => {
            say!("[1] {name}: SPAWN FAILED: {e:#}");
            return;
        }
    };
    s.pump(Duration::from_millis(1500)); // let the prompt settle
    let startup_bytes = s.buf.len();
    s.send(sh.line.as_bytes());
    let result = s.pump_until(sh.marker, Duration::from_secs(5));
    let typed_echoed = s.buf.get(startup_bytes..).is_some_and(|t| contains(t, b"echo"));
    say!(
        "[1] {name}: startup_bytes={startup_bytes} result_marker_seen={} after={result:?} typed_text_echoed={typed_echoed} dsr_answered={}",
        result.is_some(),
        s.dsr_answers
    );
    s.send(b"exit\r");
    let exit = s.wait_exit(Duration::from_secs(5));
    say!("[1] {name}: exit after `exit` -> {:?}", exit.map(|(d, st)| (d, st.exit_code())));
}
