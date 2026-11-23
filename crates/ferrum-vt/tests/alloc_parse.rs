//! `vt::parse` is a hot path (`docs/05` §6, `docs/03` §6): after warm-up it must not allocate.
//! Zero allocations in steady state; the first use of a new grapheme cluster or hyperlink
//! allocates once, bounded by the table caps (`MAX_GRAPHEMES`, `MAX_EXTRAS`,
//! `MAX_LINK_TABLE_BYTES`), like a new style in the scrollback style table; the warm-up pays
//! for the stream's own clusters and links.
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
/// History kept in the with-history run: less than three passes, so the warm-up evicts.
const RETAINED_LINES: usize = 3_000;

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
        // A title that is not UTF-8, color queries and DCS requests: every reply path.
        out.extend_from_slice(b"\x1b]2;bad\xff\xfe title\x07\x1b]4;1;?\x07\x1b]11;?\x07");
        out.extend_from_slice(b"\x1bP+q544e\x1b\\\x1bP$qm\x1b\\\x1b[c\x1b[6n\x1b[?2004$p");
        out.extend_from_slice(
            b"\x1b[2K\x1b[3G\x1b[1P\x1b[2@\x1b7\x1b[10C\x1b8\x1b[?25l\x1b[?25h\r\n",
        );
    }
    out
}

/// Parses the stream `PASSES` times to warm everything up (with history this fills it past
/// `max_lines`, so the last pass evicts), then once more. Returns the heap blocks allocated by
/// the last pass and the chunks it sealed inline (nobody takes the pending chunks here, so once
/// `MAX_PENDING_CHUNKS` wait the push path seals; ferrum-scrollback `tests/pending.rs`).
fn run(max_lines: usize, threshold: usize) -> (u64, u64) {
    let config = ScrollbackConfig::new(max_lines, threshold).expect("config");
    let mut term = Terminal::new(Size::new(COLS, ROWS), Scrollback::new(config), NullSink);
    let bytes = stream();
    for _ in 0..PASSES {
        term.parse(&bytes);
    }
    let pushed_before = term.scrollback().pushed_lines();
    let inline_before = term.scrollback().memory_report().inline_seals;
    let before = blocks();
    term.parse(&bytes);
    let allocated = blocks() - before;
    let pushed = term.scrollback().pushed_lines();
    if max_lines > 0 {
        assert!(term.scrollback().dropped_lines() > 0, "the history is full: the pass evicts");
        assert!(pushed > pushed_before, "the pass pushes lines");
    }
    let chunk = u64::try_from(HOT_CHUNK_LINES).expect("fits");
    assert!(pushed / chunk > pushed_before / chunk || max_lines == 0, "the pass fills chunks");
    (allocated, term.scrollback().memory_report().inline_seals - inline_before)
}

#[test]
fn parse_allocates_nothing_in_steady_state() {
    let _profiler = dhat::Profiler::builder().testing().build();
    // No history: every allocation would be the parser's.
    let (without_history, _) = run(0, SPILL_THRESHOLD_LINES);
    assert_eq!(without_history, 0, "parse allocates in steady state");
    // History on and full (eviction steady state): exactly one allocation per chunk sealed inline
    // (the retained blob, as in `alloc_push_line.rs` of ferrum-scrollback), none per line and
    // none for a chunk that becomes pending (pooled buffers).
    let (with_history, inline) = run(RETAINED_LINES, RETAINED_LINES);
    assert_eq!(with_history, inline, "{with_history} allocations for {inline} inline seals");
}
