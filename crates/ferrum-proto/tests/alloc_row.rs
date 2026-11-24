//! `proto::encode_row` is a hot path (`docs/05` §6): once its output buffer and link table have
//! the capacity, encoding rows allocates nothing.
//! One test per binary: `dhat` profiles the whole process.

#![cfg(not(miri))] // dhat is not supported under Miri
#![allow(clippy::unwrap_used, reason = "test code: a failed expectation is a failed test")]

use ferrum_proto::{LinkTable, encode_row};

#[path = "support/sample.rs"]
mod sample;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// Rows encoded per pass: a 200×60 screen.
const ROWS: u16 = 60;
const PASSES: usize = 100;

fn blocks() -> u64 {
    dhat::HeapStats::get().total_blocks
}

fn encode_screen(out: &mut Vec<u8>, links: &mut LinkTable) {
    out.clear();
    links.clear();
    for _ in 0..ROWS {
        encode_row(sample::COLS, sample::sample_cells(), out, links).unwrap();
    }
}

#[test]
fn encoding_rows_allocates_nothing_in_steady_state() {
    let _profiler = dhat::Profiler::builder().testing().build();
    let (mut out, mut links) = (Vec::new(), LinkTable::new());
    // Warm-up: the buffers grow to the size of a screen once.
    encode_screen(&mut out, &mut links);

    let before = blocks();
    for _ in 0..PASSES {
        encode_screen(&mut out, &mut links);
    }
    assert_eq!(blocks() - before, 0, "steady-state row encoding allocates");
}
