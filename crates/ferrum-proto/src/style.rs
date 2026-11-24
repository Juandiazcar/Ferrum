//! Cell styles as they travel in an encoded row: resolved colors, attribute bits and the
//! hyperlink URI itself (ADR 0028: a protocol row never carries a session-local id).

/// Bits of a packed [`Color`] that hold its kind.
const KIND_SHIFT: u32 = 30;
const KIND_DEFAULT: u32 = 0;
const KIND_INDEXED: u32 = 1;
const KIND_RGB: u32 = 2;
/// The payload: a palette index or `r << 16 | g << 8 | b`.
const PAYLOAD_MASK: u32 = 0x00FF_FFFF;
/// Largest palette index payload (256 colors).
const MAX_INDEX: u32 = 0xFF;
const BYTE_MASK: u32 = 0xFF;
const RED_SHIFT: u32 = 16;
const GREEN_SHIFT: u32 = 8;

/// A terminal color, packed in a `u32` with the same layout as `ferrum-vt` colors without their
/// extra-id bits: bits 31-30 are the kind (0 default, 1 indexed, 2 RGB), bits 23-0 the payload,
/// bits 29-24 are zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Color(u32);

/// The decoded form of a [`Color`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ColorKind {
    /// The theme's default foreground or background.
    Default,
    /// An entry of the 256-color palette.
    Indexed(u8),
    /// A 24-bit color.
    Rgb(u8, u8, u8),
}

impl Color {
    /// The default color.
    pub const DEFAULT: Self = Self(KIND_DEFAULT << KIND_SHIFT);

    /// A palette color.
    pub const fn indexed(index: u8) -> Self {
        Self(KIND_INDEXED << KIND_SHIFT | index as u32)
    }

    /// A 24-bit color.
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(
            KIND_RGB << KIND_SHIFT
                | (red as u32) << RED_SHIFT
                | (green as u32) << GREEN_SHIFT
                | blue as u32,
        )
    }

    /// The packed bits.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// The color packed in `bits`, or `None` when they are not a valid color.
    pub const fn from_bits(bits: u32) -> Option<Self> {
        let payload = bits & !(0b11 << KIND_SHIFT);
        let valid = match bits >> KIND_SHIFT {
            KIND_DEFAULT => payload == 0,
            KIND_INDEXED => payload <= MAX_INDEX,
            KIND_RGB => payload <= PAYLOAD_MASK,
            _ => false,
        };
        if valid { Some(Self(bits)) } else { None }
    }

    /// The decoded color.
    #[allow(clippy::cast_possible_truncation, reason = "every channel is masked to 8 bits")]
    pub fn kind(self) -> ColorKind {
        let byte = |shift: u32| ((self.0 >> shift) & BYTE_MASK) as u8;
        match self.0 >> KIND_SHIFT {
            KIND_INDEXED => ColorKind::Indexed(byte(0)),
            KIND_RGB => ColorKind::Rgb(byte(RED_SHIFT), byte(GREEN_SHIFT), byte(0)),
            _ => ColorKind::Default,
        }
    }
}

/// Position and mask of the 3-bit underline style inside [`Attrs`].
const UNDERLINE_SHIFT: u16 = 3;
const UNDERLINE_MASK: u16 = 0b111 << UNDERLINE_SHIFT;
/// Highest defined underline style value ([`Underline::Dashed`]).
const MAX_UNDERLINE: u16 = 5;
/// Every defined attribute bit, underline field included.
const DEFINED_ATTRS: u16 = 0x07FF;

/// Text attribute bits, with the same layout as `ferrum-vt`: bold 0, dim 1, italic 2, underline
/// style 3-5, blink 6, inverse 7, hidden 8, strike 9, overline 10. Bits 11-15 are not defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Attrs(u16);

