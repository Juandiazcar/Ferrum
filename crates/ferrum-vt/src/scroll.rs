//! Scrolling, line insertion and deletion; rows that leave the top of the primary screen go to
//! the scrollback (`docs/02` §3.2).

use ferrum_scrollback::LineRef;

use crate::codec::encode_row;
use crate::core::Core;
use crate::events::{Event, EventSink};

impl<S: EventSink> Core<S> {
    /// Encodes row `y` of the primary grid into the scrollback.
    pub(crate) fn push_history_row(&mut self, y: usize) {
        let Some(row) = self.primary.grid.row(y) else { return };
        encode_row(row, self.primary.grid.graphemes(), &mut self.scratch.line);
        let encoded = &self.scratch.line;
        let line = LineRef { flags: row.flags(), text: &encoded.text, runs: &encoded.runs };
        // Recording its extra ids on the way (`history_extras.rs`).
        if let Err(error) = self.extras.history.push(&mut self.scrollback, &line) {
            // A row over the scrollback's size limits (a full-width row of long clusters) is
            // still stored and counted, its text clipped and without styles; never fatal.
            tracing::warn!(%error, "scrollback stored a row clipped");
        }
        if let Some(event) = self.scrollback.take_event() {
            self.sink.event(Event::Scrollback(event));
        }
    }

    /// Scrolls the scroll region up by `n` rows. On the primary screen, when the region starts at
    /// the top of the screen, the rows that leave go to the scrollback.
    pub(crate) fn scroll_up(&mut self, n: usize) {
        let alt = self.alt_active();
        let region = self.screen().region;
        let count = n.min(region.height());
        if !alt && region.top() == 0 {
            for y in 0..count {
                self.push_history_row(y);
            }
        }
        let blank = self.screen().cursor.pen.blank();
        self.screen_mut().grid.scroll_up(region.top(), region.bottom(), count, blank);
    }

    /// Scrolls the scroll region down by `n` rows.
    pub(crate) fn scroll_down(&mut self, n: usize) {
        let region = self.screen().region;
        let blank = self.screen().cursor.pen.blank();
        self.screen_mut().grid.scroll_down(region.top(), region.bottom(), n, blank);
    }

    /// IL: inserts `n` blank rows at the cursor, inside the scroll region only.
    pub(crate) fn insert_lines(&mut self, n: usize) {
        let region = self.screen().region;
        let screen = self.screen_mut();
        if !region.contains(screen.cursor.y) {
            return;
        }
        let blank = screen.cursor.pen.blank();
        screen.grid.scroll_down(screen.cursor.y, region.bottom(), n, blank);
        screen.cursor.x = 0;
        screen.cursor.wrap_pending = false;
    }

    /// DL: deletes `n` rows at the cursor, inside the scroll region only.
    pub(crate) fn delete_lines(&mut self, n: usize) {
        let region = self.screen().region;
        let screen = self.screen_mut();
        if !region.contains(screen.cursor.y) {
            return;
        }
        let blank = screen.cursor.pen.blank();
        screen.grid.scroll_up(screen.cursor.y, region.bottom(), n, blank);
        screen.cursor.x = 0;
        screen.cursor.wrap_pending = false;
    }
}
