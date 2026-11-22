//! Helper child subcommands (`flood`, `sleep`, `exit`, `env`, `spawn-sleep`, `echo-bye`): tiny
//! portable children so the experiments do not depend on shell tricks.

use crate::out::say;
use crate::sysinfo::exe;
use anyhow::{Context, Result};
use std::io::Write as _;
use std::process::Command;
use std::time::Duration;

/// Last line written by `flood`; the readers look for it.
pub(crate) const MARKER: &str = "FERRUM_FLOOD_END";
/// Lines of 64 bytes (CRLF included): 16 384 lines per MiB.
pub(crate) const LINES_PER_MIB: usize = 16 * 1024;

/// Writes the one 64-byte line number `i`: `L` + 9 digits + space + 51 fill + CRLF.
pub(crate) fn write_flood_line(out: &mut impl std::io::Write, i: usize) -> std::io::Result<()> {
    write!(out, "L{i:09} {:.>51}\r\n", "")
}

/// Runs a helper subcommand; `Ok(false)` means `args` is not a helper command.
pub(crate) fn run(args: &[String]) -> Result<bool> {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["flood", mib] => flood(mib.parse().context("flood MiB")?)?,
        ["sleep", secs] => std::thread::sleep(Duration::from_secs(secs.parse().context("secs")?)),
        ["exit", code] => std::process::exit(code.parse().context("exit code")?),
        ["echo-bye", tag] => say!("BYE_{tag}"),
        ["env"] => env_report()?,
        ["spawn-sleep"] => spawn_sleep()?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn flood(mib: usize) -> Result<()> {
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::with_capacity(64 * 1024, stdout.lock());
    for i in 0..mib * LINES_PER_MIB {
        write_flood_line(&mut out, i)?;
    }
    write!(out, "{MARKER}\r\n")?;
    out.flush()?;
    Ok(())
}

fn env_report() -> Result<()> {
    for key in ["TERM", "COLORTERM", "TERM_PROGRAM", "FERRUM_ID", "S4_UNICODE"] {
        say!("ENV {key}={:?}", std::env::var(key).ok());
    }
    say!("ENV cwd={}", std::env::current_dir()?.display());
    Ok(())
}

fn spawn_sleep() -> Result<()> {
    let mut child = Command::new(exe()?).args(["sleep", "30"]).spawn()?;
    say!("GRANDCHILD {}", child.id());
    std::io::stdout().flush()?;
    child.wait()?;
    Ok(())
}
