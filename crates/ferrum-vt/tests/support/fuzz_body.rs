//! The body of the `parse` fuzz target (`crates/ferrum-vt/fuzz`), shared with the unit test
//! `tests/fuzz_seeds.rs` so that the seeds and a smoke run execute in CI on every platform.
//!
//! The first bytes of the input choose the sizes and the read size; the rest is fed to a terminal
//! in reads of that size, with a resize in the middle (reflow) and queries of the state afterwards.
//! NFR-REL-01: nothing may panic, hang or grow without bound whatever the bytes.

use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{NullSink, Point, Size, Terminal};

/// Header bytes: columns, rows, read size, columns after the resize, rows after the resize.
const HEADER_LEN: usize = 5;
const MAX_SIZE_STEP: u16 = 200;
const HISTORY_LINES: usize = 300;
const SPILL_THRESHOLD_LINES: usize = 1_024;

fn dimension(byte: Option<&u8>) -> u16 {
    1 + u16::from(byte.copied().unwrap_or(0)) % MAX_SIZE_STEP
}

/// Runs one fuzz input.
pub(crate) fn run(input: &[u8]) {
    let Some((header, rest)) = input.split_at_checked(HEADER_LEN) else { return };
    let size = Size::new(dimension(header.first()), dimension(header.get(1)));
    let read = 1 + usize::from(header.get(2).copied().unwrap_or(0));
    let after = Size::new(dimension(header.get(3)), dimension(header.get(4)));
    let config = ScrollbackConfig::new(HISTORY_LINES, SPILL_THRESHOLD_LINES).unwrap_or_default();
    let mut term = Terminal::new(size, Scrollback::new(config), NullSink);
    let (first, second) = rest.split_at(rest.len() / 2);
    for chunk in first.chunks(read) {
        term.parse(chunk);
    }
    term.resize(after).ok();
    for chunk in second.chunks(read) {
        term.parse(chunk);
    }
    term.resize(size).ok();
    let at = Point { line: term.screen_line(term.cursor().y), col: term.cursor().x };
    let word = term.select_word(at);
    let line = term.select_logical_line(at);
    std::hint::black_box((term.selection_text(&word), term.selection_text(&line)));
    std::hint::black_box((term.dump(), term.logical_text()));
}
