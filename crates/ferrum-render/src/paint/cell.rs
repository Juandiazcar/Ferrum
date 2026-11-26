//! What the painter draws: rows of cells in plain values. The UI converts the rows it holds
//! (decoded `ferrum-proto` rows) into these, with colors already resolved (palette, inverse,
//! dim, hidden), so the renderer needs no terminal type (`docs/04` §2).

use std::hash::{Hash, Hasher};

use crate::color::Rgb;
use crate::font::TextStyle;
use crate::synth::Underline;

/// The text of a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CellText {
    /// Nothing drawn but the background.
    #[default]
    Empty,
    /// One character.
    Char(char),
    /// A grapheme cluster of several characters: `len` bytes from `start` in the row's
    /// [`Row::clusters`] text.
    Cluster {
        /// Byte offset in [`Row::clusters`].
        start: u32,
        /// Byte length.
        len: u32,
    },
}

/// How many columns a cell's text takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CellWidth {
    /// One column.
    #[default]
    Single,
    /// The first column of a wide (two-column) character.
    Wide,
    /// The second column of a wide character: its background only.
    Spacer,
}

/// Lines drawn through or over a cell, besides the underline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Strokes(u8);

impl Strokes {
    /// None.
    pub const NONE: Self = Self(0);
    /// Strikethrough (SGR 9).
    pub const STRIKEOUT: Self = Self(1);
    /// Overline (SGR 53).
    pub const OVERLINE: Self = Self(2);

    /// These strokes plus `other`.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether every stroke of `other` is set.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// One cell, colors resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// Its text.
    pub text: CellText,
    /// Text color.
    pub fg: Rgb,
    /// Background color.
    pub bg: Rgb,
    /// Underline color (the caller passes `fg` when the terminal sets none).
    pub underline_color: Rgb,
    /// Weight and slant.
    pub style: TextStyle,
    /// Underline style.
    pub underline: Underline,
    /// Strikethrough and overline.
    pub strokes: Strokes,
    /// Columns taken.
    pub width: CellWidth,
}

impl Cell {
    /// An empty cell of background `bg`.
    pub const fn blank(fg: Rgb, bg: Rgb) -> Self {
        Self {
            text: CellText::Empty,
            fg,
            bg,
            underline_color: fg,
            style: TextStyle::REGULAR,
            underline: Underline::None,
            strokes: Strokes::NONE,
            width: CellWidth::Single,
        }
    }
}

impl Hash for Cell {
    /// Three words per cell instead of one write per field: rows are hashed every frame.
    // perf: `render_repaint/damage_4_rows` hashes 12 000 cells per frame.
    fn hash<H: Hasher>(&self, state: &mut H) {
        let text = match self.text {
            CellText::Empty => 0,
            CellText::Char(ch) => 1 << 32 | u64::from(ch),
            CellText::Cluster { start, len } => 2 << 62 | u64::from(len) << 32 | u64::from(start),
        };
        let colors = u64::from(self.fg.pixel()) << 32 | u64::from(self.bg.pixel());
        let underline = self.underline as u64;
        let slant = self.style.slant as u64;
        let width = self.width as u64;
        let style = u64::from(self.underline_color.pixel())
            | u64::from(self.style.weight) << 24
            | slant << 40
            | underline << 41
            | u64::from(self.strokes.0) << 44
            | width << 46;
        state.write_u64(text);
        state.write_u64(colors);
        state.write_u64(style);
    }
}

/// One row: its cells and the text of its clusters.
#[derive(Debug, Clone, Copy)]
pub struct Row<'a> {
    /// The cells, left to right; a row shorter than the grid is padded with the grid's
    /// background.
    pub cells: &'a [Cell],
    /// The characters of the row's [`CellText::Cluster`] cells.
    pub clusters: &'a str,
}

impl<'a> Row<'a> {
    /// The text of a cluster cell, if it is inside `clusters` on character boundaries.
    pub(crate) fn cluster(&self, start: u32, len: u32) -> Option<&'a str> {
        let start = usize::try_from(start).ok()?;
        let end = start.checked_add(usize::try_from(len).ok()?)?;
        self.clusters.get(start..end)
    }
}

/// Where the painter reads rows from: the UI's viewport cache.
pub trait Rows {
    /// Row `y` of the viewport, or `None` past its end (drawn empty).
    fn row(&self, y: u16) -> Option<Row<'_>>;
}

impl Rows for [Row<'_>] {
    fn row(&self, y: u16) -> Option<Row<'_>> {
        self.get(usize::from(y)).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clusters_are_read_on_character_boundaries_only() {
        let row = Row { cells: &[], clusters: "e\u{301}👍🏽" };
        assert_eq!(row.cluster(0, 3), Some("e\u{301}"));
        assert_eq!(row.cluster(3, 8), Some("👍🏽"));
        assert_eq!(row.cluster(1, 1), None, "inside a character");
        assert_eq!(row.cluster(3, 100), None);
    }

    #[test]
    fn cells_that_differ_hash_differently() {
        use std::hash::BuildHasher;
        let hash = |cell: &Cell| rustc_hash::FxBuildHasher.hash_one(cell);
        let base = Cell { text: CellText::Char('a'), ..Cell::blank(Rgb::new(1, 2, 3), Rgb::BLACK) };
        let variants = [
            Cell { text: CellText::Char('b'), ..base },
            Cell { text: CellText::Cluster { start: 0, len: 97 }, ..base },
            Cell { fg: Rgb::new(1, 2, 4), ..base },
            Cell { bg: Rgb::new(0, 0, 1), ..base },
            Cell { underline_color: Rgb::BLACK, ..base },
            Cell { style: TextStyle { weight: 700, ..TextStyle::REGULAR }, ..base },
            Cell {
                style: TextStyle { slant: crate::font::Slant::Italic, ..TextStyle::REGULAR },
                ..base
            },
            Cell { underline: Underline::Dashed, ..base },
            Cell { strokes: Strokes::OVERLINE, ..base },
            Cell { width: CellWidth::Spacer, ..base },
        ];
        for variant in variants {
            assert_ne!(hash(&variant), hash(&base), "{variant:?}");
        }
        assert_eq!(hash(&base), hash(&{ base }));
    }

    #[test]
    fn strokes_combine() {
        let both = Strokes::STRIKEOUT.with(Strokes::OVERLINE);
        assert!(both.contains(Strokes::OVERLINE) && both.contains(Strokes::STRIKEOUT));
        assert!(!Strokes::NONE.contains(Strokes::STRIKEOUT));
    }
}
