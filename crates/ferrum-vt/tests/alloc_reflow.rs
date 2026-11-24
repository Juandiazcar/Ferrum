//! Reflow streams the history (FR-TERM-010, ADR 0028): it never holds a second, uncompressed copy
//! of a long soft-wrapped line. One test per binary: `dhat` profiles the whole process.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{NullSink, Size, Terminal};

#[cfg(not(miri))] // dhat is not supported under Miri
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const COLS: u16 = 1_000;
const ROWS: u16 = 24;
/// Bytes of the one logical line (it wraps onto 8 000 rows).
const LINE_BYTES: usize = 8_000_000;
const HISTORY_LINES: usize = 20_000;
/// Extra peak the rewrite may add on top of the history it started with: the hot chunk, its
/// compressed copy and the decoded chunk being read, about 1 MB each for rows of 1 000 columns.
/// A second copy of the line would cost 8 MB and more.
const ALLOWED_EXTRA_BYTES: usize = 4 * 1_024 * 1_024;

/// One row of printable text that LZ4 cannot shrink much (xorshift), so the history has a real
/// size. `state` carries on from row to row.
fn noise(state: &mut u64, len: usize) -> Vec<u8> {
    (0..len)
        .map(|_| {
            *state ^= *state << 13;
            *state ^= *state >> 7;
            *state ^= *state << 17;
            b'!' + u8::try_from(*state % 90).unwrap_or(0)
        })
        .collect()
}

fn reflowing_one_huge_wrapped_line_needs_no_second_copy() {
    let _profiler = dhat::Profiler::builder().testing().build();
    let config = ScrollbackConfig::new(HISTORY_LINES, HISTORY_LINES).expect("config");
    let mut term = Terminal::new(Size::new(COLS, ROWS), Scrollback::new(config), NullSink);
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    // Fed in rows, so the input itself never counts as a copy of the line.
    for _ in 0..LINE_BYTES / usize::from(COLS) {
        term.parse(&noise(&mut state, usize::from(COLS)));
    }
    let before = dhat::HeapStats::get();
    term.resize(Size::new(COLS / 2, ROWS)).expect("resize");
    let after = dhat::HeapStats::get();
    assert!(term.scrollback().len() > 10_000, "the line is in the history");
    assert!(
        after.max_bytes <= before.curr_bytes + ALLOWED_EXTRA_BYTES,
        "peak {} bytes over {} bytes of history",
        after.max_bytes,
        before.curr_bytes
    );
}

fn main() {
    ferrum_testkit::single_test::single_test(
        "reflowing_one_huge_wrapped_line_needs_no_second_copy",
        reflowing_one_huge_wrapped_line_needs_no_second_copy,
    );
}
