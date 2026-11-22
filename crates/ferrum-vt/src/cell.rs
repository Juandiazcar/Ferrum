//! The 16-byte grid cell (`docs/02` §3.1).

use bitflags::bitflags;

use crate::color::{Color, EXTRA_HALF_BITS, EXTRA_MASK, EXTRA_SHIFT};
use crate::extras::ExtraId;

bitflags! {
    /// Text attributes of a cell. The three underline bits are not flags; use
    /// [`Cell::underline`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Attrs: u16 {
        /// Bold (SGR 1).
        const BOLD = 1;
        /// Dim (SGR 2).
        const DIM = 1 << 1;
        /// Italic (SGR 3).
        const ITALIC = 1 << 2;
        /// Blinking text (SGR 5, 6).
        const BLINK = 1 << 6;
        /// Reverse video (SGR 7).
        const INVERSE = 1 << 7;
        /// Hidden text (SGR 8).
        const HIDDEN = 1 << 8;
        /// Strikethrough (SGR 9).
        const STRIKE = 1 << 9;
        /// Overline (SGR 53).
        const OVERLINE = 1 << 10;
        /// Protected from selective erase (DECSCA).
        const PROTECTED = 1 << 11;
    }
}

/// Position of the 3-bit underline style inside the attribute bits.
const UNDERLINE_SHIFT: u16 = 3;
pub(crate) const UNDERLINE_MASK: u16 = 0b111 << UNDERLINE_SHIFT;

/// How a cell is underlined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Underline {
    /// Not underlined.
    #[default]
    None,
    /// A single line (SGR 4, 4:1).
    Single,
    /// Two lines (SGR 21, 4:2).
    Double,
    /// A wavy line (SGR 4:3).
    Curly,
    /// A dotted line (SGR 4:4).
    Dotted,
    /// A dashed line (SGR 4:5).
    Dashed,
}

impl Underline {
    /// The 3 bits placed where the attribute word keeps them.
    pub(crate) const fn to_bits(self) -> u16 {
        (self as u16) << UNDERLINE_SHIFT
    }

    const fn from_bits(bits: u16) -> Self {
        match (bits & UNDERLINE_MASK) >> UNDERLINE_SHIFT {
            1 => Self::Single,
            2 => Self::Double,
            3 => Self::Curly,
            4 => Self::Dotted,
            5 => Self::Dashed,
            _ => Self::None,
        }
    }
}

bitflags! {
    /// Structural flags of a cell.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct CellFlags: u16 {
        /// First half of a double-width character.
        const WIDE = 1;
        /// Second half of a double-width character; its `ch` is meaningless.
        const SPACER = 1 << 1;
        /// Blank at the end of a row left by a double-width character that wrapped.
        const LEADING_SPACER = 1 << 2;
        /// `ch` is an index into the grapheme table, not a scalar.
        const GRAPHEME = 1 << 3;
    }
}

/// One terminal cell: 16 bytes, as `docs/02` §3.1 says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Cell {
    ch: u32,
    fg: u32,
    bg: u32,
    attrs: u16,
    flags: u16,
}

const _: () = assert!(size_of::<Cell>() == 16);

/// The scalar of a blank cell.
pub const BLANK_CHAR: char = ' ';

impl Default for Cell {
    fn default() -> Self {
        Self::blank(Color::DEFAULT)
    }
}

impl Cell {
    /// A blank cell with background `bg` (background color erase).
    pub const fn blank(bg: Color) -> Self {
        Self { ch: BLANK_CHAR as u32, fg: 0, bg: bg.bits(), attrs: 0, flags: 0 }
    }

    /// A cell showing `ch` with the given style words (extras included).
    pub(crate) const fn styled(ch: char, fg: u32, bg: u32, attrs: u16) -> Self {
        Self { ch: ch as u32, fg, bg, attrs, flags: 0 }
    }

    /// The scalar, or the grapheme table index when [`CellFlags::GRAPHEME`] is set.
    pub const fn raw_char(&self) -> u32 {
        self.ch
    }

    /// The scalar of a plain cell; [`BLANK_CHAR`] for spacers and grapheme cells.
    pub fn scalar(&self) -> char {
        if self.flags().intersects(CellFlags::GRAPHEME.union(CellFlags::SPACER)) {
            return BLANK_CHAR;
        }
        char::from_u32(self.ch).unwrap_or(BLANK_CHAR)
    }

    /// Sets a plain scalar, clearing any grapheme or spacer state.
    pub fn set_scalar(&mut self, ch: char) {
        self.ch = ch as u32;
        self.flags &= !(CellFlags::GRAPHEME | CellFlags::SPACER | CellFlags::LEADING_SPACER).bits();
    }

    /// Replaces the raw character word (a scalar, or a grapheme index for `GRAPHEME` cells).
    pub(crate) fn set_raw_char(&mut self, raw: u32) {
        self.ch = raw;
    }

