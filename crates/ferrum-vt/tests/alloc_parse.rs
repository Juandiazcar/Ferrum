//! `vt::parse` is a hot path (`docs/05` §6, `docs/03` §6): after warm-up it must not allocate.
//! Zero allocations in steady state; the first use of a new grapheme cluster or hyperlink
//! allocates once, bounded by the table caps (`MAX_GRAPHEMES`, `MAX_EXTRAS`,
//! `MAX_LINK_TABLE_BYTES`), like a new style in the scrollback style table; the warm-up pays
//! for the stream's own clusters and links.
//! The stream mixes plain text, SGR, cursor moves, scrolling, OSC titles and links, UTF-8,
//! wide characters and clusters. With history on, full chunks are sealed as the daemon does
//! (`docs/02` §5: the I/O thread hands pending chunks to the worker between reads; here the
//! worker's part runs on the same thread so that `dhat` sees it, and is counted apart), and once
//! with nobody sealing (the inline fallback). One test per binary: `dhat` profiles the whole
//! process.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

use ferrum_scrollback::{ChunkSealer, HOT_CHUNK_LINES, PendingChunk, Scrollback, ScrollbackConfig};
use ferrum_vt::{NullSink, Size, Terminal};

#[cfg(not(miri))] // dhat is not supported under Miri
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const COLS: u16 = 100;
const ROWS: u16 = 30;
const LINES_PER_PASS: usize = 3_000;
const PASSES: usize = 3;
const SPILL_THRESHOLD_LINES: usize = 50_000;
/// History kept in the with-history run: less than three passes, so the warm-up evicts.
const RETAINED_LINES: usize = 6_000;
/// History shorter than one read of the stream: a chunk handed to a lagging worker is evicted
/// before the worker returns it.
const SHORT_HISTORY_LINES: usize = 64;
/// Bytes the daemon reads from a PTY at a time (`docs/02` §5: at most 64 KiB).
const READ_BYTES: usize = 64 * 1_024;

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

/// Who seals full history chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sealing {
    /// The worker, between reads.
    Worker,
    /// The worker, one read late: it holds the handles it took across the next read.
    Lagging,
    /// Nobody: the push path seals once `MAX_PENDING_CHUNKS` wait.
    Nobody,
}

/// The worker's side: its sealer and the handles it holds (`Sealing::Lagging`).
#[derive(Default)]
struct Worker {
    sealer: ChunkSealer,
    held: Vec<PendingChunk>,
}

impl Worker {
    /// Seals and installs every chunk the worker holds; returns how many.
    fn finish_held(&mut self, term: &mut Terminal<NullSink>) -> u64 {
        let mut sealed = 0;
        for pending in self.held.drain(..) {
            term.install_sealed_chunk(self.sealer.seal(pending).expect("seal")).expect("install");
            sealed += 1;
        }
        sealed
    }

    /// The worker's part between two reads.
    fn between_reads(&mut self, term: &mut Terminal<NullSink>, sealing: Sealing) -> u64 {
        match sealing {
            Sealing::Worker => {
                self.held.extend(std::iter::from_fn(|| term.take_pending_chunk()));
                self.finish_held(term)
            }
            Sealing::Lagging => {
                let sealed = self.finish_held(term);
                self.held.extend(std::iter::from_fn(|| term.take_pending_chunk()));
                sealed
            }
            Sealing::Nobody => 0,
        }
    }
}

/// Heap blocks of the measured pass.
#[derive(Debug, Default)]
struct Pass {
    parse_blocks: u64,
    seal_blocks: u64,
    sealed: u64,
    inline_seals: u64,
}

fn feed(
    term: &mut Terminal<NullSink>,
    bytes: &[u8],
    sealing: Sealing,
    worker: &mut Worker,
) -> Pass {
    let mut pass = Pass::default();
    let inline_before = term.scrollback().memory_report().inline_seals;
    for piece in bytes.chunks(READ_BYTES) {
        let before = blocks();
        term.parse(piece);
        pass.parse_blocks += blocks() - before;
        let before = blocks();
        pass.sealed += worker.between_reads(term, sealing);
        pass.seal_blocks += blocks() - before;
    }
    pass.inline_seals = term.scrollback().memory_report().inline_seals - inline_before;
    pass
}

/// Parses the stream `PASSES` times to warm everything up (with history this fills it past
/// `max_lines`, so the last pass evicts), then once more, and returns what that pass allocated.
fn run(max_lines: usize, threshold: usize, sealing: Sealing) -> Pass {
    let config = ScrollbackConfig::new(max_lines, threshold).expect("config");
    let mut term = Terminal::new(Size::new(COLS, ROWS), Scrollback::new(config), NullSink);
    let mut worker = Worker::default();
    let bytes = stream();
    for _ in 0..PASSES {
        feed(&mut term, &bytes, sealing, &mut worker);
    }
    let pushed_before = term.scrollback().pushed_lines();
    let pass = feed(&mut term, &bytes, sealing, &mut worker);
    if max_lines > 0 {
        let pushed = term.scrollback().pushed_lines();
        let chunk = u64::try_from(HOT_CHUNK_LINES).expect("fits");
        assert!(term.scrollback().dropped_lines() > 0, "the history is full: the pass evicts");
        assert!(pushed / chunk > pushed_before / chunk, "the pass fills chunks");
    }
    pass
}

fn parse_allocates_nothing_in_steady_state() {
    let _profiler = dhat::Profiler::builder().testing().build();
    // No history: every allocation would be the parser's.
    let pass = run(0, SPILL_THRESHOLD_LINES, Sealing::Worker);
    assert_eq!(pass.parse_blocks, 0, "parse allocates in steady state: {pass:?}");
    // History on and full (eviction steady state), the worker keeps up: parsing allocates
    // nothing, the pushes that retire full chunks included (pooled buffers); sealing allocates
    // exactly the retained blob per chunk (as `alloc_push_line.rs` of ferrum-scrollback).
    let pass = run(RETAINED_LINES, RETAINED_LINES, Sealing::Worker);
    assert!(pass.sealed > 0, "the pass seals chunks: {pass:?}");
    assert_eq!((pass.parse_blocks, pass.inline_seals), (0, 0), "{pass:?}");
    assert_eq!(pass.seal_blocks, pass.sealed, "one allocation per sealed chunk: {pass:?}");
    // The worker holds each chunk across the next read and the history is shorter than a read,
    // so every chunk is evicted while it is out: its buffer still comes back to the pool.
    let pass = run(SHORT_HISTORY_LINES, SPILL_THRESHOLD_LINES, Sealing::Lagging);
    assert!(pass.sealed > 0, "the pass seals chunks: {pass:?}");
    assert_eq!((pass.parse_blocks, pass.inline_seals), (0, 0), "{pass:?}");
    assert_eq!(pass.seal_blocks, pass.sealed, "one allocation per sealed chunk: {pass:?}");
    // Nobody seals: one allocation per chunk sealed inline, none per line.
    let pass = run(RETAINED_LINES, RETAINED_LINES, Sealing::Nobody);
    assert!(pass.inline_seals > 0, "{pass:?}");
    assert_eq!(pass.parse_blocks, pass.inline_seals, "one allocation per inline seal: {pass:?}");
}

fn main() {
    ferrum_testkit::single_test::single_test(
        "parse_allocates_nothing_in_steady_state",
        parse_allocates_nothing_in_steady_state,
    );
}
