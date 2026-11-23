//! One screen (primary or alternate): its grid, cursor and per-screen state.

use crate::charset::Charsets;
use crate::cursor::{Cursor, SavedCursor};
use crate::grid::Grid;
use crate::keyboard::KeyboardStack;
use crate::size::Size;

/// The scroll region, rows `top..=bottom` (DECSTBM). `top <= bottom` always holds: the fields
/// are private and [`Region::new`] checks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    top: usize,
    bottom: usize,
}

impl Region {
    /// The region of rows `top..=bottom`; `None` when it is empty or inverted.
    pub const fn new(top: usize, bottom: usize) -> Option<Self> {
        if top <= bottom { Some(Self { top, bottom }) } else { None }
    }

    /// The region covering `lines` rows.
    pub fn full(lines: usize) -> Self {
        Self { top: 0, bottom: lines.saturating_sub(1) }
    }

    /// First row of the region.
    pub const fn top(&self) -> usize {
        self.top
    }

    /// Last row of the region.
    pub const fn bottom(&self) -> usize {
        self.bottom
    }

    /// True when `y` is inside the region.
    pub fn contains(&self, y: usize) -> bool {
        (self.top..=self.bottom).contains(&y)
    }

    /// Rows in the region.
    pub fn height(&self) -> usize {
        self.bottom.saturating_sub(self.top).saturating_add(1)
    }
}

/// A grid with its cursor, saved cursor, scroll region and character sets.
#[derive(Debug)]
pub struct Screen {
    pub(crate) grid: Grid,
    pub(crate) cursor: Cursor,
    pub(crate) saved: Option<SavedCursor>,
    pub(crate) region: Region,
    pub(crate) charsets: Charsets,
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
            charsets: Charsets::default(),
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
        assert_eq!((region.top(), region.bottom(), region.height()), (0, 4, 5));
        assert!(region.contains(4));
        assert!(!region.contains(5));
    }

    #[test]
    fn an_inverted_region_cannot_be_built_and_the_height_never_overflows() {
        assert_eq!(Region::new(3, 2), None);
        let widest = Region::new(0, usize::MAX).map(|region| region.height());
        assert_eq!(widest, Some(usize::MAX));
    }
}
