//! One screen (primary or alternate): its grid, cursor and per-screen state.

use crate::cursor::{Cursor, SavedCursor};
use crate::grid::Grid;
use crate::keyboard::KeyboardStack;
use crate::size::Size;

/// The scroll region, rows `top..=bottom` (DECSTBM).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    /// First row of the region.
    pub top: usize,
    /// Last row of the region.
    pub bottom: usize,
}

impl Region {
    /// The region covering `lines` rows.
    pub fn full(lines: usize) -> Self {
        Self { top: 0, bottom: lines.saturating_sub(1) }
    }

    /// True when `y` is inside the region.
    pub fn contains(&self, y: usize) -> bool {
        (self.top..=self.bottom).contains(&y)
    }

    /// Rows in the region.
    pub fn height(&self) -> usize {
        self.bottom + 1 - self.top
    }
}

/// A grid with its cursor, saved cursor, scroll region and character sets.
#[derive(Debug)]
pub struct Screen {
    pub(crate) grid: Grid,
    pub(crate) cursor: Cursor,
    pub(crate) saved: Option<SavedCursor>,
    pub(crate) region: Region,
    pub(crate) keyboard: KeyboardStack,
}

impl Screen {
    /// A blank screen of `size`.
    pub fn new(size: Size) -> Self {
        Self {
            grid: Grid::new(size),
            cursor: Cursor::default(),
            saved: None,
            region: Region::full(size.rows()),
            keyboard: KeyboardStack::default(),
        }
    }

    /// The grid.
    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    /// The cursor.
    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    /// The scroll region.
    pub fn region(&self) -> Region {
        self.region
    }

    /// The kitty keyboard protocol stack of this screen.
    pub fn keyboard(&self) -> &KeyboardStack {
        &self.keyboard
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_region_spans_every_row() {
        let region = Region::full(5);
        assert_eq!((region.top, region.bottom, region.height()), (0, 4, 5));
        assert!(region.contains(4));
        assert!(!region.contains(5));
    }
}
