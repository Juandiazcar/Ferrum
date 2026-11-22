//! Font discovery (`fontdb`, memory mapped) for `swash`.

use std::sync::Arc;

use swash::FontRef;

/// One monospace face kept alive as its (memory-mapped) file bytes.
pub(crate) struct Font {
    data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    index: u32,
    pub(crate) name: String,
    pub(crate) file_bytes: usize,
}

impl Font {
    /// `path` = explicit font file (`JetBrains Mono`); otherwise the first system monospace face.
    pub(crate) fn load(path: Option<&str>) -> Option<Font> {
        let mut db = fontdb::Database::new();
        match path {
            Some(p) => {
                db.load_font_file(p).map_err(|e| out!("RESULT error load_font_file {e}")).ok()?;
            }
            None => db.load_system_fonts(),
        }
        let face = pick_face(&db, path.is_some())?;
        let name = face.families.first().map(|f| f.0.clone()).unwrap_or_default();
        let bytes = match &face.source {
            fontdb::Source::SharedFile(_, d) | fontdb::Source::Binary(d) => Arc::clone(d),
            fontdb::Source::File(p) => Arc::new(std::fs::read(p).ok()?),
        };
        let file_bytes = (*bytes).as_ref().len();
        Some(Font { data: bytes, index: face.index, name, file_bytes })
    }

    pub(crate) fn font_ref(&self) -> Option<FontRef<'_>> {
        FontRef::from_index((*self.data).as_ref(), self.index as usize)
    }
}

/// An explicit file takes its first face; otherwise a regular upright monospace face, preferring
/// well-known system families.
fn pick_face(db: &fontdb::Database, explicit: bool) -> Option<&fontdb::FaceInfo> {
    let preferred = ["Consolas", "Menlo", "DejaVu Sans Mono", "Liberation Mono", "Courier New"];
    db.faces()
        .filter(|f| {
            explicit
                || (f.monospaced
                    && f.weight == fontdb::Weight::NORMAL
                    && f.style == fontdb::Style::Normal)
        })
        .min_by_key(|f| {
            preferred.iter().position(|p| f.families.iter().any(|(n, _)| n == p)).unwrap_or(99)
        })
}
