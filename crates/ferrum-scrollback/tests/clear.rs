//! `clear`: drops all history without decoding it (CSI 3 J, RIS in `ferrum-vt`).

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

#[path = "support/lines.rs"]
mod lines;

use ferrum_scrollback::{LineCursor, MemorySpill, Scrollback, ScrollbackConfig};
use lines::{numbered, push, read};

const THRESHOLD: usize = 2_048;
const BIG_DISK: u64 = 64 * 1024 * 1024;
const LINES: usize = 9_000;

#[test]
fn clear_drops_hot_sealed_and_spilled_lines_and_the_store_keeps_working() {
    let config = ScrollbackConfig::new(20_000, THRESHOLD).expect("config");
    let mut store = Scrollback::with_spill(config, Box::new(MemorySpill::new(BIG_DISK)));
    for n in 0..LINES {
        push(&mut store, &numbered(n), &[]);
    }
    assert!(store.spilled_chunks() > 0);
    store.clear();
    let report = store.memory_report();
    assert_eq!((store.len(), store.spilled_chunks(), report.spill_disk_bytes), (0, 0, 0));
    assert_eq!(store.dropped_lines(), u64::try_from(LINES).expect("fits"));
    assert_eq!(store.pushed_lines(), u64::try_from(LINES).expect("fits"));
    let mut cursor = LineCursor::new();
    assert!(read(&store, 0, &mut cursor).is_none());
    push(&mut store, &numbered(7), &[]);
    assert_eq!(read(&store, 0, &mut cursor), Some(numbered(7).expected()));
}
