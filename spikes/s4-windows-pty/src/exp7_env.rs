//! Experiment 7: environment injection and working directory.

use crate::out::say;
use crate::session::spawn;
use crate::sysinfo::helper;
use anyhow::Result;
use portable_pty::CommandBuilder;
use std::time::Duration;

pub(crate) fn run() -> Result<()> {
    say!("\n=== [7] env and cwd ===");
    let temp = std::env::temp_dir();
    report("defaults (inherit)", helper(&["env"])?)?;
    let mut cmd = helper(&["env"])?;
    cmd.env_remove("TERM");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("TERM_PROGRAM", "Ferrum");
    cmd.env("FERRUM_ID", "s-1");
    cmd.env("S4_UNICODE", "x\u{e9}-\u{4e16}\u{754c}");
    cmd.cwd(&temp);
    report("env_remove(TERM)+COLORTERM+TERM_PROGRAM+FERRUM_ID+unicode, cwd=temp", cmd)?;
    say!("[7] expected cwd = {}", temp.display());
    Ok(())
}

fn report(label: &str, cmd: CommandBuilder) -> Result<()> {
    let mut s = spawn(cmd, 80, 24)?;
    s.wait_exit(Duration::from_secs(5));
    s.pump(Duration::from_millis(300));
    say!("[7] {label}:");
    for line in String::from_utf8_lossy(&s.buf).lines().filter(|l| l.contains("ENV ")) {
        say!("[7]     {}", line.trim());
    }
    Ok(())
}
