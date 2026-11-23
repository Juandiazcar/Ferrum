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
            bits if bits == Self::Single as u16 => Self::Single,
            bits if bits == Self::Double as u16 => Self::Double,
            bits if bits == Self::Curly as u16 => Self::Curly,
            bits if bits == Self::Dotted as u16 => Self::Dotted,
            bits if bits == Self::Dashed as u16 => Self::Dashed,
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

/// Bits of a scalar that are set in ASCII.
const ASCII_MASK: u64 = 0x7F;
/// Bits of the first word: the character (low 32), the attributes (next 16), the flags (top 16).
const CH_MASK: u64 = 0xFFFF_FFFF;
const ATTRS_SHIFT: u32 = 32;
/// A 16-bit field (attributes, flags) before it is shifted into its place.
const FIELD16_MASK: u64 = 0xFFFF;
const FLAGS_SHIFT: u32 = 48;
/// Bits of the second word: the foreground (low 32), the background (high 32).
const BG_SHIFT: u32 = 32;

/// The style words of a plain ASCII cell (see [`Cell::differs_from`]).
#[derive(Debug, Clone, Copy)]
pub(crate) struct AsciiStyle {
    /// The first word of a cell of this style with the character and the flags at zero.
    lo: u64,
    hi: u64,
}

/// A 16-bit field of a word.
#[allow(clippy::cast_possible_truncation, reason = "the value is shifted and masked to 16 bits")]
const fn field16(word: u64, shift: u32) -> u16 {
    (word >> shift) as u16
}

/// The low 32 bits of a word.
#[allow(clippy::cast_possible_truncation, reason = "the value is masked to 32 bits")]
const fn low32(word: u64) -> u32 {
    (word & CH_MASK) as u32
}

/// Joins two 32-bit halves.
const fn join32(low: u32, high: u32) -> u64 {
    low as u64 | (high as u64) << BG_SHIFT
}

