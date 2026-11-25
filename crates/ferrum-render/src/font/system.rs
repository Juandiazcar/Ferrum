//! The system fonts: discovered once, off the UI thread, then handed to [`super::Fonts`].
//!
//! `fontdb` scans the font directories of each OS (`docs/11` §2–§4; no fontconfig).

use fontdb::{Database, Family, ID, Query, Style, Weight};

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
}

impl SystemFonts {
    /// Scans the system font directories. This reads every font file once, so call it off the
    /// UI thread (`docs/05` §5) and hand the result to [`super::Fonts::new`].
    pub fn scan() -> Self {
        let mut database = Database::new();
        database.load_system_fonts();
        Self { database }
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
        self.database.len()
    }

    /// Whether no face was found.
    pub fn is_empty(&self) -> bool {
        self.database.is_empty()
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
}
