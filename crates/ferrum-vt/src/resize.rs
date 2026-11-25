//! Resizing the terminal (FR-TERM-010).
//!
//! A change of width reflows the primary screen and its history (`reflow.rs`). A change of height
//! alone keeps the history as it is: when the cursor would fall off the bottom the top rows go to
//! the scrollback, otherwise rows are cut or added at the bottom. The alternate screen never
//! reflows: it is cut or padded, as full-screen programs redraw it after the resize anyway.

use crate::core::Core;
use crate::cursor::Cursor;
use crate::error::VtError;
use crate::events::EventSink;
use crate::reflow::Reflowed;
use crate::screen::{Region, Screen};
use crate::size::Size;

impl<S: EventSink> Core<S> {
    /// Changes the size of both screens.
    ///
    /// The terminal is consistent afterwards even when the scrollback fails while it is rewritten;
    /// the error is then returned and the history may be shorter.
    pub(crate) fn resize(
        &mut self,
        size: Size,
        stop: &mut dyn FnMut() -> bool,
    ) -> Result<Reflowed, VtError> {
        let old = self.primary.grid.size();
        if old == size {
            return Ok(Reflowed::Complete);
        }
        let result = if size.cols() == old.cols() {
            self.resize_lines(size);
            Ok(Reflowed::Complete)
        } else {
            self.reflow_primary(size, stop)
        };
        self.resize_alt(size);
        self.settle_after_resize(size);
        result
    }

    /// Height only: the history and the other rows stay as they are.
    fn resize_lines(&mut self, size: Size) {
        let lines = size.rows();
        let cursor_y = self.primary.cursor.y;
        if cursor_y >= lines {
            let dropped = cursor_y + 1 - lines;
            for y in 0..dropped {
                self.push_history_row(y);
            }
            self.primary.grid.drop_top_rows(dropped);
            self.primary.cursor.y -= dropped;
        }
        self.primary.grid.set_lines(lines);
    }

    fn resize_alt(&mut self, size: Size) {
        let Some(old) = self.alt.take() else { return };
        let mut alt = Screen::new(size);
        alt.grid.copy_clipped_from(&old.grid);
        alt.cursor = old.cursor;
        alt.keyboard = old.keyboard;
        alt.saved = old.saved;
        self.alt = Some(alt);
    }

    /// Resets what depends on the size: margins, tab stops, cursors, damage.
    fn settle_after_resize(&mut self, size: Size) {
        self.tabs.resize(size.cols());
        for screen in std::iter::once(&mut self.primary).chain(self.alt.as_mut()) {
            screen.region = Region::full(size.rows());
            let clamp = |cursor: &mut Cursor| {
                cursor.x = cursor.x.min(size.cols() - 1);
                cursor.y = cursor.y.min(size.rows() - 1);
                cursor.wrap_pending = false;
            };
            clamp(&mut screen.cursor);
            if let Some(saved) = screen.saved.as_mut() {
                clamp(&mut saved.cursor);
            }
            screen.grid.damage_mut().mark_all();
        }
    }
}
