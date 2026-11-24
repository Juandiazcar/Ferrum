//! Criterion benches of `ferrum-proto` (`docs/08` §2: protocol encode): one 200-column row and a
//! full 200×60 `Damage` frame. The budget (`docs/03` §4) names no target for them, so they have
//! no absolute ceiling; `cargo xtask bench` gates them against the previous commit (+10 %).

#![allow(clippy::expect_used, reason = "bench setup: a failed expectation aborts the run")]

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ferrum_proto::{
    Cursor, CursorShape, DaemonMessage, Damage, LinkTable, RowBlock, SessionId, encode_frame,
    encode_row,
};

#[path = "../tests/support/sample.rs"]
mod sample;

/// Rows of a full-screen damage frame (`docs/03` §4: a 200×60 screen).
const ROWS: u16 = 60;

fn row(c: &mut Criterion) {
    let mut group = c.benchmark_group("proto_encode");
    group.throughput(Throughput::Elements(1));
    let mut out = Vec::new();
    let mut links = LinkTable::new();
    group.bench_function("row_200_cols", |b| {
        b.iter(|| {
            out.clear();
            links.clear();
            encode_row(sample::COLS, black_box(sample::sample_cells()), &mut out, &mut links)
                .expect("row");
            black_box(&out);
        });
    });
    group.finish();
}

fn damage(c: &mut Criterion) {
    let mut group = c.benchmark_group("proto_encode");
    group.throughput(Throughput::Elements(u64::from(ROWS)));
    let cursor =
        Cursor { col: 0, row: 0, shape: CursorShape::Block, is_blinking: false, is_visible: true };
    let mut message = DaemonMessage::Damage(Damage {
        session_id: SessionId(1),
        rows: RowBlock::new(),
        cursor,
        scroll_delta: 0,
        modes: None,
        has_more: false,
    });
    let mut frame = Vec::new();
    group.bench_function("damage_200x60", |b| {
        b.iter(|| {
            if let DaemonMessage::Damage(damage) = &mut message {
                damage.rows.clear();
                for y in 0..ROWS {
                    damage.rows.push_row(y, sample::COLS, sample::sample_cells()).expect("row");
                }
            }
            frame.clear();
            encode_frame(&message, &mut frame).expect("frame");
            black_box(&frame);
        });
    });
    group.finish();
}

criterion_group!(benches, row, damage);
criterion_main!(benches);
