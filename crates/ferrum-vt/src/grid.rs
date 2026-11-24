//! The visible grid: a ring of rows (`docs/02` §3.1).
//!
//! Scrolling the whole screen only advances the ring start, so the common case (a program
//! printing lines at the bottom) moves no cells. Scrolling a partial region swaps row headers.
//! Cell-level edits live in `grid/edit.rs`.

mod clusters;
mod edit;

use crate::cell::{Cell, CellFlags};
use crate::damage::Damage;
use crate::graphemes::GraphemeTable;
use crate::limits::MAX_UTF8_BYTES;
use crate::row::Row;
use crate::size::Size;

/// A rectangular screen of rows with its grapheme table and damage tracker.
#[derive(Debug)]
pub struct Grid {
    rows: Vec<Row>,
    /// Physical index of logical row 0.
    top: usize,
    cols: usize,
    graphemes: GraphemeTable,
    /// Refusals left before the grapheme table may be compacted again (`grid/clusters.rs`).
    compact_cooldown: usize,
    /// Cells scanned by every compaction so far, for the test of the bound.
    compact_work: u64,
    damage: Damage,
}

/// `index` reduced into `0..len`, for an `index` below `2 * len` (a ring position plus an
/// offset below the ring length).
const fn wrap(index: usize, len: usize) -> usize {
    if index >= len { index - len } else { index }
}

impl Grid {
    /// A blank grid of `size`.
    pub fn new(size: Size) -> Self {
        let rows = (0..size.rows()).map(|_| Row::new(size.cols(), Cell::default())).collect();
        Self {
            rows,
            top: 0,
            cols: size.cols(),
            graphemes: GraphemeTable::new(),
            compact_cooldown: 0,
            compact_work: 0,
            damage: Damage::new(size.rows()),
        }
    }

    /// Columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Rows.
    pub fn lines(&self) -> usize {
        self.rows.len()
    }

    /// The size.
    pub fn size(&self) -> Size {
        // INVARIANT: the dimensions were clamped when the grid was built or resized.
        Size::new(
            u16::try_from(self.cols).unwrap_or(u16::MAX),
            u16::try_from(self.rows.len()).unwrap_or(u16::MAX),
        )
    }

    fn physical(&self, y: usize) -> Option<usize> {
        let len = self.rows.len();
        // perf: `top` and `y` are below `len`, so one subtraction wraps the ring; an integer
        // division per row access was a tenth of the scroll cost on x86 (bench
        // vt_parse/ascii_8mib).
        (y < len).then(|| wrap(self.top + y, len))
    }

    /// Row `y` (0 = top of the screen).
    pub fn row(&self, y: usize) -> Option<&Row> {
        self.rows.get(self.physical(y)?)
    }

    /// Row `y`, mutable. The row is marked dirty.
    pub fn row_mut(&mut self, y: usize) -> Option<&mut Row> {
        let physical = self.physical(y)?;
        self.damage.mark(y);
        self.rows.get_mut(physical)
    }

    /// Cluster text of a grapheme cell.
    pub fn graphemes(&self) -> &GraphemeTable {
        &self.graphemes
    }

    /// The text of a cell: its scalar, or its whole cluster.
    pub fn cell_text<'a>(&'a self, cell: &Cell, scalar: &'a mut [u8; MAX_UTF8_BYTES]) -> &'a str {
        if cell.flags().contains(CellFlags::GRAPHEME) {
            return self.graphemes.get(cell.raw_char()).unwrap_or(" ");
        }
        cell.scalar().encode_utf8(scalar)
    }

    /// The damage tracker.
    pub fn damage(&self) -> &Damage {
        &self.damage
    }

    /// The damage tracker, mutable.
    pub fn damage_mut(&mut self) -> &mut Damage {
        &mut self.damage
    }

    /// Puts logical row 0 at physical index 0 again.
    fn normalize(&mut self) {
        // INVARIANT: `top` is always below the row count (every update takes it modulo).
        self.rows.rotate_left(self.top);
        self.top = 0;
    }

    /// Removes the first `n` rows (they have been saved elsewhere); the grid has `n` rows fewer
    /// until [`set_lines`](Self::set_lines) restores the count.
    pub(crate) fn drop_top_rows(&mut self, n: usize) {
        self.normalize();
        self.rows.drain(..n.min(self.rows.len()));
    }

    /// Cuts or pads the bottom so that the grid has `lines` rows.
    pub(crate) fn set_lines(&mut self, lines: usize) {
        self.normalize();
        self.rows.truncate(lines);
        while self.rows.len() < lines {
            self.rows.push(Row::new(self.cols, Cell::default()));
        }
        self.damage.resize(lines);
    }

    /// Copies the part of `other` that fits (used for the alternate screen, which does not
    /// reflow). A double-width character cut by the new right edge is blanked.
    pub(crate) fn copy_clipped_from(&mut self, other: &Self) {
        for y in 0..self.rows.len().min(other.rows.len()) {
            let (Some(from), Some(to)) = (other.row(y), self.row_mut(y)) else { continue };
            let width = from.len().min(to.len());
            if let (Some(src), Some(dst)) =
                (from.cells().get(..width), to.cells_mut().get_mut(..width))
            {
                dst.copy_from_slice(src);
            }
            if width < from.len()
                && let Some(last) = to.cells_mut().get_mut(width.saturating_sub(1))
                && last.flags().contains(CellFlags::WIDE)
            {
                *last = Cell::default();
            }
        }
        self.graphemes = other.graphemes.clone();
    }

