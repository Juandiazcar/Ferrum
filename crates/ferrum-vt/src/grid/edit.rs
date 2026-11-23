//! Cell-level edits of one row: erase, insert and delete characters. They never leave half of a
//! double-width character behind (FR-TERM-003).

use super::Grid;
use crate::cell::{Cell, CellFlags};
use crate::row::Row;

fn is_spacer(cell: Option<&Cell>) -> bool {
    cell.is_some_and(|cell| cell.flags().contains(CellFlags::SPACER))
}

/// Blanks the partner of a double-width character when `x` falls on one of its halves.
fn break_wide_at(row: &mut Row, x: usize, blank: Cell) {
    match row.get(x).map(Cell::flags) {
        Some(flags) if flags.contains(CellFlags::SPACER) => {
            if let Some(head) = x.checked_sub(1).and_then(|h| row.get_mut(h)) {
                *head = blank;
            }
        }
        Some(flags) if flags.contains(CellFlags::WIDE) => {
            if let Some(tail) = row.get_mut(x + 1) {
                *tail = blank;
            }
        }
        _ => {}
    }
}

impl Grid {
    /// Prepares `from..to` of row `y` to be overwritten: blanks the partner of a double-width
    /// character that the range cuts in two.
    pub(crate) fn clear_wide_edges(&mut self, y: usize, from: usize, to: usize, blank: Cell) {
        if from >= to {
            return;
        }
        let Some(row) = self.row_mut(y) else { return };
        break_wide_at(row, from, blank);
        break_wide_at(row, to - 1, blank);
    }

    /// Erases cells `from..to` of row `y` with `blank` (clamped to the row).
    pub fn erase_cells(&mut self, y: usize, from: usize, to: usize, blank: Cell) {
        let Some(row) = self.row_mut(y) else { return };
        let to = to.min(row.len());
        if from >= to {
            return;
        }
        break_wide_at(row, from, blank);
        if to < row.len() && is_spacer(row.get(to)) {
            break_wide_at(row, to, blank);
        }
        row.fill(from, to, blank);
        if to >= row.len() {
            // The end of the row is gone, so the row no longer continues on the next one.
            row.remove_flag(ferrum_scrollback::LineFlags::WRAPPED);
        }
    }

    /// Inserts `n` blanks at `x`, pushing the rest of the row right; cells that fall off the
    /// end are lost.
    pub fn insert_blank(&mut self, y: usize, x: usize, n: usize, blank: Cell) {
        let Some(row) = self.row_mut(y) else { return };
        let cols = row.len();
        if x >= cols || n == 0 {
            return;
        }
        let n = n.min(cols - x);
        break_wide_at(row, x, blank);
        let cells = row.cells_mut();
        // INVARIANT: x < cols and n <= cols - x, so both ranges are inside the row.
        cells.copy_within(x..cols - n, x + n);
        if let Some(last) = cells.last_mut().filter(|cell| cell.flags().contains(CellFlags::WIDE)) {
            *last = blank;
        }
        row.fill(x, x + n, blank);
    }

    /// Deletes `n` cells at `x`, pulling the rest of the row left and blanking the end.
    pub fn delete_cells(&mut self, y: usize, x: usize, n: usize, blank: Cell) {
        let Some(row) = self.row_mut(y) else { return };
        let cols = row.len();
        if x >= cols || n == 0 {
            return;
        }
        let n = n.min(cols - x);
        break_wide_at(row, x, blank);
        if x + n < cols && is_spacer(row.get(x + n)) {
            break_wide_at(row, x + n, blank);
        }
        // INVARIANT: x < cols and n <= cols - x, so both ranges are inside the row.
        row.cells_mut().copy_within(x + n..cols, x);
        row.fill(cols - n, cols, blank);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::size::Size;

    fn grid_of(text: &str) -> Grid {
        let mut grid = Grid::new(Size::new(u16::try_from(text.chars().count()).unwrap_or(1), 1));
        for (x, c) in text.chars().enumerate() {
            if let Some(cell) = grid.row_mut(0).and_then(|row| row.get_mut(x)) {
                cell.set_scalar(c);
            }
        }
        grid
    }

    fn text(grid: &Grid) -> String {
        grid.row(0).map_or_else(String::new, |row| row.cells().iter().map(Cell::scalar).collect())
    }

    fn make_wide(grid: &mut Grid, x: usize) {
        if let Some(row) = grid.row_mut(0) {
            if let Some(head) = row.get_mut(x) {
                head.insert_flags(CellFlags::WIDE);
            }
            if let Some(tail) = row.get_mut(x + 1) {
                tail.insert_flags(CellFlags::SPACER);
            }
        }
    }

    #[test]
    fn erase_insert_and_delete_shift_and_clamp() {
        let mut grid = grid_of("abcdef");
        grid.erase_cells(0, 1, 3, Cell::default());
        assert_eq!(text(&grid), "a  def");
        grid.insert_blank(0, 1, 2, Cell::default());
        assert_eq!(text(&grid), "a    d");
        grid.delete_cells(0, 0, 99, Cell::default());
        assert_eq!(text(&grid), "      ");
        grid.erase_cells(0, 5, 2, Cell::default());
        grid.insert_blank(0, 9, 1, Cell::default());
        grid.delete_cells(0, 9, 1, Cell::default());
    }

    #[test]
    fn cutting_a_wide_character_blanks_its_other_half() {
        let mut grid = grid_of("a漢b");
        make_wide(&mut grid, 1);
        grid.erase_cells(0, 2, 3, Cell::default());
        let row = grid.row(0).map(|row| row.cells().to_vec()).unwrap_or_default();
        assert!(!row.get(1).is_some_and(|cell| cell.flags().contains(CellFlags::WIDE)));

        let mut grid = grid_of("a漢b");
        make_wide(&mut grid, 1);
        grid.delete_cells(0, 2, 1, Cell::default());
        assert!(grid.row(0).is_some_and(|row| {
            row.cells()
                .iter()
                .all(|cell| !cell.flags().intersects(CellFlags::WIDE | CellFlags::SPACER))
        }));
    }

    #[test]
    fn a_wide_character_pushed_off_the_edge_is_removed() {
        let mut grid = grid_of("ab漢c");
        make_wide(&mut grid, 2);
        grid.insert_blank(0, 0, 1, Cell::default());
        assert!(grid.row(0).is_some_and(|row| {
            row.cells().iter().all(|cell| !cell.flags().contains(CellFlags::WIDE))
        }));
    }
}
