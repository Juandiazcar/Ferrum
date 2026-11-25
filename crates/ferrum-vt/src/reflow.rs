//! Reflow of the primary screen and its scrollback at a new width (FR-TERM-010, AC-TERM-02).
//!
//! The history is rewritten as a stream: the oldest line is popped from the scrollback, joined
//! with the rows soft-wrapped into it, re-cut at the new width and pushed back at the other end,
//! so sealed chunks are freed as they are consumed and no second copy of the history exists. The
//! last rows of the stream become the new screen; the cursor row stays where it was on screen
//! whenever the content allows.

use ferrum_scrollback::{LineCursor, LineFlags, LineRef, Run, Scrollback, StyleTable, StyledRun};

use crate::codec::{Tail, encode_cells, pad_trailing_blanks};
use crate::core::Core;
use crate::error::VtError;
use crate::events::EventSink;
use crate::fill::fill_row;
use crate::grid::Grid;
use crate::history_extras::HistoryExtras;
use crate::rewrap::{Feed, OutRow, Rewrapper, Rows};
use crate::size::Size;

/// Rows of history rewrapped between two checks of the stop request: one scrollback chunk.
const STOP_CHECK_LINES: usize = ferrum_scrollback::HOT_CHUNK_LINES;

/// How a resize ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Reflowed {
    /// The screen and the whole history have the new width.
    Complete,
    /// Stopped on request: the screen has the new width; the history rows not reached yet keep
    /// the old one until the next resize.
    Interrupted,
}

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

    /// Moves every pending row to the scrollback (an interrupted reflow). When the logical line
    /// continues in the old rows that follow, its last row is marked wrapped, unpadded, so the
    /// next reflow joins the two parts exactly.
    fn flush_all(&mut self, out: &mut History<'_>, last: LastRow) {
        while let Some(mut row) = self.rows.pending.pop_front() {
            self.front += 1;
            if last == LastRow::Continues && self.rows.pending.is_empty() {
                row.flags = row.flags.union(LineFlags::WRAPPED);
                let line = LineRef { flags: row.flags, text: &row.text, runs: &row.runs };
                if let Err(error) = out.extras.push(out.scrollback, &line) {
                    tracing::warn!(%error, "scrollback stored a reflowed row clipped");
                }
            } else {
                push_row(out, &mut row);
            }
            self.rows.recycle(row);
        }
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

/// The styles of a stored line's runs, into `out`.
fn styled_runs(styles: &StyleTable, runs: &[Run], out: &mut Vec<StyledRun>) {
    out.clear();
    out.extend(runs.iter().map(|run| StyledRun {
        style: styles.get(run.style).unwrap_or_default(),
        len: usize::try_from(run.len).unwrap_or(usize::MAX),
    }));
}

/// Whether the logical line of the last rewrapped row goes on in the rows not reached.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LastRow {
    Ends,
    Continues,
}

/// The reading side of one pass over the history.
struct Pass {
    /// `pushed_lines` when the pass started: rows from there on are the pass's own output.
    boundary: u64,
    cursor: LineCursor,
    runs: Vec<StyledRun>,
    /// The first row that could not be read (it is dropped, the pass goes on).
    first_error: Option<ferrum_scrollback::ScrollbackError>,
}

impl Pass {
    fn result(&mut self) -> Result<(), VtError> {
        self.first_error.take().map_or(Ok(()), |error| Err(error.into()))
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
    pub(crate) fn reflow_primary(
        &mut self,
        size: Size,
        stop: &mut dyn FnMut() -> bool,
    ) -> Result<Reflowed, VtError> {
        let mut state = Reflow::new(size, self.primary.cursor.y);
        let (history, reflowed) = self.reflow_history(&mut state, stop);
        self.reflow_screen(&mut state);
        if state.open {
            state.finish_line(&mut self.history_out());
        }
        self.install_reflowed(&state, size);
        history.map(|()| reflowed)
    }

    fn history_out(&mut self) -> History<'_> {
        History { scrollback: &mut self.scrollback, extras: &mut self.extras.history }
    }

    /// Pops the old lines oldest first into `state`, asking `stop` every chunk of rows.
    fn reflow_history(
        &mut self,
        state: &mut Reflow,
        stop: &mut dyn FnMut() -> bool,
    ) -> (Result<(), VtError>, Reflowed) {
        let mut pass = Pass {
            boundary: self.scrollback.pushed_lines(),
            cursor: LineCursor::new(),
            runs: Vec::new(),
            first_error: None,
        };
        let mut since_check = 0;
        while self.scrollback.dropped_lines() < pass.boundary {
            since_check += 1;
            if since_check == STOP_CHECK_LINES {
                since_check = 0;
                if stop() {
                    self.interrupt(state, &mut pass);
                    return (pass.result(), Reflowed::Interrupted);
                }
            }
            if !self.rewrap_front(state, &mut pass) {
                break;
            }
        }
        (pass.result(), Reflowed::Complete)
    }

    /// Rewraps the oldest row of the history into `state`; false when the history is empty.
    fn rewrap_front(&mut self, state: &mut Reflow, pass: &mut Pass) -> bool {
        let line = match self.scrollback.pop_front(&mut pass.cursor) {
            Ok(Some(line)) => line,
            Ok(None) => return false,
            Err(error) => {
                // The row stays at the front: drop it, or the rewritten rows would end up behind
                // the ones not yet read. Its loss is reported once, at the end, and the logical
                // line it continued ends here.
                pass.first_error.get_or_insert(error);
                self.scrollback.skip_front();
                if state.open {
                    state.finish_line(&mut self.history_out());
                }
                return true;
            }
        };
        styled_runs(self.scrollback.styles(), &line.runs, &mut pass.runs);
        let row = Feed { flags: line.flags, text: &line.text, runs: &pass.runs, cursor_x: None };
        state.feed(&mut self.history_out(), &row);
        true
    }

    /// Stops rewrapping (`docs/05` §5: a long job checks its cancel token at chunk boundaries):
    /// the rows rewrapped so far go to the history, and the old rows not read yet follow them
    /// unchanged, so the order is kept and nothing is lost. Those rows keep their old width until
    /// the next reflow, which joins and cuts every logical line again. A row that cannot be read
    /// is dropped and reported, as the rewrapping loop does.
    fn interrupt(&mut self, state: &mut Reflow, pass: &mut Pass) {
        let last = if state.open { LastRow::Continues } else { LastRow::Ends };
        if state.open {
            state.finish_line(&mut self.history_out());
        }
        state.flush_all(&mut self.history_out(), last);
        while self.scrollback.dropped_lines() < pass.boundary {
            let line = match self.scrollback.pop_front(&mut pass.cursor) {
                Ok(Some(line)) => line,
                Ok(None) => break,
                Err(error) => {
                    pass.first_error.get_or_insert(error);
                    self.scrollback.skip_front();
                    continue;
                }
            };
            styled_runs(self.scrollback.styles(), &line.runs, &mut pass.runs);
            let copy = LineRef { flags: line.flags, text: &line.text, runs: &pass.runs };
            let out = self.history_out();
            if let Err(error) = out.extras.push(out.scrollback, &copy) {
                tracing::warn!(%error, "scrollback stored a moved row clipped");
            }
        }
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
