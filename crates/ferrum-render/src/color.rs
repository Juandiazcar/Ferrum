//! Colors in the pixel format of the target buffer.

/// An opaque color as the target buffer stores it: `0x00RRGGBB`, the format `softbuffer` uses on
/// every platform (`docs/02` §6). The top byte is always zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgb(u32);

impl Rgb {
    /// Black.
    pub const BLACK: Self = Self(0);

    /// The color of the three channels.
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self(u32::from_be_bytes([0, red, green, blue]))
    }

    /// The color of a `0xRRGGBB` value; the top byte is ignored.
    pub const fn from_pixel(pixel: u32) -> Self {
        Self(pixel & 0x00FF_FFFF)
    }

    /// The pixel value, `0x00RRGGBB`.
    pub const fn pixel(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_land_in_the_softbuffer_layout() {
        assert_eq!(Rgb::new(0x12, 0x34, 0x56).pixel(), 0x0012_3456);
        assert_eq!(Rgb::from_pixel(0xFF12_3456), Rgb::new(0x12, 0x34, 0x56));
    }
}
