//! The color palette and the default colors (OSC 4, 10, 11, 12 and their resets).
//!
//! The UI gives the terminal its theme with [`Palette::with_theme`]; programs may then change
//! entries at run time and reset them back to the theme.

use crate::color::{Color, ColorKind};

/// A 24-bit color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgb {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

impl Rgb {
    /// A color from its channels.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// Entries of the palette.
pub const PALETTE_SIZE: usize = 256;
/// Entries of the ANSI part (normal and bright colors).
pub const ANSI_COLORS: usize = 16;
/// First entry of the 6x6x6 color cube.
const CUBE_START: usize = 16;
/// First entry of the grayscale ramp.
const GRAY_START: usize = 232;
/// Levels per channel of the color cube, and entries of one blue-green plane.
const CUBE_SIDE: usize = 6;
const CUBE_PLANE: usize = CUBE_SIDE * CUBE_SIDE;
/// Channel levels of the color cube.
const CUBE_LEVELS: [u8; CUBE_SIDE] = [0, 95, 135, 175, 215, 255];
/// First gray level and step of the ramp.
const GRAY_BASE: u8 = 8;
const GRAY_STEP: u8 = 10;

/// Channel values of xterm's default ANSI colors.
const NORMAL: u8 = 205;
const NORMAL_BLUE: u8 = 238;
const LIGHT_GRAY: u8 = 229;
const GRAY: u8 = 127;
const BRIGHT_BLUE: u8 = 92;
const FULL: u8 = u8::MAX;

/// The default foreground and cursor color.
const DEFAULT_LIGHT: Rgb = Rgb::new(LIGHT_GRAY, LIGHT_GRAY, LIGHT_GRAY);

/// xterm's default ANSI colors.
const XTERM_ANSI: [Rgb; ANSI_COLORS] = [
    Rgb::new(0, 0, 0),
    Rgb::new(NORMAL, 0, 0),
    Rgb::new(0, NORMAL, 0),
    Rgb::new(NORMAL, NORMAL, 0),
    Rgb::new(0, 0, NORMAL_BLUE),
    Rgb::new(NORMAL, 0, NORMAL),
    Rgb::new(0, NORMAL, NORMAL),
    DEFAULT_LIGHT,
    Rgb::new(GRAY, GRAY, GRAY),
    Rgb::new(FULL, 0, 0),
    Rgb::new(0, FULL, 0),
    Rgb::new(FULL, FULL, 0),
    Rgb::new(BRIGHT_BLUE, BRIGHT_BLUE, FULL),
    Rgb::new(FULL, 0, FULL),
    Rgb::new(0, FULL, FULL),
    Rgb::new(FULL, FULL, FULL),
];

/// Which default color a [`Color::DEFAULT`] stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Role {
    /// The foreground.
    Foreground,
    /// The background.
    Background,
}

/// Which of the settable default colors an OSC 10, 11 or 12 addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DynamicColor {
    Foreground,
    Background,
    Cursor,
}

/// The 256-color palette plus foreground, background and cursor colors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    theme: Colors,
    current: Colors,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Colors {
    entries: [Rgb; PALETTE_SIZE],
    foreground: Rgb,
    background: Rgb,
    cursor: Rgb,
}

impl Colors {
    fn xterm() -> Self {
        let mut entries = [Rgb::default(); PALETTE_SIZE];
        for (slot, color) in entries.iter_mut().zip(XTERM_ANSI) {
            *slot = color;
        }
        for (offset, slot) in
            entries.iter_mut().skip(CUBE_START).take(GRAY_START - CUBE_START).enumerate()
        {
            let level = |n: usize| CUBE_LEVELS.get(n % CUBE_LEVELS.len()).copied().unwrap_or(0);
            *slot = Rgb::new(level(offset / CUBE_PLANE), level(offset / CUBE_SIDE), level(offset));
        }
        for (step, slot) in entries.iter_mut().skip(GRAY_START).enumerate() {
            let gray =
                GRAY_BASE.saturating_add(GRAY_STEP.saturating_mul(u8::try_from(step).unwrap_or(0)));
            *slot = Rgb::new(gray, gray, gray);
        }
        Self {
            entries,
            foreground: DEFAULT_LIGHT,
            background: Rgb::new(0, 0, 0),
            cursor: DEFAULT_LIGHT,
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        let colors = Colors::xterm();
        Self { theme: colors.clone(), current: colors }
    }
}

impl Palette {
    /// A palette whose ANSI colors, foreground, background and cursor come from the theme; the
    /// cube and the grays are the standard ones.
    pub fn with_theme(
        ansi: [Rgb; ANSI_COLORS],
        foreground: Rgb,
        background: Rgb,
        cursor: Rgb,
    ) -> Self {
        let mut colors = Colors::xterm();
        for (slot, color) in colors.entries.iter_mut().zip(ansi) {
            *slot = color;
        }
        colors.foreground = foreground;
        colors.background = background;
        colors.cursor = cursor;
        Self { theme: colors.clone(), current: colors }
    }

