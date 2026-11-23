//! Erase and edit sequences: ED, EL, ECH, ICH, DCH. The selective forms (DECSCA, DECSED, DECSEL)
//! are not implemented, like in Alacritty (maintainer decision Q-25, reference test
//! `selective_erasure`).

use crate::args::Args;
use crate::core::Core;
use crate::events::EventSink;

/// ED and EL parameters: from the cursor to the end, from the start to the cursor, everything.
const ERASE_TO_END: u16 = 0;
const ERASE_TO_CURSOR: u16 = 1;
/// ED 2 / EL 2: the whole screen or line.
pub(crate) const ED_ALL: u16 = 2;

/// ED parameter that drops the saved lines (`CSI 3 J`).
const ED_SCROLLBACK: u16 = 3;

impl<S: EventSink> Core<S> {
    /// Erases `from..to` of row `y`.
    fn erase_span(&mut self, y: usize, from: usize, to: usize) {
        let blank = self.screen().cursor.pen.blank();
        self.screen_mut().grid.erase_cells(y, from, to, blank);
    }

    fn erase_rows(&mut self, from: usize, to: usize) {
        let cols = self.screen().grid.cols();
        for y in from..to {
            self.erase_span(y, 0, cols);
            if let Some(row) = self.screen_mut().grid.row_mut(y) {
                row.set_flags(ferrum_scrollback::LineFlags::NONE);
            }
        }
    }

    /// ED.
    pub(crate) fn erase_display(&mut self, mode: u16) {
        let screen = self.screen();
        let (x, y, lines, cols) =
            (screen.cursor.x, screen.cursor.y, screen.grid.lines(), screen.grid.cols());
        match mode {
            ERASE_TO_END => {
                self.erase_span(y, x, cols);
                self.erase_rows(y + 1, lines);
            }
            ERASE_TO_CURSOR => {
                self.erase_rows(0, y);
                self.erase_span(y, 0, x + 1);
            }
            ED_ALL => self.erase_rows(0, lines),
            ED_SCROLLBACK => self.clear_scrollback(),
            _ => {}
        }
        self.screen_mut().cursor.wrap_pending = false;
    }

    /// EL.
    pub(crate) fn erase_line(&mut self, mode: u16) {
        let screen = self.screen();
        let (x, y, cols) = (screen.cursor.x, screen.cursor.y, screen.grid.cols());
        match mode {
            ERASE_TO_END => self.erase_span(y, x, cols),
            ERASE_TO_CURSOR => self.erase_span(y, 0, x + 1),
            ED_ALL => self.erase_span(y, 0, cols),
            _ => {}
        }
        self.screen_mut().cursor.wrap_pending = false;
    }

    /// ECH: blanks `n` cells from the cursor without moving anything.
    pub(crate) fn erase_chars(&mut self, args: Args<'_>) {
        let n = args.count(0, 1);
        let (x, y) = (self.screen().cursor.x, self.screen().cursor.y);
        self.erase_span(y, x, x.saturating_add(n));
        self.screen_mut().cursor.wrap_pending = false;
    }

    /// ICH.
    pub(crate) fn insert_chars(&mut self, n: usize) {
        let screen = self.screen_mut();
        let (x, y, blank) = (screen.cursor.x, screen.cursor.y, screen.cursor.pen.blank());
        screen.grid.insert_blank(y, x, n, blank);
        screen.cursor.wrap_pending = false;
    }

    /// DCH.
    pub(crate) fn delete_chars(&mut self, n: usize) {
        let screen = self.screen_mut();
        let (x, y, blank) = (screen.cursor.x, screen.cursor.y, screen.cursor.pen.blank());
        screen.grid.delete_cells(y, x, n, blank);
        screen.cursor.wrap_pending = false;
    }

    /// `CSI 3 J`: drops the saved lines without decoding them.
    fn clear_scrollback(&mut self) {
        self.scrollback.clear();
        self.extras.history.prune(&self.scrollback);
        // The ids that only the dropped lines used are free again.
        self.sweep_extras(crate::extras::LinkId::NONE);
    }
}
