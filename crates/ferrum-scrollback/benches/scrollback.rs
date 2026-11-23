//! Criterion benches of `ferrum-scrollback` (`docs/08` §2): push, seal, read (RAM, spilled) and
//! search,
//! over the seeded product fixture (`fixtures/output/`). Baselines live in `benches/baselines/`
//! and are compared by `cargo xtask bench`.

#![allow(clippy::expect_used, reason = "bench setup: a failed expectation aborts the run")]

#[path = "../../../fixtures/output/output.rs"]
mod fixture;

use std::hint::black_box;
use std::path::PathBuf;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ferrum_scrollback::{
    CaseMode, ChunkSealer, Direction, FileSpill, HOT_CHUNK_LINES, LineCursor, LiteralMatcher,
    MemorySpill, PendingChunk, Scrollback, ScrollbackConfig, SpillStore,
};
use fixture::{Fixture, Profile, REFERENCE_COLS, Row};

const SEED: u64 = 1;
const LOGICAL_LINES: usize = 12_000;
const RETAINED_LINES: usize = 10_000;
const SPILL_THRESHOLD_LINES: usize = 2_048;
const RAM_THRESHOLD_LINES: usize = 50_000;
const SPILLED_HISTORY_LINES: usize = 30_000;
const SPILL_CAP_BYTES: u64 = 64 * 1024 * 1024;
const SEARCH_NEEDLE: &str = "warning";
/// Multiplier that makes consecutive reads land in different chunks (defeats the chunk cache).
const STRIDE: usize = 7_919;
const RANDOM_READS: usize = 1_000;
/// Encoded bytes of the full chunks of the fixture rows; the `scrollback_seal` ceiling in
/// `xtask/src/bench/baseline.rs` is derived from it.
const SEAL_RAW_BYTES: usize = 857_466;

fn rows() -> Vec<Row> {
    Fixture::rows(SEED, Profile::Mixed, REFERENCE_COLS, LOGICAL_LINES).0
}

fn filled(
    rows: &[Row],
    max_lines: usize,
    threshold: usize,
    store: Option<Box<dyn SpillStore>>,
) -> Scrollback {
    let config = ScrollbackConfig::new(max_lines, threshold).expect("config");
    let mut scrollback = match store {
        Some(store) => Scrollback::with_spill(config, store),
        None => Scrollback::new(config),
    };
    for row in rows {
        scrollback.push_line(&row.as_line()).expect("push_line");
    }
    scrollback
}

fn bench_push(c: &mut Criterion) {
    let rows = rows();
    let mut group = c.benchmark_group("scrollback_push");
    group.throughput(Throughput::Elements(u64::try_from(rows.len()).expect("fits")));
    group.bench_function("fixture_rows_retain_10k", |b| {
        b.iter(|| black_box(filled(&rows, RETAINED_LINES, RAM_THRESHOLD_LINES, None)));
    });
    group.finish();
}

/// Every full chunk of the fixture rows, each from its own store, pending (nobody sealed them).
fn pending_chunks(rows: &[Row]) -> Vec<PendingChunk> {
    rows.as_chunks::<HOT_CHUNK_LINES>()
        .0
        .iter()
        .map(|chunk| {
            let mut store = filled(chunk, RETAINED_LINES, RAM_THRESHOLD_LINES, None);
            store.take_pending().expect("a full chunk is pending")
        })
        .collect()
}

/// The compression the daemon worker does for the push path (Q-21 amendment, ADR 0028): it
/// must keep up with the parser, so `cargo xtask bench` gates it with an absolute ceiling.
fn bench_seal(c: &mut Criterion) {
    let rows = rows();
    let raw: usize = pending_chunks(&rows).iter().map(PendingChunk::raw_bytes).sum();
    assert_eq!(raw, SEAL_RAW_BYTES, "the fixture changed: update the xtask ceiling with it");
    let mut group = c.benchmark_group("scrollback_seal");
    group.throughput(Throughput::Bytes(u64::try_from(raw).expect("fits")));
    let mut sealer = ChunkSealer::new();
    group.bench_function("fixture_chunks", |b| {
        b.iter_batched(
            || pending_chunks(&rows),
            |chunks| {
                for pending in chunks {
                    black_box(sealer.seal(pending).expect("seal"));
                }
            },
            criterion::BatchSize::LargeInput,
        );
    });
    group.finish();
}

fn read_all(store: &Scrollback, order: impl Iterator<Item = usize>) -> usize {
    let mut cursor = LineCursor::new();
    let mut bytes = 0;
    for index in order {
        let line = store.read_line(index, &mut cursor).expect("read_line").expect("in range");
        bytes += line.text.len();
    }
    bytes
}

fn bench_read(c: &mut Criterion) {
    let rows = rows();
    let store = filled(&rows, RETAINED_LINES, RAM_THRESHOLD_LINES, None);
    let len = store.len();
    let mut group = c.benchmark_group("scrollback_read");
    group.throughput(Throughput::Elements(u64::try_from(len).expect("fits")));
    group.bench_function("sequential_10k", |b| b.iter(|| black_box(read_all(&store, 0..len))));
    // Every random read of a sealed chunk decompresses it (about 0.14 ms), so sample 1 000 reads.
    group.throughput(Throughput::Elements(u64::try_from(RANDOM_READS).expect("fits")));
    group.bench_function("random_1k", |b| {
        b.iter(|| black_box(read_all(&store, (0..RANDOM_READS).map(|i| i * STRIDE % len))));
    });
    group.finish();
    bench_read_spilled(c, &rows);
}

fn spill_path() -> PathBuf {
    std::env::temp_dir().join(format!("ferrum-bench-{}.spill", std::process::id()))
}

fn bench_read_spilled(c: &mut Criterion, rows: &[Row]) {
    let history: Vec<Row> = rows.iter().cycle().take(SPILLED_HISTORY_LINES).cloned().collect();
    let backends: [(&str, Box<dyn SpillStore>); 2] = [
        ("spilled_memory_sequential", Box::new(MemorySpill::new(SPILL_CAP_BYTES))),
        ("spilled_file_sequential", Box::new(FileSpill::new(spill_path(), SPILL_CAP_BYTES))),
    ];
    let mut group = c.benchmark_group("scrollback_read");
    for (name, backend) in backends {
        let store = filled(&history, SPILLED_HISTORY_LINES, SPILL_THRESHOLD_LINES, Some(backend));
        let cold = store.spilled_chunks() * HOT_CHUNK_LINES;
        assert!(cold > 0, "the history must reach the spill store");
        group.throughput(Throughput::Elements(u64::try_from(cold).expect("fits")));
        group.bench_function(name, |b| b.iter(|| black_box(read_all(&store, 0..cold))));
    }
    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let rows = rows();
    let store = filled(&rows, RETAINED_LINES, RAM_THRESHOLD_LINES, None);
    let mut group = c.benchmark_group("scrollback_search");
    group.throughput(Throughput::Elements(u64::try_from(store.len()).expect("fits")));
    for (name, case) in [
        ("literal_10k_lines", CaseMode::Sensitive),
        ("literal_insensitive_10k_lines", CaseMode::Insensitive),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| {
                let matcher = LiteralMatcher::new(SEARCH_NEEDLE, case);
                black_box(store.search(matcher, Direction::Forward, 0).count())
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_push, bench_seal, bench_read, bench_search);
criterion_main!(benches);
