//! Damage frames are a hot path (`docs/03` §3 rule 8): once the buffers have grown, encoding a
//! frame of a scrolling screen into the client's frame buffer allocates nothing. The terminal
//! scrolls (so frames carry `scroll_delta` and only the new rows), the cursor moves, and every
//! frame is encoded into one reused buffer as `Client::send` does. One test per binary: `dhat`
//! profiles the whole process.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

use std::io::Write as _;

use ferrum_daemon::View;
use ferrum_daemon::frame::FrameEncoder;
use ferrum_proto::{DaemonMessage, SessionId, encode_frame};
use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{NullSink, Size, Terminal};

#[cfg(not(miri))] // dhat is not supported under Miri
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// The reference grid (`docs/03` §1).
const COLS: u16 = 200;
const ROWS: u16 = 55;
/// Frames before measuring (buffers grow to their working size) and measured.
const WARM_UP_FRAMES: usize = 200;
const MEASURED_FRAMES: usize = 1_000;
/// Lines of output per frame: a fast scroll, as `cat` of a log shows it.
const LINES_PER_FRAME: usize = 3;
/// The output scrolls in rows 2 to 55 (a status line above, as in a pager or a build tool), so
/// no line goes to the history: the history's own allocations (a sealed chunk) are not frames'.
const SCROLL_REGION: &[u8] = b"\x1b[2;55r\x1b[55;1H";

fn blocks() -> u64 {
    dhat::HeapStats::get().total_blocks
}

/// Output of frame `n`: a few lines, then a cursor move.
fn output(n: usize, out: &mut Vec<u8>) {
    out.clear();
    for line in 0..LINES_PER_FRAME {
        // `write!` into the reused buffer: the stream itself allocates nothing.
        write!(out, "\x1b[32mframe {n:06}\x1b[0m line {line} of ordinary output\r\n")
            .expect("a Vec takes the bytes");
    }
}

fn frames_allocate_nothing_in_steady_state() {
    let _profiler = dhat::Profiler::builder().testing().build();
    let history = Scrollback::new(ScrollbackConfig::default());
    let mut term = Terminal::new(Size::new(COLS, ROWS), history, NullSink);
    term.parse(SCROLL_REGION);
    let (mut encoder, mut view) = (FrameEncoder::default(), View::attached(0));
    let mut buffer = Vec::new();
    let mut send = |message: &DaemonMessage| {
        buffer.clear();
        encode_frame(message, &mut buffer).expect("a frame");
    };
    let id = SessionId(1);
    encoder.snapshot(&term, (id, 0), &mut view, &mut send);
    let mut bytes = Vec::with_capacity(1_024);
    let mut frame =
        |n: usize, term: &mut Terminal<NullSink>, send: &mut dyn FnMut(&DaemonMessage)| {
            output(n, &mut bytes);
            term.parse(&bytes);
            view.merge(term.damage(), usize::from(ROWS));
            term.take_damage();
            encoder.damage(term, id, &mut view, send);
        };
    for n in 0..WARM_UP_FRAMES {
        frame(n, &mut term, &mut send);
    }
    let before = blocks();
    for n in WARM_UP_FRAMES..WARM_UP_FRAMES + MEASURED_FRAMES {
        frame(n, &mut term, &mut send);
    }
    assert_eq!(blocks() - before, 0, "damage frames allocate in steady state");
    assert_eq!(term.scrollback().len(), 0, "the measurement pushed no history");
}

fn main() {
    ferrum_testkit::single_test::single_test(
        "frames_allocate_nothing_in_steady_state",
        frames_allocate_nothing_in_steady_state,
    );
}
