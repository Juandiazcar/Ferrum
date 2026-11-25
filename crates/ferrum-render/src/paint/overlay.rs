//! What is drawn over the cells: the cursor (`FR-TERM-017`) and the selection (`FR-TERM-014`;
//! the selection model lives in `ferrum-vt`, this only paints its span).

use crate::color::Rgb;

/// Cursor shapes. The UI picks [`CursorShape::Hollow`] for an unfocused pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CursorShape {
    /// The whole cell, the text drawn in the cursor text color.
    Block,
    /// A line under the cell.
    Underline,
    /// A vertical bar at the left of the cell.
    Bar,
    /// The outline of the cell (unfocused).
    Hollow,
}

/// The cursor to draw, if visible (blink phases are the UI's: it passes `None` when hidden).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cursor {
    /// Column.
    pub col: u16,
    /// Row.
    pub row: u16,
    /// Shape.
    pub shape: CursorShape,
    /// Cursor color.
    pub color: Rgb,
    /// Text color inside a block cursor.
    pub text_color: Rgb,
}

/// Linear (stream) or rectangular (Alt+drag) selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionKind {
    /// From the start cell to the end cell in reading order.
    Linear,
    /// The rectangle with the two cells as corners.
    Block,
}

/// The selected cells, both ends included, in viewport coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Selection {
    /// One end: (column, row).
    pub start: (u16, u16),
    /// The other end: (column, row).
    pub end: (u16, u16),
    /// Linear or rectangular.
    pub kind: SelectionKind,
    /// Background of selected cells.
    pub bg: Rgb,
    /// Text color of selected cells; `None` keeps each cell's own.
    pub fg: Option<Rgb>,
}

impl Selection {
    /// The selected columns of `row` as a half-open range, clipped to `cols`.
    pub(crate) fn columns(&self, row: u16, cols: u16) -> Option<(u16, u16)> {
        let (first, last) = if (self.start.1, self.start.0) <= (self.end.1, self.end.0) {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        };
        if row < first.1 || row > last.1 {
            return None;
        }
        let (from, to) = match self.kind {
            SelectionKind::Block => (first.0.min(last.0), first.0.max(last.0).saturating_add(1)),
            SelectionKind::Linear => (
                if row == first.1 { first.0 } else { 0 },
                if row == last.1 { last.0.saturating_add(1) } else { cols },
            ),
        };
        let to = to.min(cols);
        (from < to).then_some((from, to))
    }
}

/// Everything drawn over the cells of one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Overlay {
    /// The cursor, when visible.
    pub cursor: Option<Cursor>,
    /// The selection, when there is one.
    pub selection: Option<Selection>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(start: (u16, u16), end: (u16, u16), kind: SelectionKind) -> Selection {
        Selection { start, end, kind, bg: Rgb::BLACK, fg: None }
    }

    #[test]
    fn a_linear_selection_runs_in_reading_order() {
        let linear = selection((8, 3), (2, 1), SelectionKind::Linear);
        assert_eq!(linear.columns(0, 10), None);
        assert_eq!(linear.columns(1, 10), Some((2, 10)));
        assert_eq!(linear.columns(2, 10), Some((0, 10)));
        assert_eq!(linear.columns(3, 10), Some((0, 9)));
        assert_eq!(linear.columns(4, 10), None);
    }

    #[test]
    fn a_block_selection_is_a_rectangle_clipped_to_the_grid() {
        let block = selection((8, 3), (2, 1), SelectionKind::Block);
        assert_eq!(block.columns(2, 20), Some((2, 9)));
        assert_eq!(block.columns(2, 5), Some((2, 5)));
        assert_eq!(selection((9, 0), (9, 0), SelectionKind::Block).columns(0, 5), None);
    }
}
