//! Memory accounting and the per-session allowances of `docs/03` §2 for the reference fixture mix
//! (10 000 retained rows out of 12 000 logical lines). One test per binary: `dhat` profiles the
//! whole process.

#![cfg(not(miri))] // dhat is not supported under Miri
#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

#[path = "../../../fixtures/output/output.rs"]
mod fixture;

use ferrum_scrollback::{MemoryReport, Scrollback, ScrollbackConfig};
use fixture::{Fixture, Profile, REFERENCE_COLS};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const LOGICAL_LINES: usize = 12_000;
const RETAINED_LINES: usize = 10_000;
const SPILL_THRESHOLD_LINES: usize = 50_000;
const SEEDS: [u64; 3] = [1, 2, 3];
/// `dhat` counts requested bytes; the report must agree within this slack (hash table layout).
const RECONCILE_SLACK_BYTES: usize = 1_024;

// `docs/03` §2 allowances in bytes (1 MB = 1 000 000).
const HOT_CHUNK_ALLOWANCE: usize = 100_000;
const SEALED_ALLOWANCE: usize = 450_000;
const MISC_ALLOWANCE: usize = 200_000;

fn fill(seed: u64, profile: Profile) -> (usize, MemoryReport) {
    let (rows, _) = Fixture::rows(seed, profile, REFERENCE_COLS, LOGICAL_LINES);
    let before = dhat::HeapStats::get().curr_bytes;
    let config = ScrollbackConfig::new(RETAINED_LINES, SPILL_THRESHOLD_LINES).expect("config");
    let mut store = Scrollback::new(config);
    for row in &rows {
        store.push_line(&row.as_line()).expect("push_line");
    }
    let live = dhat::HeapStats::get().curr_bytes - before;
    assert_eq!(store.len(), RETAINED_LINES);
    (live, store.memory_report())
}

#[test]
fn report_reconciles_with_dhat_and_reference_mix_fits_the_allowances() {
    let _profiler = dhat::Profiler::builder().testing().build();
    for profile in [Profile::Mixed, Profile::Entropy] {
        for seed in SEEDS {
            let (live, report) = fill(seed, profile);
            let reported = report.total_ram_bytes();
            assert!(
                live.abs_diff(reported) <= RECONCILE_SLACK_BYTES,
                "{profile:?} seed {seed}: dhat {live} vs report {reported}"
            );
            // The hot chunk bound is a requirement for every content (ADR 0024).
            assert!(report.hot_chunk_bytes <= HOT_CHUNK_ALLOWANCE, "{profile:?}: hot {report:?}");
            assert!(report.misc_bytes <= MISC_ALLOWANCE, "{profile:?}: misc {report:?}");
            if profile == Profile::Mixed {
                assert!(report.sealed_bytes <= SEALED_ALLOWANCE, "seed {seed}: sealed {report:?}");
            }
        }
    }
}
