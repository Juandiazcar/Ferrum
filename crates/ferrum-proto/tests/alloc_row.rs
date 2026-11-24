//! `proto::encode_row` is a hot path (`docs/05` §6): once its output buffer has the capacity,
//! encoding rows, filling a `RowBlock` and framing a `Damage` allocate nothing.
//! One test per binary: `dhat` profiles the whole process.

#![cfg(not(miri))] // dhat is not supported under Miri
#![allow(clippy::unwrap_used, reason = "test code: a failed expectation is a failed test")]

use ferrum_proto::{
    Cursor, CursorShape, DaemonMessage, Damage, LinkTable, RowBlock, SessionId, encode_frame,
    encode_row,
};

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

fn fill(damage: &mut DaemonMessage) {
    if let DaemonMessage::Damage(damage) = damage {
        damage.rows.clear();
        for y in 0..ROWS {
            damage.rows.push_row(y, sample::COLS, sample::sample_cells()).unwrap();
        }
    }
}

#[test]
fn encoding_rows_allocates_nothing_in_steady_state() {
    let _profiler = dhat::Profiler::builder().testing().build();
    let cursor =
        Cursor { col: 0, row: 0, shape: CursorShape::Block, is_blinking: false, is_visible: true };
    let mut damage = DaemonMessage::Damage(Damage {
        session_id: SessionId(1),
        rows: RowBlock::new(),
        cursor,
        scroll_delta: 0,
        modes: None,
        has_more: false,
    });
    let mut row = Vec::new();
    let mut links = LinkTable::new();
    let mut frame = Vec::new();
    // Warm-up: the buffers grow to their working size once.
    encode_row(sample::COLS, sample::sample_cells(), &mut row, &mut links).unwrap();
    fill(&mut damage);
    encode_frame(&damage, &mut frame).unwrap();

    let before = blocks();
    for _ in 0..PASSES {
        row.clear();
        links.clear();
        encode_row(sample::COLS, sample::sample_cells(), &mut row, &mut links).unwrap();
        fill(&mut damage);
        frame.clear();
        encode_frame(&damage, &mut frame).unwrap();
    }
    assert_eq!(blocks() - before, 0, "steady-state row encoding allocates");
}
