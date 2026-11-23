//! Scrolling, line insertion and deletion; rows that leave the top of the primary screen go to
//! the scrollback (`docs/02` §3.2).

use ferrum_scrollback::{LineRef, Scrollback};

use crate::codec::{Tail, encode_cells};
use crate::core::Core;
use crate::events::{Event, EventSink};
use crate::extras::Extras;

impl<S: EventSink> Core<S> {
    /// Encodes row `y` of the primary grid into the scrollback.
    pub(crate) fn push_history_row(&mut self, y: usize) {
        if self.scrollback.config().max_lines() == 0 {
            // No history: the row is only counted, so that line numbers stay stable.
            let empty = LineRef { flags: ferrum_scrollback::LineFlags::NONE, text: "", runs: &[] };
            Self::store_history_line(
                &mut self.scrollback,
                &mut self.extras,
                &mut self.sink,
                &empty,
            );
            return;
        }
        let Some(row) = self.primary.grid.row(y) else { return };
        // Blanks after the last character are trimmed by the scrollback anyway, so only a
        // wrapped row (whose blanks are content) is encoded to its full width.
        let width = if row.is_wrapped() { row.len() } else { row.used_len() };
        let cells = row.cells().get(..width).unwrap_or_default();
        let encoded = &mut self.scratch.line;
        encode_cells(cells, Tail::of(row), self.primary.grid.graphemes(), encoded);
        let line = LineRef { flags: row.flags(), text: &encoded.text, runs: &encoded.runs };
        Self::store_history_line(&mut self.scrollback, &mut self.extras, &mut self.sink, &line);
    }

    /// Pushes one line into the scrollback, recording its extra ids (`history_extras.rs`). A row
    /// over the scrollback's size limits (a full-width row of long clusters) is still stored and
    /// counted, its text clipped and without styles (`Scrollback::push_line`); the loss is
    /// logged, never fatal.
    fn store_history_line(
        scrollback: &mut Scrollback,
        extras: &mut Extras,
        sink: &mut S,
        line: &LineRef<'_>,
    ) {
        if let Err(error) = extras.history.push(scrollback, line) {
            tracing::warn!(%error, "scrollback stored a row clipped");
        }
        if let Some(event) = scrollback.take_event() {
            sink.event(Event::Scrollback(event));
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
