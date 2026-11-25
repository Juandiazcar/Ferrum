//! Painting one row: backgrounds, text, decorations, then the cursor.
// perf: `render_repaint/*` (benches/repaint.rs) — backgrounds are filled on the row's first pixel
// line and copied down, glyphs are drawn after every background so they may overhang.

use super::Grid;
use super::cell::{Cell, CellText, CellWidth, Row, Strokes};
use super::overlay::{Cursor, CursorShape, Overlay};
use crate::color::Rgb;
use crate::glyph::{Glyphs, Placement};
use crate::synth::{Decoration, Underline};
use crate::target::{Rect, Target};

/// What painting a row needs.
pub(crate) struct Context<'a, 'b> {
    pub(crate) glyphs: &'a mut Glyphs,
    pub(crate) target: &'a mut Target<'b>,
    pub(crate) grid: &'a Grid,
    /// The pane, the grid and the target intersected: nothing is drawn outside.
    pub(crate) clip: Rect,
    /// Text and background color of every column after the selection and the block cursor.
    pub(crate) colors: &'a mut Vec<(Rgb, Rgb)>,
}

/// Paints row `y` (`row` is `None` past the end of the viewport).
pub(crate) fn paint_row(cx: &mut Context<'_, '_>, y: u16, row: Option<Row<'_>>, overlay: &Overlay) {
    let blank = Cell::blank(cx.grid.background, cx.grid.background);
    let cell =
        |col: u16| row.and_then(|row| row.cells.get(usize::from(col)).copied()).unwrap_or(blank);
    resolve_colors(cx, y, &cell, overlay);
    fill_backgrounds(cx, y);
    for col in 0..cx.grid.cols {
        draw_text(cx, y, col, (cell(col), row));
    }
    for col in 0..cx.grid.cols {
        draw_decorations(cx, y, col, &cell(col));
    }
    if let Some(cursor) =
        overlay.cursor.filter(|cursor| cursor.row == y && cursor.col < cx.grid.cols)
    {
        let wide = cell(cursor.col).width == CellWidth::Wide;
        draw_cursor(cx, &cursor, wide);
    }
}

fn resolve_colors(
    cx: &mut Context<'_, '_>,
    y: u16,
    cell: &impl Fn(u16) -> Cell,
    overlay: &Overlay,
) {
    cx.colors.clear();
    cx.colors.extend((0..cx.grid.cols).map(|col| {
        let cell = cell(col);
        (cell.fg, cell.bg)
    }));
    if let Some(selection) = overlay.selection
        && let Some((from, to)) = selection.columns(y, cx.grid.cols)
    {
        for colors in cx.colors.iter_mut().take(usize::from(to)).skip(usize::from(from)) {
            *colors = (selection.fg.unwrap_or(colors.0), selection.bg);
        }
    }
    if let Some(cursor) =
        overlay.cursor.filter(|cursor| cursor.row == y && cursor.shape == CursorShape::Block)
    {
        let span = if cell(cursor.col).width == CellWidth::Wide { 2 } else { 1 };
        let start = usize::from(cursor.col);
        for colors in cx.colors.iter_mut().skip(start).take(span) {
            *colors = (cursor.text_color, cursor.color);
        }
    }
}

/// Fills the backgrounds run by run on the row's first pixel line, then copies that line down.
fn fill_backgrounds(cx: &mut Context<'_, '_>, y: u16) {
    let rect = cx.grid.row_rect(y).intersect(&cx.clip);
    if rect.is_empty() {
        return;
    }
    let width = cx.grid.metrics.width;
    let mut col: u16 = 0;
    while let Some(&(_, bg)) = cx.colors.get(usize::from(col)) {
        let run =
            cx.colors.iter().skip(usize::from(col)).take_while(|(_, other)| *other == bg).count();
        let run = u16::try_from(run).unwrap_or(u16::MAX).max(1);
        let x = cx.grid.x + u32::from(col) * width;
        let line = Rect::new(x, rect.y, u32::from(run) * width, 1).intersect(&rect);
        cx.target.fill(line, bg);
        col = col.saturating_add(run);
    }
    cx.target.copy_line(rect.y, rect.y + 1..rect.bottom(), (rect.x, rect.width));
}

fn placement(cx: &Context<'_, '_>, y: u16, col: u16, color: Rgb) -> Placement {
    let cell = cx.grid.cell_rect(col, y);
    Placement {
        x: cell.x,
        y: cell.y,
        metrics: cx.grid.metrics,
        color,
        clip: cx.clip,
        shift: (0, 0),
    }
}

fn draw_text(cx: &mut Context<'_, '_>, y: u16, col: u16, (cell, row): (Cell, Option<Row<'_>>)) {
    if cell.width == CellWidth::Spacer {
        return;
    }
    let fg = cx.colors.get(usize::from(col)).map_or(cell.fg, |&(fg, _)| fg);
    let place = placement(cx, y, col, fg);
    match cell.text {
        CellText::Empty | CellText::Char(' ') => {}
        CellText::Char(ch) => cx.glyphs.draw_char(cx.target, ch, cell.style, &place),
        CellText::Cluster { start, len } => {
            if let Some(text) = row.and_then(|row| row.cluster(start, len)) {
                cx.glyphs.draw_cluster(cx.target, text, cell.style, &place);
            }
        }
    }
}

fn draw_decorations(cx: &mut Context<'_, '_>, y: u16, col: u16, cell: &Cell) {
    let fg = cx.colors.get(usize::from(col)).map_or(cell.fg, |&(fg, _)| fg);
    if cell.underline != Underline::None {
        let place = placement(cx, y, col, cell.underline_color);
        cx.glyphs.draw_decoration(cx.target, Decoration::Underline(cell.underline), &place);
    }
    for (stroke, decoration) in
        [(Strokes::STRIKEOUT, Decoration::Strikeout), (Strokes::OVERLINE, Decoration::Overline)]
    {
        if cell.strokes.contains(stroke) {
            let place = placement(cx, y, col, fg);
            cx.glyphs.draw_decoration(cx.target, decoration, &place);
        }
    }
}

/// The underline, bar and hollow shapes, drawn over the text (the block is a color swap).
fn draw_cursor(cx: &mut Context<'_, '_>, cursor: &Cursor, wide: bool) {
    let cell = cx.grid.cell_rect(cursor.col, cursor.row);
    let width = if wide { cell.width * 2 } else { cell.width };
    let stroke = cx.grid.metrics.stroke.min(cell.height).min(width);
    let rects = match cursor.shape {
        CursorShape::Block => return,
        CursorShape::Underline => [Rect::new(cell.x, cell.bottom() - stroke, width, stroke); 4],
        CursorShape::Bar => [Rect::new(cell.x, cell.y, stroke, cell.height); 4],
        CursorShape::Hollow => [
            Rect::new(cell.x, cell.y, width, stroke),
            Rect::new(cell.x, cell.bottom() - stroke, width, stroke),
            Rect::new(cell.x, cell.y, stroke, cell.height),
            Rect::new(cell.x + width - stroke, cell.y, stroke, cell.height),
        ],
    };
    for rect in rects {
        cx.target.fill(rect.intersect(&cx.clip), cursor.color);
    }
}
