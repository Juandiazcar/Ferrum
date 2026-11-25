//! Criterion bench of the daemon's search path (FR-TERM-015, `docs/03` §4: scrollback search over
//! 10 000 lines at most 15 ms): the history snapshot and screen copy taken on the I/O thread and
//! the search the worker runs over them, literal and regular expression, both case-insensitive,
//! over 10 000 retained lines of the reference fixture mix (`fixtures/output/`). Compared by
//! `cargo xtask bench`.

#![allow(clippy::expect_used, reason = "bench setup: a failed expectation aborts the run")]

#[path = "../../../fixtures/output/output.rs"]
mod fixture;

use std::hint::black_box;
use std::sync::atomic::AtomicBool;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ferrum_daemon::search::{Matcher, SearchInput, search};
use ferrum_proto::SearchOptions;
use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{NullSink, Size, Terminal};
use fixture::{Fixture, Profile, REFERENCE_COLS};

const SEED: u64 = 1;
/// Lines fed and kept (`docs/03` §1: 12 000 fed, 10 000 retained).
const FED_LINES: usize = 12_000;
const RETAINED_LINES: usize = 10_000;
/// Lines kept in RAM: all of them, so the bench measures the search, not the disk.
const RAM_THRESHOLD_LINES: usize = 50_000;
const SCREEN_ROWS: u16 = 55;

fn terminal() -> Terminal<NullSink> {
    let (rows, _) = Fixture::rows(SEED, Profile::Mixed, REFERENCE_COLS, FED_LINES);
    let mut bytes = Vec::new();
    fixture::stream::write_rows(&rows, &mut bytes).expect("the fixture stream");
    let config = ScrollbackConfig::new(RETAINED_LINES, RAM_THRESHOLD_LINES).expect("config");
    let cols = u16::try_from(REFERENCE_COLS).expect("fits");
    let mut term = Terminal::new(Size::new(cols, SCREEN_ROWS), Scrollback::new(config), NullSink);
    term.parse(&bytes);
    assert_eq!(term.scrollback().len(), RETAINED_LINES, "10 000 lines are retained");
    term
}

fn bench_search(c: &mut Criterion) {
    let term = terminal();
    let cancel = AtomicBool::new(false);
    let mut group = c.benchmark_group("daemon_search");
    group.throughput(Throughput::Elements(u64::try_from(RETAINED_LINES).expect("fits")));
    let regex = SearchOptions { is_regex: true, ..SearchOptions::default() };
    for (name, query, options) in [
        ("literal_insensitive_10k_lines", "warning", SearchOptions::default()),
        ("regex_insensitive_10k_lines", r"warn\w+ \d+", regex),
    ] {
        let matcher = Matcher::new(query, options).expect("a valid query");
        group.bench_function(name, |b| {
            b.iter(|| black_box(search(&SearchInput::of(&term), &matcher, &cancel).matches.len()));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_search);
criterion_main!(benches);
