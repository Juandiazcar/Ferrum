//! Per-session scrollback memory for the reference scenario (`docs/03` §1, Q-17): feeds 12 000
//! logical lines of the product fixture generator, keeps 10 000 rows, seals full chunks as the
//! daemon worker does (`ChunkSealer`, here on the same thread), and prints the bytes that `dhat`
//! counted as live next to the crate's own `memory_report()`.
//!
//! `cargo run --release -p ferrum-scrollback --example memory_report`

#[path = "../../../fixtures/output/output.rs"]
mod fixture;

use std::io::{self, Write};

use ferrum_scrollback::{ChunkSealer, MemoryReport, Scrollback, ScrollbackConfig};
use fixture::{Fixture, Profile, REFERENCE_COLS};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const LOGICAL_LINES: usize = 12_000;
const RETAINED_LINES: usize = 10_000;
const SEEDS: [u64; 3] = [1, 2, 3];

struct Measured {
    live_delta: usize,
    report: MemoryReport,
    rows_pushed: usize,
    styles: usize,
    raw_bytes_per_row: f64,
}

fn measure(seed: u64, profile: Profile) -> Result<Measured, Box<dyn std::error::Error>> {
    let (rows, _) = Fixture::rows(seed, profile, REFERENCE_COLS, LOGICAL_LINES);
    let before = dhat::HeapStats::get().curr_bytes;
    let mut store = Scrollback::new(ScrollbackConfig::new(RETAINED_LINES, 50_000)?);
    let mut sealer = ChunkSealer::new();
    for row in &rows {
        store.push_line(&row.as_line())?;
        while let Some(pending) = store.take_pending() {
            store.install(sealer.seal(pending)?)?;
        }
    }
    // The worker's scratch is shared by every session, not part of this store.
    drop(sealer);
    let live_delta = dhat::HeapStats::get().curr_bytes - before;
    assert_eq!(store.len(), RETAINED_LINES, "the scenario retains exactly 10 000 rows");
    let raw: usize = rows.iter().map(|row| row.text.len()).sum();
    let rows_pushed = rows.len();
    Ok(Measured {
        live_delta,
        report: store.memory_report(),
        rows_pushed,
        styles: store.styles().len(),
        raw_bytes_per_row: f64::from(u32::try_from(raw)?) / f64::from(u32::try_from(rows_pushed)?),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _profiler = dhat::Profiler::builder().testing().build();
    let mut out = io::stdout().lock();
    writeln!(
        out,
        "profile seed rows styles text_B/row | dhat_live hot sealed pending misc | session_sum +scratch"
    )?;
    for profile in [Profile::Mixed, Profile::Entropy] {
        for seed in SEEDS {
            let m = measure(seed, profile)?;
            let r = m.report;
            writeln!(
                out,
                "{profile:?} {seed} {} {} {:.1} | {} {} {} {} {} | {} {}",
                m.rows_pushed,
                m.styles,
                m.raw_bytes_per_row,
                m.live_delta,
                r.hot_chunk_bytes,
                r.sealed_bytes,
                r.pending_bytes,
                r.misc_bytes,
                r.session_ram_bytes(),
                r.total_ram_bytes(),
            )?;
        }
    }
    Ok(())
}
