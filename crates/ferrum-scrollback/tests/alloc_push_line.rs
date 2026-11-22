//! `push_line` is a hot path (`docs/05` §6): after warm-up it must not allocate, except for the
//! sealed blob, which is the retained data itself (exactly one allocation per sealed chunk).
//! One test per binary: `dhat` profiles the whole process.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

#[path = "support/lines.rs"]
mod lines;

use ferrum_scrollback::{HOT_CHUNK_LINES, LineRef, Scrollback, ScrollbackConfig, StyledRun};
use lines::numbered;

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

#[test]
fn push_line_allocates_nothing_in_steady_state() {
    let prepared: Vec<Prepared> = (0..DISTINCT_LINES)
        .map(|n| {
            let line = numbered(n);
            Prepared { text: line.text(), runs: line.runs(&[]), flags: line.flags }
        })
        .collect();
    let _profiler = dhat::Profiler::builder().testing().build();
    let config = ScrollbackConfig::new(RETAINED_LINES, RETAINED_LINES).expect("config");
    let mut store = Scrollback::new(config);
    let mut pushed = 0usize;
    let mut push = |store: &mut Scrollback| {
        let line = &prepared[pushed % DISTINCT_LINES];
        let line_ref = LineRef { flags: line.flags, text: &line.text, runs: &line.runs };
        store.push_line(&line_ref).expect("push_line");
        pushed += 1;
    };

    // Warm-up: fill the history, wrap the chunk list, grow the hot buffer, intern every style.
    for _ in 0..HOT_CHUNK_LINES * 8 {
        push(&mut store);
    }

    // Within a chunk: zero allocations (the hot chunk is empty here, so 1 023 pushes do not seal).
    let before = blocks();
    for _ in 0..HOT_CHUNK_LINES - 1 {
        push(&mut store);
    }
    assert_eq!(blocks() - before, 0, "pushes that stay inside the hot chunk allocate nothing");

    // The push that seals allocates the blob and nothing else.
    let before = blocks();
    push(&mut store);
    assert_eq!(blocks() - before, 1, "sealing allocates exactly the retained blob");

    // Over many chunks: one allocation per sealed chunk, none per line.
    let before = blocks();
    for _ in 0..HOT_CHUNK_LINES * usize::try_from(MEASURED_CHUNKS).expect("fits") {
        push(&mut store);
    }
    assert_eq!(blocks() - before, MEASURED_CHUNKS);
    assert_eq!(store.len(), RETAINED_LINES);
}
