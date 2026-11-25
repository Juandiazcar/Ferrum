//! Fonts: the embedded default, system families, and the cell metrics derived from them.
//!
//! The primary family is either the embedded `JetBrains Mono` (`docs/07` §5) or a system family
//! the user picked ("system fonts are preferred when the user selects one"). The system color
//! emoji fonts of `docs/07` §5 are loaded when system fonts were scanned.

mod face;
pub(crate) mod metrics;
mod system;

pub use metrics::{CellMetrics, FontSpec, MAX_FONT_PX};
pub use system::SystemFonts;

use face::Face;

use crate::error::RenderError;

/// Family name of the embedded font.
pub const EMBEDDED_FAMILY: &str = "JetBrains Mono";
/// CSS `normal` weight.
pub const REGULAR_WEIGHT: u16 = 400;

/// System color emoji families (`docs/07` §5).
const EMOJI_FAMILIES: [&str; 3] = ["Noto Color Emoji", "Apple Color Emoji", "Segoe UI Emoji"];

/// Which family draws the terminal text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family<'a> {
    /// The embedded `JetBrains Mono`.
    Embedded,
    /// A system family, by name (as listed by [`SystemFonts::monospace_families`]).
    System(&'a str),
}

/// The fonts of one terminal view: the primary face, the emoji faces and the system fonts.
#[derive(Debug)]
pub struct Fonts {
    faces: Vec<Face>,
    primary: usize,
    emoji: Vec<usize>,
    system: Option<SystemFonts>,
}

impl Fonts {
    /// The fonts for `family`. `system` is the result of [`SystemFonts::scan`] (made off the UI
    /// thread), or `None` to draw with the embedded font alone.
    ///
    /// # Errors
    ///
    /// [`RenderError::FamilyNotFound`] when a system family is asked for and not installed;
    /// [`RenderError::UnreadableFont`] when its file is not a usable font.
    pub fn new(family: Family<'_>, system: Option<SystemFonts>) -> Result<Self, RenderError> {
        let embedded = Face::embedded().ok_or(RenderError::UnreadableFont)?;
        let mut fonts = Self { faces: vec![embedded], primary: 0, emoji: Vec::new(), system };
        if let Family::System(name) = family {
            fonts.primary = fonts.load_family(name).ok_or(RenderError::FamilyNotFound)?;
        }
        fonts.emoji = EMOJI_FAMILIES.iter().filter_map(|name| fonts.load_family(name)).collect();
        Ok(fonts)
    }

    /// The cell metrics of the primary family at `spec`.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidFontSpec`] for an unusable spec; [`RenderError::UnreadableFont`]
    /// when the primary face's file can no longer be read.
    pub fn cell_metrics(&self, spec: &FontSpec) -> Result<CellMetrics, RenderError> {
        let face = self.faces.get(self.primary).ok_or(RenderError::UnreadableFont)?;
        face.with_font(self.system.as_ref(), |font| CellMetrics::of(font, spec))
            .ok_or(RenderError::UnreadableFont)?
    }

    /// Whether a system color emoji font was found.
    pub fn has_color_emoji(&self) -> bool {
        self.emoji.iter().any(|&index| self.faces.get(index).is_some_and(Face::is_color))
    }

    /// Loads the regular upright face of a system family; `None` when there is no such family.
    fn load_family(&mut self, name: &str) -> Option<usize> {
        let system = self.system.as_ref()?;
        let (id, _) = system.find(name, REGULAR_WEIGHT, system::Slant::Upright)?;
        let face = Face::system(system, id)?;
        self.faces.push(face);
        Some(self.faces.len() - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> FontSpec {
        FontSpec { size_px: 12.0, scale_factor: 1.0, line_height: 1.0, letter_spacing_px: 0.0 }
    }

    #[test]
    fn the_embedded_family_needs_no_system_fonts() {
        let fonts = Fonts::new(Family::Embedded, None).unwrap();
        let metrics = fonts.cell_metrics(&spec()).unwrap();
        assert_eq!((metrics.width, metrics.height), (7, 16));
        assert!(!fonts.has_color_emoji());
    }

    #[test]
    fn a_system_family_needs_system_fonts() {
        let missing = Fonts::new(Family::System("JetBrains Mono"), None);
        assert_eq!(missing.err(), Some(RenderError::FamilyNotFound));
    }

    #[test]
    #[cfg_attr(miri, ignore = "fontdb maps the font files; Miri cannot map memory")]
    fn an_installed_monospace_family_loads_and_has_metrics() {
        let system = SystemFonts::scan();
        let family = system.monospace_families().first().map(|name| (*name).to_owned());
        let family = family.expect("every CI image and desktop has a monospace font");
        let fonts = Fonts::new(Family::System(&family), Some(system)).unwrap();
        let metrics = fonts.cell_metrics(&spec()).unwrap();
        assert!(metrics.width > 0 && metrics.height > 0, "{family}: {metrics:?}");
    }

    #[test]
    #[cfg_attr(miri, ignore = "fontdb maps the font files; Miri cannot map memory")]
    fn an_unknown_system_family_is_refused() {
        let fonts = Fonts::new(Family::System("No Such Family 7b1d"), Some(SystemFonts::scan()));
        assert_eq!(fonts.err(), Some(RenderError::FamilyNotFound));
    }
}
