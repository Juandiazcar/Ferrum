//! An interrupted reflow (`docs/02` §3.2, `docs/05` §5: a large reflow checks its stop request
//! at chunk boundaries) loses nothing: after the next, complete resize the logical text is the
//! text a single uninterrupted resize gives.

#![allow(clippy::unwrap_used, reason = "test code: a failure is a failed test")]

use ferrum_scrollback::{
    MemorySpill, Scrollback, ScrollbackConfig, SpillError, SpillHandle, SpillStore,
};
use ferrum_vt::{NullSink, Reflowed, Size, Terminal};

/// Logical lines of history: several chunks of rows (the stop request is asked more than once),
/// but fewer rows than the 10 000 kept at every width, so no width drops a line.
const LINES: usize = 1_500;

fn filled() -> Terminal<NullSink> {
    let config = ScrollbackConfig::new(10_000, 50_000).unwrap();
    let mut term = Terminal::new(Size::new(80, 24), Scrollback::new(config), NullSink);
    for index in 0..LINES {
        let line = format!("line {index:05} {}\r\n", "word ".repeat(index % 30));
        term.parse(line.as_bytes());
    }
    term
}

#[test]
fn an_interrupted_reflow_then_a_complete_one_keeps_every_logical_line() {
    let mut direct = filled();
    direct.resize(Size::new(33, 24)).unwrap();
    let mut interrupted = filled();
    let mut asked = 0;
    let mut stop = || {
        asked += 1;
        asked == 2
    };
    let (_, first) = interrupted.resize_until(Size::new(50, 24), &mut stop).unwrap();
    assert_eq!(first, Reflowed::Interrupted);
    let (_, second) = interrupted.resize_until(Size::new(33, 24), &mut || false).unwrap();
    assert_eq!(second, Reflowed::Complete);
    assert_eq!(interrupted.scrollback().len(), direct.scrollback().len());
    assert_eq!(interrupted.logical_text(), direct.logical_text());
}

#[test]
fn reflow_until_finishes_an_interrupted_reflow_at_the_current_size() {
    let mut direct = filled();
    direct.resize(Size::new(33, 24)).unwrap();
    let mut resumed = filled();
    let mut asked = 0;
    let mut stop = || {
        asked += 1;
        asked == 1
    };
    let (_, first) = resumed.resize_until(Size::new(33, 24), &mut stop).unwrap();
    assert_eq!(first, Reflowed::Interrupted);
    assert_eq!(resumed.reflow_until(&mut || false).unwrap(), Reflowed::Complete);
    assert_eq!(resumed.logical_text(), direct.logical_text());
}

/// A spill store whose `n`th read fails once (a transient I/O error).
#[derive(Debug)]
struct FlakyStore {
    inner: MemorySpill,
    reads: std::sync::atomic::AtomicUsize,
    fail_at: usize,
}

impl SpillStore for FlakyStore {
    fn append(&mut self, record: &[u8]) -> Result<SpillHandle, SpillError> {
        self.inner.append(record)
    }

    fn read(&self, handle: SpillHandle, out: &mut Vec<u8>) -> Result<(), SpillError> {
        let n = self.reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if n == self.fail_at {
            return Err(SpillError::Io(std::io::Error::other("injected")));
        }
        self.inner.read(handle, out)
    }

    fn discard_oldest(&mut self) -> Result<(), SpillError> {
        self.inner.discard_oldest()
    }

    fn live_bytes(&self) -> u64 {
        self.inner.live_bytes()
    }

    fn capacity_bytes(&self) -> u64 {
        self.inner.capacity_bytes()
    }
}

#[test]
fn a_row_lost_while_an_interrupted_reflow_moves_the_rest_is_reported_and_order_kept() {
    // Spilled history: the rows the stop leaves behind are read back from the store, and one of
    // those reads fails.
    let config = ScrollbackConfig::new(10_000, 1_024).unwrap();
    let store = FlakyStore { inner: MemorySpill::new(64 << 20), reads: 0.into(), fail_at: 2 };
    let scrollback = Scrollback::with_spill(config, Box::new(store));
    let mut term = Terminal::new(Size::new(20, 4), scrollback, NullSink);
    for n in 0..6_000 {
        term.parse(format!("row {n:05}\r\n").as_bytes());
    }
    let stopped = term.resize_until(Size::new(40, 4), &mut || true);
    assert!(stopped.is_err(), "the lost row is reported");
    let numbers: Vec<u32> = term
        .logical_text()
        .lines()
        .filter_map(|line| line.strip_prefix("row ")?.parse().ok())
        .collect();
    assert!(numbers.len() > 4_000, "most history survives: {}", numbers.len());
    assert!(numbers.windows(2).all(|pair| pair[0] < pair[1]), "history stays in order");
}
