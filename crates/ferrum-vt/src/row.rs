//! One grid row: its cells and its line flags (`docs/02` §3.1).

use ferrum_scrollback::LineFlags;

use crate::cell::Cell;

/// A row of cells with the flags reflow and shell integration need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    cells: Vec<Cell>,
    flags: LineFlags,
}

impl Row {
    /// A row of `cols` copies of `blank`.
    pub fn new(cols: usize, blank: Cell) -> Self {
        Self { cells: vec![blank; cols], flags: LineFlags::NONE }
    }

    /// The cells.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// The cells, mutable.
    pub fn cells_mut(&mut self) -> &mut [Cell] {
        &mut self.cells
    }

    /// The cell at `col`.
    pub fn get(&self, col: usize) -> Option<&Cell> {
        self.cells.get(col)
    }

    /// The cell at `col`, mutable.
    pub fn get_mut(&mut self, col: usize) -> Option<&mut Cell> {
        self.cells.get_mut(col)
    }

    /// Width in cells.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// True for a row of no cells.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// The line flags.
    pub fn flags(&self) -> LineFlags {
        self.flags
    }

    /// Replaces the line flags.
    pub fn set_flags(&mut self, flags: LineFlags) {
        self.flags = flags;
    }

    /// Sets or clears one flag.
    pub fn set_flag(&mut self, flag: LineFlags, on: bool) {
        let bits =
            if on { self.flags.bits() | flag.bits() } else { self.flags.bits() & !flag.bits() };
        // INVARIANT: both operands only hold defined bits, so the result does too.
        self.flags = LineFlags::from_bits(bits).unwrap_or(self.flags);
    }

    /// True when the row continues on the next one (soft wrap).
    pub fn is_wrapped(&self) -> bool {
        self.flags.contains(LineFlags::WRAPPED)
    }

    /// Fills `from..to` (clamped to the row) with `cell`.
    pub fn fill(&mut self, from: usize, to: usize, cell: Cell) {
        let to = to.min(self.cells.len());
        if let Some(span) = self.cells.get_mut(from..to) {
            span.fill(cell);
        }
    }

    /// Blanks the whole row and clears its flags.
    pub fn clear(&mut self, blank: Cell) {
        self.cells.fill(blank);
        self.flags = LineFlags::NONE;
    }

    /// Changes the width, padding with `blank` or cutting the end.
    pub fn resize(&mut self, cols: usize, blank: Cell) {
        self.cells.resize(cols, blank);
    }

    /// Number of cells up to and including the last one that is not a default blank.
    pub fn used_len(&self) -> usize {
        self.cells.iter().rposition(|cell| !cell.is_default_blank()).map_or(0, |last| last + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_clamps_and_clear_resets_flags() {
        let mut row = Row::new(4, Cell::default());
        let mut x = Cell::default();
        x.set_scalar('x');
        row.fill(2, 99, x);
        assert_eq!(row.used_len(), 4);
        row.set_flag(LineFlags::WRAPPED, true);
        assert!(row.is_wrapped());
        row.clear(Cell::default());
        assert_eq!((row.used_len(), row.is_wrapped()), (0, false));
    }

    #[test]
    fn flags_are_set_and_cleared_independently() {
        let mut row = Row::new(1, Cell::default());
        row.set_flag(LineFlags::WRAPPED, true);
        row.set_flag(LineFlags::PROMPT_MARK, true);
        row.set_flag(LineFlags::WRAPPED, false);
        assert_eq!(row.flags(), LineFlags::PROMPT_MARK);
    }
}