impl Attrs {
    /// Bold (SGR 1).
    pub const BOLD: Self = Self(1);
    /// Dim (SGR 2).
    pub const DIM: Self = Self(1 << 1);
    /// Italic (SGR 3).
    pub const ITALIC: Self = Self(1 << 2);
    /// Blinking (SGR 5, 6).
    pub const BLINK: Self = Self(1 << 6);
    /// Reverse video (SGR 7).
    pub const INVERSE: Self = Self(1 << 7);
    /// Hidden (SGR 8).
    pub const HIDDEN: Self = Self(1 << 8);
    /// Strikethrough (SGR 9).
    pub const STRIKE: Self = Self(1 << 9);
    /// Overline (SGR 53).
    pub const OVERLINE: Self = Self(1 << 10);

    /// No attribute set.
    pub const NONE: Self = Self(0);

    /// The attributes in `bits`, or `None` when undefined bits or an undefined underline style
    /// are set.
    pub const fn from_bits(bits: u16) -> Option<Self> {
        let underline = (bits & UNDERLINE_MASK) >> UNDERLINE_SHIFT;
        if bits & !DEFINED_ATTRS == 0 && underline <= MAX_UNDERLINE {
            Some(Self(bits))
        } else {
            None
        }
    }

    /// The raw bits.
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Whether every bit of `other` is set.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// These attributes plus `other`.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// These attributes with the underline style set to `underline`.
    #[must_use]
    pub const fn with_underline(self, underline: Underline) -> Self {
        Self(self.0 & !UNDERLINE_MASK | (underline as u16) << UNDERLINE_SHIFT)
    }

    /// The underline style.
    pub const fn underline(self) -> Underline {
        match (self.0 & UNDERLINE_MASK) >> UNDERLINE_SHIFT {
            1 => Underline::Single,
            2 => Underline::Double,
            3 => Underline::Curly,
            4 => Underline::Dotted,
            5 => Underline::Dashed,
            _ => Underline::None,
        }
    }
}

/// How a cell is underlined (the 3-bit field of [`Attrs`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Underline {
    /// Not underlined.
    #[default]
    None = 0,
    /// A single line.
    Single = 1,
    /// Two lines.
    Double = 2,
    /// A wavy line.
    Curly = 3,
    /// A dotted line.
    Dotted = 4,
    /// A dashed line.
    Dashed = 5,
}

/// The style of a run of cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style<'a> {
    /// Attribute bits.
    pub attrs: Attrs,
    /// Foreground.
    pub fg: Color,
    /// Background.
    pub bg: Color,
    /// Underline color; [`Color::DEFAULT`] follows the foreground.
    pub underline_color: Color,
    /// The hyperlink URI (OSC 8), resolved by the daemon.
    pub link: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_round_trip_through_their_bits() {
        for color in [Color::DEFAULT, Color::indexed(200), Color::rgb(1, 2, 3)] {
            assert_eq!(Color::from_bits(color.bits()), Some(color));
        }
        assert_eq!(Color::rgb(1, 2, 3).kind(), ColorKind::Rgb(1, 2, 3));
        assert_eq!(Color::indexed(7).kind(), ColorKind::Indexed(7));
    }

    #[test]
    fn invalid_color_bits_are_refused() {
        assert_eq!(Color::from_bits(3 << KIND_SHIFT), None);
        assert_eq!(Color::from_bits(KIND_INDEXED << KIND_SHIFT | 0x100), None);
        assert_eq!(Color::from_bits(1), None);
        assert_eq!(Color::from_bits(KIND_RGB << KIND_SHIFT | 1 << 24), None);
    }

    #[test]
    fn attrs_keep_the_underline_field() {
        let attrs = Attrs::BOLD.with_underline(Underline::Curly);
        assert_eq!(attrs.underline(), Underline::Curly);
        assert!(attrs.contains(Attrs::BOLD));
        assert_eq!(Attrs::from_bits(attrs.bits()), Some(attrs));
        assert_eq!(Attrs::from_bits(6 << UNDERLINE_SHIFT), None);
        assert_eq!(Attrs::from_bits(1 << 11), None);
    }
}
