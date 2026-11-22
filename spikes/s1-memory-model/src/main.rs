//! Spike S1 (docs/12): scrollback memory-model measurement. THROWAWAY; results in docs/adr/0024.
//!
//! One process measures one model (`ferrum`, `naive`, `alacritty`) so allocator deltas are clean.
//! Heap figures need the default `dhat-heap` feature; `--no-default-features` gives undistorted throughput only.
//! Usage: `ferrum-spike-s1 --model ferrum|naive|alacritty [--seed N] [--profile mixed|corpus|entropy]
//! [--corpus-dir DIR] [--lines 12000] [--retain 10000]`

mod alloc;
mod content;
mod grid;
mod scrollback;

use std::collections::VecDeque;
use std::io::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use content::{Generator, Profile, Stats};
use grid::{COLS, Cell, FLAG_SPACER, Grid, ROWS, Sink};
use scrollback::{Fnv, Scrollback};

const FEED_BUF: usize = 64 * 1024; // docs/03 §2 "read buffer 64 KiB"

struct Args {
    model: String,
    seed: u64,
    profile: Profile,
    corpus_dir: PathBuf,
    lines: usize,
    retain: usize,
    hot_step: usize,
}

fn parse_args() -> Result<Args> {
    let mut a = Args {
        model: "ferrum".into(),
        seed: 1,
        profile: Profile::Mixed,
        corpus_dir: PathBuf::from("docs/spec"),
        lines: 12_000,
        retain: 10_000,
        hot_step: 0,
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let v = it.next().with_context(|| format!("{k} needs a value"))?;
        match k.as_str() {
            "--model" => a.model = v,
            "--seed" => a.seed = v.parse()?,
            "--lines" => a.lines = v.parse()?,
            "--retain" => a.retain = v.parse()?,
            "--hot-step" => a.hot_step = v.parse()?,
            "--corpus-dir" => a.corpus_dir = v.into(),
            "--profile" => {
                a.profile = match v.as_str() {
                    "mixed" => Profile::Mixed,
                    "corpus" => Profile::Corpus,
                    "entropy" => Profile::Entropy,
                    o => bail!("unknown profile {o}"),
                }
            }
            o => bail!("unknown argument {o}"),
        }
    }
    Ok(a)
}

/// Naive scrollback: every scrolled-off row kept as a full row of 16-byte cells.
struct NaiveSink {
    rows: VecDeque<Box<[Cell]>>,
    cap: usize,
}

impl Sink for NaiveSink {
    fn push_row(&mut self, cells: &[Cell], _wrapped: bool) -> Result<()> {
        if self.rows.len() == self.cap {
            self.rows.pop_front();
        }
        self.rows.push_back(cells.into());
        Ok(())
    }
}

/// Reference: hashes each scrolled-off row straight from its cells (independent of the encoder).
#[derive(Default)]
struct RefSink(Vec<u64>);

impl Sink for RefSink {
    fn push_row(&mut self, cells: &[Cell], wrapped: bool) -> Result<()> {
        let mut end = cells.len();
        while end > 0
            && cells.get(end - 1).is_some_and(|c| {
                (c.ch == 0 || c.ch == 32) && c.fg == 0 && c.bg == 0 && c.attrs == 0
            })
        {
            end -= 1;
        }
        let mut h = Fnv::new();
        h.byte(u8::from(wrapped));
        for c in cells.iter().take(end).filter(|c| c.flags & FLAG_SPACER == 0) {
            h.cell(if c.ch == 0 { 32 } else { c.ch }, c.fg, c.bg, c.attrs);
        }
        self.0.push(h.0);
        Ok(())
    }
}

/// Result of feeding the scenario to one model.
struct Run {
    generator: Stats,
    advance_ns: u128,
    feed_cap: usize,
    /// Kept alive so the 64 KiB read buffer counts in the session total.
    _feed_buf: Vec<u8>,
}

fn new_gen(a: &Args) -> Result<Generator> {
    let corpus =
        if a.profile == Profile::Corpus { content::load_corpus(&a.corpus_dir)? } else { vec![] };
    if a.profile == Profile::Corpus && corpus.is_empty() {
        bail!("empty corpus in {}", a.corpus_dir.display());
    }
    Ok(Generator::new(a.seed, a.profile, corpus))
}

/// Generates `lines` logical lines into 64 KiB feeds and passes each feed to `advance`.
fn feed(
    generator: &mut Generator,
    lines: usize,
    buf: &mut Vec<u8>,
    mut advance: impl FnMut(&[u8]) -> Result<()>,
) -> Result<u128> {
    let mut ns = 0u128;
    let mut done = 0;
    while done < lines {
        buf.clear();
        while done < lines && buf.len() + 2048 < FEED_BUF {
            generator.next_line(buf);
            done += 1;
        }
        let t = Instant::now();
        advance(buf)?;
        ns += t.elapsed().as_nanos();
    }
    Ok(ns)
}

