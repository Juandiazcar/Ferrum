//! NFR-PERF-03: full repaint of a 200×60 screen at most 2.5 ms CPU, on dense text (every cell a
//! glyph, maintainer decision Q-19) and on the typical mix, plus the incremental frame of four
//! changed rows (`docs/02` §6). The embedded font at the default size (13 px, `docs/14` Q-30), scale
//! 1.0, glyphs already cached: steady state. `cargo xtask bench` gates the full repaints at 5 ms,
//! 2× the target (`docs/08` §8).

#![allow(clippy::expect_used, reason = "bench setup: a failed expectation aborts the run")]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use ferrum_render::font::{Family, FontSpec, Fonts};
use ferrum_render::{Frame, Glyphs, Grid, Overlay, Painter, Rect, Row, Target};

#[path = "../tests/support/screens.rs"]
mod screens;

use screens::{BACKGROUND, COLS, Mix, ROWS};

/// The default font size of the settings (13 pt drawn as 13 logical px, `docs/14` Q-30).
const FONT_PX: f32 = 13.0;
/// The glyph cache default (`docs/02` §6).
const CACHE_BYTES: usize = 3 << 20;

struct Bench {
    glyphs: Glyphs,
    painter: Painter,
    pixels: Vec<u32>,
    grid: Grid,
}

impl Bench {
    fn new() -> Self {
        let fonts = Fonts::new(Family::Embedded, None).expect("embedded font");
        let spec = FontSpec {
            size_px: FONT_PX,
            scale_factor: 1.0,
            line_height: 1.0,
            letter_spacing_px: 0.0,
        };
        let metrics = fonts.cell_metrics(&spec).expect("metrics");
        let (width, height) = (u32::from(COLS) * metrics.width, u32::from(ROWS) * metrics.height);
        let grid = Grid {
            pane: Rect::new(0, 0, width, height),
            x: 0,
            y: 0,
            cols: COLS,
            rows: ROWS,
            metrics,
            background: BACKGROUND,
        };
        let pixels = vec![0; usize::try_from(width * height).expect("buffer")];
        Self { glyphs: Glyphs::new(fonts, CACHE_BYTES), painter: Painter::new(), pixels, grid }
    }

    fn paint(&mut self, rows: &[Row<'_>], buffer_age: u8) -> usize {
        let (width, height) = (self.grid.pane.width, self.grid.pane.height);
        let mut target = Target::new(&mut self.pixels, width, height).expect("target");
        let frame = Frame { grid: self.grid, rows, overlay: Overlay::default(), buffer_age };
        self.painter.paint(&mut self.glyphs, &mut target, &frame).len()
    }
}

fn full_repaint(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_repaint");
    for (name, mix) in [("dense_200x60", Mix::Dense), ("typical_200x60", Mix::Typical)] {
        let screen = screens::screen(mix, COLS, ROWS, 0x5EED);
        let rows = screens::rows(&screen);
        let mut bench = Bench::new();
        let _warm = bench.paint(&rows, 0);
        group.bench_function(name, |b| b.iter(|| black_box(bench.paint(black_box(&rows), 0))));
    }
    group.finish();
}

fn four_rows(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_repaint");
    let screen = screens::screen(Mix::Typical, COLS, ROWS, 0x5EED);
    let mut changed = screen.clone();
    for row in changed.iter_mut().skip(10).take(4) {
        row.rotate_left(1);
    }
    let (a, b) = (screens::rows(&screen), screens::rows(&changed));
    let mut bench = Bench::new();
    let _warm = (bench.paint(&a, 0), bench.paint(&b, 1));
    let mut flip = false;
    group.bench_function("damage_4_rows", |bencher| {
        bencher.iter(|| {
            flip = !flip;
            black_box(bench.paint(if flip { &a } else { &b }, 1))
        });
    });
    group.finish();
}

criterion_group!(benches, full_repaint, four_rows);
criterion_main!(benches);