    /// The current color of palette entry `index`.
    pub fn entry(&self, index: u8) -> Rgb {
        self.current.entries.get(usize::from(index)).copied().unwrap_or_default()
    }

    /// The RGB value of a cell color; `Color::DEFAULT` resolves to the foreground or background
    /// depending on `role`.
    pub fn resolve(&self, color: Color, role: Role) -> Rgb {
        match color.kind() {
            ColorKind::Indexed(index) => self.entry(index),
            ColorKind::Rgb(r, g, b) => Rgb::new(r, g, b),
            _ => match role {
                Role::Foreground => self.current.foreground,
                Role::Background => self.current.background,
            },
        }
    }

    /// The current default foreground.
    pub fn foreground(&self) -> Rgb {
        self.current.foreground
    }

    /// The current default background.
    pub fn background(&self) -> Rgb {
        self.current.background
    }

    /// The current cursor color.
    pub fn cursor(&self) -> Rgb {
        self.current.cursor
    }

    pub(crate) fn set_entry(&mut self, index: u8, color: Rgb) -> bool {
        let Some(slot) = self.current.entries.get_mut(usize::from(index)) else { return false };
        let changed = *slot != color;
        *slot = color;
        changed
    }

    pub(crate) fn reset_entry(&mut self, index: u8) -> bool {
        let theme = self.theme.entries.get(usize::from(index)).copied().unwrap_or_default();
        self.set_entry(index, theme)
    }

    pub(crate) fn reset_all_entries(&mut self) -> bool {
        let changed = self.current.entries != self.theme.entries;
        self.current.entries = self.theme.entries;
        changed
    }

    pub(crate) fn dynamic(&self, which: DynamicColor) -> Rgb {
        match which {
            DynamicColor::Foreground => self.current.foreground,
            DynamicColor::Background => self.current.background,
            DynamicColor::Cursor => self.current.cursor,
        }
    }

    pub(crate) fn set_dynamic(&mut self, which: DynamicColor, color: Rgb) -> bool {
        let slot = match which {
            DynamicColor::Foreground => &mut self.current.foreground,
            DynamicColor::Background => &mut self.current.background,
            DynamicColor::Cursor => &mut self.current.cursor,
        };
        let changed = *slot != color;
        *slot = color;
        changed
    }

