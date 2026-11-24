//! Criterion benches of `ferrum-proto` (`docs/08` §2: protocol encode): one 200-column row. The
//! budget (`docs/03` §4) names no target for it, so it has no absolute ceiling; `cargo xtask
//! bench` gates it against the previous commit (+10 %).

#![allow(clippy::expect_used, reason = "bench setup: a failed expectation aborts the run")]

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ferrum_proto::{LinkTable, encode_row};

#[path = "../tests/support/sample.rs"]
mod sample;

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

criterion_group!(benches, row);
criterion_main!(benches);
