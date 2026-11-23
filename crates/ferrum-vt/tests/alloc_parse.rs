//! `vt::parse` is a hot path (`docs/05` §6, `docs/03` §6): after warm-up it must not allocate.
//! The stream mixes plain text, SGR, cursor moves, scrolling, OSC titles and links, UTF-8,
//! wide characters and clusters. One test per binary: `dhat` profiles the whole process.

#![cfg(not(miri))] // dhat is not supported under Miri
#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

use ferrum_scrollback::{HOT_CHUNK_LINES, Scrollback, ScrollbackConfig};
use ferrum_vt::{NullSink, Size, Terminal};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const COLS: u16 = 100;
const ROWS: u16 = 30;
const LINES_PER_PASS: usize = 3_000;
const PASSES: usize = 3;
const SPILL_THRESHOLD_LINES: usize = 50_000;

fn blocks() -> u64 {
    dhat::HeapStats::get().total_blocks
}

/// One pass of mixed output; every line scrolls the screen once it is full.
fn stream() -> Vec<u8> {
    let mut out = Vec::new();
    for n in 0..LINES_PER_PASS {
        out.extend_from_slice(
            b"\x1b[1;32mok\x1b[0m plain text of an ordinary build log line, number ",
        );
        out.extend_from_slice(n.to_string().as_bytes());
        out.extend_from_slice(
            b" \x1b[38;2;10;200;30mrgb\x1b[48;5;17m256\x1b[0m \x1b[4:3;58;5;9mcurly\x1b[m",
        );
        out.extend_from_slice(
            "  \u{6F22}\u{5B57} e\u{301} \u{1F468}\u{200D}\u{1F469} \u{2764}\u{FE0F}".as_bytes(),
        );
        out.extend_from_slice(
            b"\x1b]8;;https://example.test/x\x07link\x1b]8;;\x07 \x1b]2;title\x07",
        );
        out.extend_from_slice(
            b"\x1b[2K\x1b[3G\x1b[1P\x1b[2@\x1b7\x1b[10C\x1b8\x1b[?25l\x1b[?25h\r\n",
        );
    }
    out
}

/// Parses the stream `PASSES` times to warm everything up, then once more; returns the heap
/// blocks of the last pass and the lines pushed to the scrollback.
fn run(max_lines: usize) -> (u64, u64) {
    let config = ScrollbackConfig::new(max_lines, SPILL_THRESHOLD_LINES).expect("config");
    let mut term = Terminal::new(Size::new(COLS, ROWS), Scrollback::new(config), NullSink);
    let bytes = stream();
    for _ in 0..PASSES {
        term.parse(&bytes);
    }
    let before = blocks();
    term.parse(&bytes);
    (blocks() - before, term.scrollback().pushed_lines())
}

#[test]
fn parse_allocates_nothing_in_steady_state() {
    let _profiler = dhat::Profiler::builder().testing().build();
    // No history: every allocation would be the parser's.
    let (without_history, _) = run(0);
    assert_eq!(without_history, 0, "parse allocates in steady state");
    // With history the only allocations are the sealed chunks, which are the retained data, and
    // at most one doubling of the scrollback's list of chunks.
    let (with_history, pushed) = run(SPILL_THRESHOLD_LINES);
    let sealed = u64::try_from(LINES_PER_PASS / HOT_CHUNK_LINES + 2).expect("fits");
    assert!(pushed > 0);
    assert!(
        with_history <= sealed,
        "{with_history} allocations for at most {sealed} sealed chunks"
    );
}
