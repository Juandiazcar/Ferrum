//! `push_line` is a hot path (`docs/05` §6): after warm-up it must not allocate. A full chunk
//! becomes a pending chunk from the buffer pool (no allocation); sealing it with `ChunkSealer`
//! allocates the blob, which is the retained data itself, and nothing else. When nobody seals
//! (the inline fallback), the push that seals allocates exactly the blob. One test per binary:
//! `dhat` profiles the whole process.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

#[path = "support/lines.rs"]
mod lines;

use ferrum_scrollback::{
    ChunkSealer, HOT_CHUNK_LINES, LineRef, Scrollback, ScrollbackConfig, StyledRun,
};
use lines::numbered;

#[cfg(not(miri))] // dhat is not supported under Miri
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const DISTINCT_LINES: usize = 64;
const RETAINED_LINES: usize = 3_000;
const MEASURED_CHUNKS: u64 = 10;

struct Prepared {
    text: String,
    runs: Vec<StyledRun>,
    flags: ferrum_scrollback::LineFlags,
}

fn blocks() -> u64 {
    dhat::HeapStats::get().total_blocks
}

struct Feeder {
    prepared: Vec<Prepared>,
    pushed: usize,
}

impl Feeder {
    fn new() -> Self {
        let prepared = (0..DISTINCT_LINES)
            .map(|n| {
                let line = numbered(n);
                Prepared { text: line.text(), runs: line.runs(&[]), flags: line.flags }
            })
            .collect();
        Self { prepared, pushed: 0 }
    }

    fn push(&mut self, store: &mut Scrollback) {
        let line = self.prepared.get(self.pushed % DISTINCT_LINES).expect("in range");
        let line_ref = LineRef { flags: line.flags, text: &line.text, runs: &line.runs };
        store.push_line(&line_ref).expect("push_line");
        self.pushed += 1;
    }
}

/// What the worker does, on this thread so that `dhat` sees it: seal and install every pending
/// chunk. Returns the chunks sealed.
fn seal_pending(store: &mut Scrollback, sealer: &mut ChunkSealer) -> u64 {
    let mut count = 0;
    while let Some(pending) = store.take_pending() {
        store.install(sealer.seal(pending).expect("seal")).expect("install");
        count += 1;
    }
    count
}

fn push_line_and_sealing_allocate_nothing_but_the_blob_in_steady_state() {
    let mut feeder = Feeder::new();
    let _profiler = dhat::Profiler::builder().testing().build();
    let config = ScrollbackConfig::new(RETAINED_LINES, RETAINED_LINES).expect("config");
    let (mut store, mut sealer) = (Scrollback::new(config), ChunkSealer::new());

    // Warm-up: fill the history, wrap the chunk list, grow the hot and pooled buffers and the
    // sealer scratch, intern every style.
    for _ in 0..HOT_CHUNK_LINES * 8 {
        feeder.push(&mut store);
        seal_pending(&mut store, &mut sealer);
    }
    assert_eq!(store.memory_report().inline_seals, 0, "the worker kept up");

    // Pushing, including every push that retires a full chunk: zero allocations.
    let (mut push_blocks, mut seal_blocks, mut chunks) = (0, 0, 0);
    for _ in 0..HOT_CHUNK_LINES * usize::try_from(MEASURED_CHUNKS).expect("fits") {
        let before = blocks();
        feeder.push(&mut store);
        push_blocks += blocks() - before;
        let before = blocks();
        chunks += seal_pending(&mut store, &mut sealer);
        seal_blocks += blocks() - before;
    }
    assert_eq!(push_blocks, 0, "pushes allocate nothing, the pending chunk included");
    assert_eq!(chunks, MEASURED_CHUNKS);
    assert_eq!(seal_blocks, MEASURED_CHUNKS, "sealing allocates exactly the retained blob");
    assert_eq!(store.memory_report().inline_seals, 0);
    assert_eq!(store.len(), RETAINED_LINES);

    // Nobody seals (the inline fallback): after its own warm-up (the pooled buffer becomes the
    // pending chunk), the pushes that seal allocate the blob and nothing else.
    for _ in 0..HOT_CHUNK_LINES * usize::try_from(MEASURED_CHUNKS).expect("fits") {
        feeder.push(&mut store);
    }
    let before = (blocks(), store.memory_report().inline_seals);
    for _ in 0..HOT_CHUNK_LINES * usize::try_from(MEASURED_CHUNKS).expect("fits") {
        feeder.push(&mut store);
    }
    let inline = store.memory_report().inline_seals - before.1;
    assert!(inline > 0, "the pending cap was reached");
    assert_eq!(blocks() - before.0, inline, "one allocation per inline seal");
}

fn main() {
    ferrum_testkit::single_test::single_test(
        "push_line_and_sealing_allocate_nothing_but_the_blob_in_steady_state",
        push_line_and_sealing_allocate_nothing_but_the_blob_in_steady_state,
    );
}
