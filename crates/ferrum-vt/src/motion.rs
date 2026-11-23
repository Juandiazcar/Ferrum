//! Cursor movement (xterm ctlseqs: CUU, CUD, CUF, CUB, CUP, HT, BS, CR, IND, NEL, RI, ...).

use crate::core::Core;
use crate::events::EventSink;
use crate::modes::Mode;

impl<S: EventSink> Core<S> {
    fn origin_top(&self) -> usize {
        if self.modes.contains(Mode::ORIGIN) { self.screen().region.top() } else { 0 }
    }

    /// CR.
    pub(crate) fn carriage_return(&mut self) {
        let cursor = &mut self.screen_mut().cursor;
        cursor.x = 0;
        cursor.wrap_pending = false;
    }

    /// BS.
    pub(crate) fn backspace(&mut self) {
        let cursor = &mut self.screen_mut().cursor;
        cursor.x = cursor.x.saturating_sub(1);
        cursor.wrap_pending = false;
    }

    /// IND: down one row, scrolling at the bottom of the scroll region.
    pub(crate) fn index(&mut self) {
        let screen = self.screen();
        let (y, bottom, lines) = (screen.cursor.y, screen.region.bottom(), screen.grid.lines());
        if y == bottom {
            self.scroll_up(1);
        } else if y + 1 < lines {
            self.screen_mut().cursor.y += 1;
        }
        self.screen_mut().cursor.wrap_pending = false;
    }

    /// RI: up one row, scrolling down at the top of the scroll region.
    pub(crate) fn reverse_index(&mut self) {
        let screen = self.screen();
        let (y, top) = (screen.cursor.y, screen.region.top());
        if y == top {
            self.scroll_down(1);
        } else {
            self.screen_mut().cursor.y = y.saturating_sub(1);
        }
        self.screen_mut().cursor.wrap_pending = false;
    }

    /// LF, VT, FF: IND, plus CR in new-line mode.
    pub(crate) fn line_feed(&mut self) {
        self.index();
        if self.modes.contains(Mode::LINE_FEED_NEWLINE) {
            self.carriage_return();
        }
    }

    /// NEL: CR then IND.
    pub(crate) fn next_line(&mut self) {
        self.carriage_return();
        self.index();
    }

    /// CUP/HVP/VPA/CHA-style absolute move; `row` is relative to the scroll region in origin mode.
    /// `None` keeps the current coordinate.
    pub(crate) fn goto(&mut self, row: Option<usize>, col: Option<usize>) {
        let origin = self.modes.contains(Mode::ORIGIN);
        let top = self.origin_top();
        let screen = self.screen_mut();
        let last_row =
            if origin { screen.region.bottom() } else { screen.grid.lines().saturating_sub(1) };
        let last_col = screen.grid.cols().saturating_sub(1);
        if let Some(row) = row {
            screen.cursor.y = (top + row).min(last_row);
        }
        if let Some(col) = col {
            screen.cursor.x = col.min(last_col);
        }
        screen.cursor.wrap_pending = false;
    }

    /// CUU: up, stopping at the top margin when the cursor is below it.
    pub(crate) fn cursor_up(&mut self, n: usize) {
        let screen = self.screen_mut();
        let limit = if screen.region.contains(screen.cursor.y) { screen.region.top() } else { 0 };
        screen.cursor.y = screen.cursor.y.saturating_sub(n).max(limit);
        screen.cursor.wrap_pending = false;
    }

    /// CUD: down, stopping at the bottom margin when the cursor is above it.
    pub(crate) fn cursor_down(&mut self, n: usize) {
        let screen = self.screen_mut();
        let last = screen.grid.lines().saturating_sub(1);
        let limit =
            if screen.region.contains(screen.cursor.y) { screen.region.bottom() } else { last };
        screen.cursor.y = screen.cursor.y.saturating_add(n).min(limit);
        screen.cursor.wrap_pending = false;
    }

    /// CUF.
    pub(crate) fn cursor_forward(&mut self, n: usize) {
        let screen = self.screen_mut();
        let last = screen.grid.cols().saturating_sub(1);
        screen.cursor.x = screen.cursor.x.saturating_add(n).min(last);
        screen.cursor.wrap_pending = false;
    }

    /// CUB.
    pub(crate) fn cursor_back(&mut self, n: usize) {
        let cursor = &mut self.screen_mut().cursor;
        cursor.x = cursor.x.saturating_sub(n);
        cursor.wrap_pending = false;
    }

    /// HT / CHT: forward `n` tab stops.
    pub(crate) fn tab_forward(&mut self, n: usize) {
        let mut x = self.screen().cursor.x;
        for _ in 0..n {
            x = self.tabs.next(x);
        }
        let cursor = &mut self.screen_mut().cursor;
        cursor.x = x;
        cursor.wrap_pending = false;
    }

    /// CBT: back `n` tab stops.
    pub(crate) fn tab_back(&mut self, n: usize) {
        let mut x = self.screen().cursor.x;
        for _ in 0..n {
            x = self.tabs.prev(x);
        }
        let cursor = &mut self.screen_mut().cursor;
        cursor.x = x;
        cursor.wrap_pending = false;
    }
}
