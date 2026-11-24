//! The selection model (FR-TERM-014): geometry only. Mouse handling, auto-scroll and the
//! clipboard belong to the UI (WP-11).
//!
//! Positions use stable line numbers: history rows and screen rows are numbered from the first
//! line ever written, so a selection survives scrolling (`Terminal::screen_line`).

/// A position: a stable line number and a display column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point {
    /// Stable line number (see the module docs).
    pub line: u64,
    /// Display column.
    pub col: usize,
}

/// The shape of a selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SelectionKind {
    /// Click and drag: from the anchor to the head along the text.
    Simple,
    /// Double-click: whole words (the range was expanded when the selection was made).
    Word,
    /// Triple-click: whole lines.
    Line,
    /// Alt+drag: a rectangle.
    Block,
}

/// An anchor and a head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    kind: SelectionKind,
    anchor: Point,
    head: Point,
}

impl Selection {
    /// A selection that covers `anchor` only.
    pub const fn new(kind: SelectionKind, anchor: Point) -> Self {
        Self { kind, anchor, head: anchor }
    }

    /// A selection from `anchor` to `head`.
    pub const fn between(kind: SelectionKind, anchor: Point, head: Point) -> Self {
        Self { kind, anchor, head }
    }

    /// The shape.
    pub const fn kind(&self) -> SelectionKind {
        self.kind
    }

    /// Moves the head (dragging, or shift-click to extend).
    pub fn update(&mut self, head: Point) {
        self.head = head;
    }

    /// The anchor and the head in reading order.
    pub fn range(&self) -> (Point, Point) {
        if self.anchor <= self.head { (self.anchor, self.head) } else { (self.head, self.anchor) }
    }

    /// True for a simple selection that covers no more than its anchor cell and has not moved.
    pub fn is_empty(&self) -> bool {
        self.kind == SelectionKind::Simple && self.anchor == self.head
    }

    /// The first and last selected columns (inclusive) of `line` on a `cols` wide screen, or
    /// `None` when the line is outside the selection.
    pub fn row_span(&self, line: u64, cols: usize) -> Option<(usize, usize)> {
        let (start, end) = self.range();
        if line < start.line || line > end.line || cols == 0 {
            return None;
        }
        let last = cols - 1;
        match self.kind {
            SelectionKind::Line => Some((0, last)),
            SelectionKind::Block => {
                let (left, right) =
                    (self.anchor.col.min(self.head.col), self.anchor.col.max(self.head.col));
                Some((left.min(last), right.min(last)))
            }
            _ => {
                let from = if line == start.line { start.col } else { 0 };
                let to = if line == end.line { end.col } else { last };
                Some((from.min(last), to.min(last)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn at(line: u64, col: usize) -> Point {
        Point { line, col }
    }

    #[test]
    fn a_simple_selection_runs_along_the_text() {
        let selection = Selection::between(SelectionKind::Simple, at(5, 7), at(3, 2));
        assert_eq!(selection.range(), (at(3, 2), at(5, 7)));
        assert_eq!(selection.row_span(3, 10), Some((2, 9)));
        assert_eq!(selection.row_span(4, 10), Some((0, 9)));
        assert_eq!(selection.row_span(5, 10), Some((0, 7)));
        assert_eq!(selection.row_span(6, 10), None);
        assert_eq!(selection.row_span(2, 10), None);
    }

    #[test]
    fn a_block_selection_is_a_rectangle_whichever_way_it_is_dragged() {
        let selection = Selection::between(SelectionKind::Block, at(1, 8), at(3, 2));
        assert_eq!(selection.row_span(1, 10), Some((2, 8)));
        assert_eq!(selection.row_span(2, 10), Some((2, 8)));
        assert_eq!(selection.row_span(3, 10), Some((2, 8)));
    }

    #[test]
    fn a_line_selection_covers_whole_rows_and_columns_are_clamped() {
        let selection = Selection::between(SelectionKind::Line, at(2, 4), at(2, 4));
        assert_eq!(selection.row_span(2, 6), Some((0, 5)));
        let wide = Selection::between(SelectionKind::Simple, at(0, 3), at(0, 900));
        assert_eq!(wide.row_span(0, 6), Some((3, 5)));
        assert_eq!(wide.row_span(0, 0), None);
    }

    #[test]
    fn an_unmoved_simple_selection_is_empty_and_updating_moves_the_head() {
        let mut selection = Selection::new(SelectionKind::Simple, at(1, 1));
        assert!(selection.is_empty());
        selection.update(at(1, 4));
        assert!(!selection.is_empty());
        assert_eq!(selection.range(), (at(1, 1), at(1, 4)));
    }
}
