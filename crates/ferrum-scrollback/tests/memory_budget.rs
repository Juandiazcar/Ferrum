//! Memory accounting and the per-session allowances of `docs/03` §2 for the reference fixture mix
//! (10 000 retained rows out of 12 000 logical lines). Full chunks are sealed as the daemon does
//! (`ChunkSealer` on the worker; here on the same thread so that `dhat` sees everything), and
//! once with nobody sealing, so the pending chunk is at its cap. One test per binary: `dhat`
//! profiles the whole process.

#![cfg(not(miri))] // dhat is not supported under Miri
#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

#[path = "../../../fixtures/output/output.rs"]
mod fixture;

use ferrum_scrollback::{
    ChunkSealer, MAX_PENDING_CHUNKS, MemoryReport, Scrollback, ScrollbackConfig,
};

const _: () = assert!(MAX_PENDING_CHUNKS == 2, "PENDING_ALLOWANCE is two chunk buffers");
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
const SEALED_ALLOWANCE: usize = 360_000;
const MISC_ALLOWANCE: usize = 110_000;
/// The pending chunks and the pooled buffers: together at most two full chunk buffers
/// (`MAX_PENDING_CHUNKS` = 2, maintainer decision Q-26 amended; ADR 0028 decision 8).
const PENDING_ALLOWANCE: usize = 180_000;

/// Who seals full chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sealing {
    /// The worker keeps up (simulated on this thread).
    Worker,
    /// Nobody: pending chunks stay at their cap and the push path seals the rest inline.
    Nobody,
}

fn fill(seed: u64, profile: Profile, sealing: Sealing) -> (usize, MemoryReport) {
    let (rows, _) = Fixture::rows(seed, profile, REFERENCE_COLS, LOGICAL_LINES);
    let before = dhat::HeapStats::get().curr_bytes;
    let config = ScrollbackConfig::new(RETAINED_LINES, SPILL_THRESHOLD_LINES).expect("config");
    let mut store = Scrollback::new(config);
    let mut sealer = ChunkSealer::new();
    for row in &rows {
        store.push_line(&row.as_line()).expect("push_line");
        while let Some(pending) =
            (sealing == Sealing::Worker).then(|| store.take_pending()).flatten()
        {
            store.install(sealer.seal(pending).expect("seal")).expect("install");
        }
    }
    // The worker's scratch is shared by every session; it is not part of this store.
    drop(sealer);
    let live = dhat::HeapStats::get().curr_bytes - before;
    assert_eq!(store.len(), RETAINED_LINES);
    (live, store.memory_report())
}

#[test]
fn report_reconciles_with_dhat_and_reference_mix_fits_the_allowances() {
    let _profiler = dhat::Profiler::builder().testing().build();
    let cases = [Profile::Mixed, Profile::Entropy]
        .into_iter()
        .flat_map(|profile| [Sealing::Worker, Sealing::Nobody].map(|sealing| (profile, sealing)));
    for (profile, sealing) in cases {
        for seed in SEEDS {
            let (live, report) = fill(seed, profile, sealing);
            let reported = report.total_ram_bytes();
            assert!(
                live.abs_diff(reported) <= RECONCILE_SLACK_BYTES,
                "{profile:?} seed {seed}: dhat {live} vs report {reported}"
            );
            // The hot chunk bound is a requirement for every content (ADR 0024).
            assert!(report.hot_chunk_bytes <= HOT_CHUNK_ALLOWANCE, "{profile:?}: hot {report:?}");
            assert!(report.misc_bytes <= MISC_ALLOWANCE, "{profile:?}: misc {report:?}");
            assert!(report.pending_bytes <= PENDING_ALLOWANCE, "{profile:?}: pending {report:?}");
            if sealing == Sealing::Worker {
                assert_eq!(report.inline_seals, 0, "{profile:?} seed {seed}: {report:?}");
            }
            if profile == Profile::Mixed {
                assert!(report.sealed_bytes <= SEALED_ALLOWANCE, "seed {seed}: sealed {report:?}");
            }
        }
    }
}