    fn swap_rows(&mut self, a: usize, b: usize) {
        if let (Some(a), Some(b)) = (self.physical(a), self.physical(b)) {
            self.rows.swap(a, b);
        }
    }

    fn clear_row(&mut self, y: usize, blank: Cell) {
        if let Some(row) = self.physical(y).and_then(|p| self.rows.get_mut(p)) {
            row.clear(blank);
        }
    }

    /// Scrolls rows `top..=bottom` up by `n`: the first `n` rows disappear and `n` blank rows
    /// appear at the bottom. The caller has already saved the rows that must go to the scrollback.
    pub fn scroll_up(&mut self, top: usize, bottom: usize, n: usize, blank: Cell) {
        let Some(count) = self.region_count(top, bottom, n) else { return };
        if top == 0 && bottom + 1 == self.rows.len() {
            for _ in 0..count {
                self.clear_row(0, blank);
                self.top = wrap(self.top + 1, self.rows.len());
            }
        } else {
            for y in top..bottom + 1 - count {
                self.swap_rows(y, y + count);
            }
            for y in bottom + 1 - count..=bottom {
                self.clear_row(y, blank);
            }
        }
        self.damage.mark_range(top, bottom + 1);
        self.damage.add_scroll(count);
    }

    /// Scrolls rows `top..=bottom` down by `n`: `n` blank rows appear at the top.
    pub fn scroll_down(&mut self, top: usize, bottom: usize, n: usize, blank: Cell) {
        let Some(count) = self.region_count(top, bottom, n) else { return };
        if top == 0 && bottom + 1 == self.rows.len() {
            for _ in 0..count {
                self.top = (self.top + self.rows.len() - 1) % self.rows.len();
                self.clear_row(0, blank);
            }
        } else {
            for y in (top + count..=bottom).rev() {
                self.swap_rows(y, y - count);
            }
            for y in top..top + count {
                self.clear_row(y, blank);
            }
        }
        self.damage.mark_range(top, bottom + 1);
        self.damage.add_scroll(count);
    }

    /// `n` clamped to the region height; `None` for an empty request or an invalid region.
    fn region_count(&self, top: usize, bottom: usize, n: usize) -> Option<usize> {
        if top > bottom || bottom >= self.rows.len() || n == 0 {
            return None;
        }
        Some(n.min(bottom - top + 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_with_labels(rows: usize) -> Grid {
        let mut grid = Grid::new(Size::new(3, u16::try_from(rows).unwrap_or(1)));
        for y in 0..rows {
            let label = char::from_digit(u32::try_from(y).unwrap_or(0), 10).unwrap_or('?');
            if let Some(cell) = grid.row_mut(y).and_then(|row| row.get_mut(0)) {
                cell.set_scalar(label);
            }
        }
        grid
    }

    fn labels(grid: &Grid) -> String {
        (0..grid.lines())
            .map(|y| grid.row(y).and_then(|row| row.get(0)).map_or('?', Cell::scalar))
            .collect()
    }

    #[test]
    fn full_screen_scroll_moves_only_the_ring_start() {
        let mut grid = grid_with_labels(4);
        grid.scroll_up(0, 3, 1, Cell::default());
        assert_eq!(labels(&grid), "123 ");
        grid.scroll_down(0, 3, 2, Cell::default());
        assert_eq!(labels(&grid), "  12");
    }

    #[test]
    fn partial_region_scrolls_leave_other_rows_alone() {
        let mut grid = grid_with_labels(5);
        grid.scroll_up(1, 3, 1, Cell::default());
        assert_eq!(labels(&grid), "023 4");
        grid.scroll_down(1, 3, 1, Cell::default());
        assert_eq!(labels(&grid), "0 234");
    }

    #[test]
    fn a_partial_region_starting_at_the_top_scrolls_by_its_whole_height_without_underflow() {
        // Found by the parse fuzz target: rows 0..=2 of a taller screen, scrolled by 3 or more.
        let mut grid = grid_with_labels(5);
        grid.scroll_up(0, 2, 3, Cell::default());
        assert_eq!(labels(&grid), "   34");
        grid.scroll_up(0, 2, 99, Cell::default());
        grid.scroll_down(0, 2, 99, Cell::default());
        assert_eq!(labels(&grid), "   34");
    }

    #[test]
    fn scroll_counts_are_clamped_and_invalid_regions_ignored() {
        let mut grid = grid_with_labels(3);
        grid.scroll_up(0, 2, 99, Cell::default());
        assert_eq!(labels(&grid), "   ");
        grid.scroll_up(2, 1, 1, Cell::default());
        grid.scroll_up(0, 9, 1, Cell::default());
        grid.scroll_up(0, 2, 0, Cell::default());
    }

    #[test]
    fn scrolling_marks_the_region_dirty_and_counts_lines() {
        let mut grid = grid_with_labels(5);
        grid.damage_mut().take();
        grid.scroll_up(1, 3, 2, Cell::default());
        assert_eq!(grid.damage().dirty_rows().collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(grid.damage().scrolled_lines(), 2);
    }
}
