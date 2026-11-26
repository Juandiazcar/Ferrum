//! Character → (face, glyph) resolution through the fallback chain, and its memo.

use super::face::Face;
use super::system;
use super::{Fonts, Presentation, Slant, TextStyle};

/// Most fallback faces one [`Fonts`] keeps loaded besides the primary family, the generic
/// families and the emoji fonts. A screen of text needs a handful (one or two other scripts);
/// past 64 the least recently used one is dropped (and loaded again when needed), which bounds
/// what a stream of rare characters can pin.
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

/// A loaded face: its slot and the generation of the slot, so an id of a dropped face never
/// names the face loaded after it in the same slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FaceId {
    index: u16,
    generation: u16,
}

impl FaceId {
    /// The embedded face, always loaded first.
    pub(crate) const EMBEDDED: Self = Self { index: 0, generation: 0 };
}

/// Whether a loaded face may be dropped when the fallback faces are full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Keep {
    /// The primary family, generic families and emoji fonts: kept for the fonts' lifetime.
    Always,
    /// A face found by the coverage search: dropped when least recently used.
    WhileUsed,
}

/// One loaded face.
#[derive(Debug)]
pub(super) struct FaceSlot {
    pub(super) face: Face,
    pub(super) id: FaceId,
    keep: Keep,
    last_used: u64,
}

impl FaceSlot {
    pub(super) fn embedded(face: Face) -> Self {
        Self { face, id: FaceId::EMBEDDED, keep: Keep::Always, last_used: 0 }
    }
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
            self.touch(glyph.face);
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

    /// Tries, in scan order, the system faces whose coverage may hold the character. A face not
    /// loaded yet is only loaded once its character map is seen to map the character.
    fn search_system(&mut self, key: &MemoKey) -> Option<Glyph> {
        let candidates: Vec<fontdb::ID> = self.system.as_ref()?.candidates(key.ch).collect();
        for id in candidates {
            if let Some(face) = self.loaded(id) {
                if let Some(glyph) = self.probe(face, key, None) {
                    return Some(glyph);
                }
            } else if self.maps(id, key.ch) {
                let face = self.load_system_face(id, Keep::WhileUsed)?;
                return self.probe(face, key, None);
            }
        }
        None
    }

    /// Whether system face `id` maps `ch`, read without loading the face.
    fn maps(&self, id: fontdb::ID, ch: char) -> bool {
        let Some(system) = self.system.as_ref() else { return false };
        let mapped = system.database().with_face_data(id, |bytes, index| {
            let index = usize::try_from(index).ok()?;
            swash::FontRef::from_index(bytes, index).map(|font| font.charmap().map(ch) != 0)
        });
        mapped.flatten().unwrap_or(false)
    }

    /// The loaded face of system face `id`, if any.
    fn loaded(&self, id: fontdb::ID) -> Option<FaceId> {
        self.faces.iter().find(|slot| slot.face.system_id() == Some(id)).map(|slot| slot.id)
    }

    /// The loaded face `id`; `None` once it was dropped.
    pub(crate) fn face(&self, id: FaceId) -> Option<&Face> {
        self.faces.get(usize::from(id.index)).filter(|slot| slot.id == id).map(|slot| &slot.face)
    }

    /// Lowers the fallback face cap (tests drop faces without loading 64 of them).
    #[cfg(test)]
    pub(crate) fn set_face_cap(&mut self, cap: usize) {
        self.face_cap = cap;
    }

    /// Marks `face` as just used (the order fallback faces are dropped in).
    fn touch(&mut self, face: FaceId) {
        self.tick += 1;
        let tick = self.tick;
        if let Some(slot) = self.slot_mut(face) {
            slot.last_used = tick;
        }
    }

    fn slot_mut(&mut self, face: FaceId) -> Option<&mut FaceSlot> {
        self.faces.get_mut(usize::from(face.index)).filter(|slot| slot.id == face)
    }

    /// Drops the least recently used fallback face: its slot is reused, its memo entries go, and
    /// its id is queued for the glyph cache ([`Fonts::drain_dropped`]). The slot's index.
    fn drop_least_used(&mut self) -> Option<usize> {
        let (index, slot) = self
            .faces
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.keep == Keep::WhileUsed)
            .min_by_key(|(_, slot)| slot.last_used)?;
        let dropped = slot.id;
        self.memo.retain(|_, glyph| glyph.face != dropped);
        self.dropped.push(dropped);
        Some(index)
    }

    /// Hands every face dropped since the last call to `forget` (the glyph cache removes their
    /// glyphs), then forgets them.
    pub(crate) fn drain_dropped(&mut self, mut forget: impl FnMut(FaceId)) {
        for face in self.dropped.drain(..) {
            forget(face);
        }
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

    /// The face of system face `id`, loading it if needed; a fallback face beyond the cap drops
    /// the least recently used one first. `None` when the file is not a font.
    pub(super) fn load_system_face(&mut self, id: fontdb::ID, keep: Keep) -> Option<FaceId> {
        if let Some(face) = self.loaded(id) {
            if keep == Keep::Always
                && let Some(slot) = self.slot_mut(face)
            {
                slot.keep = Keep::Always;
            }
            self.touch(face);
            return Some(face);
        }
        let face = Face::system(self.system.as_ref()?, id)?;
        let fallbacks = self.faces.iter().filter(|slot| slot.keep == Keep::WhileUsed).count();
        let reused = if keep == Keep::WhileUsed && fallbacks >= self.face_cap {
            self.drop_least_used()
        } else {
            None
        };
        self.tick += 1;
        let last_used = self.tick;
        if let Some(slot) = reused.and_then(|index| self.faces.get_mut(index)) {
            let id =
                FaceId { index: slot.id.index, generation: slot.id.generation.wrapping_add(1) };
            *slot = FaceSlot { face, id, keep, last_used };
            return Some(id);
        }
        let id = FaceId { index: u16::try_from(self.faces.len()).ok()?, generation: 0 };
        self.faces.push(FaceSlot { face, id, keep, last_used });
        Some(id)
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
        let face = self.load_system_face(id, Keep::Always)?;
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
