//! The search iterator the daemon drives (`FR-TERM-015`): both directions, case toggle, cold
//! (spilled) history, and a custom matcher standing in for a regex engine.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

#[path = "support/lines.rs"]
mod lines;

use std::ops::Range;

use ferrum_scrollback::{
    CaseMode, Direction, LineMatcher, LiteralMatcher, MemorySpill, Scrollback, ScrollbackConfig,
};
use lines::{numbered, push};

fn filled(count: usize, store: Scrollback) -> Scrollback {
    let mut store = store;
    for n in 0..count {
        push(&mut store, &numbered(n), &[]);
    }
    store
}

fn hits(store: &Scrollback, needle: &str, direction: Direction, start: usize) -> Vec<usize> {
    let matcher = LiteralMatcher::new(needle, CaseMode::Sensitive);
    store.search(matcher, direction, start).map(|hit| hit.expect("search").line).collect()
}

#[test]
fn forward_and_backward_visit_lines_in_order() {
    let store = filled(3_000, Scrollback::new(ScrollbackConfig::default()));
    let forward = hits(&store, "line 29", Direction::Forward, 0);
    assert_eq!(forward.first(), Some(&29));
    assert!(forward.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(forward.len(), 1 + 10 + 100); // 29, 290..=299, 2900..=2999
    let mut backward = hits(&store, "line 29", Direction::Backward, 2_999);
    backward.reverse();
    assert_eq!(forward, backward);
}

#[test]
fn start_is_inclusive_and_clamped() {
    let store = filled(100, Scrollback::new(ScrollbackConfig::default()));
    assert_eq!(hits(&store, "line 50 ", Direction::Forward, 50), vec![50]);
    assert_eq!(hits(&store, "line 50 ", Direction::Forward, 51), Vec::<usize>::new());
    assert_eq!(hits(&store, "line 50 ", Direction::Backward, 50), vec![50]);
    assert_eq!(hits(&store, "line 99 ", Direction::Backward, usize::MAX), vec![99]);
    assert_eq!(hits(&store, "line", Direction::Forward, 100), Vec::<usize>::new());
}

#[test]
fn ranges_are_reported_and_case_can_be_ignored() {
    let store = filled(10, Scrollback::new(ScrollbackConfig::default()));
    let matcher = LiteralMatcher::new("LINE 3", CaseMode::Insensitive);
    let found: Vec<_> =
        store.search(matcher, Direction::Forward, 0).map(|hit| hit.expect("search")).collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
    assert_eq!(found[0].ranges.first(), Some(&(0..6)));
    let sensitive = LiteralMatcher::new("LINE 3", CaseMode::Sensitive);
    assert_eq!(store.search(sensitive, Direction::Forward, 0).count(), 0);
}

#[test]
fn search_reads_spilled_history() {
    let config = ScrollbackConfig::new(100_000, 2_048).expect("config");
    let store =
        filled(12_000, Scrollback::with_spill(config, Box::new(MemorySpill::new(64 << 20))));
    assert!(store.spilled_chunks() > 0);
    assert_eq!(hits(&store, "line 5 ", Direction::Forward, 0), vec![5]);
    assert_eq!(hits(&store, "line 11999 ", Direction::Backward, usize::MAX), vec![11_999]);
    assert_eq!(hits(&store, "line 1", Direction::Forward, 0).len(), 1 + 10 + 100 + 1_000 + 2_000);
}

/// A matcher written outside the crate, standing in for the daemon's regex matcher.
struct FindSome;

impl LineMatcher for FindSome {
    fn find_matches(&self, text: &str, out: &mut Vec<Range<usize>>) {
        if let Some(at) = text.find("some") {
            out.push(at..at + "some".len());
        }
    }
}

#[test]
fn custom_matchers_plug_in_for_regex() {
    let store = filled(5, Scrollback::new(ScrollbackConfig::default()));
    assert_eq!(store.search(FindSome, Direction::Forward, 0).count(), 5);
}
