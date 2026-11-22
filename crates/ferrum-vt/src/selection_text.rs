//! Reading text under a selection: words, lines and the text to copy (FR-TERM-014).
//!
//! History rows are decoded from the scrollback and screen rows are read from the grid; both are
//! turned into the same [`RowView`] so that the rules (word characters, soft-wrapped rows joined
//! without a newline, trailing blanks dropped) are written once.

use ferrum_scrollback::{LineCursor, LineFlags};

use crate::cell::CellFlags;
use crate::events::EventSink;
use crate::selection::{Point, Selection, SelectionKind};
use crate::terminal::Terminal;
use crate::width::{cluster_width, clusters};

/// Characters that end a word on a double-click (Alacritty's default set).
const WORD_SEPARATORS: &str = ",│`|:\"' ()[]{}<>\t";

/// One cluster of a row: where it starts and how wide it is.
struct Cluster {
    text: String,
    col: usize,
    width: usize,
}

/// A row as a list of clusters.
struct RowView {
    clusters: Vec<Cluster>,
    wrapped: bool,
}

impl RowView {
    fn from_text(text: &str, wrapped: bool) -> Self {
        let mut col = 0;
        let mut found: Vec<Cluster> = Vec::new();
        for piece in clusters(text) {
            let width = cluster_width(piece);
            if width == 0 {
                if let Some(previous) = found.last_mut() {
                    previous.text.push_str(piece);
                }
                continue;
            }
            found.push(Cluster { text: piece.to_owned(), col, width });
            col += width;
        }
        Self { clusters: found, wrapped }
    }

    /// The index of the cluster that covers display column `col`.
    fn cluster_at(&self, col: usize) -> Option<usize> {
        self.clusters.iter().position(|c| (c.col..c.col + c.width).contains(&col))
    }

    fn is_separator(&self, index: usize) -> bool {
        self.clusters
            .get(index)
            .is_some_and(|c| c.text.chars().any(|ch| WORD_SEPARATORS.contains(ch)))
    }

    /// The text of the clusters that overlap columns `from..=to`, without trailing blanks unless
    /// the row is soft-wrapped into the next one.
    fn slice(&self, from: usize, to: usize) -> String {
        let mut text: String = self
            .clusters
            .iter()
            .filter(|c| c.col + c.width > from && c.col <= to)
            .map(|c| c.text.as_str())
            .collect();
        if !self.wrapped {
            text.truncate(text.trim_end_matches(' ').len());
        }
        text
    }
}

impl<S: EventSink> Terminal<S> {
    /// The stable line number of screen row `y` (see [`Point`]).
    pub fn screen_line(&self, y: usize) -> u64 {
        self.core.scrollback.pushed_lines() + u64::try_from(y).unwrap_or(u64::MAX)
    }

    fn row_view(&self, line: u64) -> Option<RowView> {
        let screen_top = self.core.scrollback.pushed_lines();
        if line >= screen_top {
            let y = usize::try_from(line - screen_top).ok()?;
            return self.screen_row_view(y);
        }
        let index =
            usize::try_from(line.checked_sub(self.core.scrollback.dropped_lines())?).ok()?;
        let mut cursor = LineCursor::new();
        let decoded = self.core.scrollback.read_line(index, &mut cursor).ok().flatten()?;
        Some(RowView::from_text(&decoded.text, decoded.flags.contains(LineFlags::WRAPPED)))
    }

    fn screen_row_view(&self, y: usize) -> Option<RowView> {
        let grid = &self.core.screen().grid;
        let row = grid.row(y)?;
        let mut text = String::new();
        for cell in row
            .cells()
            .iter()
            .filter(|c| !c.flags().intersects(CellFlags::SPACER | CellFlags::LEADING_SPACER))
        {
            let mut buffer = [0; 4];
            text.push_str(grid.cell_text(cell, &mut buffer));
        }
        Some(RowView::from_text(&text, row.is_wrapped()))
    }

    /// The word under `at` (a run of word characters, or of separators), as a selection.
    pub fn select_word(&self, at: Point) -> Selection {
        let plain = Selection::new(SelectionKind::Word, at);
        let Some(view) = self.row_view(at.line) else { return plain };
        let Some(index) = view.cluster_at(at.col) else { return plain };
        let class = view.is_separator(index);
        let mut first = index;
        while first > 0 && view.is_separator(first - 1) == class {
            first -= 1;
        }
        let mut last = index;
        while last + 1 < view.clusters.len() && view.is_separator(last + 1) == class {
            last += 1;
        }
        let (Some(start), Some(end)) = (view.clusters.get(first), view.clusters.get(last)) else {
            return plain;
        };
        Selection::between(
            SelectionKind::Word,
            Point { line: at.line, col: start.col },
            Point { line: at.line, col: end.col + end.width - 1 },
        )
    }

    /// The whole logical line under `at` (all the rows soft-wrapped together), as a selection.
    pub fn select_logical_line(&self, at: Point) -> Selection {
        let (mut first, mut last) = (at.line, at.line);
        while first > 0 && self.row_view(first - 1).is_some_and(|row| row.wrapped) {
            first -= 1;
        }
        while self.row_view(last).is_some_and(|row| row.wrapped)
            && self.row_view(last + 1).is_some()
        {
            last += 1;
        }
        let cols = self.size().cols();
        Selection::between(
            SelectionKind::Line,
            Point { line: first, col: 0 },
            Point { line: last, col: cols.saturating_sub(1) },
        )
    }

    /// The text to copy for `selection`: rows are separated by a newline, except where a row is
    /// soft-wrapped into the next one; trailing blanks of a row are dropped.
    pub fn selection_text(&self, selection: &Selection) -> String {
        let (start, end) = selection.range();
        let cols = self.size().cols();
        let mut out = String::new();
        let mut joined = false;
        for line in start.line..=end.line {
            let (Some(view), Some((from, to))) =
                (self.row_view(line), selection.row_span(line, cols))
            else {
                continue;
            };
            if line != start.line && !joined {
                out.push('\n');
            }
            out.push_str(&view.slice(from, to));
            joined = view.wrapped && selection.kind() != SelectionKind::Block;
        }
        out
    }
}
