//! Erase and edit sequences: ED, EL, ECH, ICH, DCH and their selective forms (DECSED, DECSEL).

use crate::args::Args;
use crate::cell::Attrs;
use crate::core::Core;
use crate::events::EventSink;

/// Whether protected cells (DECSCA) survive an erase.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Protection {
    /// ED, EL: everything is erased.
    Ignore,
    /// DECSED, DECSEL: protected cells stay.
    Respect,
}

impl<S: EventSink> Core<S> {
    /// Erases `from..to` of row `y` honoring `protection`.
    fn erase_span(&mut self, y: usize, from: usize, to: usize, protection: Protection) {
        let blank = self.screen().cursor.pen.blank();
        let grid = &mut self.screen_mut().grid;
        match protection {
            Protection::Ignore => grid.erase_cells(y, from, to, blank),
            Protection::Respect => {
                let Some(row) = grid.row_mut(y) else { return };
                let to = to.min(row.len());
                let cells = row.cells_mut().get_mut(from..to).unwrap_or_default();
                for cell in cells.iter_mut().filter(|cell| !cell.attrs().contains(Attrs::PROTECTED))
                {
                    *cell = blank;
                }
            }
        }
    }

    fn erase_rows(&mut self, from: usize, to: usize, protection: Protection) {
        let cols = self.screen().grid.cols();
        for y in from..to {
            self.erase_span(y, 0, cols, protection);
            if protection == Protection::Ignore
                && let Some(row) = self.screen_mut().grid.row_mut(y)
            {
                row.set_flags(ferrum_scrollback::LineFlags::NONE);
            }
        }
    }

    /// ED and DECSED.
    pub(crate) fn erase_display(&mut self, mode: u16, protection: Protection) {
        let screen = self.screen();
        let (x, y, lines, cols) =
            (screen.cursor.x, screen.cursor.y, screen.grid.lines(), screen.grid.cols());
        match mode {
            0 => {
                self.erase_span(y, x, cols, protection);
                self.erase_rows(y + 1, lines, protection);
            }
            1 => {
                self.erase_rows(0, y, protection);
                self.erase_span(y, 0, x + 1, protection);
            }
            2 => self.erase_rows(0, lines, protection),
            3 => self.clear_scrollback(),
            _ => {}
        }
    }

    /// EL and DECSEL.
    pub(crate) fn erase_line(&mut self, mode: u16, protection: Protection) {
        let screen = self.screen();
        let (x, y, cols) = (screen.cursor.x, screen.cursor.y, screen.grid.cols());
        match mode {
            // Erasing to the right with a wrap pending changes nothing: the cursor is "after"
            // the last cell (the Alacritty reference test erase_in_line).
            0 if self.screen().cursor.wrap_pending => {}
            0 => self.erase_span(y, x, cols, protection),
            1 => self.erase_span(y, 0, x + 1, protection),
            2 => self.erase_span(y, 0, cols, protection),
            _ => {}
        }
    }

    /// ECH: blanks `n` cells from the cursor without moving anything.
    pub(crate) fn erase_chars(&mut self, args: Args<'_>) {
        let n = args.count(0, 1);
        let (x, y) = (self.screen().cursor.x, self.screen().cursor.y);
        self.erase_span(y, x, x.saturating_add(n), Protection::Ignore);
    }

    /// ICH.
    pub(crate) fn insert_chars(&mut self, n: usize) {
        let screen = self.screen_mut();
        let (x, y, blank) = (screen.cursor.x, screen.cursor.y, screen.cursor.pen.blank());
        screen.grid.insert_blank(y, x, n, blank);
    }

    /// DCH.
    pub(crate) fn delete_chars(&mut self, n: usize) {
        let screen = self.screen_mut();
        let (x, y, blank) = (screen.cursor.x, screen.cursor.y, screen.cursor.pen.blank());
        screen.grid.delete_cells(y, x, n, blank);
    }

    /// `CSI 3 J`: drops the saved lines.
    pub(crate) fn clear_scrollback(&mut self) {
        let mut cursor = ferrum_scrollback::LineCursor::new();
        while matches!(self.scrollback.pop_front(&mut cursor), Ok(Some(_))) {}
    }
}
