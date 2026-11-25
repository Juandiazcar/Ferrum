//! One font face: where its bytes live and what the renderer needs to know about it.
//!
//! The embedded face is compiled in. A system face is never copied: its file is memory-mapped by
//! `fontdb` only while a glyph is looked up or rasterized (a cache miss), so only the pages that
//! were touched count against the fonts allowance (`docs/03` §2), and `ferrum-render` needs no
//! `unsafe` outside its blit kernels (`docs/05` §4).

use swash::{CacheKey, FontRef, Setting, TableProvider, Tag, tag_from_bytes};

use super::system::SystemFonts;

/// The embedded default font (`assets/fonts`, `docs/07` §5).
pub(crate) static EMBEDDED: &[u8] =
    include_bytes!("../../../../assets/fonts/JetBrainsMono-wght-subset.ttf");

/// The variation axis of weight.
const WEIGHT_AXIS: Tag = tag_from_bytes(b"wght");
/// One CSS weight step: a static face at least this much lighter than the asked weight is
/// emboldened.
const WEIGHT_STEP: u16 = 100;

/// Tables that hold color glyphs: layered outlines, PNG strikes (Google), `sbix` (Apple).
const COLOR_TABLES: [&[u8; 4]; 3] = [b"COLR", b"CBDT", b"sbix"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Data {
    Embedded,
    System(fontdb::ID),
}

/// A loaded face.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Face {
    data: Data,
    offset: u32,
    key: CacheKey,
    /// Range of the `wght` axis, when the face is variable in weight.
    weight_axis: Option<(f32, f32)>,
    /// The face's own weight (OS/2), used when it has no `wght` axis.
    static_weight: u16,
    is_color: bool,
}

impl Face {
    /// The embedded face. `None` only if the compiled-in file is broken, which a unit test rules
    /// out.
    pub(crate) fn embedded() -> Option<Self> {
        FontRef::from_index(EMBEDDED, 0).map(|font| Self::describe(Data::Embedded, font))
    }

    /// The system face `id` of `fonts`, or `None` if its file cannot be read as a font.
    pub(crate) fn system(fonts: &SystemFonts, id: fontdb::ID) -> Option<Self> {
        let index = fonts.face_index(id)?;
        fonts
            .database()
            .with_face_data(id, |bytes, _| {
                FontRef::from_index(bytes, index).map(|font| Self::describe(Data::System(id), font))
            })
            .flatten()
    }

    fn describe(source: Data, font: FontRef<'_>) -> Self {
        let is_color =
            COLOR_TABLES.iter().any(|tag| font.table_by_tag(tag_from_bytes(tag)).is_some());
        let weight_axis = font
            .variations()
            .find_by_tag(WEIGHT_AXIS)
            .map(|axis| (axis.min_value(), axis.max_value()));
        Self {
            data: source,
            offset: font.offset,
            key: font.key,
            weight_axis,
            static_weight: font.attributes().weight().0,
            is_color,
        }
    }

    /// Runs `f` on the face's font data. `None` when the system file can no longer be mapped
    /// (removed or replaced since the scan) or when no system fonts were given for a system face.
    pub(crate) fn with_font<T>(
        &self,
        system: Option<&SystemFonts>,
        f: impl FnOnce(FontRef<'_>) -> T,
    ) -> Option<T> {
        let (offset, key) = (self.offset, self.key);
        match self.data {
            Data::Embedded => Some(f(FontRef { data: EMBEDDED, offset, key })),
            Data::System(id) => system?
                .database()
                .with_face_data(id, |bytes, _| f(FontRef { data: bytes, offset, key })),
        }
    }

    /// The system face this is, if any.
    pub(crate) fn system_id(&self) -> Option<fontdb::ID> {
        match self.data {
            Data::Embedded => None,
            Data::System(id) => Some(id),
        }
    }

    /// The variation setting that selects `weight` (clamped to the axis), if the face is
    /// variable in weight.
    pub(crate) fn weight_setting(&self, weight: u16) -> Option<Setting<f32>> {
        let (min, max) = self.weight_axis?;
        Some(Setting { tag: WEIGHT_AXIS, value: f32::from(weight).clamp(min, max) })
    }

    /// Whether drawing `weight` with this face needs synthetic emboldening: the face is static
    /// and lighter than `weight` by at least one CSS weight step.
    pub(crate) fn needs_embolden(&self, weight: u16) -> bool {
        self.weight_axis.is_none() && weight >= self.static_weight.saturating_add(WEIGHT_STEP)
    }

    /// Whether the face draws color glyphs (emoji).
    pub(crate) fn is_color(&self) -> bool {
        self.is_color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_face_is_variable_in_weight_and_not_color() {
        let face = Face::embedded();
        assert!(face.is_some(), "the embedded font parses");
        let Some(face) = face else { return };
        assert_eq!(face.weight_axis, Some((100.0, 800.0)));
        assert!(!face.is_color());
        assert!(!face.needs_embolden(700));
        assert_eq!(face.weight_setting(900).map(|setting| setting.value), Some(800.0));
    }

    #[test]
    fn the_embedded_face_maps_latin_box_drawing_and_powerline() {
        let face = Face::embedded();
        let mapped = face.and_then(|face| {
            face.with_font(None, |font| {
                ['A', 'é', '─', '█', '\u{E0B0}'].map(|ch| font.charmap().map(ch) != 0)
            })
        });
        assert_eq!(mapped, Some([true; 5]));
    }
}
