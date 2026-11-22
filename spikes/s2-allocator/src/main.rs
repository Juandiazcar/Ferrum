//! Spike S2 (docs/12, docs/03 §7): allocator comparison on spike S1's workload. THROWAWAY;
//! results in docs/adr/0016.
//!
//! The workload is S1's, reused verbatim: the generator, the `vte` grid and the scrollback encoder
//! are S1's source files compiled into this crate (attribution: `spikes/s1-memory-model`, ADR
//! 0024). Ten sessions (seeds 1 to 10), each fed 12 000 logical lines with 10 000 rows retained,
//! are held in one process, then the process idles 10 s (steady state, docs/03 §1), prints its
//! PID and waits so that an external probe (`run-all.sh`) can sample the OS memory metric.
//! Usage: `ferrum-spike-s2 [--sessions 10] [--lines 12000] [--retain 10000] [--idle-secs 10]
//! [--hold-secs 120]`

// S1's modules are reused through `#[path]`; they carry S1 items this spike does not call
// (corpus/entropy profiles, hash round-trip verification, capacity reports), hence `dead_code`.
#[allow(dead_code)] // INVARIANT: unused S1 items are not ours to delete (reuse verbatim)
#[path = "../../s1-memory-model/src/content.rs"]
mod content;
#[allow(dead_code)] // INVARIANT: see `content`
#[path = "../../s1-memory-model/src/grid.rs"]
mod grid;
#[allow(dead_code)] // INVARIANT: see `content`
#[path = "../../s1-memory-model/src/scrollback.rs"]
mod scrollback;

mod alloc;

use std::io::Write as _;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use content::{Generator, Profile};
use grid::Grid;
use scrollback::Scrollback;

const FEED_BUF: usize = 64 * 1024; // docs/03 §2 "read buffer 64 KiB" (as S1)
const HOT_STEP: usize = 16_384; // S1's bounded hot-chunk growth (ADR 0024, requirement for WP-01)

struct Args {
    sessions: u64,
    lines: usize,
    retain: usize,
    idle_secs: u64,
    hold_secs: u64,
}

fn parse_args() -> Result<Args> {
    let mut a = Args { sessions: 10, lines: 12_000, retain: 10_000, idle_secs: 10, hold_secs: 120 };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let v = it.next().with_context(|| format!("{k} needs a value"))?;
        match k.as_str() {
            "--sessions" => a.sessions = v.parse()?,
            "--lines" => a.lines = v.parse()?,
            "--retain" => a.retain = v.parse()?,
            "--idle-secs" => a.idle_secs = v.parse()?,
            "--hold-secs" => a.hold_secs = v.parse()?,
            o => bail!("unknown argument {o}"),
        }
    }
    Ok(a)
}

/// One session as S1's `run_grid` builds it; the 64 KiB read buffer stays alive with it (as in S1).
struct Session {
    grid: Grid<Scrollback>,
    _feed_buf: Vec<u8>,
}

/// Feeds `lines` logical lines of the `mixed` profile into a new session. Returns the session and
/// the nanoseconds spent in `parser.advance` (generation excluded, as in S1).
fn fill_session(seed: u64, lines: usize, retain: usize) -> Result<(Session, u128)> {
    let mut generator = Generator::new(seed, Profile::Mixed, vec![]);
    let mut parser = vte::Parser::new();
    let mut buf = Vec::with_capacity(FEED_BUF);
    let mut grid = Grid::new(Scrollback::new(retain, HOT_STEP));
    let mut ns = 0u128;
    let mut done = 0;
    while done < lines {
        buf.clear();
        while done < lines && buf.len() + 2048 < FEED_BUF {
            generator.next_line(&mut buf);
            done += 1;
        }
        let t = Instant::now();
        parser.advance(&mut grid, &buf);
        ns += t.elapsed().as_nanos();
    }
    if let Some(e) = grid.error.take() {
        return Err(e);
    }
    if grid.sink.retained() != retain {
        bail!("seed {seed}: retained {} != {retain}", grid.sink.retained());
    }
    Ok((Session { grid, _feed_buf: buf }, ns))
}

fn out(line: &str) -> Result<()> {
    let mut o = std::io::stdout().lock();
    writeln!(o, "{line}")?;
    o.flush()?; // the probe polls this output
    Ok(())
}

fn main() -> Result<()> {
    let a = parse_args()?;
    let mut sessions = Vec::new();
    let (mut total_lines, mut total_ns) = (0usize, 0u128);
    for seed in 1..=a.sessions {
        let (s, ns) = fill_session(seed, a.lines, a.retain)?;
        sessions.push(s);
        total_lines += a.lines;
        total_ns += ns;
    }
    let rows: usize = sessions.iter().map(|s| s.grid.sink.retained()).sum();
    let lps = fl(total_lines) / (fl(total_ns) / 1e9);
    out(&format!("allocator {}", alloc::NAME))?;
    out(&format!("sessions {} lines_each {} rows_retained_total {rows}", a.sessions, a.lines))?;
    out(&format!("throughput.lines_per_s {lps:.0}"))?;
    out("FILL_DONE")?;
    std::thread::sleep(Duration::from_secs(a.idle_secs));
    out(&format!("STEADY pid={}", std::process::id()))?;
    // The probe samples now and kills this process; the timeout only bounds a forgotten run.
    std::thread::sleep(Duration::from_secs(a.hold_secs));
    drop(sessions);
    Ok(())
}

/// Report-only conversion; counts and nanoseconds are far below 2^52, so it is exact.
#[allow(clippy::cast_precision_loss)] // INVARIANT: values < 2^52
fn fl(n: impl TryInto<u64>) -> f64 {
    n.try_into().unwrap_or(u64::MAX) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_session_retains_exactly() -> Result<()> {
        let (s, ns) = fill_session(1, 12_000, 10_000)?;
        assert_eq!(s.grid.sink.retained(), 10_000);
        assert!(ns > 0);
        Ok(())
    }
}
