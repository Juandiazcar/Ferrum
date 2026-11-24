//! Grid dimensions.

use crate::limits::{MAX_COLS, MAX_ROWS, MIN_DIMENSION};

/// Columns and rows of a terminal. Always within `1..=MAX_COLS` and `1..=MAX_ROWS`: a hostile or
/// absurd request is clamped, never honored at its full size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Size {
    cols: u16,
    rows: u16,
}

impl Size {
    /// The size `cols` x `rows` exactly, or `None` when it is outside the supported range. The
    /// daemon uses this to tell the UI that a request cannot be honored, instead of letting the
    /// UI believe in a size the terminal does not have (ADR 0028).
    pub fn exact(cols: u16, rows: u16) -> Option<Self> {
        let size = Self::new(cols, rows);
        (size.cols == cols && size.rows == rows).then_some(size)
    }

    /// A size clamped into the supported range. Compare with [`Size::exact`] or read back
    /// [`Terminal::size`](crate::Terminal::size) to learn what was applied.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols: cols.clamp(MIN_DIMENSION, MAX_COLS),
            rows: rows.clamp(MIN_DIMENSION, MAX_ROWS),
        }
    }

    /// Columns.
    pub const fn cols(self) -> usize {
        self.cols as usize
    }

    /// Rows.
    pub const fn rows(self) -> usize {
        self.rows as usize
    }
}

impl Default for Size {
    fn default() -> Self {
        Self::new(crate::limits::DEFAULT_COLS, crate::limits::DEFAULT_ROWS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_clamped_into_the_supported_range() {
        assert_eq!((Size::new(0, 0).cols(), Size::new(0, 0).rows()), (1, 1));
        let huge = Size::new(u16::MAX, u16::MAX);
        assert_eq!((huge.cols(), huge.rows()), (usize::from(MAX_COLS), usize::from(MAX_ROWS)));
        assert_eq!(Size::exact(u16::MAX, 1), None);
        assert_eq!(Size::exact(80, 24), Some(Size::new(80, 24)));
    }
}
