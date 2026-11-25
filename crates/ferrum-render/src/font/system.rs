//! The system fonts: discovered once, off the UI thread, then handed to [`super::Fonts`].
//!
//! `fontdb` scans the font directories of each OS (`docs/11` §2–§4; no fontconfig). The scan
//! also records which blocks of 256 code points every face covers, so that finding a fallback
//! for a missing character maps only the few faces that can have it instead of every file.

use fontdb::{Database, Family, ID, Query, Style, Weight};
use swash::FontRef;

/// Which pages of 256 code points a face maps: the sorted page numbers (`code point >> 8`, at
/// most 0x10FF, so a `u16`). A text face maps a few dozen pages, a CJK face a few hundred.
#[derive(Debug, Clone, Default)]
struct Coverage(Box<[u16]>);

impl Coverage {
    fn page(code_point: u32) -> Option<u16> {
        u16::try_from(code_point >> 8).ok()
    }

    fn of(code_points: impl IntoIterator<Item = u32>) -> Self {
        let mut pages: Vec<u16> = code_points.into_iter().filter_map(Self::page).collect();
        pages.sort_unstable();
        pages.dedup();
        Self(pages.into_boxed_slice())
    }

    fn may_contain(&self, ch: char) -> bool {
        Self::page(u32::from(ch)).is_some_and(|page| self.0.binary_search(&page).is_ok())
    }
}

/// Whether a face is upright or italic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slant {
    Upright,
    Italic,
}

/// The fonts installed on the system.
#[derive(Debug, Default)]
pub struct SystemFonts {
    database: Database,
    coverage: Vec<(ID, Coverage)>,
}

impl SystemFonts {
    /// Scans the system font directories. This reads every font file once, so call it off the
    /// UI thread (`docs/05` §5) and hand the result to [`super::Fonts::new`].
    pub fn scan() -> Self {
        let mut database = Database::new();
        database.load_system_fonts();
        let mut coverage = Vec::with_capacity(database.len());
        for face in database.faces() {
            let index = face.index;
            let pages = database.with_face_data(face.id, |bytes, _| page_coverage(bytes, index));
            if let Some(Some(pages)) = pages {
                coverage.push((face.id, pages));
            }
        }
        Self { database, coverage }
    }

    /// The families of the monospaced faces, sorted and without duplicates (the system part of
    /// the font family list, `docs/09` §8).
    pub fn monospace_families(&self) -> Vec<&str> {
        let mut families: Vec<&str> = self
            .database
            .faces()
            .filter(|face| face.monospaced)
            .filter_map(|face| face.families.first().map(|(name, _)| name.as_str()))
            .collect();
        families.sort_unstable();
        families.dedup();
        families
    }

    /// Number of faces found.
    pub fn len(&self) -> usize {
        self.coverage.len()
    }

    /// Whether no face was found.
    pub fn is_empty(&self) -> bool {
        self.coverage.is_empty()
    }

    pub(crate) fn database(&self) -> &Database {
        &self.database
    }

    pub(crate) fn face_index(&self, id: ID) -> Option<usize> {
        self.database.face(id).and_then(|face| usize::try_from(face.index).ok())
    }

    /// The best face of `family` for `weight` and `slant`, plus whether it really has that slant
    /// (CSS font matching through `fontdb`).
    pub(crate) fn find(&self, family: &str, weight: u16, slant: Slant) -> Option<(ID, Slant)> {
        let style = match slant {
            Slant::Upright => Style::Normal,
            Slant::Italic => Style::Italic,
        };
        let families = [Family::Name(family)];
        let query =
            Query { families: &families, weight: Weight(weight), style, ..Query::default() };
        let id = self.database.query(&query)?;
        let found = match self.database.face(id)?.style {
            Style::Normal => Slant::Upright,
            Style::Italic | Style::Oblique => Slant::Italic,
        };
        Some((id, found))
    }

    /// Faces that may map `ch`, in scan order.
    pub(crate) fn candidates(&self, ch: char) -> impl Iterator<Item = ID> + '_ {
        self.coverage.iter().filter(move |(_, pages)| pages.may_contain(ch)).map(|(id, _)| *id)
    }
}

fn page_coverage(bytes: &[u8], index: u32) -> Option<Coverage> {
    let font = FontRef::from_index(bytes, usize::try_from(index).ok()?)?;
    let mut code_points = Vec::new();
    font.charmap().enumerate(|code_point, _| code_points.push(code_point));
    Some(Coverage::of(code_points))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_pages_hold_what_was_inserted() {
        let pages = Coverage::of([u32::from('A'), 0x4E2D, 0x1F600, 0xF_0001, 0x4E2E]);
        assert_eq!(pages.0.len(), 4, "one entry per page");
        assert!(pages.may_contain('B'), "same page as A");
        assert!(pages.may_contain('中'));
        assert!(pages.may_contain('😀'));
        assert!(pages.may_contain('\u{F0002}'), "plane 15, private use (icon fonts)");
        assert!(!pages.may_contain('λ'), "page 3 is empty");
        assert!(!pages.may_contain('\u{10FFFF}'));
    }

    #[test]
    fn the_embedded_font_covers_its_subset_pages() {
        let pages = page_coverage(super::super::face::EMBEDDED, 0);
        assert!(pages.is_some_and(|pages| pages.may_contain('A') && pages.may_contain('─')));
    }
}
