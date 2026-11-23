//! Packed cell colors.
//!
//! A color is a `u32`: bits 31-30 are the kind (0 default, 1 indexed, 2 RGB), bits 23-0 the
//! payload (palette index, or `r << 16 | g << 8 | b`). Bits 29-24 are not part of the color: the
//! cell keeps 6 bits of its [`ExtraId`](crate::ExtraId) in each of `fg` and `bg` there, so that a
//! scrollback `Style` of `(fg, bg, attrs)` carries hyperlinks and underline colors without a
//! conversion (`docs/02` §3.1).

const KIND_SHIFT: u32 = 30;
const KIND_DEFAULT: u32 = 0;
const KIND_INDEXED: u32 = 1;
const KIND_RGB: u32 = 2;
const PAYLOAD_MASK: u32 = 0x00FF_FFFF;
/// One 8-bit channel (or palette index) of the payload, and where red and green sit.
const BYTE_MASK: u32 = 0xFF;
const RED_SHIFT: u32 = 16;
const GREEN_SHIFT: u32 = 8;
/// Mask of the 6 spare bits in a packed color.
pub(crate) const EXTRA_MASK: u32 = 0x3F00_0000;
/// Position of the spare bits.
pub(crate) const EXTRA_SHIFT: u32 = 24;
/// Bits of the extra id kept in each of `fg` and `bg`.
pub(crate) const EXTRA_HALF_BITS: u32 = 6;
/// Mask of the bits that make up the color itself.
const COLOR_MASK: u32 = !EXTRA_MASK;

/// A color word without the extra-id bits of the cell it came from.
pub(crate) const fn without_extra(word: u32) -> u32 {
    word & COLOR_MASK
}

/// A terminal color.
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
    #[allow(clippy::cast_lossless, reason = "const fn: `u32::from` cannot be used here")]
    pub const fn indexed(index: u8) -> Self {
        Self(KIND_INDEXED << KIND_SHIFT | index as u32)
    }

    /// A 24-bit color.
    #[allow(clippy::cast_lossless, reason = "const fn: `u32::from` cannot be used here")]
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(
            KIND_RGB << KIND_SHIFT
                | (red as u32) << RED_SHIFT
                | (green as u32) << GREEN_SHIFT
                | blue as u32,
        )
    }

    /// The packed bits without any cell extra bits.
    pub const fn bits(self) -> u32 {
        self.0 & COLOR_MASK
    }

    /// The color held in `raw` (a packed value that may carry extra bits).
    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(raw & COLOR_MASK)
    }

    /// The decoded color.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "every cast keeps one byte that was masked or shifted into place first"
    )]
    pub const fn kind(self) -> ColorKind {
        match self.0 >> KIND_SHIFT {
            KIND_INDEXED => ColorKind::Indexed((self.0 & BYTE_MASK) as u8),
            KIND_RGB => {
                let payload = self.0 & PAYLOAD_MASK;
                ColorKind::Rgb(
                    (payload >> RED_SHIFT) as u8,
                    (payload >> GREEN_SHIFT) as u8,
                    payload as u8,
                )
            }
            _ => ColorKind::Default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_round_trip_through_their_kind() {
        assert_eq!(Color::DEFAULT.kind(), ColorKind::Default);
        assert_eq!(Color::indexed(200).kind(), ColorKind::Indexed(200));
        assert_eq!(Color::rgb(1, 2, 255).kind(), ColorKind::Rgb(1, 2, 255));
    }

    #[test]
    fn extra_bits_are_not_part_of_the_color() {
        let raw = Color::rgb(9, 8, 7).bits() | EXTRA_MASK;
        assert_eq!(Color::from_raw(raw), Color::rgb(9, 8, 7));
    }
}