fn run_grid<S: Sink>(a: &Args, sink: S) -> Result<(Run, Grid<S>)> {
    let mut generator = new_gen(a)?;
    let mut parser = vte::Parser::new();
    let mut buf = Vec::with_capacity(FEED_BUF);
    let mut grid = Grid::new(sink);
    let advance_ns = feed(&mut generator, a.lines, &mut buf, |b| {
        parser.advance(&mut grid, b);
        Ok(())
    })?;
    if let Some(e) = grid.error.take() {
        return Err(e);
    }
    let run = Run {
        generator: generator.stats.clone(),
        advance_ns,
        feed_cap: buf.capacity(),
        _feed_buf: buf,
    };
    Ok((run, grid))
}

fn out(line: &str) -> Result<()> {
    writeln!(std::io::stdout(), "{line}")?;
    Ok(())
}

fn kv(k: &str, v: impl std::fmt::Display) -> Result<()> {
    out(&format!("{k} {v}"))
}

fn report_common(a: &Args, r: &Run, live: usize, peak: usize, calls: u64) -> Result<()> {
    let s = &r.generator;
    let pct = |n: usize| fl(n) * 100.0 / fl(s.lines);
    kv("model", &a.model)?;
    kv("profile", format!("{:?}", a.profile).to_lowercase())?;
    kv("seed", a.seed)?;
    kv("generator.lines", s.lines)?;
    kv("generator.colored_pct", format!("{:.2}", pct(s.colored)))?;
    kv("generator.non_ascii_pct", format!("{:.2}", pct(s.non_ascii)))?;
    kv("generator.long_pct", format!("{:.2}", pct(s.long)))?;
    kv("generator.avg_visible_cols", format!("{:.2}", fl(s.cols) / fl(s.lines)))?;
    kv(
        "generator.avg_sgr_runs_per_colored",
        format!("{:.2}", fl(s.sgr_runs) / fl(s.colored.max(1))),
    )?;
    kv("generator.avg_bytes_per_line_on_wire", format!("{:.1}", fl(s.bytes) / fl(s.lines)))?;
    kv("alloc.enabled", alloc::ENABLED)?;
    if alloc::ENABLED {
        kv("alloc.session_live_bytes", live)?;
        kv("alloc.process_peak_bytes", peak)?;
        kv("alloc.session_blocks", calls)?;
    }
    kv("throughput.lines_per_s", format!("{:.0}", fl(s.lines) / (fl(r.advance_ns) / 1e9)))?;
    Ok(())
}

fn run_ferrum(a: &Args) -> Result<()> {
    let base = alloc::live();
    let calls0 = alloc::blocks();
    let (run, grid) = run_grid(a, Scrollback::new(a.retain, a.hot_step))?;
    let live = alloc::live() - base;
    let peak = alloc::peak();
    let calls = alloc::blocks() - calls0;
    report_common(a, &run, live, peak, calls)?;
    let sb = &grid.sink;
    let cap = sb.capacity();
    let cnt = &sb.counters;
    let rows = fl(cnt.rows);
    kv("rows_pushed", grid.rows_pushed)?;
    kv("rows_retained", sb.retained())?;
    if sb.retained() != a.retain {
        bail!("retained {} != {}", sb.retained(), a.retain);
    }
    kv("hot_lines", sb.hot_lines())?;
    kv("sealed_chunks", sb.sealed_chunks())?;
    kv("styles", sb.styles.len())?;
    kv("avg_runs_per_row", format!("{:.3}", fl(cnt.runs) / rows))?;
    kv("raw_bytes_per_row", format!("{:.2}", fl(cnt.raw_bytes) / rows))?;
    let sl = fl(cnt.sealed_lines);
    kv("compressed_bytes_per_row", format!("{:.2}", fl(cnt.sealed_compressed) / sl))?;
    kv("index_bytes_per_row", format!("{:.2}", fl(cnt.sealed_index) / sl))?;
    kv(
        "compressed_plus_index_bytes_per_row",
        format!("{:.2}", fl(cnt.sealed_compressed + cnt.sealed_index) / sl),
    )?;
    kv("compression_ratio", format!("{:.3}", fl(cnt.sealed_raw) / fl(cnt.sealed_compressed)))?;
    kv("default_only_row_pct", format!("{:.1}", fl(cnt.default_only_lines) * 100.0 / rows))?;
    kv("default_only_run_bytes_per_row", format!("{:.2}", fl(cnt.default_only_run_bytes) / rows))?;
    kv("throughput.encode_rows_per_s", format!("{:.0}", rows / (fl(cnt.encode_ns) / 1e9)))?;
    kv("hot_step", a.hot_step)?;
    kv("hot_len_max_bytes", cnt.hot_len_max)?;
    if alloc::ENABLED {
        report_capacity(&grid, &cap, run.feed_cap, live)?;
    }
    verify(a, &grid.sink)?;
    Ok(())
}

