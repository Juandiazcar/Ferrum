//! Reflow of the primary screen and its scrollback at a new width (FR-TERM-010, AC-TERM-02).
//!
//! The history is rewritten as a stream: the oldest line is popped from the scrollback, joined
//! with the rows soft-wrapped into it, re-cut at the new width and pushed back at the other end,
//! so sealed chunks are freed as they are consumed and no second copy of the history exists. The
//! last rows of the stream become the new screen; the cursor row stays where it was on screen
//! whenever the content allows.

use std::collections::VecDeque;

use ferrum_scrollback::{LineCursor, LineFlags, LineRef, Scrollback, StyledRun};

use crate::codec::{WRAP_PAD, encode_cells, pad_trailing_blanks};
use crate::core::Core;
use crate::error::VtError;
use crate::events::EventSink;
use crate::fill::fill_row;
use crate::grid::Grid;
use crate::rewrap::{CursorPlace, LogicalLine, OutRow, rewrap};
use crate::size::Size;
use crate::width::{cluster_width, clusters};

/// One input row of the stream.
struct InputRow<'a> {
    flags: LineFlags,
    text: &'a str,
    runs: &'a [StyledRun],
    cursor_x: Option<usize>,
}

/// The state of one reflow.
struct Reflow {
    cols: usize,
    lines: usize,
    /// Row of the cursor on the old screen: it stays there when possible.
    cursor_row: usize,
    line: LogicalLine,
    /// The newest rows, not yet in the scrollback; at most `lines` once flushed.
    pending: VecDeque<OutRow>,
    /// Index in the whole output of the first pending row.
    front: usize,
    /// Rows produced so far.
    produced: usize,
    /// The cursor in the whole output: row index and column.
    cursor: Option<(usize, usize)>,
    /// A logical line has been started and not finished.
    open: bool,
}

impl Reflow {
    fn new(size: Size, cursor_row: usize) -> Self {
        Self {
            cols: size.cols(),
            lines: size.rows(),
            cursor_row,
            line: LogicalLine::default(),
            pending: VecDeque::new(),
            front: 0,
            produced: 0,
            cursor: None,
            open: false,
        }
    }

    fn feed(&mut self, scrollback: &mut Scrollback, row: &InputRow<'_>) {
        self.open = true;
        if let Some(x) = row.cursor_x {
            self.line.cursor_offset = Some(self.line.columns + x);
        }
        self.line.text.push_str(row.text);
        for run in row.runs {
            let mut style = run.style;
            style.attrs &= !WRAP_PAD;
            match self.line.runs.last_mut() {
                Some(last) if last.style == style => last.len += run.len,
                _ => self.line.runs.push(StyledRun { style, len: run.len }),
            }
        }
        self.line.columns += clusters(row.text).map(cluster_width).sum::<usize>();
        let marks = row.flags.bits() & !LineFlags::WRAPPED.bits();
        self.line.marks =
            LineFlags::from_bits(self.line.marks.bits() | marks).unwrap_or(self.line.marks);
        if !row.flags.contains(LineFlags::WRAPPED) {
            self.finish_line(scrollback);
        }
    }

    fn finish_line(&mut self, scrollback: &mut Scrollback) {
        self.open = false;
        let (rows, place) = rewrap(&self.line, self.cols);
        if let Some(CursorPlace { row, col }) = place {
            self.cursor = Some((self.produced + row, col));
        }
        self.produced += rows.len();
        self.pending.extend(rows);
        self.line.clear();
        self.flush(scrollback);
    }

    /// Moves rows that can no longer be on the screen to the scrollback.
    fn flush(&mut self, scrollback: &mut Scrollback) {
        loop {
            let too_many = self.pending.len() > self.lines;
            let above_anchor =
                self.cursor.is_some_and(|(row, _)| self.front + self.cursor_row < row);
            if !(too_many || above_anchor) {
                break;
            }
            let Some(mut row) = self.pending.pop_front() else { break };
            self.front += 1;
            push_row(scrollback, &mut row);
        }
    }
}

fn push_row(scrollback: &mut Scrollback, row: &mut OutRow) {
    if row.flags.contains(LineFlags::WRAPPED) {
        pad_trailing_blanks(&row.text, &mut row.runs);
    }
    let line = LineRef { flags: row.flags, text: &row.text, runs: &row.runs };
    if let Err(error) = scrollback.push_line(&line) {
        tracing::warn!(%error, "scrollback refused a reflowed row");
    }
}

impl<S: EventSink> Core<S> {
    /// Rewrites history and screen of the primary screen for `size` (the columns changed).
    pub(crate) fn reflow_primary(&mut self, size: Size) -> Result<(), VtError> {
        let mut state = Reflow::new(size, self.primary.cursor.y);
        let history = self.reflow_history(&mut state);
        self.reflow_screen(&mut state);
        if state.open {
            state.finish_line(&mut self.scrollback);
        }
        self.install_reflowed(state, size);
        history
    }

    /// Pops the old lines oldest first into `state`.
    fn reflow_history(&mut self, state: &mut Reflow) -> Result<(), VtError> {
        let boundary = self.scrollback.pushed_lines();
        let mut cursor = LineCursor::new();
        let mut runs: Vec<StyledRun> = Vec::new();
        while self.scrollback.dropped_lines() < boundary {
            let line = match self.scrollback.pop_front(&mut cursor) {
                Ok(Some(line)) => line,
                Ok(None) => break,
                Err(error) => return Err(error.into()),
            };
            runs.clear();
            runs.extend(line.runs.iter().map(|run| StyledRun {
                style: self.scrollback.styles().get(run.style).unwrap_or_default(),
                len: usize::try_from(run.len).unwrap_or(usize::MAX),
            }));
            let row = InputRow { flags: line.flags, text: &line.text, runs: &runs, cursor_x: None };
            state.feed(&mut self.scrollback, &row);
        }
        Ok(())
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
            let (text, runs) = (&mut self.scratch.text, &mut self.scratch.runs);
            encode_cells(cells, row.is_wrapped(), screen.grid.graphemes(), text, runs);
            let cursor_x = (y == screen.cursor.y).then_some(screen.cursor.x);
            let input = InputRow {
                flags: row.flags(),
                text: &self.scratch.text,
                runs: &self.scratch.runs,
                cursor_x,
            };
            state.feed(&mut self.scrollback, &input);
        }
    }

    /// Builds the new primary grid from the pending rows.
    fn install_reflowed(&mut self, mut state: Reflow, size: Size) {
        let mut grid = Grid::new(size);
        for (y, row) in state.pending.iter().enumerate() {
            fill_row(&mut grid, y, row);
        }
        let (row, col) = state.cursor.unwrap_or((state.front, 0));
        let y = row.saturating_sub(state.front).min(size.rows() - 1);
        self.primary.grid = grid;
        self.primary.cursor.y = y;
        self.primary.cursor.x = col.min(size.cols() - 1);
        self.primary.cursor.wrap_pending = false;
        state.pending.clear();
    }
}