    /// Structural flags.
    pub const fn flags(&self) -> CellFlags {
        CellFlags::from_bits_retain(self.flags)
    }

    /// Sets or clears structural flags.
    pub fn set_flags(&mut self, flags: CellFlags, on: bool) {
        if on {
            self.flags |= flags.bits();
        } else {
            self.flags &= !flags.bits();
        }
    }

    /// Foreground color.
    pub const fn fg(&self) -> Color {
        Color::from_raw(self.fg)
    }

    /// Background color.
    pub const fn bg(&self) -> Color {
        Color::from_raw(self.bg)
    }

    /// Foreground word with the extra bits, as the scrollback `Style` stores it.
    pub const fn fg_word(&self) -> u32 {
        self.fg
    }

    /// Background word with the extra bits, as the scrollback `Style` stores it.
    pub const fn bg_word(&self) -> u32 {
        self.bg
    }

    /// Attribute bits as the scrollback `Style` stores them (underline style included).
    pub const fn attr_word(&self) -> u16 {
        self.attrs
    }

    /// Text attributes without the underline style.
    pub const fn attrs(&self) -> Attrs {
        Attrs::from_bits_truncate(self.attrs)
    }

    /// The underline style.
    pub const fn underline(&self) -> Underline {
        Underline::from_bits(self.attrs)
    }

    /// The hyperlink and underline-color record of this cell, `ExtraId::NONE` when there is none.
    pub const fn extra(&self) -> ExtraId {
        let high = (self.fg & EXTRA_MASK) >> EXTRA_SHIFT;
        let low = (self.bg & EXTRA_MASK) >> EXTRA_SHIFT;
        // INVARIANT: each half is at most 6 bits, so the 12-bit result fits a u16.
        #[allow(clippy::cast_possible_truncation)]
        let bits = (high << EXTRA_HALF_BITS | low) as u16;
        ExtraId::from_bits(bits)
    }

    /// Stores the extra id in the spare color bits.
    pub fn set_extra(&mut self, id: ExtraId) {
        let bits = u32::from(id.bits());
        let low_mask = (1 << EXTRA_HALF_BITS) - 1;
        self.fg = (self.fg & !EXTRA_MASK) | (bits >> EXTRA_HALF_BITS) << EXTRA_SHIFT;
        self.bg = (self.bg & !EXTRA_MASK) | (bits & low_mask) << EXTRA_SHIFT;
    }

    /// Overwrites the style of the cell, keeping its character and structure.
    pub fn set_style(&mut self, fg: u32, bg: u32, attrs: u16) {
        self.fg = fg;
        self.bg = bg;
        self.attrs = attrs;
    }

    /// True for a plain blank in the default style: what the scrollback trims.
    pub fn is_default_blank(&self) -> bool {
        self.ch == BLANK_CHAR as u32
            && self.fg == 0
            && self.bg == 0
            && self.attrs == 0
            && self.flags == 0
    }

    /// Display columns the cell occupies: 2 for the first half of a wide character, 0 for its
    /// spacers, 1 otherwise.
    pub const fn columns(&self) -> usize {
        let flags = self.flags();
        if flags.contains(CellFlags::WIDE) {
            2
        } else if flags.intersects(CellFlags::SPACER.union(CellFlags::LEADING_SPACER)) {
            0
        } else {
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_is_sixteen_bytes() {
        assert_eq!(size_of::<Cell>(), 16);
    }

    #[test]
    fn blank_cells_are_default_blank_until_styled() {
        let mut cell = Cell::default();
        assert!(cell.is_default_blank());
        cell.set_style(Color::indexed(1).bits(), 0, 0);
        assert!(!cell.is_default_blank());
        assert!(!Cell::blank(Color::indexed(4)).is_default_blank());
    }

    #[test]
    fn underline_style_lives_beside_the_attribute_flags() {
        let mut cell = Cell::default();
        cell.set_style(0, 0, Attrs::BOLD.bits() | (3 << UNDERLINE_SHIFT));
        assert_eq!(cell.underline(), Underline::Curly);
        assert_eq!(cell.attrs(), Attrs::BOLD);
    }

    #[test]
    fn wide_and_spacer_cells_count_columns() {
        let mut wide = Cell::default();
        wide.set_flags(CellFlags::WIDE, true);
        let mut spacer = Cell::default();
        spacer.set_flags(CellFlags::SPACER, true);
        assert_eq!((Cell::default().columns(), wide.columns(), spacer.columns()), (1, 2, 0));
    }

    #[test]
    fn setting_a_scalar_clears_grapheme_state() {
        let mut cell = Cell::default();
        cell.set_flags(CellFlags::GRAPHEME, true);
        cell.set_scalar('x');
        assert_eq!((cell.scalar(), cell.flags()), ('x', CellFlags::empty()));
    }
}
