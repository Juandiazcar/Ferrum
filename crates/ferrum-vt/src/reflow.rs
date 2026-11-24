//! Reflow of the primary screen and its scrollback at a new width (FR-TERM-010, AC-TERM-02).
//!
//! The history is rewritten as a stream: the oldest line is popped from the scrollback, joined
//! with the rows soft-wrapped into it, re-cut at the new width and pushed back at the other end,
//! so sealed chunks are freed as they are consumed and no second copy of the history exists. The
//! last rows of the stream become the new screen; the cursor row stays where it was on screen
//! whenever the content allows.

use ferrum_scrollback::{LineCursor, LineFlags, LineRef, Scrollback, StyledRun};

use crate::codec::{Tail, encode_cells, pad_trailing_blanks};
use crate::core::Core;
use crate::error::VtError;
use crate::events::EventSink;
use crate::fill::fill_row;
use crate::grid::Grid;
use crate::history_extras::HistoryExtras;
use crate::rewrap::{Feed, OutRow, Rewrapper, Rows};
use crate::size::Size;

/// The state of one reflow.
struct Reflow {
    lines: usize,
    /// Row of the cursor on the old screen: it stays there when possible.
    cursor_row: usize,
    rewrapper: Rewrapper,
    /// The newest rows, not yet in the scrollback; at most `lines` once flushed.
    rows: Rows,
    /// Index in the whole output of the first pending row.
    front: usize,
    /// Rows of the lines finished so far.
    produced: usize,
    /// The cursor in the whole output: row index and column.
    cursor: Option<(usize, usize)>,
    /// A logical line has been started and not finished.
    open: bool,
}

impl Reflow {
    fn new(size: Size, cursor_row: usize) -> Self {
        Self {
            lines: size.rows(),
            cursor_row,
            rewrapper: Rewrapper::new(size.cols()),
            rows: Rows::default(),
            front: 0,
            produced: 0,
            cursor: None,
            open: false,
        }
    }

    fn feed(&mut self, out: &mut History<'_>, row: &Feed<'_>) {
        self.open = true;
        self.rewrapper.feed(row, &mut self.rows);
        self.flush(out);
        if !row.flags.contains(LineFlags::WRAPPED) {
            self.finish_line(out);
        }
    }

    fn finish_line(&mut self, out: &mut History<'_>) {
        self.open = false;
        let done = self.rewrapper.finish(&mut self.rows);
        if let Some(place) = done.cursor {
            self.cursor = Some((self.produced + place.row, place.col));
        }
        self.produced += done.rows;
        self.flush(out);
    }

    /// Moves rows that can no longer be on the screen to the scrollback.
    fn flush(&mut self, out: &mut History<'_>) {
        loop {
            let too_many = self.rows.pending.len() > self.lines;
            let above_anchor =
                self.cursor.is_some_and(|(row, _)| self.front + self.cursor_row < row);
            if !(too_many || above_anchor) {
                break;
            }
            let Some(mut row) = self.rows.pending.pop_front() else { break };
            self.front += 1;
            push_row(out, &mut row);
            self.rows.recycle(row);
        }
    }
}

/// Where reflowed rows go: the scrollback, through the summary of its extra ids.
struct History<'a> {
    scrollback: &'a mut Scrollback,
    extras: &'a mut HistoryExtras,
}

fn push_row(out: &mut History<'_>, row: &mut OutRow) {
    if row.flags.contains(LineFlags::WRAPPED) {
        pad_trailing_blanks(&row.text, &mut row.runs);
    }
    let line = LineRef { flags: row.flags, text: &row.text, runs: &row.runs };
    if let Err(error) = out.extras.push(out.scrollback, &line) {
        tracing::warn!(%error, "scrollback stored a reflowed row clipped");
    }
}

