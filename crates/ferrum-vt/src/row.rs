//! One grid row: its cells and its line flags (`docs/02` §3.1).

use ferrum_scrollback::LineFlags;

use crate::cell::Cell;

/// A row of cells with the flags reflow and shell integration need.
#[derive(Debug, Clone)]
pub struct Row {
    cells: Vec<Cell>,
    flags: LineFlags,
    /// INVARIANT: every cell from `used` on is a default blank, so blanking or scanning the row
    /// only has to look at the first `used` cells. Any access that may write a cell raises it.
    used: usize,
}

impl PartialEq for Row {
    fn eq(&self, other: &Self) -> bool {
        self.flags == other.flags && self.cells == other.cells
    }
}

impl Eq for Row {}

impl Row {
    /// A row of `cols` copies of `blank`.
    pub fn new(cols: usize, blank: Cell) -> Self {
        let used = if blank.is_default_blank() { 0 } else { cols };
        Self { cells: vec![blank; cols], flags: LineFlags::NONE, used }
    }

    /// The cells.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// The cells, mutable.
    pub fn cells_mut(&mut self) -> &mut [Cell] {
        self.used = self.cells.len();
        &mut self.cells
    }

    /// The cell at `col`.
    pub fn get(&self, col: usize) -> Option<&Cell> {
        self.cells.get(col)
    }

    /// The cell at `col`, mutable.
    pub fn get_mut(&mut self, col: usize) -> Option<&mut Cell> {
        self.used = self.used.max(col.saturating_add(1).min(self.cells.len()));
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

    /// Sets one flag.
    pub fn insert_flag(&mut self, flag: LineFlags) {
        self.flags = self.flags.union(flag);
    }

    /// Clears one flag.
    pub fn remove_flag(&mut self, flag: LineFlags) {
        let bits = self.flags.bits() & !flag.bits();
        // INVARIANT: clearing bits of a valid flag set leaves a valid flag set.
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
            if !cell.is_default_blank() {
                self.used = self.used.max(to);
            }
        }
    }

    /// Blanks the whole row and clears its flags.
    pub fn clear(&mut self, blank: Cell) {
        self.flags = LineFlags::NONE;
        if blank.is_default_blank() {
            // perf: only the cells that were written are blanked (bench vt_parse/ascii_8mib).
            if let Some(written) = self.cells.get_mut(..self.used) {
                written.fill(blank);
            }
            self.used = 0;
        } else {
            self.cells.fill(blank);
            self.used = self.cells.len();
        }
    }

    /// Writes printable ASCII `bytes` from column `x` in the style `words` (`fg`, `bg`, `attrs`).
    /// The caller has checked that the text fits in the row.
    pub(crate) fn write_ascii(&mut self, x: usize, bytes: &[u8], words: (u32, u32, u16)) {
        let end = x + bytes.len();
        let Some(span) = self.cells.get_mut(x..end) else { return };
        for (cell, &byte) in span.iter_mut().zip(bytes) {
            *cell = Cell::styled(char::from(byte), words.0, words.1, words.2);
        }
        self.used = self.used.max(end);
    }

    /// Changes the width, padding with `blank` or cutting the end.
    pub fn resize(&mut self, cols: usize, blank: Cell) {
        self.cells.resize(cols, blank);
        self.used = if blank.is_default_blank() { self.used.min(cols) } else { cols };
    }

    /// Number of cells up to and including the last one that is not a default blank.
    pub fn used_len(&self) -> usize {
        let written = self.cells.get(..self.used).unwrap_or_default();
        written.iter().rposition(|cell| !cell.is_default_blank()).map_or(0, |last| last + 1)
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
        row.insert_flag(LineFlags::WRAPPED);
        assert!(row.is_wrapped());
        row.clear(Cell::default());
        assert_eq!((row.used_len(), row.is_wrapped()), (0, false));
    }

    #[test]
    fn every_write_path_is_seen_by_clear_and_used_len() {
        let mut x = Cell::default();
        x.set_scalar('x');
        let blank = Row::new(8, Cell::default());
        let writes: [fn(&mut Row, Cell); 4] = [
            |row, _| row.write_ascii(5, b"ab", (0, 0, 0)),
            |row, x| row.get_mut(6).into_iter().for_each(|cell| *cell = x),
            |row, x| row.fill(2, 4, x),
            |row, x| row.cells_mut().iter_mut().last().into_iter().for_each(|cell| *cell = x),
        ];
        for write in writes {
            let mut row = Row::new(8, Cell::default());
            write(&mut row, x);
            assert!(row.used_len() > 0);
            row.clear(Cell::default());
            assert_eq!(row, blank);
            assert_eq!(row.used_len(), 0);
        }
        let mut row = Row::new(8, Cell::default());
        row.fill(0, 8, x);
        row.resize(3, Cell::default());
        row.resize(8, Cell::default());
        assert_eq!(row.used_len(), 3);
    }

    #[test]
    fn flags_are_set_and_cleared_independently() {
        let mut row = Row::new(1, Cell::default());
        row.insert_flag(LineFlags::WRAPPED);
        row.insert_flag(LineFlags::PROMPT_MARK);
        row.remove_flag(LineFlags::WRAPPED);
        assert_eq!(row.flags(), LineFlags::PROMPT_MARK);
    }
}