    pub(crate) fn reset_dynamic(&mut self, which: DynamicColor) -> bool {
        let theme = match which {
            DynamicColor::Foreground => self.theme.foreground,
            DynamicColor::Background => self.theme.background,
            DynamicColor::Cursor => self.theme.cursor,
        };
        self.set_dynamic(which, theme)
    }
}

/// Channels of a color, hex digits per channel at most, and the numeric constants of hex.
const CHANNELS: usize = 3;
const MAX_CHANNEL_DIGITS: usize = 4;
const HEX_RADIX: u32 = 16;
const HEX_DIGIT_BITS: usize = 4;

/// Parses an X11-style color: `rgb:R/G/B` (1 to 4 hex digits per channel) or `#RGB`, `#RRGGBB`,
/// `#RRRGGGBBB`, `#RRRRGGGGBBBB`. Color names are not supported.
pub(crate) fn parse_color(spec: &[u8]) -> Option<Rgb> {
    let text = std::str::from_utf8(spec).ok()?;
    if let Some(channels) = text.strip_prefix("rgb:") {
        let mut parts = channels.split('/');
        let color = Rgb::new(scale(parts.next()?)?, scale(parts.next()?)?, scale(parts.next()?)?);
        return parts.next().is_none().then_some(color);
    }
    let digits = text.strip_prefix('#')?;
    let width = digits.len() / CHANNELS;
    if digits.len() != width * CHANNELS
        || !(1..=MAX_CHANNEL_DIGITS).contains(&width)
        || !digits.is_ascii()
    {
        return None;
    }
    Some(Rgb::new(
        scale(digits.get(..width)?)?,
        scale(digits.get(width..2 * width)?)?,
        scale(digits.get(2 * width..)?)?,
    ))
}

/// Scales a 1 to 4 digit hex channel to 8 bits.
fn scale(digits: &str) -> Option<u8> {
    if digits.is_empty()
        || digits.len() > MAX_CHANNEL_DIGITS
        || !digits.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return None;
    }
    let value = u32::from_str_radix(digits, HEX_RADIX).ok()?;
    let max = (1u32 << (HEX_DIGIT_BITS * digits.len())) - 1;
    u8::try_from((value * u32::from(u8::MAX) + max / 2) / max).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_palette_has_the_cube_and_the_grays() {
        let palette = Palette::default();
        assert_eq!(palette.entry(1), Rgb::new(205, 0, 0));
        assert_eq!(palette.entry(16), Rgb::new(0, 0, 0));
        assert_eq!(palette.entry(21), Rgb::new(0, 0, 255));
        assert_eq!(palette.entry(196), Rgb::new(255, 0, 0));
        assert_eq!(palette.entry(231), Rgb::new(255, 255, 255));
        assert_eq!(palette.entry(232), Rgb::new(8, 8, 8));
        assert_eq!(palette.entry(255), Rgb::new(238, 238, 238));
    }

    #[test]
    fn color_specs_in_every_supported_form() {
        assert_eq!(parse_color(b"rgb:ff/00/80"), Some(Rgb::new(255, 0, 128)));
        assert_eq!(parse_color(b"rgb:ffff/0000/8000"), Some(Rgb::new(255, 0, 128)));
        assert_eq!(parse_color(b"rgb:f/0/8"), Some(Rgb::new(255, 0, 136)));
        assert_eq!(parse_color(b"#ff0080"), Some(Rgb::new(255, 0, 128)));
        assert_eq!(parse_color(b"#f08"), Some(Rgb::new(255, 0, 136)));
        assert_eq!(parse_color(b"#ffff00008000"), Some(Rgb::new(255, 0, 128)));
    }

    #[test]
    fn malformed_color_specs_are_rejected() {
        for spec in [
            &b""[..],
            b"red",
            b"rgb:ff/00",
            b"rgb:ff/00/00/00",
            b"rgb:gg/00/00",
            b"#ff00",
            b"#",
            b"rgb:fffff/0/0",
            "#é1".as_bytes(),
        ] {
            assert_eq!(parse_color(spec), None, "{}", String::from_utf8_lossy(spec));
        }
    }

    #[test]
    fn entries_change_and_return_to_the_theme() {
        let mut palette = Palette::default();
        assert!(palette.set_entry(3, Rgb::new(1, 2, 3)));
        assert!(!palette.set_entry(3, Rgb::new(1, 2, 3)));
        assert_eq!(palette.resolve(Color::indexed(3), Role::Foreground), Rgb::new(1, 2, 3));
        assert!(palette.reset_entry(3));
        assert_eq!(palette.entry(3), Rgb::new(205, 205, 0));
        palette.set_entry(4, Rgb::default());
        assert!(palette.reset_all_entries());
        assert_eq!(palette, Palette::default());
    }

    #[test]
    fn default_colors_follow_the_role() {
        let palette = Palette::default();
        assert_eq!(palette.resolve(Color::DEFAULT, Role::Background), Rgb::new(0, 0, 0));
        assert_eq!(palette.resolve(Color::DEFAULT, Role::Foreground), Rgb::new(229, 229, 229));
        assert_eq!(palette.resolve(Color::rgb(1, 2, 3), Role::Background), Rgb::new(1, 2, 3));
    }
}