impl<S: EventSink> Core<S> {
    /// Rewrites history and screen of the primary screen for `size` (the columns changed).
    ///
    /// A history row that cannot be read is dropped from the history and the first such error is
    /// returned at the end: the rows around it keep their order, and the logical line it belonged
    /// to ends at the lost row, so the text before it is never joined to the text after it.
    pub(crate) fn reflow_primary(&mut self, size: Size) -> Result<(), VtError> {
        let mut state = Reflow::new(size, self.primary.cursor.y);
        let history = self.reflow_history(&mut state);
        self.reflow_screen(&mut state);
        if state.open {
            state.finish_line(&mut self.history_out());
        }
        self.install_reflowed(&state, size);
        history
    }

    fn history_out(&mut self) -> History<'_> {
        History { scrollback: &mut self.scrollback, extras: &mut self.extras.history }
    }

    /// Pops the old lines oldest first into `state`.
    fn reflow_history(&mut self, state: &mut Reflow) -> Result<(), VtError> {
        let boundary = self.scrollback.pushed_lines();
        let mut cursor = LineCursor::new();
        let mut runs: Vec<StyledRun> = Vec::new();
        let mut first_error = None;
        while self.scrollback.dropped_lines() < boundary {
            let line = match self.scrollback.pop_front(&mut cursor) {
                Ok(Some(line)) => line,
                Ok(None) => break,
                Err(error) => {
                    // The row stays at the front: drop it, or the rewritten rows would end up
                    // behind the ones not yet read. Its loss is reported once, at the end, and
                    // the logical line it continued ends here.
                    first_error.get_or_insert(error);
                    self.scrollback.skip_front();
                    if state.open {
                        state.finish_line(&mut self.history_out());
                    }
                    continue;
                }
            };
            runs.clear();
            runs.extend(line.runs.iter().map(|run| StyledRun {
                style: self.scrollback.styles().get(run.style).unwrap_or_default(),
                len: usize::try_from(run.len).unwrap_or(usize::MAX),
            }));
            let row = Feed { flags: line.flags, text: &line.text, runs: &runs, cursor_x: None };
            state.feed(&mut self.history_out(), &row);
        }
        first_error.map_or(Ok(()), |error| Err(error.into()))
    }

    /// Feeds the rows of the old screen up to the last one with content or the cursor.
    fn reflow_screen(&mut self, state: &mut Reflow) {
        let screen = &self.primary;
        let last = (0..screen.grid.lines())
            .rev()
            .find(|&y| {
                screen
                    .grid
                    .row(y)
                    .is_some_and(|row| row.used_len() > 0 || row.flags() != LineFlags::NONE)
            })
            .map_or(screen.cursor.y, |y| y.max(screen.cursor.y));
        for y in 0..=last {
            let Some(row) = screen.grid.row(y) else { break };
            // Blanks after the last character are not content, except in a wrapped row.
            let width = if row.is_wrapped() { row.len() } else { row.used_len() };
            let cells = row.cells().get(..width).unwrap_or_default();
            let encoded = &mut self.scratch.line;
            encode_cells(cells, Tail::of(row), screen.grid.graphemes(), encoded);
            let cursor_x = (y == screen.cursor.y).then_some(screen.cursor.x);
            let input = Feed {
                flags: row.flags(),
                text: self.scratch.line.text(),
                runs: &self.scratch.line.runs,
                cursor_x,
            };
            let mut out =
                History { scrollback: &mut self.scrollback, extras: &mut self.extras.history };
            state.feed(&mut out, &input);
        }
    }

    /// Builds the new primary grid from the pending rows.
    fn install_reflowed(&mut self, state: &Reflow, size: Size) {
        let mut grid = Grid::new(size);
        for (y, row) in state.rows.pending.iter().enumerate() {
            fill_row(&mut grid, y, row);
        }
        let (row, col) = state.cursor.unwrap_or((state.front, 0));
        let y = row.saturating_sub(state.front).min(size.rows() - 1);
        self.primary.grid = grid;
        self.primary.cursor.y = y;
        self.primary.cursor.x = col.min(size.cols() - 1);
        self.primary.cursor.wrap_pending = false;
    }
}
