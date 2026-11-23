//! Criterion benches of `ferrum-vt` (`docs/03` §4, NFR-PERF-02): parse throughput over plain
//! ASCII and SGR-heavy output (single core), UTF-8 text, and the reflow of 10 000 lines.
//! Throughput is bytes per second; `cargo xtask bench` gates the medians.

#![allow(clippy::expect_used, reason = "bench setup: a failed expectation aborts the run")]
#![allow(
    clippy::indexing_slicing,
    reason = "bench input generation: indices are taken modulo the length"
)]

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{NullSink, Size, Terminal};
use fixture::{Fixture, Profile};

#[path = "../../../fixtures/output/output.rs"]
mod fixture;

const COLS: u16 = 200;
const ROWS: u16 = 55;
/// Bytes of every parse input: large enough to time, small enough to stay in cache-friendly sizes.
const INPUT_BYTES: usize = 8 * 1_024 * 1_024;
const HISTORY_LINES: usize = 10_000;
const SPILL_THRESHOLD_LINES: usize = 50_000;
const REFLOW_LINES: usize = 12_000;
/// Seed of the product fixture and logical lines generated from it (enough for 8 MiB).
const SEED: u64 = 1;
const FIXTURE_LINES: usize = 40_000;

fn terminal() -> Terminal<NullSink> {
    with_history(HISTORY_LINES)
}

fn with_history(lines: usize) -> Terminal<NullSink> {
    let config = ScrollbackConfig::new(lines, SPILL_THRESHOLD_LINES).expect("config");
    Terminal::new(Size::new(COLS, ROWS), Scrollback::new(config), NullSink)
}

/// Logical lines (a soft-wrapped line is several rows) of the seeded product fixture
/// (`fixtures/output/`, `docs/08` §4), ASCII only, until `INPUT_BYTES` of text.
fn fixture_lines() -> Vec<Vec<String>> {
    let (rows, _) = Fixture::rows(SEED, Profile::Mixed, usize::from(COLS), FIXTURE_LINES);
    let mut lines: Vec<Vec<String>> = Vec::new();
    let mut open: Vec<String> = Vec::new();
    for row in rows {
        let wrapped = row.flags.contains(ferrum_scrollback::LineFlags::WRAPPED);
        open.push(row.text);
        if !wrapped {
            let line = std::mem::take(&mut open);
            if line.iter().all(|row| row.is_ascii()) {
                lines.push(line);
            }
        }
    }
    lines
}

/// The fixture lines as plain text (`\r\n` after each logical line; a wrapped row continues in
/// the next one through the terminal's own auto-wrap), repeated up to `INPUT_BYTES`.
fn ascii() -> Vec<u8> {
    let lines = fixture_lines();
    let mut out = Vec::with_capacity(INPUT_BYTES + 1_024);
    for line in lines.iter().cycle() {
        if out.len() >= INPUT_BYTES {
            break;
        }
        for row in line {
            out.extend_from_slice(row.as_bytes());
        }
        out.extend_from_slice(b"\r\n");
    }
    out
}

/// The same fixture text with an SGR change before every word (colored build and test output).
fn sgr_heavy() -> Vec<u8> {
    let colors = [
        &b"\x1b[31m"[..],
        b"\x1b[1;32m",
        b"\x1b[38;5;208m",
        b"\x1b[0m",
        b"\x1b[38;2;10;120;250m",
        b"\x1b[4m",
    ];
    let lines = fixture_lines();
    let mut out = Vec::with_capacity(INPUT_BYTES + 1_024);
    let mut n = 0usize;
    for line in lines.iter().cycle() {
        if out.len() >= INPUT_BYTES {
            break;
        }
        for row in line {
            for word in row.split_inclusive(' ') {
                out.extend_from_slice(colors[n % colors.len()]);
                out.extend_from_slice(word.as_bytes());
                n += 1;
            }
        }
        out.extend_from_slice(b"\x1b[0m\r\n");
    }
    out
}

/// Non-ASCII text: accented Latin, CJK and emoji mixed with ASCII.
fn unicode() -> Vec<u8> {
    let line =
        "héllo wörld 漢字かな 😀 e\u{301} 👨\u{200d}👩 plain ascii tail of the line ".repeat(2);
    let mut out = Vec::with_capacity(INPUT_BYTES + 256);
    while out.len() < INPUT_BYTES {
        out.extend_from_slice(line.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out
}

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("vt_parse");
    group.throughput(Throughput::Bytes(INPUT_BYTES as u64));
    let cases = [
        ("ascii_8mib", ascii(), HISTORY_LINES),
        ("sgr_heavy_8mib", sgr_heavy(), HISTORY_LINES),
        ("unicode_8mib", unicode(), HISTORY_LINES),
        ("ascii_8mib_no_history", ascii(), 0),
        ("sgr_heavy_8mib_no_history", sgr_heavy(), 0),
    ];
    for (name, input, history) in cases {
        let mut term = with_history(history);
        term.parse(&input); // warm-up: grow every buffer to its working size
        group.bench_function(name, |b| {
            b.iter(|| {
                term.parse(black_box(&input));
            });
        });
    }
    group.finish();
}

fn bench_reflow(c: &mut Criterion) {
    let mut log = Vec::new();
    for n in 0..REFLOW_LINES {
        log.extend_from_slice(b"\x1b[3");
        log.extend_from_slice((n % 8).to_string().as_bytes());
        log.extend_from_slice(b"mline ");
        log.extend_from_slice(n.to_string().as_bytes());
        log.extend_from_slice(b" of a log with some words to make rows wrap around\x1b[0m ");
        log.resize(log.len() + n % 150, b'x');
        log.extend_from_slice(b"\r\n");
    }
    let mut group = c.benchmark_group("vt_reflow");
    group.bench_function("10k_lines_200_to_100_columns", |b| {
        b.iter_batched(
            || {
                let mut term = terminal();
                term.parse(&log);
                term
            },
            |mut term| {
                term.resize(Size::new(COLS / 2, ROWS)).expect("resize");
                term
            },
            criterion::BatchSize::LargeInput,
        );
    });
    group.finish();
}

criterion_group!(benches, bench_parse, bench_reflow);
criterion_main!(benches);
