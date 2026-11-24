//! Pending chunks: a full hot chunk waits, uncompressed, for a worker to seal it (`docs/02` §5,
//! ADR 0028 Q-21 amendment). Reads, `pop_front`, `clear`, eviction and spilling see pending
//! chunks; the cap makes the push path seal inline; sealing works from another thread.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

#[path = "support/lines.rs"]
mod lines;

use std::collections::VecDeque;
use std::sync::mpsc;

use ferrum_scrollback::{
    ChunkSealer, HOT_CHUNK_LINES, LineCursor, LineFlags, LineRef, MAX_LINE_TEXT_BYTES,
    MAX_PENDING_CHUNKS, MemorySpill, POOLED_BUFFER_KEEP_BYTES, PendingChunk, Scrollback,
    ScrollbackConfig, ScrollbackError,
};
use lines::{numbered, push, read};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

const MAX_LINES: usize = 20_000;
const SPILL_THRESHOLD_LINES: usize = 50_000;
const BIG_DISK: u64 = 64 * 1024 * 1024;

fn store(max_lines: usize) -> Scrollback {
    Scrollback::new(ScrollbackConfig::new(max_lines, SPILL_THRESHOLD_LINES).expect("config"))
}

fn push_range(store: &mut Scrollback, range: std::ops::Range<usize>) {
    for n in range {
        push(store, &numbered(n), &[]);
    }
}

/// Every retained line reads back as `first..first + len`.
fn assert_lines(store: &Scrollback, first: usize) {
    let mut cursor = LineCursor::new();
    for index in 0..store.len() {
        let line = read(store, index, &mut cursor).expect("in range");
        assert_eq!(line, numbered(first + index).expected(), "line {index}");
    }
    assert!(read(store, store.len(), &mut cursor).is_none());
}

#[test]
fn a_full_chunk_is_pending_readable_and_reads_the_same_once_installed() {
    let mut store = store(MAX_LINES);
    push_range(&mut store, 0..HOT_CHUNK_LINES + 10);
    let report = store.memory_report();
    assert!(report.pending_bytes > 0 && report.inline_seals == 0, "{report:?}");
    assert_lines(&store, 0);

    let pending = store.take_pending().expect("one full chunk");
    assert_eq!(pending.line_count(), HOT_CHUNK_LINES);
    assert!(store.take_pending().is_none(), "each chunk is handed out once");
    assert_lines(&store, 0);

    let sealed = ChunkSealer::new().seal(pending).expect("seal");
    assert!(!sealed.is_empty());
    store.install(sealed).expect("install");
    assert!(store.memory_report().sealed_bytes > report.sealed_bytes);
    assert_lines(&store, 0);
}

#[test]
fn pending_chunks_are_handed_out_oldest_first_and_installed_in_any_order() {
    let mut store = store(MAX_LINES);
    push_range(&mut store, 0..HOT_CHUNK_LINES * MAX_PENDING_CHUNKS);
    let taken: Vec<PendingChunk> = std::iter::from_fn(|| store.take_pending()).collect();
    assert_eq!(taken.len(), MAX_PENDING_CHUNKS);
    let mut sealer = ChunkSealer::new();
    let mut blobs: Vec<_> = taken.into_iter().map(|p| sealer.seal(p).expect("seal")).collect();
    blobs.reverse();
    for blob in blobs {
        store.install(blob).expect("install");
        assert_lines(&store, 0);
    }
    assert_eq!(store.memory_report().inline_seals, 0);
}

#[test]
fn the_cap_makes_the_push_path_seal_inline_and_counts_it() {
    let mut store = store(MAX_LINES);
    let chunks = MAX_PENDING_CHUNKS + 3;
    push_range(&mut store, 0..HOT_CHUNK_LINES * chunks);
    let report = store.memory_report();
    assert_eq!(report.inline_seals, 3, "{report:?}");
    assert_eq!(std::iter::from_fn(|| store.take_pending()).count(), MAX_PENDING_CHUNKS);
    assert_lines(&store, 0);
}

