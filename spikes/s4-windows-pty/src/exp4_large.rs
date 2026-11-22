//! Experiment 4: 64 MiB of output through the pty (helper flood and `type`/`cat` of a file).

use crate::helpers::{LINES_PER_MIB, MARKER, write_flood_line};
use crate::out::say;
use crate::session::spawn;
use crate::stats::consume;
use crate::sysinfo::{helper, mem};
use anyhow::{Context, Result};
use portable_pty::CommandBuilder;
use std::io::Write as _;
use std::path::Path;
use std::time::Duration;

const DEFAULT_MIB: usize = 64;
const DRAIN_TIMEOUT: Duration = Duration::from_secs(180);

pub(crate) fn run() -> Result<()> {
    say!("\n=== [4] large output ===");
    let mib = std::env::var("S4_FLOOD_MB").ok().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_MIB);
    helper_flood(mib)?;
    file_flood(mib)
}

/// Bytes the flood child writes, marker line included.
fn sent_bytes(mib: usize) -> usize {
    (mib << 20) + MARKER.len() + 2
}

fn helper_flood(mib: usize) -> Result<()> {
    let before = mem();
    let mut s = spawn(helper(&["flood", &mib.to_string()])?, 200, 55)?;
    let stats = consume(&mut s, DRAIN_TIMEOUT, None);
    stats.print(&format!("4a helper flood {mib} MB"), sent_bytes(mib));
    say!("[4a] process mem KiB (ws,peak,private) before={before:?} after={:?}", mem());
    Ok(())
}

fn write_flood_file(path: &Path, mib: usize) -> Result<()> {
    let mut out =
        std::io::BufWriter::new(std::fs::File::create(path).context("create flood file")?);
    for i in 0..mib * LINES_PER_MIB {
        write_flood_line(&mut out, i)?;
    }
    write!(out, "{MARKER}\r\n")?;
    out.flush()?;
    Ok(())
}

fn print_file_command(path: &Path) -> CommandBuilder {
    #[cfg(windows)]
    {
        let mut cmd = CommandBuilder::new("cmd.exe");
        cmd.args(["/c", "type", &path.to_string_lossy()]);
        cmd
    }
    #[cfg(unix)]
    {
        let mut cmd = CommandBuilder::new("cat");
        cmd.arg(path);
        cmd
    }
}

fn file_flood(mib: usize) -> Result<()> {
    let path = std::env::temp_dir().join("s4-flood.txt");
    write_flood_file(&path, mib)?;
    let before = mem();
    let mut s = spawn(print_file_command(&path), 200, 55)?;
    let stats = consume(&mut s, DRAIN_TIMEOUT, None);
    stats.print(&format!("4b file {mib} MB via cmd type / cat"), sent_bytes(mib));
    say!("[4b] process mem KiB (ws,peak,private) before={before:?} after={:?}", mem());
    std::fs::remove_file(&path).context("remove flood file")
}
