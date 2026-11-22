//! Spill behavior (`docs/02` §3.2, §10; `docs/10` §3 item 11): threshold, disk cap, disk-full.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

#[path = "support/lines.rs"]
mod lines;

use ferrum_scrollback::{
    FileSpill, LineCursor, MemorySpill, Scrollback, ScrollbackConfig, ScrollbackEvent,
    SpillDisableCause, SpillState, SpillStore,
};
use lines::{numbered, push, read};

const THRESHOLD: usize = 2_048;
const BIG_DISK: u64 = 64 * 1024 * 1024;

fn config(max_lines: usize) -> ScrollbackConfig {
    ScrollbackConfig::new(max_lines, THRESHOLD).expect("config")
}

fn assert_window(store: &Scrollback, total: usize) {
    let retained = store.len();
    let first = total - retained;
    let mut cursor = LineCursor::new();
    for index in 0..retained {
        assert_eq!(
            read(store, index, &mut cursor),
            Some(numbered(first + index).expected()),
            "line {index}"
        );
    }
    assert!(read(store, retained, &mut cursor).is_none());
}

#[test]
fn history_beyond_the_threshold_spills_and_stays_readable() {
    let store_backend = Box::new(MemorySpill::new(BIG_DISK));
    let mut store = Scrollback::with_spill(config(20_000), store_backend);
    for n in 0..9_000 {
        push(&mut store, &numbered(n), &[]);
    }
    assert_eq!(store.len(), 9_000);
    assert!(store.spilled_chunks() > 0, "cold chunks left RAM");
    assert!(store.memory_report().spill_disk_bytes > 0);
    assert_window(&store, 9_000);
}

#[test]
fn ram_resident_lines_never_exceed_the_threshold() {
    let mut store = Scrollback::with_spill(config(100_000), Box::new(MemorySpill::new(BIG_DISK)));
    let mut peak_sealed = 0;
    for n in 0..30_000 {
        push(&mut store, &numbered(n), &[]);
        peak_sealed = peak_sealed.max(store.memory_report().sealed_bytes);
    }
    // 2 048 lines in RAM at roughly 40 bytes each, plus one chunk of slack: far below all 30 000.
    assert!(peak_sealed < 30_000 * 20, "sealed RAM stayed bounded: {peak_sealed}");
    assert_window(&store, 30_000);
}

#[test]
fn disk_cap_drops_the_oldest_spilled_chunks() {
    let mut store = Scrollback::with_spill(config(1_000_000), Box::new(MemorySpill::new(40_000)));
    for n in 0..30_000 {
        push(&mut store, &numbered(n), &[]);
    }
    assert_eq!(store.spill_state(), SpillState::Active);
    assert!(store.take_event().is_none(), "reaching the cap is not a failure");
    assert!(store.len() < 30_000 && store.len() > THRESHOLD, "len {}", store.len());
    assert!(store.dropped_lines() > 0);
    assert!(store.memory_report().spill_disk_bytes <= 40_000);
    assert_window(&store, 30_000);
}

#[test]
fn disk_full_disables_spill_reports_once_and_drops_the_oldest_lines() {
    let mut backend = MemorySpill::new(BIG_DISK);
    backend.fail_appends_after(2);
    let mut store = Scrollback::with_spill(config(50_000), Box::new(backend));
    for n in 0..20_000 {
        push(&mut store, &numbered(n), &[]);
    }
    assert_eq!(store.spill_state(), SpillState::Disabled);
    assert_eq!(store.take_event(), Some(ScrollbackEvent::SpillDisabled(SpillDisableCause::Io)));
    assert_eq!(store.take_event(), None, "reported once");
    assert!(store.len() <= THRESHOLD, "RAM history is capped at the threshold: {}", store.len());
    assert!(store.dropped_lines() > 0);
    assert_window(&store, 20_000);
}

#[test]
fn without_a_store_history_is_capped_at_the_threshold() {
    let mut store = Scrollback::new(config(50_000));
    for n in 0..10_000 {
        push(&mut store, &numbered(n), &[]);
    }
    assert_eq!(store.spill_state(), SpillState::Unavailable);
    assert_eq!(store.len(), THRESHOLD);
    assert!(store.take_event().is_none());
    assert_window(&store, 10_000);
}

#[test]
fn a_record_larger_than_the_whole_store_disables_spill() {
    let mut store = Scrollback::with_spill(config(50_000), Box::new(MemorySpill::new(10)));
    for n in 0..5_000 {
        push(&mut store, &numbered(n), &[]);
    }
    assert_eq!(
        store.take_event(),
        Some(ScrollbackEvent::SpillDisabled(SpillDisableCause::RecordTooLarge))
    );
}

fn temp_spill_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("ferrum-scrollback-test-{}-{name}.spill", std::process::id()))
}

#[test]
fn file_spill_round_trips_wraps_and_cleans_up() {
    let path = temp_spill_path("wrap");
    let spill = FileSpill::new(&path, 60_000);
    assert!(!path.exists(), "the file is created on the first spill, not before");
    let mut store = Scrollback::with_spill(config(1_000_000), Box::new(spill));
    for n in 0..40_000 {
        push(&mut store, &numbered(n), &[]);
    }
    assert!(path.exists());
    let on_disk = std::fs::metadata(&path).expect("metadata").len();
    assert!(on_disk <= 60_000, "the file never exceeds its cap: {on_disk}");
    assert!(store.dropped_lines() > 0, "the ring wrapped and old chunks were dropped");
    assert_window(&store, 40_000);
    drop(store);
    assert!(!path.exists(), "the spill file is removed when the session ends");
}

#[test]
fn file_spill_failure_to_create_is_a_disk_error() {
    let path = std::env::temp_dir().join("ferrum-no-such-dir-xyz").join("x.spill");
    let mut store =
        Scrollback::with_spill(config(50_000), Box::new(FileSpill::new(path, BIG_DISK)));
    for n in 0..6_000 {
        push(&mut store, &numbered(n), &[]);
    }
    assert_eq!(store.take_event(), Some(ScrollbackEvent::SpillDisabled(SpillDisableCause::Io)));
    assert_window(&store, 6_000);
}

#[test]
fn spill_store_trait_object_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn SpillStore>>();
    assert_send::<Scrollback>();
}