#[test]
fn pop_front_and_eviction_consume_pending_chunks_in_order() {
    let mut store = store(MAX_LINES);
    let total = HOT_CHUNK_LINES * 3 + 5;
    push_range(&mut store, 0..total);
    let mut cursor = LineCursor::new();
    for n in 0..total {
        let line = store.pop_front(&mut cursor).expect("pop").expect("a line");
        assert_eq!(line.text, numbered(n).text());
    }
    assert!(store.is_empty());
    assert!(store.memory_report().pending_bytes > 0, "buffers stay pooled");

    // Eviction of a pending chunk that is out with the worker: its late blob is dropped.
    let mut small = self::store(HOT_CHUNK_LINES);
    push_range(&mut small, 0..HOT_CHUNK_LINES);
    let pending = small.take_pending().expect("full chunk");
    push_range(&mut small, HOT_CHUNK_LINES..HOT_CHUNK_LINES * 2 + 1);
    small.install(ChunkSealer::new().seal(pending).expect("seal")).expect("install");
    assert_eq!(small.len(), HOT_CHUNK_LINES);
    assert_lines(&small, HOT_CHUNK_LINES + 1);
}

/// Lines kept by a store whose history is shorter than one chunk: a handed-out chunk is evicted
/// before the worker returns it.
const SHORT_HISTORY_LINES: usize = 64;

#[test]
fn a_chunk_evicted_while_handed_out_gives_its_buffer_back_when_the_worker_is_done() {
    let mut store = store(SHORT_HISTORY_LINES);
    push_range(&mut store, 0..HOT_CHUNK_LINES);
    let pending = store.take_pending().expect("full chunk");
    let end = HOT_CHUNK_LINES + SHORT_HISTORY_LINES;
    push_range(&mut store, HOT_CHUNK_LINES..end);
    assert_eq!(store.len(), SHORT_HISTORY_LINES, "the handed-out chunk was evicted");
    store.install(ChunkSealer::new().seal(pending).expect("seal")).expect("install");
    let report = store.memory_report();
    assert!(report.pending_bytes > 0, "the buffer is pooled again: {report:?}");
    push_range(&mut store, end..end + HOT_CHUNK_LINES);
    assert_eq!(store.memory_report().inline_seals, 0);
    assert_lines(&store, end + HOT_CHUNK_LINES - SHORT_HISTORY_LINES);
}

/// A chunk of rows at the text limit (16 MiB) grows every chunk buffer; the buffers return to
/// their working size once normal rows follow, instead of pinning the burst forever.
#[test]
fn buffers_grown_by_a_burst_of_huge_rows_are_freed_after_use() {
    let huge = "x".repeat(MAX_LINE_TEXT_BYTES);
    let huge = LineRef { flags: LineFlags::NONE, text: &huge, runs: &[] };
    let mut store = store(MAX_LINES);
    let mut worker = ChunkSealer::new();
    let mut held = Vec::new();
    for _ in 0..MAX_PENDING_CHUNKS {
        for _ in 0..HOT_CHUNK_LINES {
            store.push_line(&huge).expect("push");
        }
        held.push(store.take_pending().expect("full chunk"));
    }
    // One more huge chunk is sealed inline (the pending ones are out): hot and scratch grow.
    for _ in 0..HOT_CHUNK_LINES {
        store.push_line(&huge).expect("push");
    }
    assert_eq!(store.memory_report().inline_seals, 1);
    for pending in held {
        store.install(worker.seal(pending).expect("seal")).expect("install");
    }
    push_range(&mut store, 0..HOT_CHUNK_LINES * 3);
    while let Some(pending) = store.take_pending() {
        store.install(worker.seal(pending).expect("seal")).expect("install");
    }
    let report = store.memory_report();
    assert!(report.hot_chunk_bytes <= POOLED_BUFFER_KEEP_BYTES, "{report:?}");
    assert!(report.pending_bytes <= POOLED_BUFFER_KEEP_BYTES, "{report:?}");
    assert!(report.shared_scratch_bytes <= POOLED_BUFFER_KEEP_BYTES, "{report:?}");
    assert!(worker.capacity_bytes() <= POOLED_BUFFER_KEEP_BYTES, "{worker:?}");
}

