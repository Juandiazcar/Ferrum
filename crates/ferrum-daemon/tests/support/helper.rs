//! The session child of the daemon tests: the test executable itself, started by the daemon
//! through the helper profile (`harness.rs`) with `HELPER_VAR` set. It reports its directory
//! with `OSC 7` at once (before its session is announced, which `sessions.rs` relies on), prints
//! `READY <cwd>`, then answers each input line:
//!
//! | Line | Answer |
//! |---|---|
//! | `lines <n>` | `LINE0` .. `LINE<n-1>`, then `DONE<n>` |
//! | `wide <n>` | `W<i>` padded to 60 columns with `=`, for `i` in `0..n`, then `DONE<n>` |
//! | `flood` | numbered lines forever, in blocks of [`FLOOD_BLOCK_BYTES`], the title `flood-<k>` after every [`FLOOD_MARK_BYTES`] |
//! | `cd <dir>` | reports `<dir>` with `OSC 7`, then `CD-OK` |
//! | `title <text>` | sets the title with `OSC 2`, then `TITLE-OK` |
//! | anything else `x` | `OUT:x` |

#![allow(clippy::unwrap_used, dead_code, unreachable_pub, reason = "test support")]

use std::io::{BufRead, Write};

/// Bytes per write of the flood: `yes` writes blocks of this order (NFR-PERF-04, "at full speed").
pub const FLOOD_BLOCK_BYTES: usize = 32 * 1_024;
/// Bytes of flood between two title marks, from which the test computes the throughput.
pub const FLOOD_MARK_BYTES: usize = 2 * 1_024 * 1_024;
/// Columns of a `wide` line: wider than the 40 columns a test reflows to.
const WIDE_COLS: usize = 60;

/// A command a test file adds: answers `line` and returns `Some`, or `None` for other lines.
pub type Extra = fn(&str, &mut dyn Write) -> Option<std::io::Result<()>>;

/// Runs the helper when this process is the daemon's child; returns otherwise.
pub fn run_if_child() {
    run_if_child_with(|_, _| None);
}

/// Like [`run_if_child`], with the test file's own commands tried first.
pub fn run_if_child_with(extra: Extra) {
    if std::env::var_os(super::harness::HELPER_VAR).is_none() {
        return;
    }
    let mut out = std::io::stdout().lock();
    let cwd = std::env::current_dir().unwrap();
    osc7(&mut out, &cwd.to_string_lossy()).unwrap();
    writeln!(out, "READY {}", cwd.display()).unwrap();
    out.flush().unwrap();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let answered =
            extra(line.trim(), &mut out).unwrap_or_else(|| answer(&mut out, line.trim()));
        if answered.is_err() {
            break;
        }
    }
    std::process::exit(0);
}

fn answer(out: &mut impl Write, line: &str) -> std::io::Result<()> {
    if let Some(count) = line.strip_prefix("lines ") {
        let count: u64 = count.parse().unwrap_or(0);
        for index in 0..count {
            writeln!(out, "LINE{index}")?;
        }
        writeln!(out, "DONE{count}")?;
    } else if let Some(count) = line.strip_prefix("wide ") {
        let count: u64 = count.parse().unwrap_or(0);
        for index in 0..count {
            writeln!(out, "{:=<WIDE_COLS$}", format!("W{index}-"))?;
        }
        writeln!(out, "DONE{count}")?;
    } else if line == "flood" {
        flood(out)?;
    } else if let Some(dir) = line.strip_prefix("cd ") {
        osc7(out, dir)?;
        writeln!(out, "CD-OK")?;
    } else if let Some(title) = line.strip_prefix("title ") {
        write!(out, "\x1b]2;{title}\x07")?;
        writeln!(out, "TITLE-OK")?;
    } else {
        writeln!(out, "OUT:{line}")?;
    }
    out.flush()
}

/// Writes numbered lines forever in large blocks, one `write` per block, as `yes` does.
fn flood(out: &mut impl Write) -> std::io::Result<()> {
    let mut block = Vec::with_capacity(FLOOD_BLOCK_BYTES + 128);
    let (mut index, mut since_mark, mut marks) = (0_u64, 0, 0_u64);
    loop {
        block.clear();
        while block.len() < FLOOD_BLOCK_BYTES {
            writeln!(
                block,
                "F{index:08} yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy"
            )?;
            index += 1;
        }
        since_mark += block.len();
        if since_mark >= FLOOD_MARK_BYTES {
            since_mark = 0;
            marks += 1;
            write!(block, "\x1b]2;flood-{marks}\x07")?;
        }
        out.write_all(&block)?;
    }
}

/// Reports `dir` as the working directory with `OSC 7`.
fn osc7(out: &mut impl Write, dir: &str) -> std::io::Result<()> {
    let path = dir.replace('\\', "/");
    let path = if path.starts_with('/') { path } else { format!("/{path}") };
    write!(out, "\x1b]7;file://localhost{}\x07", path.replace(' ', "%20"))
}