/// Capacity-based accounting, reconciled against the dhat-measured total.
fn report_capacity(
    grid: &Grid<Scrollback>,
    cap: &scrollback::Capacity,
    feed_cap: usize,
    live: usize,
) -> Result<()> {
    kv("cap.live_grid", grid.capacity_bytes())?;
    kv("cap.hot_data", cap.hot_data)?;
    kv("cap.hot_index", cap.hot_index)?;
    kv("cap.sealed_data", cap.sealed_data)?;
    kv("cap.sealed_index", cap.sealed_index)?;
    kv("cap.sealed_structs", cap.sealed_structs)?;
    kv("cap.styles", cap.styles)?;
    kv("cap.line_scratch", cap.line_scratch)?;
    kv("cap.feed_buffer", feed_cap)?;
    kv("cap.lz_scratch_shared", cap.lz_scratch)?;
    let attributed = grid.capacity_bytes()
        + cap.hot_data
        + cap.hot_index
        + cap.sealed_data
        + cap.sealed_index
        + cap.sealed_structs
        + cap.styles
        + cap.line_scratch
        + feed_cap
        + cap.lz_scratch;
    kv("cap.total", attributed)?;
    kv(
        "cap.unattributed_alloc_minus_cap",
        format!("{}{}", if live >= attributed { "+" } else { "-" }, live.abs_diff(attributed)),
    )?;
    kv("session_bytes_excl_shared_lz_scratch", live - cap.lz_scratch)
}

/// Replays the same seed into a reference sink and compares every retained row.
fn verify(a: &Args, sb: &Scrollback) -> Result<()> {
    let (_, reference) = run_grid(a, RefSink::default())?;
    let want = &reference.sink.0;
    let tail =
        want.get(want.len().saturating_sub(a.retain)..).context("reference shorter than retain")?;
    let mut i = 0usize;
    sb.for_each_line(|l| {
        let h = sb.hash_encoded(l)?;
        if tail.get(i) != Some(&h) {
            return Err(anyhow!("row {i} differs after round trip"));
        }
        i += 1;
        Ok(())
    })?;
    if i != a.retain {
        bail!("decoded {i} rows, expected {}", a.retain);
    }
    kv("verify", format!("ok rows={i} of {} pushed", want.len()))
}

fn run_naive(a: &Args) -> Result<()> {
    let base = alloc::live();
    let calls0 = alloc::blocks();
    let (run, grid) = run_grid(a, NaiveSink { rows: VecDeque::new(), cap: a.retain })?;
    let live = alloc::live() - base;
    report_common(a, &run, live, alloc::peak(), alloc::blocks() - calls0)?;
    kv("rows_pushed", grid.rows_pushed)?;
    kv("rows_retained", grid.sink.rows.len())?;
    kv("cap.live_grid", grid.capacity_bytes())?;
    kv(
        "cap.history_cells",
        grid.sink.rows.iter().map(|r| r.len() * size_of::<Cell>()).sum::<usize>(),
    )?;
    kv("cap.history_queue", grid.sink.rows.capacity() * size_of::<Box<[Cell]>>())?;
    kv("history_bytes_per_row", COLS * size_of::<Cell>())?;
    Ok(())
}

fn run_alacritty(a: &Args) -> Result<()> {
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::grid::Dimensions;
    use alacritty_terminal::term::{Config, Term};
    use alacritty_terminal::vte::ansi::Processor;

    struct Size;
    impl Dimensions for Size {
        fn total_lines(&self) -> usize {
            ROWS
        }
        fn screen_lines(&self) -> usize {
            ROWS
        }
        fn columns(&self) -> usize {
            COLS
        }
    }

    let mut generator = new_gen(a)?;
    let base = alloc::live();
    let calls0 = alloc::blocks();
    let mut buf = Vec::with_capacity(FEED_BUF);
    let config = Config { scrolling_history: a.retain, ..Config::default() };
    let mut term = Term::new(config, &Size, VoidListener);
    let mut processor: Processor = Processor::new();
    let advance_ns = feed(&mut generator, a.lines, &mut buf, |b| {
        processor.advance(&mut term, b);
        Ok(())
    })?;
    let live = alloc::live() - base;
    let run = Run {
        generator: generator.stats.clone(),
        advance_ns,
        feed_cap: buf.capacity(),
        _feed_buf: buf,
    };
    report_common(a, &run, live, alloc::peak(), alloc::blocks() - calls0)?;
    let hist = term.grid().history_size();
    kv("rows_retained", hist)?;
    kv("alacritty.cell_size_bytes", size_of::<alacritty_terminal::term::cell::Cell>())?;
    kv("cap.feed_buffer", run.feed_cap)?;
    if hist != a.retain {
        bail!("alacritty history {hist} != {}", a.retain);
    }
    Ok(())
}

fn main() -> Result<()> {
    let _heap = alloc::start();
    let a = parse_args()?;
    match a.model.as_str() {
        "ferrum" => run_ferrum(&a),
        "naive" => run_naive(&a),
        "alacritty" => run_alacritty(&a),
        o => bail!("unknown model {o}"),
    }
}

/// Report-only conversion; counts and nanoseconds are far below 2^52, so it is exact.
#[allow(clippy::cast_precision_loss)] // INVARIANT: values < 2^52
fn fl(n: impl TryInto<u64>) -> f64 {
    n.try_into().unwrap_or(u64::MAX) as f64
}