#[test]
fn a_huge_buffer_released_while_handed_out_is_freed_when_the_worker_lets_go() {
    let huge = "x".repeat(MAX_LINE_TEXT_BYTES);
    let huge = LineRef { flags: LineFlags::NONE, text: &huge, runs: &[] };
    let mut store = store(MAX_LINES);
    for _ in 0..HOT_CHUNK_LINES {
        store.push_line(&huge).expect("push");
    }
    let held = store.take_pending().expect("full chunk");
    // Dropped while the worker holds it: its buffer is pooled, still shared.
    store.clear();
    let late = ChunkSealer::new().seal(held).expect("seal");
    // The worker let go; its late blob is dropped, and the huge pooled buffer with it.
    store.install(late).expect("install");
    let report = store.memory_report();
    assert!(report.pending_bytes <= POOLED_BUFFER_KEEP_BYTES, "{report:?}");
}

#[test]
fn a_blob_from_another_scrollback_is_refused() {
    let (mut first, mut second) = (store(MAX_LINES), store(MAX_LINES));
    push_range(&mut first, 0..HOT_CHUNK_LINES);
    push_range(&mut second, 5_000..5_000 + HOT_CHUNK_LINES);
    let mut sealer = ChunkSealer::new();
    let from_first = sealer.seal(first.take_pending().expect("full chunk")).expect("seal");
    let from_second = sealer.seal(second.take_pending().expect("full chunk")).expect("seal");
    assert!(matches!(second.install(from_first), Err(ScrollbackError::ForeignBlob)));
    assert!(matches!(first.install(from_second), Err(ScrollbackError::ForeignBlob)));
    assert_lines(&first, 0);
    assert_lines(&second, 5_000);
}

#[test]
fn clear_drops_pending_chunks_and_ignores_their_late_blobs() {
    let mut store = store(MAX_LINES);
    push_range(&mut store, 0..HOT_CHUNK_LINES * 2);
    let pending = store.take_pending().expect("full chunk");
    store.clear();
    store.install(ChunkSealer::new().seal(pending).expect("seal")).expect("install");
    assert!(store.is_empty());
    push_range(&mut store, 7..8);
    assert_lines(&store, 7);
}

#[test]
fn spilling_seals_a_pending_chunk_that_reached_the_frontier() {
    let config = ScrollbackConfig::new(MAX_LINES, HOT_CHUNK_LINES * 2).expect("config");
    let mut store = Scrollback::with_spill(config, Box::new(MemorySpill::new(BIG_DISK)));
    let pending_out = {
        push_range(&mut store, 0..HOT_CHUNK_LINES);
        store.take_pending().expect("full chunk")
    };
    push_range(&mut store, HOT_CHUNK_LINES..HOT_CHUNK_LINES * 6);
    assert!(store.spilled_chunks() > 0, "nobody sealed, yet history spilled");
    assert!(store.memory_report().inline_seals > 0);
    store.install(ChunkSealer::new().seal(pending_out).expect("seal")).expect("install");
    assert_lines(&store, 0);
}

#[test]
fn a_worker_thread_seals_while_the_owner_pushes_and_reads() {
    let (to_worker, jobs) = mpsc::sync_channel::<PendingChunk>(MAX_PENDING_CHUNKS);
    let (done, results) = mpsc::sync_channel(MAX_PENDING_CHUNKS);
    let worker = std::thread::Builder::new()
        .name("scrollback-seal-test".into())
        .spawn(move || {
            let mut sealer = ChunkSealer::new();
            for pending in jobs {
                if done.send(sealer.seal(pending).expect("seal")).is_err() {
                    break;
                }
            }
        })
        .expect("spawn");
    let mut store = store(MAX_LINES);
    let total = HOT_CHUNK_LINES * 12 + 3;
    let mut in_flight = 0usize;
    for n in 0..total {
        push(&mut store, &numbered(n), &[]);
        while let Ok(blob) = results.try_recv() {
            store.install(blob).expect("install");
            in_flight -= 1;
        }
        while let Some(pending) = store.take_pending() {
            to_worker.send(pending).expect("worker alive");
            in_flight += 1;
        }
    }
    for _ in 0..in_flight {
        store.install(results.recv().expect("result")).expect("install");
    }
    drop(to_worker);
    worker.join().expect("worker");
    assert_lines(&store, 0);
    let report = store.memory_report();
    assert!(report.sealed_bytes > 0, "{report:?}");
}

