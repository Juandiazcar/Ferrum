//! Character → (face, glyph) resolution through the fallback chain, and its memo.

use super::face::Face;
use super::system;
use super::{Fonts, Presentation, Slant, TextStyle};

/// Most faces one [`Fonts`] loads. Fallback faces are loaded on demand; a screen of text needs a
/// handful (primary, emoji, one or two other scripts), and 64 bounds what a stream of rare
/// characters can pin (each face keeps its description, and its glyphs fill the glyph cache)
/// while face ids stay small in glyph cache keys.
pub(crate) const MAX_FACES: usize = 64;
/// Most remembered characters outside printable ASCII: one per cell of a 200×60 screen (12 000)
/// plus margin, so a screen of distinct characters never evicts its own entries. The memo is
/// emptied when full, which bounds it (`docs/03` §3 rule 4) at about 0.4 MB.
pub(crate) const MAX_MEMO_CHARS: usize = 16_384;
/// Most (weight, slant) pairs of the primary family kept: nine CSS weights times two slants.
pub(crate) const MAX_STYLES: usize = 18;

/// First and number of the printable ASCII characters every style maps up front.
const ASCII_FIRST: u32 = 0x20;
const ASCII_LEN: usize = 95;

/// Index of a loaded face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FaceId(pub(crate) u16);

impl FaceId {
    /// The embedded face, always loaded first.
    pub(crate) const EMBEDDED: Self = Self(0);
}

/// What the rasterizer fakes because the face lacks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct Synthesis {
    /// Thicken the outlines (no heavier face or axis).
    pub(crate) embolden: bool,
    /// Slant the outlines (no italic face).
    pub(crate) oblique: bool,
}

/// A resolved glyph: the face, the glyph in it, and how to draw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Glyph {
    pub(crate) face: FaceId,
    pub(crate) id: u16,
    pub(crate) weight: u16,
    pub(crate) synth: Synthesis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MemoKey {
    ch: char,
    style: TextStyle,
    presentation: Presentation,
}

/// The primary face chosen for one style, with its printable ASCII glyphs.
#[derive(Debug)]
pub(crate) struct Styled {
    style: TextStyle,
    face: FaceId,
    synth: Synthesis,
    ascii: [u16; ASCII_LEN],
}

impl Fonts {
    /// The glyph that draws `ch` in `style`. Never fails: an unmapped character resolves to the
    /// primary face's `.notdef`. Allocation-free once `ch` was seen (printable ASCII always).
    pub(crate) fn glyph(
        &mut self,
        ch: char,
        style: TextStyle,
        presentation: Presentation,
    ) -> Glyph {
        let ascii = u32::from(ch).checked_sub(ASCII_FIRST).and_then(|i| usize::try_from(i).ok());
        let styled = self.styled_index(style).and_then(|index| self.styles.get(index));
        let (primary, synth, ascii_id) =
            styled.map_or((FaceId::EMBEDDED, Synthesis::default(), None), |styled| {
                (
                    styled.face,
                    styled.synth,
                    ascii.and_then(|index| styled.ascii.get(index)).copied(),
                )
            });
        if presentation == Presentation::Text
            && let Some(id) = ascii_id.filter(|&id| id != 0)
        {
            return Glyph { face: primary, id, weight: style.weight, synth };
        }
        let key = MemoKey { ch, style, presentation };
        if let Some(&glyph) = self.memo.get(&key) {
            return glyph;
        }
        let notdef = Glyph { face: primary, id: 0, weight: style.weight, synth };
        let glyph = self.search(&key, primary, synth).unwrap_or(notdef);
        if self.memo.len() >= MAX_MEMO_CHARS {
            self.memo.clear();
        }
        let _previous = self.memo.insert(key, glyph);
        glyph
    }

    fn search(&mut self, key: &MemoKey, primary: FaceId, synth: Synthesis) -> Option<Glyph> {
        let fallbacks = self.text_fallbacks.iter().chain(&self.emoji_fallbacks);
        let emoji_first = self.emoji_fallbacks.iter().chain(&self.text_fallbacks);
        let mut order: Vec<FaceId> = Vec::with_capacity(1 + self.faces.len());
        match key.presentation {
            Presentation::Text => order.extend(std::iter::once(&primary).chain(fallbacks)),
            Presentation::Emoji => order.extend(emoji_first.chain(std::iter::once(&primary))),
        }
        let found = order.into_iter().find_map(|face| {
            let face_synth = if face == primary { Some(synth) } else { None };
            self.probe(face, key, face_synth)
        });
        found.or_else(|| self.search_system(key))
    }

