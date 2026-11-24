//! The rows a rewrap produces and the cursor over the style runs of its input.

use std::collections::VecDeque;

use ferrum_scrollback::{LineFlags, Style, StyledRun};

/// One output row.
#[derive(Debug, Default)]
pub(crate) struct OutRow {
    pub(crate) text: String,
    pub(crate) runs: Vec<StyledRun>,
    pub(crate) flags: LineFlags,
}

impl OutRow {
    pub(super) fn push(&mut self, text: &str, style: Style, scalars: usize) {
        self.text.push_str(text);
        match self.runs.last_mut() {
            Some(last) if last.style == style => last.len += scalars,
            _ => self.runs.push(StyledRun { style, len: scalars }),
        }
    }

    pub(super) fn clear(&mut self) {
        self.text.clear();
        self.runs.clear();
        self.flags = LineFlags::NONE;
    }
}

/// The finished rows waiting for the screen, and spare buffers for the next ones.
#[derive(Debug, Default)]
pub(crate) struct Rows {
    pub(crate) pending: VecDeque<OutRow>,
    spare: Vec<OutRow>,
}

impl Rows {
    /// Gives a consumed row back so its buffers are reused.
    pub(crate) fn recycle(&mut self, mut row: OutRow) {
        row.clear();
        self.spare.push(row);
    }

    pub(super) fn fresh(&mut self) -> OutRow {
        self.spare.pop().unwrap_or_default()
    }
}

/// Where the cursor ended up: a row of the line's output and a column in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CursorPlace {
    pub(crate) row: usize,
    pub(crate) col: usize,
}

/// What a finished line produced.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Finished {
    /// Rows the line produced.
    pub(crate) rows: usize,
    pub(crate) cursor: Option<CursorPlace>,
}

/// One old row fed to the rewrapper.
pub(crate) struct Feed<'a> {
    pub(crate) flags: LineFlags,
    pub(crate) text: &'a str,
    pub(crate) runs: &'a [StyledRun],
    /// The cursor column in this row, when it is on it.
    pub(crate) cursor_x: Option<usize>,
}

/// Hands out the style of each scalar of a line, run by run.
pub(crate) struct RunCursor<'a> {
    runs: &'a [StyledRun],
    index: usize,
    used: usize,
}

impl<'a> RunCursor<'a> {
    pub(crate) fn new(runs: &'a [StyledRun]) -> Self {
        Self { runs, index: 0, used: 0 }
    }

    /// The style at the current scalar; advances by `scalars`.
    pub(crate) fn take(&mut self, scalars: usize) -> Style {
        let style = self.runs.get(self.index).map_or_else(Style::default, |run| run.style);
        let mut left = scalars;
        while left > 0 {
            let Some(run) = self.runs.get(self.index) else { break };
            let available = run.len.saturating_sub(self.used);
            if left < available {
                self.used += left;
                break;
            }
            left -= available;
            self.index += 1;
            self.used = 0;
        }
        style
    }

    /// The style at the current scalar and how many of the next `max` scalars (at least 1) share
    /// it; advances by that many.
    pub(super) fn take_run(&mut self, max: usize) -> (Style, usize) {
        loop {
            let Some(run) = self.runs.get(self.index) else { return (Style::default(), max) };
            let available = run.len.saturating_sub(self.used);
            if available == 0 {
                self.index += 1;
                self.used = 0;
                continue;
            }
            let count = available.min(max);
            self.used += count;
            if self.used >= run.len {
                self.index += 1;
                self.used = 0;
            }
            return (run.style, count);
        }
    }
}
