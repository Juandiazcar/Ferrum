//! The cursor, the drawing pen and the state DECSC saves.

use crate::cell::{Attrs, Cell, UNDERLINE_MASK, Underline};
use crate::charset::Charsets;
use crate::color::{Color, EXTRA_HALF_BITS, EXTRA_SHIFT};
use crate::extras::{ExtraId, LinkId};

/// Shape of the cursor (DECSCUSR).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum CursorShape {
    /// A filled cell.
    #[default]
    Block,
    /// A line under the cell.
    Underline,
    /// A vertical bar.
    Bar,
}

/// Shape and blink as DECSCUSR sets them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorStyle {
    /// The shape.
    pub shape: CursorShape,
    /// Whether it blinks.
    pub blinking: bool,
}

/// The style of the next cell: colors, attributes and the hyperlink/underline-color record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pen {
    /// Foreground.
    pub fg: Color,
    /// Background.
    pub bg: Color,
    /// Attribute word (flags and underline style, see `Cell::attr_word`).
    pub attrs: u16,
    /// Hyperlink and underline color, interned from `link` and `underline_color`.
    pub extra: ExtraId,
    /// The active hyperlink (OSC 8).
    pub link: LinkId,
    /// Underline color (SGR 58).
    pub underline_color: Color,
}

impl Pen {
    /// The foreground word with the high half of the extra id.
    pub fn fg_word(&self) -> u32 {
        self.fg.bits() | u32::from(self.extra.bits() >> EXTRA_HALF_BITS) << EXTRA_SHIFT
    }

    /// The background word with the low half of the extra id.
    pub fn bg_word(&self) -> u32 {
        let low = self.extra.bits() & ((1 << EXTRA_HALF_BITS) - 1);
        self.bg.bits() | u32::from(low) << EXTRA_SHIFT
    }

    /// A cell showing `ch` in this style.
    pub fn cell(&self, ch: char) -> Cell {
        let mut cell = Cell::default();
        cell.set_scalar(ch);
        cell.set_style(self.fg_word(), self.bg_word(), self.attrs);
        cell
    }

    /// An erased cell: blank with the pen background (background color erase).
    pub fn blank(&self) -> Cell {
        Cell::blank(self.bg)
    }

    /// True when `attr` is set.
    pub fn has(&self, attr: Attrs) -> bool {
        self.attrs & attr.bits() != 0
    }

    /// Sets one attribute flag.
    pub fn insert_attr(&mut self, attr: Attrs) {
        self.attrs |= attr.bits();
    }

    /// Clears one attribute flag.
    pub fn remove_attr(&mut self, attr: Attrs) {
        self.attrs &= !attr.bits();
    }

    /// The underline style.
    pub fn underline(&self) -> Underline {
        let mut cell = Cell::default();
        cell.set_style(0, 0, self.attrs);
        cell.underline()
    }

    /// Sets the underline style.
    pub fn set_underline(&mut self, underline: Underline) {
        self.attrs = (self.attrs & !UNDERLINE_MASK) | underline.to_bits();
    }
}

/// Cursor position and pending-wrap state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    /// Column.
    pub x: usize,
    /// Row.
    pub y: usize,
    /// The last character filled the line: the next one wraps first (xterm "last column flag").
    pub wrap_pending: bool,
    /// The drawing style.
    pub pen: Pen,
}

/// What DECSC saves and DECRC restores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SavedCursor {
    /// Position, pen and wrap state.
    pub cursor: Cursor,
    /// Origin mode at the time of the save.
    pub origin: bool,
    /// Character sets.
    pub charsets: Charsets,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extras::Extras;

    #[test]
    fn the_extra_id_is_split_over_both_color_words() {
        let mut extras = Extras::new();
        let mut key = String::new();
        let link = extras.intern_link("", "https://x.test", &mut key).unwrap_or_default();
        let mut pen = Pen { fg: Color::indexed(3), bg: Color::rgb(1, 2, 3), ..Pen::default() };
        pen.extra = extras.intern(link, Color::indexed(9));
        let cell = pen.cell('x');
        assert_eq!(cell.extra(), pen.extra);
        assert_eq!((cell.fg(), cell.bg()), (Color::indexed(3), Color::rgb(1, 2, 3)));
    }

    #[test]
    fn erased_cells_take_the_pen_background_only() {
        let pen = Pen {
            fg: Color::indexed(1),
            bg: Color::indexed(4),
            attrs: Attrs::BOLD.bits(),
            ..Pen::default()
        };
        let blank = pen.blank();
        assert_eq!(
            (blank.bg(), blank.fg(), blank.attr_word()),
            (Color::indexed(4), Color::DEFAULT, 0)
        );
    }
}
