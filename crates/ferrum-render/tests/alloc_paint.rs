//! Painting is a hot path (`docs/05` §6, `docs/08` §2): once the glyphs on screen are
//! cached, painting frames (full and incremental, cursor and selection moving, clusters, box
//! drawing, decorations) allocates nothing. One test per binary: `dhat` profiles the process.

#![allow(clippy::unwrap_used, reason = "test code: a failed expectation is a failed test")]

use ferrum_render::font::{CellMetrics, Family, FontSpec, Fonts};
use ferrum_render::{
    Cell, CellText, CellWidth, Cursor, CursorShape, Frame, Glyphs, Grid, Overlay, Painter, Rect,
    Row, Selection, SelectionKind, Strokes, Target, Underline,
};

#[path = "support/screens.rs"]
mod screens;

#[cfg(not(miri))] // dhat is not supported under Miri
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// Frames measured: enough to move the cursor through all four shapes and the selection
/// through both kinds many times, over both screens and every buffer age the history keeps.
const PASSES: u16 = 100;
/// Frames painted before measuring, so every glyph is cached and every buffer has grown to its
/// working size: the overlays repeat with a period of 8 (four cursor shapes × two kinds of
/// selection).
const WARM_UP: u16 = 8;
const SHAPES: [CursorShape; 4] =
    [CursorShape::Block, CursorShape::Bar, CursorShape::Underline, CursorShape::Hollow];

fn blocks() -> u64 {
    dhat::HeapStats::get().total_blocks
}

/// A screen of every kind of cell: dense text, then a row of the rarer ones.
fn screen(seed: u64) -> Vec<Vec<Cell>> {
    let mut screen = screens::screen(screens::Mix::Dense, screens::COLS, screens::ROWS, seed);
    let special = [
        Cell {
            text: CellText::Cluster { start: 0, len: 3 },
            ..Cell::blank(screens::FOREGROUND, screens::BACKGROUND)
        },
        Cell {
            text: CellText::Char('╋'),
            underline: Underline::Curly,
            ..Cell::blank(screens::FOREGROUND, screens::BACKGROUND)
        },
        Cell {
            text: CellText::Char('▒'),
            strokes: Strokes::STRIKEOUT.with(Strokes::OVERLINE),
            ..Cell::blank(screens::FOREGROUND, screens::BACKGROUND)
        },
        Cell {
            text: CellText::Char('\u{E0B0}'),
            underline: Underline::Double,
            ..Cell::blank(screens::FOREGROUND, screens::BACKGROUND)
        },
        Cell {
            text: CellText::Char('W'),
            width: CellWidth::Wide,
            underline: Underline::Dotted,
            ..Cell::blank(screens::FOREGROUND, screens::BACKGROUND)
        },
        Cell {
            text: CellText::Empty,
            width: CellWidth::Spacer,
            underline: Underline::Dashed,
            ..Cell::blank(screens::FOREGROUND, screens::BACKGROUND)
        },
    ];
    if let Some(row) = screen.get_mut(0) {
        row.splice(0..special.len(), special);
    }
    screen
}

fn rows(screen: &[Vec<Cell>]) -> Vec<Row<'_>> {
    screen.iter().map(|cells| Row { cells, clusters: "e\u{301}" }).collect()
}

fn overlay(pass: u16) -> Overlay {
    let cursor = Cursor {
        col: pass % screens::COLS,
        row: pass % screens::ROWS,
        shape: SHAPES.get(usize::from(pass % 4)).copied().unwrap_or(CursorShape::Block),
        color: screens::FOREGROUND,
        text_color: screens::BACKGROUND,
    };
    let selection = Selection {
        start: (pass % 7, 2),
        end: (pass % 11, 5 + pass % 3),
        kind: if pass.is_multiple_of(2) { SelectionKind::Linear } else { SelectionKind::Block },
        bg: screens::FOREGROUND,
        fg: Some(screens::BACKGROUND),
    };
    Overlay { cursor: Some(cursor), selection: Some(selection) }
}

fn grid(metrics: CellMetrics) -> Grid {
    let (width, height) =
        (u32::from(screens::COLS) * metrics.width, u32::from(screens::ROWS) * metrics.height);
    Grid {
        pane: Rect::new(0, 0, width, height),
        x: 0,
        y: 0,
        cols: screens::COLS,
        rows: screens::ROWS,
        metrics,
        background: screens::BACKGROUND,
    }
}

fn painting_allocates_nothing_in_steady_state() {
    let fonts = Fonts::new(Family::Embedded, None).unwrap();
    let spec =
        FontSpec { size_px: 13.0, scale_factor: 1.0, line_height: 1.0, letter_spacing_px: 0.0 };
    let grid = grid(fonts.cell_metrics(&spec).unwrap());
    let (width, height) = (grid.pane.width, grid.pane.height);
    let (a, b) = (screen(1), screen(2));
    let (rows_a, rows_b) = (rows(&a), rows(&b));
    let mut pixels = vec![0; usize::try_from(width * height).unwrap()];
    let mut glyphs = Glyphs::new(fonts, 3 << 20);
    let mut painter = Painter::new();
    let mut paint = |rows: &[Row<'_>], overlay: Overlay, buffer_age: u8| {
        let mut target = Target::new(&mut pixels, width, height).unwrap();
        let frame = Frame { grid, rows, overlay, buffer_age };
        painter.paint(&mut glyphs, &mut target, &frame).len()
    };
    // Warm-up: every glyph is rasterized and every buffer grows to its working size once.
    for pass in 0..WARM_UP {
        let _ = (paint(&rows_a, overlay(pass), 0), paint(&rows_b, overlay(pass), 1));
    }

    let _profiler = dhat::Profiler::builder().testing().build();
    let before = blocks();
    let mut damaged = 0;
    for pass in 0..PASSES {
        let rows = if pass.is_multiple_of(2) { &rows_a } else { &rows_b };
        damaged += paint(rows, overlay(pass), u8::try_from(pass % 3).unwrap());
    }
    assert!(damaged > 0, "frames were painted");
    assert_eq!(blocks() - before, 0, "steady-state painting allocates");
}

fn main() {
    ferrum_testkit::single_test::single_test(
        "painting_allocates_nothing_in_steady_state",
        painting_allocates_nothing_in_steady_state,
    );
}
