//! A full extras table under a hostile stream must not allocate on the parse path (`docs/05` §6,
//! ADR 0028 decision 3): every new underline color forces a sweep, and a sweep marks the history
//! from its per-block summaries, never by reading it back; a refused hyperlink is refused before
//! its key is copied. Zero allocations in steady state; the first use of a new grapheme cluster
//! or hyperlink allocates once, bounded by the table caps (`MAX_GRAPHEMES`, `MAX_EXTRAS`,
//! `MAX_LINK_TABLE_BYTES`). A new underline color allocates nothing (its record is two words).
//! One test per binary: `dhat` profiles the whole process.

#![cfg(not(miri))] // dhat is not supported under Miri
#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{MAX_EXTRAS, NullSink, Size, Terminal};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const HISTORY_LINES: usize = 10_000;
const SPILL_THRESHOLD_LINES: usize = 50_000;
/// Distinct colors or links per pass.
const SEQUENCES: u32 = 3_000;

fn blocks() -> u64 {
    dhat::HeapStats::get().total_blocks
}

/// A terminal whose extras table holds `entries` hyperlinks, all used by history lines.
fn full_of_history_links(entries: usize) -> Terminal<NullSink> {
    let config = ScrollbackConfig::new(HISTORY_LINES, SPILL_THRESHOLD_LINES).expect("config");
    let mut term = Terminal::new(Size::new(80, 24), Scrollback::new(config), NullSink);
    for n in 0..entries {
        term.parse(format!("\x1b]8;;https://x.test/{n}\x07x\x1b]8;;\x07\r\n").as_bytes());
    }
    term
}

/// One distinct underline color per sequence, each over the same cell: every new color frees
/// exactly the previous one, the worst case for the sweep.
fn colors(first: u32) -> Vec<u8> {
    let mut out = Vec::new();
    for n in first..first + SEQUENCES {
        let [_, r, g, b] = n.to_be_bytes();
        out.extend_from_slice(format!("\x1b[1;1H\x1b[58;2;{r};{g};{b}mx").as_bytes());
    }
    out
}

/// One distinct hyperlink per sequence, opened and never used by a cell.
fn links(first: u32) -> Vec<u8> {
    let mut out = Vec::new();
    for n in first..first + SEQUENCES {
        out.extend_from_slice(format!("\x1b]8;;https://new.test/{n}\x07").as_bytes());
    }
    out
}

#[test]
fn a_full_extras_table_allocates_nothing_in_steady_state() {
    let _profiler = dhat::Profiler::builder().testing().build();
    // One entry left: every new color takes it and frees the previous one.
    let mut term = full_of_history_links(MAX_EXTRAS - 1);
    let (warm_up, measured) = (colors(0), colors(SEQUENCES));
    term.parse(&warm_up);
    let before = blocks();
    term.parse(&measured);
    assert_eq!(blocks() - before, 0, "the full-table color stream allocates");
    assert!(term.extras().overflowed() > 0, "the stream hits the full table");
    // No room at all: every new link is refused, and a refusal allocates nothing.
    let mut term = full_of_history_links(MAX_EXTRAS);
    let (warm_up, measured) = (links(0), links(SEQUENCES));
    term.parse(&warm_up);
    let (before, refused) = (blocks(), term.extras().overflowed());
    term.parse(&measured);
    assert_eq!(blocks() - before, 0, "a refused hyperlink allocates");
    assert_eq!(term.extras().overflowed() - refused, u64::from(SEQUENCES), "all refused");
}
