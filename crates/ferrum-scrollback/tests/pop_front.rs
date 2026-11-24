//! `pop_front`: the streaming rewrite primitive used by reflow in `ferrum-vt`.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

#[path = "support/lines.rs"]
mod lines;

use ferrum_scrollback::{ChunkSealer, HOT_CHUNK_LINES, LineCursor, Scrollback, ScrollbackConfig};
use lines::{numbered, push};

const LINES: usize = HOT_CHUNK_LINES * 2 + 17;
/// Above `LINES`, so nothing is evicted by the limits.
const MAX_LINES: usize = 10_000;
const SPILL_THRESHOLD_LINES: usize = 50_000;

fn filled(count: usize) -> Scrollback {
    let config = ScrollbackConfig::new(MAX_LINES, SPILL_THRESHOLD_LINES).expect("config");
    let mut store = Scrollback::new(config);
    for index in 0..count {
        push(&mut store, &numbered(index), &[]);
    }
    store
}

#[test]
fn pops_every_line_oldest_first_across_hot_and_sealed_chunks() {
    let mut store = filled(LINES);
    let mut cursor = LineCursor::new();
    for index in 0..LINES {
        let expected = numbered(index);
        let popped = store.pop_front(&mut cursor).expect("pop").expect("a line");
        assert_eq!(popped.text, expected.text(), "line {index}");
        assert_eq!(popped.flags, expected.flags);
        assert_eq!(store.len(), LINES - index - 1);
        assert_eq!(store.dropped_lines(), u64::try_from(index + 1).expect("fits"));
    }
    assert!(store.pop_front(&mut cursor).expect("pop").is_none());
    assert!(store.is_empty());
}

#[test]
fn rotating_front_to_back_keeps_the_order_and_frees_chunks() {
    let mut store = filled(LINES);
    // Full chunks are sealed as the daemon worker does, so the sealed bytes are the history.
    let mut sealer = ChunkSealer::new();
    let mut seal = |store: &mut Scrollback| {
        while let Some(pending) = store.take_pending() {
            store.install(sealer.seal(pending).expect("seal")).expect("install");
        }
    };
    seal(&mut store);
    let history = |store: &Scrollback| {
        let report = store.memory_report();
        report.sealed_bytes + report.pending_bytes
    };
    let before = history(&store);
    let boundary = store.pushed_lines();
    let mut cursor = LineCursor::new();
    let mut moved = 0usize;
    while store.dropped_lines() < boundary {
        let text = store.pop_front(&mut cursor).expect("pop").expect("a line").text.clone();
        assert_eq!(text, numbered(moved).text());
        moved += 1;
        push(&mut store, &numbered(moved + LINES), &[]);
    }
    assert_eq!(moved, LINES);
    assert_eq!(store.len(), LINES);
    assert!(history(&store) <= before + before / 4, "old chunks are released");
}

#[test]
fn popping_an_empty_store_is_none() {
    let mut store = Scrollback::new(ScrollbackConfig::default());
    assert!(store.pop_front(&mut LineCursor::new()).expect("pop").is_none());
}

#[test]
fn skip_front_drops_the_oldest_line_unread_and_counts_it() {
    let mut store = filled(5);
    store.skip_front();
    assert_eq!((store.len(), store.dropped_lines()), (4, 1));
    let mut cursor = LineCursor::new();
    let first = store.pop_front(&mut cursor).expect("pop").expect("a line");
    assert_eq!(first.text, numbered(1).text());
    let mut empty = filled(0);
    empty.skip_front();
    assert_eq!(empty.dropped_lines(), 0);
}
