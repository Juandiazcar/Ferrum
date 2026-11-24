//! The extras table (hyperlinks, underline colors) under pressure: ids of rows that are gone are
//! released and reused, ids of rows that still exist are never re-pointed (Q-24).

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

use ferrum_scrollback::{LineCursor, Scrollback, ScrollbackConfig};
use ferrum_vt::{Cell, MAX_EXTRAS, NullSink, Size, Terminal};

fn terminal(history: usize) -> Terminal<NullSink> {
    let config = ScrollbackConfig::new(history, SPILL_THRESHOLD_LINES)
        .expect("a valid scrollback configuration");
    Terminal::new(Size::new(20, 4), Scrollback::new(config), NullSink)
}

fn open_link(n: usize) -> Vec<u8> {
    format!("\x1b]8;;https://x.test/{n}\x07").into_bytes()
}

const SPILL_THRESHOLD_LINES: usize = 50_000;
const CLOSE_LINK: &[u8] = b"\x1b]8;;\x07";

/// The URI of the cell at (`x`, `y`) of the screen.
fn screen_uri(term: &Terminal<NullSink>, x: usize, y: usize) -> Option<String> {
    let cell = term.grid().row(y)?.get(x)?;
    let link = term.extras().link(cell.extra());
    term.extras().uri(link).map(str::to_owned)
}

/// The URI of the first run of history line `index`.
fn history_uri(term: &Terminal<NullSink>, index: usize) -> Option<String> {
    let mut cursor = LineCursor::new();
    let line = term.scrollback().read_line(index, &mut cursor).ok()??;
    let style = term.scrollback().styles().get(line.runs.first()?.style)?;
    let mut cell = Cell::default();
    cell.set_style(style.fg, style.bg, style.attrs);
    let extra = cell.extra();
    term.extras().uri(term.extras().link(extra)).map(str::to_owned)
}

#[test]
fn ids_of_overwritten_cells_are_released_and_a_live_cell_is_never_re_pointed() {
    let mut term = terminal(50);
    term.parse(b"\x1b[1;1H");
    term.parse(&open_link(0));
    term.parse(b"A");
    term.parse(CLOSE_LINK);
    for n in 1..=MAX_EXTRAS + 2_000 {
        term.parse(b"\x1b[4;1H");
        term.parse(&open_link(n));
        term.parse(b"x");
        term.parse(CLOSE_LINK);
        assert_eq!(screen_uri(&term, 0, 3), Some(format!("https://x.test/{n}")), "link {n}");
    }
    assert_eq!(term.extras().overflowed(), 0);
    assert_eq!(screen_uri(&term, 0, 0), Some("https://x.test/0".to_owned()));
    assert!(term.extras().len() < MAX_EXTRAS);
}

#[test]
fn ids_of_evicted_history_rows_are_released_and_retained_rows_keep_theirs() {
    let mut term = terminal(30);
    for n in 0..MAX_EXTRAS + 3_000 {
        term.parse(&open_link(n));
        term.parse(b"x");
        term.parse(CLOSE_LINK);
        term.parse(b"\r\n");
    }
    assert_eq!(term.extras().overflowed(), 0);
    // The oldest retained line is the one that scrolled off 30 lines before the 3 screen rows.
    let total = MAX_EXTRAS + 3_000;
    let first_retained = total - term.scrollback().len() - 3;
    for index in 0..term.scrollback().len() {
        let expected = format!("https://x.test/{}", first_retained + index);
        assert_eq!(history_uri(&term, index), Some(expected), "history line {index}");
    }
}

#[test]
fn a_table_full_of_live_ids_refuses_and_counts_without_re_pointing_anything() {
    let mut term = terminal(10_000);
    let total = MAX_EXTRAS + 200;
    for n in 0..total {
        term.parse(&open_link(n));
        term.parse(b"x");
        term.parse(CLOSE_LINK);
        term.parse(b"\r\n");
    }
    assert!(term.extras().overflowed() > 0);
    for index in (0..MAX_EXTRAS - 10).step_by(97) {
        let line = total - term.scrollback().len() - 3 + index;
        assert_eq!(history_uri(&term, index), Some(format!("https://x.test/{line}")));
    }
}

#[test]
fn a_full_reset_releases_every_id() {
    let mut term = terminal(30);
    for n in 0..100 {
        term.parse(&open_link(n));
        term.parse(b"x");
    }
    assert!(!term.extras().is_empty());
    term.parse(b"\x1bc");
    assert!(term.extras().is_empty());
}

#[test]
fn clearing_the_scrollback_releases_the_ids_only_it_used() {
    let mut term = terminal(30);
    for n in 0..40 {
        term.parse(&open_link(n));
        term.parse(b"x");
        term.parse(CLOSE_LINK);
        term.parse(b"\r\n");
    }
    let before = term.extras().len();
    assert!(before > 30);
    term.parse(b"\x1b[3J");
    assert!(term.extras().len() < before - 25);
}

/// A spill store that counts its reads.
#[derive(Debug)]
struct CountingStore {
    inner: ferrum_scrollback::MemorySpill,
    reads: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl ferrum_scrollback::SpillStore for CountingStore {
    fn append(
        &mut self,
        record: &[u8],
    ) -> Result<ferrum_scrollback::SpillHandle, ferrum_scrollback::SpillError> {
        self.inner.append(record)
    }

    fn read(
        &self,
        handle: ferrum_scrollback::SpillHandle,
        out: &mut Vec<u8>,
    ) -> Result<(), ferrum_scrollback::SpillError> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.inner.read(handle, out)
    }

    fn discard_oldest(&mut self) -> Result<(), ferrum_scrollback::SpillError> {
        self.inner.discard_oldest()
    }

    fn live_bytes(&self) -> u64 {
        self.inner.live_bytes()
    }

    fn capacity_bytes(&self) -> u64 {
        self.inner.capacity_bytes()
    }
}

/// Bytes the in-memory spill store may hold.
const SPILL_BYTES: u64 = 64 * 1_024 * 1_024;
/// Lines kept in RAM before chunks spill: one chunk, so most of the history is on the store.
const SMALL_THRESHOLD_LINES: usize = 1_024;

#[test]
fn a_full_table_never_reads_the_history_back_on_the_parse_path() {
    let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let store = CountingStore {
        inner: ferrum_scrollback::MemorySpill::new(SPILL_BYTES),
        reads: std::sync::Arc::clone(&reads),
    };
    let config =
        ScrollbackConfig::new(10_000, SMALL_THRESHOLD_LINES).expect("a valid configuration");
    let mut term =
        Terminal::new(Size::new(20, 4), Scrollback::with_spill(config, Box::new(store)), NullSink);
    for n in 0..MAX_EXTRAS + 200 {
        term.parse(&open_link(n));
        term.parse(b"x");
        term.parse(CLOSE_LINK);
        term.parse(b"\r\n");
    }
    for n in 0..2_000_u32 {
        let [_, r, g, b] = n.to_be_bytes();
        term.parse(format!("\x1b[1;1H\x1b[58;2;{r};{g};{b}mx").as_bytes());
        term.parse(&open_link(100_000 + usize::try_from(n).expect("fits")));
    }
    assert!(term.scrollback().spilled_chunks() > 0, "the history is on the store");
    assert!(term.extras().overflowed() > 0, "the table is full of live ids");
    assert_eq!(reads.load(std::sync::atomic::Ordering::Relaxed), 0, "no history read");
}
