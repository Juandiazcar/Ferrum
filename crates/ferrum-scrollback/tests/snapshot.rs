//! A history snapshot (`docs/02` §5: search receives immutable chunk handles) reads the lines it
//! was taken with — hot, pending, sealed and spilled — while the store keeps pushing and evicting.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

#[path = "support/lines.rs"]
mod lines;

use ferrum_scrollback::{
    FileSpill, LineCursor, MemorySpill, Scrollback, ScrollbackConfig, SpillStore,
};
use lines::{numbered, push};

/// Lines retained in RAM; the rest of the 6 000 pushed go to the spill store.
const RAM_LINES: usize = 2_000;
/// Lines pushed before the snapshot: several sealed chunks, a spilled part and a hot tail.
const PUSHED: usize = 6_000;

fn text(snapshot: &ferrum_scrollback::HistorySnapshot, index: usize) -> String {
    let mut cursor = LineCursor::new();
    snapshot.read_line(index, &mut cursor).expect("read").expect("in range").text.clone()
}

fn keeps_its_lines_while_the_store_goes_on(spill: Box<dyn SpillStore>) {
    let config = ScrollbackConfig::new(RAM_LINES, 50_000).expect("config");
    let mut store = Scrollback::with_spill(config, spill);
    for n in 0..PUSHED {
        push(&mut store, &numbered(n), &[]);
    }
    let snapshot = store.snapshot();
    let first = PUSHED - snapshot.len();
    let expected: Vec<String> = (0..snapshot.len()).map(|i| numbered(first + i).text()).collect();
    for n in PUSHED..PUSHED * 2 {
        push(&mut store, &numbered(n), &[]);
    }
    let reader = std::thread::Builder::new()
        .name("snapshot-reader".into())
        .spawn(move || (0..snapshot.len()).map(|i| text(&snapshot, i)).collect::<Vec<_>>())
        .expect("spawn");
    assert_eq!(reader.join().expect("reader"), expected);
}

#[test]
fn a_snapshot_keeps_its_lines_while_the_store_goes_on() {
    keeps_its_lines_while_the_store_goes_on(Box::new(MemorySpill::new(64 << 20)));
}

#[test]
fn a_snapshot_reads_the_spill_file_while_the_store_goes_on() {
    let name = format!("ferrum-scrollback-snapshot-{}.spill", std::process::id());
    let path = std::env::temp_dir().join(name);
    keeps_its_lines_while_the_store_goes_on(Box::new(FileSpill::new(&path, 64 << 20)));
    assert!(!path.exists(), "the spill file is removed with the store");
}