/// One step of the model-based property test.
#[derive(Debug, Clone, Copy)]
enum Op {
    Push(usize),
    Take,
    SealOldest,
    SealNewest,
    Pop(usize),
    Clear,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        6 => (1..HOT_CHUNK_LINES * 2).prop_map(Op::Push),
        3 => Just(Op::Take),
        2 => Just(Op::SealOldest),
        2 => Just(Op::SealNewest),
        2 => (1..HOT_CHUNK_LINES).prop_map(Op::Pop),
        1 => Just(Op::Clear),
    ]
}

struct Model {
    store: Scrollback,
    lines: VecDeque<usize>,
    out: VecDeque<PendingChunk>,
    next: usize,
    sealer: ChunkSealer,
    /// Lines retained: `max_lines`, or the spill threshold without a store.
    limit: usize,
}

impl Model {
    fn apply(&mut self, op: Op) {
        match op {
            Op::Push(count) => {
                for _ in 0..count {
                    push(&mut self.store, &numbered(self.next), &[]);
                    self.lines.push_back(self.next);
                    self.next += 1;
                }
                while self.lines.len() > self.limit {
                    self.lines.pop_front();
                }
            }
            Op::Take => self.out.extend(self.store.take_pending()),
            Op::SealOldest | Op::SealNewest => {
                let taken = if matches!(op, Op::SealOldest) {
                    self.out.pop_front()
                } else {
                    self.out.pop_back()
                };
                if let Some(pending) = taken {
                    self.store.install(self.sealer.seal(pending).expect("seal")).expect("install");
                }
            }
            Op::Pop(count) => {
                let mut cursor = LineCursor::new();
                for _ in 0..count.min(self.lines.len()) {
                    let line = self.store.pop_front(&mut cursor).expect("pop").expect("a line");
                    let expected = self.lines.pop_front().expect("model line");
                    assert_eq!(line.text, numbered(expected).text());
                }
            }
            Op::Clear => {
                self.store.clear();
                self.lines.clear();
            }
        }
    }

    fn check(&self) {
        assert_eq!(self.store.len(), self.lines.len());
        let mut cursor = LineCursor::new();
        for (index, &n) in self.lines.iter().enumerate() {
            assert_eq!(read(&self.store, index, &mut cursor), Some(numbered(n).expected()));
        }
        assert!(self.store.memory_report().pending_bytes <= MAX_PENDING_CHUNKS * 200_000);
    }
}

/// Failing seeds are committed under `proptest-regressions/` (`docs/08` §2); 48 cases, since each
/// one replays up to 24 operations over several full chunks.
fn config() -> ProptestConfig {
    ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "proptest-regressions/pending.txt",
        ))),
        ..ProptestConfig::with_cases(48)
    }
}

proptest! {
    #![proptest_config(config())]

    /// Any interleaving of pushes, hand-outs, out-of-order installs, pops and clears, with and
    /// without a spill store, reads back exactly the lines a plain queue model holds.
    #[test]
    fn pending_chunks_never_change_what_is_read(
        ops in proptest::collection::vec(op(), 1..24),
        spill in any::<bool>(),
    ) {
        let (max_lines, threshold) = (HOT_CHUNK_LINES * 5, HOT_CHUNK_LINES * 2);
        let config = ScrollbackConfig::new(max_lines, threshold).expect("config");
        let store = if spill {
            Scrollback::with_spill(config, Box::new(MemorySpill::new(BIG_DISK)))
        } else {
            Scrollback::new(config)
        };
        let mut model = Model {
            store,
            lines: VecDeque::new(),
            out: VecDeque::new(),
            next: 0,
            sealer: ChunkSealer::new(),
            limit: if spill { max_lines } else { threshold },
        };
        for op in ops {
            model.apply(op);
            model.check();
        }
    }
}