/// One terminal cell: 16 bytes, as `docs/02` §3.1 says.
///
/// perf: the cell is two machine words, so that printing a character or blanking a row is one
/// 16-byte store pair per cell (bench `vt_parse/ascii_8mib`). `lo` holds the character, the
/// attribute bits and the structural flags; `hi` the foreground and the background words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Cell {
    lo: u64,
    hi: u64,
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
    // perf: the five words of a cell as plain arguments, so the constructors compile to a few
    // shifts and ors (bench vt_parse/ascii_8mib).
    const fn pack(ch: u32, fg: u32, bg: u32, attrs: u16, flags: u16) -> Self {
        let lo = ch as u64 | (attrs as u64) << ATTRS_SHIFT | (flags as u64) << FLAGS_SHIFT;
        Self { lo, hi: join32(fg, bg) }
    }

    /// A blank cell with background `bg` (background color erase).
    pub const fn blank(bg: Color) -> Self {
        Self::pack(BLANK_CHAR as u32, 0, bg.bits(), 0, 0)
    }

    /// A cell showing `ch` with the given style words (extras included).
    pub(crate) const fn styled(ch: char, fg: u32, bg: u32, attrs: u16) -> Self {
        Self::pack(ch as u32, fg, bg, attrs, 0)
    }

    /// The scalar, or the grapheme table index when [`CellFlags::GRAPHEME`] is set.
    pub const fn raw_char(&self) -> u32 {
        low32(self.lo)
    }

    /// The scalar of a plain cell; [`BLANK_CHAR`] for spacers and grapheme cells.
    pub fn scalar(&self) -> char {
        if self.flags().intersects(CellFlags::GRAPHEME.union(CellFlags::SPACER)) {
            return BLANK_CHAR;
        }
        char::from_u32(self.raw_char()).unwrap_or(BLANK_CHAR)
    }

    /// Sets a plain scalar, clearing any grapheme or spacer state.
    pub fn set_scalar(&mut self, ch: char) {
        self.set_raw_char(ch as u32);
        self.remove_flags(CellFlags::GRAPHEME | CellFlags::SPACER | CellFlags::LEADING_SPACER);
    }

    /// Replaces the raw character word (a scalar, or a grapheme index for `GRAPHEME` cells).
    pub(crate) fn set_raw_char(&mut self, raw: u32) {
        self.lo = (self.lo & !CH_MASK) | u64::from(raw);
    }

    /// The low byte of the character word: the character of a plain ASCII cell.
    #[allow(clippy::cast_possible_truncation, reason = "callers check that the word is below 0x80")]
    pub(crate) const fn ascii_byte(&self) -> u8 {
        self.lo as u8
    }

    /// Zero when this cell is a plain ASCII cell (no flags) with the same style as `key`; nonzero
    /// otherwise. Branch-free so that a run of cells is checked with one test per block.
    pub(crate) const fn differs_from(&self, key: &AsciiStyle) -> u64 {
        // The character bits above ASCII, the flags and the attributes must equal the key's
        // (zero, zero and its attributes).
        ((self.lo & !ASCII_MASK) ^ key.lo) | (self.hi ^ key.hi)
    }

    /// The style of a plain ASCII cell, to find the cells that share it.
    pub(crate) const fn ascii_style(&self) -> AsciiStyle {
        AsciiStyle { lo: self.lo & !CH_MASK, hi: self.hi }
    }

    /// True for a cell without flags whose scalar is ASCII.
    pub(crate) const fn is_plain_ascii(&self) -> bool {
        self.lo & !(ASCII_MASK | (FIELD16_MASK << ATTRS_SHIFT)) == 0
    }

    /// Structural flags.
    pub const fn flags(&self) -> CellFlags {
        CellFlags::from_bits_retain(field16(self.lo, FLAGS_SHIFT))
    }

    /// Sets structural flags.
    pub fn insert_flags(&mut self, flags: CellFlags) {
        self.lo |= u64::from(flags.bits()) << FLAGS_SHIFT;
    }

    /// Clears structural flags.
    pub fn remove_flags(&mut self, flags: CellFlags) {
        self.lo &= !(u64::from(flags.bits()) << FLAGS_SHIFT);
    }

    /// Foreground color.
    pub const fn fg(&self) -> Color {
        Color::from_raw(self.fg_word())
    }

    /// Background color.
    pub const fn bg(&self) -> Color {
        Color::from_raw(self.bg_word())
    }

    /// Foreground word with the extra bits, as the scrollback `Style` stores it.
    pub const fn fg_word(&self) -> u32 {
        low32(self.hi)
    }

    /// Background word with the extra bits, as the scrollback `Style` stores it.
    pub const fn bg_word(&self) -> u32 {
        low32(self.hi >> BG_SHIFT)
    }

    /// Attribute bits as the scrollback `Style` stores them (underline style included).
    pub const fn attr_word(&self) -> u16 {
        field16(self.lo, ATTRS_SHIFT)
    }

    /// Text attributes without the underline style.
    pub const fn attrs(&self) -> Attrs {
        Attrs::from_bits_truncate(self.attr_word())
    }

    /// The underline style.
    pub const fn underline(&self) -> Underline {
        Underline::from_bits(self.attr_word())
    }

    /// The hyperlink and underline-color record of this cell, `ExtraId::NONE` when there is none.
    pub const fn extra(&self) -> ExtraId {
        extra_of(self.fg_word(), self.bg_word())
    }

    /// Stores the extra id in the spare color bits.
    pub fn set_extra(&mut self, id: ExtraId) {
        let bits = u32::from(id.bits());
        let low_mask = (1 << EXTRA_HALF_BITS) - 1;
        let fg = (self.fg_word() & !EXTRA_MASK) | (bits >> EXTRA_HALF_BITS) << EXTRA_SHIFT;
        let bg = (self.bg_word() & !EXTRA_MASK) | (bits & low_mask) << EXTRA_SHIFT;
        self.hi = join32(fg, bg);
    }

    /// Overwrites the style of the cell, keeping its character and structure.
    pub fn set_style(&mut self, fg: u32, bg: u32, attrs: u16) {
        self.hi = join32(fg, bg);
        self.lo = (self.lo & !(FIELD16_MASK << ATTRS_SHIFT)) | u64::from(attrs) << ATTRS_SHIFT;
    }

    /// True for a plain blank in the default style: what the scrollback trims.
    pub fn is_default_blank(&self) -> bool {
        self.lo == BLANK_CHAR as u64 && self.hi == 0
    }

    /// Display columns the cell occupies: 2 for the first half of a wide character, 0 for its
    /// spacers, 1 otherwise.
    pub const fn columns(&self) -> usize {
        let flags = self.flags();
        if flags.contains(CellFlags::WIDE) {
            crate::width::WIDE_COLUMNS
        } else if flags.intersects(CellFlags::SPACER.union(CellFlags::LEADING_SPACER)) {
            0
        } else {
            1
        }
    }
}

/// The extra id carried by a pair of color words (a cell's, or a scrollback `Style`'s).
pub(crate) const fn extra_of(fg: u32, bg: u32) -> ExtraId {
    let high = (fg & EXTRA_MASK) >> EXTRA_SHIFT;
    let low = (bg & EXTRA_MASK) >> EXTRA_SHIFT;
    // INVARIANT: each half is at most 6 bits, so the 12-bit result fits a u16.
    #[allow(clippy::cast_possible_truncation, reason = "12 bits, see the invariant")]
    let bits = (high << EXTRA_HALF_BITS | low) as u16;
    ExtraId::from_bits(bits)
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
        wide.insert_flags(CellFlags::WIDE);
        let mut spacer = Cell::default();
        spacer.insert_flags(CellFlags::SPACER);
        assert_eq!((Cell::default().columns(), wide.columns(), spacer.columns()), (1, 2, 0));
    }

    #[test]
    fn setting_a_scalar_clears_grapheme_state() {
        let mut cell = Cell::default();
        cell.insert_flags(CellFlags::GRAPHEME);
        cell.set_scalar('x');
        assert_eq!((cell.scalar(), cell.flags()), ('x', CellFlags::empty()));
    }
}
