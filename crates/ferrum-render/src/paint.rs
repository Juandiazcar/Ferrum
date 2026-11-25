//! The cell painter: paints a terminal grid into the target and reports what changed.
//!
//! Every row carries a content hash (cells, cluster text, the cursor and selection on it). A row
//! is repainted when its hash differs from the frame whose pixels the buffer holds, which is the
//! buffer's age (`softbuffer`'s `Buffer::age`, ADR 0025): age 0 (contents undefined, as macOS
//! always reports), an age older than the history, or a new layout repaints everything,
//! padding included, so the first frame touches every pixel. The dirty rows are returned as
//! rectangles for `present_with_damage` (`docs/02` §6). Painting allocates nothing once the
//! glyphs on screen are cached (`docs/03` §3 rule 8).

mod cell;
mod history;
mod overlay;
mod row;

use std::hash::{Hash, Hasher};

pub use cell::{Cell, CellText, CellWidth, Row, Rows, Strokes};
pub use overlay::{Cursor, CursorShape, Overlay, Selection, SelectionKind};
use rustc_hash::FxHasher;

use crate::color::Rgb;
use crate::font::CellMetrics;
use crate::glyph::Glyphs;
use crate::target::{Rect, Target};
use history::{History, MAX_BUFFER_AGE};

/// Where the grid of one pane goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Grid {
    /// The pane: the grid plus its padding. Nothing is drawn outside.
    pub pane: Rect,
    /// Left edge of the first column.
    pub x: u32,
    /// Top edge of the first row.
    pub y: u32,
    /// Columns.
    pub cols: u16,
    /// Rows.
    pub rows: u16,
    /// Cell geometry.
    pub metrics: CellMetrics,
    /// Color of the padding and of cells past the end of a row.
    pub background: Rgb,
}

impl Grid {
    /// The pixels of cell (`col`, `row`).
    pub(crate) fn cell_rect(&self, col: u16, row: u16) -> Rect {
        let (width, height) = (self.metrics.width, self.metrics.height);
        Rect::new(self.x + u32::from(col) * width, self.y + u32::from(row) * height, width, height)
    }

    /// The pixels of row `row`.
    pub(crate) fn row_rect(&self, row: u16) -> Rect {
        let cell = self.cell_rect(0, row);
        Rect { width: u32::from(self.cols) * self.metrics.width, ..cell }
    }

    /// The pixels of every cell.
    fn area(&self) -> Rect {
        Rect { height: u32::from(self.rows) * self.metrics.height, ..self.row_rect(0) }
    }
}

/// One frame of one pane.
#[derive(Debug)]
pub struct Frame<'a, R: Rows + ?Sized> {
    /// Geometry.
    pub grid: Grid,
    /// The rows of the viewport.
    pub rows: &'a R,
    /// Cursor and selection.
    pub overlay: Overlay,
    /// The target buffer's age: 0 when its contents are undefined, `n` when it holds the frame
    /// painted `n` frames ago.
    pub buffer_age: u8,
}

/// Paints panes; one per pane, called once for every frame presented.
#[derive(Debug, Default)]
pub struct Painter {
    history: History,
    layout: Option<(Grid, u32, u32)>,
    damage: Vec<Rect>,
    colors: Vec<(Rgb, Rgb)>,
}

impl Painter {
    /// A painter that has painted nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Paints `frame` into `target` and returns the rectangles that changed.
    pub fn paint<R: Rows + ?Sized>(
        &mut self,
        glyphs: &mut Glyphs,
        target: &mut Target<'_>,
        frame: &Frame<'_, R>,
    ) -> &[Rect] {
        let grid = frame.grid;
        let clip = grid.pane.intersect(&grid.area()).intersect(&target.bounds());
        self.damage.clear();
        let full = self.prepare(target, frame);
        if full {
            fill_padding(target, &grid);
            self.damage.push(grid.pane.intersect(&target.bounds()));
        }
        let age = (!full).then_some(frame.buffer_age);
        let mut cx = row::Context { glyphs, target, grid: &grid, clip, colors: &mut self.colors };
        for y in 0..grid.rows {
            let row = frame.rows.row(y);
            let hash = row_hash(row, y, &frame.overlay, grid.cols);
            if !self.history.record(age, usize::from(y), hash) {
                continue;
            }
            row::paint_row(&mut cx, y, row, &frame.overlay);
            if !full {
                push_damage(&mut self.damage, grid.row_rect(y).intersect(&clip));
            }
        }
        self.history.commit();
        &self.damage
    }

    /// Starts a frame: a new layout or an unusable buffer age means a full repaint.
    fn prepare<R: Rows + ?Sized>(&mut self, target: &Target<'_>, frame: &Frame<'_, R>) -> bool {
        let layout = (frame.grid, target.width(), target.height());
        let changed = self.layout != Some(layout);
        if changed {
            self.layout = Some(layout);
            self.history.reset(usize::from(frame.grid.rows));
        }
        changed || frame.buffer_age == 0 || frame.buffer_age > MAX_BUFFER_AGE
    }
}

/// The content hash of row `y`: what it shows and what is drawn over it.
fn row_hash(row: Option<Row<'_>>, y: u16, overlay: &Overlay, cols: u16) -> u64 {
    let mut hasher = FxHasher::default();
    if let Some(row) = row {
        let cells = row.cells.get(..usize::from(cols)).unwrap_or(row.cells);
        Hash::hash_slice(cells, &mut hasher);
        row.clusters.hash(&mut hasher);
    }
    overlay.cursor.filter(|cursor| cursor.row == y).hash(&mut hasher);
    overlay.selection.and_then(|selection| selection.columns(y, cols)).hash(&mut hasher);
    hasher.finish()
}

/// Fills the pane outside the grid with the background.
fn fill_padding(target: &mut Target<'_>, grid: &Grid) {
    let pane = grid.pane.intersect(&target.bounds());
    let area = grid.area().intersect(&pane);
    let above = Rect { height: area.y.saturating_sub(pane.y), ..pane };
    let below =
        Rect::new(pane.x, area.bottom(), pane.width, pane.bottom().saturating_sub(area.bottom()));
    let left = Rect::new(pane.x, area.y, area.x.saturating_sub(pane.x), area.height);
    let right =
        Rect::new(area.right(), area.y, pane.right().saturating_sub(area.right()), area.height);
    for rect in [above, below, left, right] {
        target.fill(rect, grid.background);
    }
}

/// Adds a dirty row, merged with the previous one when they touch.
fn push_damage(damage: &mut Vec<Rect>, rect: Rect) {
    if rect.is_empty() {
        return;
    }
    if let Some(last) = damage.last_mut()
        && last.x == rect.x
        && last.width == rect.width
        && last.bottom() == rect.y
    {
        last.height += rect.height;
        return;
    }
    damage.push(rect);
}

#[cfg(test)]
mod tests;
