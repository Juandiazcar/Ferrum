//! Shared contract suite for every [`SpillStore`] implementation (`docs/08` §2: the same suite for
//! real and fake implementations; `docs/05` §2 Liskov).
//!
//! Call [`run`] from a test with a factory that builds an empty store of the requested capacity.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used,
    reason = "test-support code: a failed expectation is a failed test"
)]

use ferrum_scrollback::{SpillError, SpillStore};

/// Capacity used by the capacity-sensitive cases.
const CAPACITY: u64 = 1_000;

/// Runs every contract case; `make(capacity)` must return a new, empty store.
pub fn run(make: &dyn Fn(u64) -> Box<dyn SpillStore>) {
    round_trips_records_of_every_size(make);
    reads_replace_the_buffer_contents(make);
    unknown_handles_are_rejected(make);
    full_is_reported_and_stores_nothing(make);
    discarding_frees_space_and_is_fifo(make);
    wrapping_around_the_end_keeps_data_intact(make);
    discarding_an_empty_store_is_a_no_op(make);
}

fn record(seed: u8, len: usize) -> Vec<u8> {
    (0..len).map(|i| seed.wrapping_add(u8::try_from(i % 251).unwrap_or(0))).collect()
}

fn round_trips_records_of_every_size(make: &dyn Fn(u64) -> Box<dyn SpillStore>) {
    let mut store = make(CAPACITY);
    let mut handles = Vec::new();
    for (seed, len) in [0usize, 1, 2, 99, 300].into_iter().enumerate() {
        let bytes = record(u8::try_from(seed).unwrap_or(0), len);
        handles.push((store.append(&bytes).unwrap(), bytes));
    }
    let total: usize = handles.iter().map(|(_, bytes)| bytes.len()).sum();
    assert_eq!(store.live_bytes(), u64::try_from(total).unwrap());
    let mut out = Vec::new();
    for (handle, bytes) in &handles {
        store.read(*handle, &mut out).unwrap();
        assert_eq!(&out, bytes);
    }
}

fn reads_replace_the_buffer_contents(make: &dyn Fn(u64) -> Box<dyn SpillStore>) {
    let mut store = make(CAPACITY);
    let handle = store.append(b"abc").unwrap();
    let mut out = b"stale and longer".to_vec();
    store.read(handle, &mut out).unwrap();
    assert_eq!(out, b"abc");
}

fn unknown_handles_are_rejected(make: &dyn Fn(u64) -> Box<dyn SpillStore>) {
    let mut store = make(CAPACITY);
    let first = store.append(b"one").unwrap();
    let second = store.append(b"two").unwrap();
    store.discard_oldest().unwrap();
    let mut out = Vec::new();
    assert!(matches!(store.read(first, &mut out), Err(SpillError::UnknownHandle)));
    store.read(second, &mut out).unwrap();
    assert_eq!(out, b"two");
    let other = make(CAPACITY);
    assert!(matches!(other.read(second, &mut out), Err(SpillError::UnknownHandle)));
}

fn full_is_reported_and_stores_nothing(make: &dyn Fn(u64) -> Box<dyn SpillStore>) {
    let mut store = make(CAPACITY);
    assert_eq!(store.capacity_bytes(), CAPACITY);
    store.append(&record(1, 600)).unwrap();
    let before = store.live_bytes();
    let refused = store.append(&record(2, 401));
    assert!(matches!(refused, Err(SpillError::Full { needed: 401, free: 400 })));
    assert_eq!(store.live_bytes(), before, "a refused append stores nothing");
    store.append(&record(3, 400)).unwrap();
    assert_eq!(store.live_bytes(), CAPACITY, "an exact fit is accepted");
}

fn discarding_frees_space_and_is_fifo(make: &dyn Fn(u64) -> Box<dyn SpillStore>) {
    let mut store = make(CAPACITY);
    let a = store.append(&record(1, 500)).unwrap();
    let b = store.append(&record(2, 500)).unwrap();
    assert!(store.append(&[0]).is_err());
    store.discard_oldest().unwrap();
    assert_eq!(store.live_bytes(), 500);
    let c = store.append(&record(3, 500)).unwrap();
    let mut out = Vec::new();
    assert!(store.read(a, &mut out).is_err());
    store.read(b, &mut out).unwrap();
    assert_eq!(out, record(2, 500));
    store.read(c, &mut out).unwrap();
    assert_eq!(out, record(3, 500));
}

fn wrapping_around_the_end_keeps_data_intact(make: &dyn Fn(u64) -> Box<dyn SpillStore>) {
    let mut store = make(CAPACITY);
    let mut live = std::collections::VecDeque::new();
    let mut out = Vec::new();
    for round in 0..200u32 {
        let len = usize::try_from(round % 7).unwrap() * 37 + 13;
        let bytes = record(u8::try_from(round % 251).unwrap(), len);
        loop {
            match store.append(&bytes) {
                Ok(handle) => {
                    live.push_back((handle, bytes));
                    break;
                }
                Err(SpillError::Full { .. }) => {
                    store.discard_oldest().unwrap();
                    live.pop_front();
                }
                Err(other) => panic!("unexpected error: {other}"),
            }
        }
        for (handle, expected) in &live {
            store.read(*handle, &mut out).unwrap();
            assert_eq!(&out, expected, "round {round}");
        }
    }
}

fn discarding_an_empty_store_is_a_no_op(make: &dyn Fn(u64) -> Box<dyn SpillStore>) {
    let mut store = make(CAPACITY);
    store.discard_oldest().unwrap();
    assert_eq!(store.live_bytes(), 0);
}
