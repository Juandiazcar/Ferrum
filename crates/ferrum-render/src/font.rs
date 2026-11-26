//! Fonts: the embedded default, system families, the fallback chain that finds a face for every
//! character, and the cell metrics.
//!
//! The primary family is either the embedded `JetBrains Mono` (`docs/07` §5) or a system family
//! the user picked ("system fonts are preferred when the user selects one"). A character the
//! primary face does not map is looked up, in order, in the generic monospace families of
//! `docs/11` §2, the system color emoji fonts of `docs/07` §5, and then every other system face
//! whose coverage may hold it (read the first time a fallback is needed); emoji presentation
//! tries the emoji fonts first. The answer is memoized, so steady-state painting never searches.
//! At most `MAX_FACES` fallback faces stay loaded: the least recently used one is dropped with
//! its memo entries, and the glyph cache forgets its glyphs. When nothing maps a character the
//! primary face's `.notdef` glyph is drawn (the "tofu" box).
//!
//! Weights follow CSS: 400 regular, 700 bold. A variable face gets the weight through its `wght`
//! axis; a static face without a heavier style is emboldened; a face without an italic style is
//! slanted (`crate::glyph`).

mod face;
pub(crate) mod metrics;
mod resolve;
mod system;

pub(crate) use face::Face;
pub use metrics::{CellMetrics, FontSpec, MAX_FONT_PX};
pub(crate) use resolve::{FaceId, Glyph, Synthesis};
use resolve::{FaceSlot, Keep};
pub use system::SystemFonts;

use rustc_hash::FxHashMap;

use crate::error::RenderError;

/// Family name of the embedded font.
pub const EMBEDDED_FAMILY: &str = "JetBrains Mono";
/// CSS `normal` weight.
pub const REGULAR_WEIGHT: u16 = 400;
/// CSS `bold` weight.
pub const BOLD_WEIGHT: u16 = 700;

/// Generic monospace families tried before any other system face (`docs/11` §2).
const GENERIC_FALLBACKS: [&str; 3] = ["DejaVu Sans Mono", "Noto Sans Mono", "Liberation Mono"];

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

/// Upright or italic text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Slant {
    /// Upright.
    #[default]
    Upright,
    /// Italic: the family's italic face, or the upright one slanted.
    Italic,
}

/// The weight and slant text is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextStyle {
    /// CSS weight, 100–900.
    pub weight: u16,
    /// Slant.
    pub slant: Slant,
}

impl TextStyle {
    /// Regular upright text.
    pub const REGULAR: Self = Self { weight: REGULAR_WEIGHT, slant: Slant::Upright };
}

/// Whether a character is drawn in its text or its emoji (color) form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Presentation {
    /// Primary and text faces first.
    Text,
    /// Color emoji faces first.
    Emoji,
}