    /// Loads, in scan order, the system faces whose coverage may hold the character until one
    /// maps it.
    fn search_system(&mut self, key: &MemoKey) -> Option<Glyph> {
        let candidates: Vec<fontdb::ID> = self.system.as_ref()?.candidates(key.ch).collect();
        candidates.into_iter().find_map(|id| {
            let face = self.load_system_face(id)?;
            self.probe(face, key, None)
        })
    }

    /// The character of `key` in `face`. `synth` is the primary style's synthesis; other faces
    /// get theirs from [`fallback_synthesis`].
    fn probe(&self, face: FaceId, key: &MemoKey, synth: Option<Synthesis>) -> Option<Glyph> {
        let loaded = self.face(face)?;
        let id = loaded
            .with_font(self.system(), |font| font.charmap().map(key.ch))
            .filter(|&id| id != 0)?;
        let synth = synth.unwrap_or_else(|| fallback_synthesis(loaded, key.style));
        Some(Glyph { face, id, weight: key.style.weight, synth })
    }

    /// The face of system face `id`, loading it if needed. `None` when [`MAX_FACES`] are loaded
    /// or the file is not a font.
    pub(super) fn load_system_face(&mut self, id: fontdb::ID) -> Option<FaceId> {
        if let Some(index) = self.faces.iter().position(|face| face.system_id() == Some(id)) {
            return u16::try_from(index).ok().map(FaceId);
        }
        if self.faces.len() >= MAX_FACES {
            return None;
        }
        let face = Face::system(self.system.as_ref()?, id)?;
        let index = u16::try_from(self.faces.len()).ok()?;
        self.faces.push(face);
        Some(FaceId(index))
    }

    /// The primary face for `style`, chosen and remembered on first use; `None` when the system
    /// family has no face at all.
    pub(super) fn styled(&mut self, style: TextStyle) -> Option<FaceId> {
        let index = self.styled_index(style)?;
        self.styles.get(index).map(|styled| styled.face)
    }

    fn styled_index(&mut self, style: TextStyle) -> Option<usize> {
        if let Some(index) = self.styles.iter().position(|styled| styled.style == style) {
            return Some(index);
        }
        if self.styles.len() >= MAX_STYLES {
            self.styles.clear();
        }
        let (face, synth) = self.choose_primary(style)?;
        let ascii = self.ascii_glyphs(face);
        self.styles.push(Styled { style, face, synth, ascii });
        Some(self.styles.len() - 1)
    }

    fn choose_primary(&mut self, style: TextStyle) -> Option<(FaceId, Synthesis)> {
        let Some(family) = self.family.clone() else {
            let embedded = self.face(FaceId::EMBEDDED)?;
            return Some((FaceId::EMBEDDED, fallback_synthesis(embedded, style)));
        };
        let wanted = match style.slant {
            Slant::Upright => system::Slant::Upright,
            Slant::Italic => system::Slant::Italic,
        };
        let (id, found) = self.system.as_ref()?.find(&family, style.weight, wanted)?;
        let face = self.load_system_face(id)?;
        let synth = Synthesis {
            embolden: self.face(face)?.needs_embolden(style.weight),
            oblique: style.slant == Slant::Italic && found == system::Slant::Upright,
        };
        Some((face, synth))
    }

    fn ascii_glyphs(&self, face: FaceId) -> [u16; ASCII_LEN] {
        let mut glyphs = [0; ASCII_LEN];
        if let Some(face) = self.face(face) {
            let _mapped = face.with_font(self.system(), |font| {
                let charmap = font.charmap();
                for (code, glyph) in (ASCII_FIRST..).zip(glyphs.iter_mut()) {
                    *glyph = charmap.map(code);
                }
            });
        }
        glyphs
    }
}

/// How a face outside the primary family draws `style`: the weight through its axis or by
/// emboldening, italic always slanted (fallback faces are looked up upright).
fn fallback_synthesis(face: &Face, style: TextStyle) -> Synthesis {
    Synthesis { embolden: face.needs_embolden(style.weight), oblique: style.slant == Slant::Italic }
}