/// The fonts of the process: the primary family, the fallbacks and the system fonts.
#[derive(Debug)]
pub struct Fonts {
    faces: Vec<FaceSlot>,
    /// Use counter for dropping the least recently used fallback face.
    tick: u64,
    /// Most fallback faces kept ([`resolve::MAX_FACES`]).
    face_cap: usize,
    /// Faces dropped since the glyph cache last forgot them.
    dropped: Vec<FaceId>,
    system: Option<SystemFonts>,
    /// The system family, or `None` for the embedded font.
    family: Option<String>,
    /// The face of regular upright text, which the cell metrics come from.
    regular: FaceId,
    styles: Vec<resolve::Styled>,
    text_fallbacks: Vec<FaceId>,
    emoji_fallbacks: Vec<FaceId>,
    memo: FxHashMap<resolve::MemoKey, Glyph>,
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
        let mut fonts = Self {
            faces: vec![FaceSlot::embedded(embedded)],
            tick: 0,
            face_cap: resolve::MAX_FACES,
            dropped: Vec::new(),
            system,
            family: None,
            regular: FaceId::EMBEDDED,
            styles: Vec::new(),
            text_fallbacks: Vec::new(),
            emoji_fallbacks: Vec::new(),
            memo: FxHashMap::default(),
        };
        if let Family::System(name) = family {
            fonts.family = Some(name.to_owned());
            fonts.regular = fonts.styled(TextStyle::REGULAR).ok_or(RenderError::FamilyNotFound)?;
        }
        fonts.text_fallbacks = fonts.load_families(&GENERIC_FALLBACKS);
        fonts.emoji_fallbacks = fonts.load_families(&EMOJI_FAMILIES);
        Ok(fonts)
    }

    /// The cell metrics of the primary family at `spec`.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidFontSpec`] for an unusable spec; [`RenderError::UnreadableFont`]
    /// when the primary face's file can no longer be read.
    pub fn cell_metrics(&self, spec: &FontSpec) -> Result<CellMetrics, RenderError> {
        let face = self.face(self.regular).ok_or(RenderError::UnreadableFont)?;
        face.with_font(self.system.as_ref(), |font| CellMetrics::of(font, spec))
            .ok_or(RenderError::UnreadableFont)?
    }

    /// Whether a system color emoji font was found.
    pub fn has_color_emoji(&self) -> bool {
        self.emoji_fallbacks.iter().any(|&id| self.face(id).is_some_and(Face::is_color))
    }

    pub(crate) fn system(&self) -> Option<&SystemFonts> {
        self.system.as_ref()
    }

    /// Loads the regular upright faces of the installed `families`.
    fn load_families(&mut self, families: &[&str]) -> Vec<FaceId> {
        let mut loaded = Vec::new();
        for family in families {
            let found = self
                .system
                .as_ref()
                .and_then(|system| system.find(family, REGULAR_WEIGHT, system::Slant::Upright));
            if let Some(face) = found.and_then(|(id, _)| self.load_system_face(id, Keep::Always)) {
                loaded.push(face);
            }
        }
        loaded
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
    fn ascii_comes_from_the_primary_face_in_every_style() {
        let mut fonts = Fonts::new(Family::Embedded, None).unwrap();
        let regular = fonts.glyph('A', TextStyle::REGULAR, Presentation::Text);
        assert_eq!(regular.face, FaceId::EMBEDDED);
        assert_ne!(regular.id, 0);
        let style = TextStyle { weight: BOLD_WEIGHT, slant: Slant::Italic };
        let bold = fonts.glyph('A', style, Presentation::Text);
        assert_eq!((bold.id, bold.weight), (regular.id, BOLD_WEIGHT));
        assert!(bold.synth.oblique && !bold.synth.embolden, "variable weight, synthetic slant");
    }

    #[test]
    fn a_character_no_face_maps_resolves_to_notdef() {
        let mut fonts = Fonts::new(Family::Embedded, None).unwrap();
        let glyph = fonts.glyph('中', TextStyle::REGULAR, Presentation::Text);
        assert_eq!((glyph.face, glyph.id), (FaceId::EMBEDDED, 0));
        let again = fonts.glyph('中', TextStyle::REGULAR, Presentation::Text);
        assert_eq!(again, glyph, "memoized");
    }

    #[test]
    #[cfg_attr(miri, ignore = "fontdb maps the font files; Miri cannot map memory")]
    fn fallbacks_find_characters_the_embedded_font_lacks() {
        let (_dir, system) = crate::test_fonts::fixture_fonts();
        let mut fonts = Fonts::new(Family::Embedded, Some(system)).unwrap();
        assert!(!fonts.system().is_some_and(SystemFonts::coverage_read), "nothing read up front");
        let glyph = fonts.glyph('λ', TextStyle::REGULAR, Presentation::Text);
        assert_ne!(glyph.face, FaceId::EMBEDDED);
        assert_ne!(glyph.id, 0);
        assert!(
            fonts.system().is_some_and(SystemFonts::coverage_read),
            "read on the first fallback"
        );
    }

    #[test]
    #[cfg_attr(miri, ignore = "fontdb maps the font files; Miri cannot map memory")]
    fn emoji_presentation_prefers_a_color_face() {
        let (_dir, system) = crate::test_fonts::fixture_fonts();
        let mut fonts = Fonts::new(Family::Embedded, Some(system)).unwrap();
        assert!(fonts.has_color_emoji());
        for ch in ['😀', '☕'] {
            let glyph = fonts.glyph(ch, TextStyle::REGULAR, Presentation::Emoji);
            assert!(fonts.face(glyph.face).is_some_and(Face::is_color), "{ch}");
            assert_ne!(glyph.id, 0);
        }
    }

    #[test]
    #[cfg_attr(miri, ignore = "fontdb maps the font files; Miri cannot map memory")]
    fn least_used_fallback_faces_are_dropped_and_loaded_again() {
        let (_dir, system) = crate::test_fonts::fixture_fonts();
        let mut fonts = Fonts::new(Family::Embedded, Some(system)).unwrap();
        fonts.set_face_cap(1);
        // Greek and Cyrillic come from two fallback faces; with room for one, each search
        // drops the other. Many rare characters in turn always resolve to a mapped glyph.
        let greek = ('α'..='ω').filter(|ch| *ch != '\u{3C2}');
        let cyrillic = 'а'..='я';
        let mut dropped = Vec::new();
        for (round, (g, c)) in greek.zip(cyrillic).enumerate() {
            for ch in [g, c] {
                let glyph = fonts.glyph(ch, TextStyle::REGULAR, Presentation::Text);
                assert_ne!(glyph.id, 0, "{ch} in round {round}");
                assert!(fonts.face(glyph.face).is_some(), "{ch}: its face is loaded");
            }
            fonts.drain_dropped(|face| dropped.push(face));
        }
        assert!(dropped.len() > 20, "faces were dropped and loaded again: {}", dropped.len());
        let fallbacks = fonts.faces.iter().filter(|slot| slot.face.system_id().is_some()).count();
        assert_eq!(fallbacks, 2, "the emoji font (kept) and one fallback face");
        let again = fonts.glyph('λ', TextStyle::REGULAR, Presentation::Text);
        assert!(fonts.face(again.face).is_some(), "a dropped face's memo entries are gone");
    }

    #[test]
    fn a_system_family_needs_system_fonts() {
        let missing = Fonts::new(Family::System("JetBrains Mono"), None);
        assert_eq!(missing.err(), Some(RenderError::FamilyNotFound));
    }

    #[test]
    #[cfg_attr(miri, ignore = "fontdb maps the font files; Miri cannot map memory")]
    fn an_installed_monospace_family_loads_and_has_metrics() {
        let (_dir, system) = crate::test_fonts::fixture_fonts();
        let families = system.monospace_families();
        assert_eq!(families, ["Ferrum Test Cyrillic", "Ferrum Test Greek", "Noto Color Emoji"]);
        let fonts = Fonts::new(Family::System("Ferrum Test Greek"), Some(system)).unwrap();
        let metrics = fonts.cell_metrics(&spec()).unwrap();
        assert_eq!((metrics.width, metrics.height), (7, 16), "JetBrains Mono's own metrics");
        assert!(fonts.has_color_emoji(), "the fixture emoji font is a color font");
    }

    #[test]
    #[cfg_attr(miri, ignore = "fontdb maps the font files; Miri cannot map memory")]
    fn an_unknown_system_family_is_refused() {
        let (_dir, system) = crate::test_fonts::fixture_fonts();
        let fonts = Fonts::new(Family::System("No Such Family 7b1d"), Some(system));
        assert_eq!(fonts.err(), Some(RenderError::FamilyNotFound));
    }
}
