//! The glyph cache: rasterized glyphs in a bounded LRU (`docs/02` §6, `docs/03` §2).
//!
//! The key is (face, glyph, size, weight, synthesis). `docs/02` §6 also names a subpixel
//! position: the cell grid puts every glyph origin on a whole pixel (cell sizes are whole pixels,
//! `crate::font::metrics`), so that position is always zero and is not stored. The cap counts
//! the pixel bytes of every entry plus its bookkeeping; the least recently drawn glyphs are
//! evicted first. A hit allocates nothing.

use rustc_hash::FxHashMap;

use crate::font::{FaceId, Synthesis};
use crate::synth::{Decoration, Lines};

/// End of the LRU list.
const NIL: u32 = u32::MAX;

/// What identifies a cached glyph image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GlyphKey {
    /// A glyph of a font.
    Font {
        face: FaceId,
        id: u16,
        /// Font size in 1/64 physical pixel.
        size_q: u32,
        weight: u16,
        synth: Synthesis,
        /// Largest image of a color glyph (two cells), which depends on the cell size.
        fit: (u32, u32),
    },
    /// A glyph drawn by `crate::synth` for a cell of this size and line thickness.
    Synthetic { ch: char, width: u32, height: u32, light: u32 },
    /// A decoration mask for a cell of these lines.
    Decoration { decoration: Decoration, lines: Lines },
}

/// Pixels of a rasterized glyph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pixels {
    /// 8-bit coverage, `width × height`.
    Mask(Vec<u8>),
    /// Premultiplied `0xAARRGGBB`, `width × height` (color emoji).
    Color(Vec<u32>),
}

/// A rasterized glyph, placed relative to its pen position on the baseline.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GlyphImage {
    /// From the pen position to the left edge.
    pub(crate) left: i32,
    /// From the baseline up to the top edge.
    pub(crate) top: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Pixels per stored row: `width` rounded up to `crate::blit::ROW_ALIGN`, the padding
    /// transparent.
    pub(crate) stride: u32,
    /// Horizontal advance in physical pixels.
    pub(crate) advance: f32,
    pub(crate) pixels: Pixels,
}

impl GlyphImage {
    /// An image with no pixels: a glyph without ink, or one that could not be rasterized
    /// (cached too, so it is not tried again every frame).
    pub(crate) fn empty(advance: f32) -> Self {
        let pixels = Pixels::Mask(Vec::new());
        Self { left: 0, top: 0, width: 0, height: 0, stride: 0, advance, pixels }
    }

    fn bytes(&self) -> usize {
        match &self.pixels {
            Pixels::Mask(mask) => mask.capacity(),
            Pixels::Color(color) => color.capacity() * size_of::<u32>(),
        }
    }
}

#[derive(Debug)]
struct Slot {
    key: GlyphKey,
    image: GlyphImage,
    prev: u32,
    next: u32,
}

/// Bytes of bookkeeping per entry: the slot, its map entry and a free-list index.
const ENTRY_OVERHEAD: usize =
    size_of::<Slot>() + size_of::<GlyphKey>() + size_of::<u32>() + size_of::<u32>();

/// A bounded LRU of glyph images.
#[derive(Debug)]
pub(crate) struct GlyphCache {
    map: FxHashMap<GlyphKey, u32>,
    slots: Vec<Slot>,
    free: Vec<u32>,
    /// Most recently used.
    head: u32,
    /// Least recently used.
    tail: u32,
    used_bytes: usize,
    capacity_bytes: usize,
    /// Incremented by every eviction: a slot remembered at another generation may hold another
    /// glyph.
    generation: u32,
}

impl GlyphCache {
    pub(crate) fn new(capacity_bytes: usize) -> Self {
        Self {
            map: FxHashMap::default(),
            slots: Vec::new(),
            free: Vec::new(),
            head: NIL,
            tail: NIL,
            used_bytes: 0,
            capacity_bytes,
            generation: 0,
        }
    }

    pub(crate) fn generation(&self) -> u32 {
        self.generation
    }

    /// Marks `slot` as the most recently used.
    pub(crate) fn touch(&mut self, slot: u32) {
        self.unlink(slot);
        self.push_front(slot);
    }

    pub(crate) fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }

    /// The slot of `key`, marked as the most recently used.
    pub(crate) fn lookup(&mut self, key: &GlyphKey) -> Option<u32> {
        let slot = *self.map.get(key)?;
        self.touch(slot);
        Some(slot)
    }

    pub(crate) fn image(&self, slot: u32) -> Option<&GlyphImage> {
        self.slots.get(usize::try_from(slot).ok()?).map(|slot| &slot.image)
    }

    /// Stores `image`, evicting the least recently used glyphs until it fits. `Err` gives the
    /// image back when it alone is larger than the whole cache (the caller draws it uncached).
    pub(crate) fn insert(&mut self, key: GlyphKey, image: GlyphImage) -> Result<u32, GlyphImage> {
        let cost = image.bytes() + ENTRY_OVERHEAD;
        if cost > self.capacity_bytes {
            return Err(image);
        }
        while self.used_bytes + cost > self.capacity_bytes && self.tail != NIL {
            self.evict(self.tail);
        }
        let index =
            self.store(Slot { key, image, prev: NIL, next: NIL }).map_err(|slot| slot.image)?;
        self.push_front(index);
        let _previous = self.map.insert(key, index);
        self.used_bytes += cost;
        Ok(index)
    }

    /// Puts `slot` in a free slot or a new one.
    fn store(&mut self, slot: Slot) -> Result<u32, Slot> {
        if let Some(index) = self.free.pop() {
            let Some(free) = self.slot_mut(index) else { return Err(slot) };
            *free = slot;
            return Ok(index);
        }
        let Ok(index) = u32::try_from(self.slots.len()) else { return Err(slot) };
        self.slots.push(slot);
        Ok(index)
    }

    fn evict(&mut self, index: u32) {
        self.unlink(index);
        let Some(slot) = self.slot_mut(index) else { return };
        let (key, bytes) = (slot.key, slot.image.bytes());
        slot.image.pixels = Pixels::Mask(Vec::new());
        let _removed = self.map.remove(&key);
        self.used_bytes = self.used_bytes.saturating_sub(bytes + ENTRY_OVERHEAD);
        self.free.push(index);
        self.generation = self.generation.wrapping_add(1);
    }

    fn slot_mut(&mut self, index: u32) -> Option<&mut Slot> {
        self.slots.get_mut(usize::try_from(index).ok()?)
    }

    fn unlink(&mut self, index: u32) {
        let Some(slot) = self.slot_mut(index) else { return };
        let (prev, next) = (slot.prev, slot.next);
        (slot.prev, slot.next) = (NIL, NIL);
        if let Some(before) = self.slot_mut(prev) {
            before.next = next;
        } else if self.head == index {
            self.head = next;
        }
        if let Some(after) = self.slot_mut(next) {
            after.prev = prev;
        } else if self.tail == index {
            self.tail = prev;
        }
    }

    fn push_front(&mut self, index: u32) {
        let head = self.head;
        if let Some(slot) = self.slot_mut(index) {
            (slot.prev, slot.next) = (NIL, head);
        }
        if let Some(old) = self.slot_mut(head) {
            old.prev = index;
        }
        self.head = index;
        if self.tail == NIL {
            self.tail = index;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(id: u16) -> GlyphKey {
        let synth = Synthesis::default();
        GlyphKey::Font {
            face: FaceId::EMBEDDED,
            id,
            size_q: 64 * 12,
            weight: 400,
            synth,
            fit: (14, 16),
        }
    }

    fn image(bytes: usize) -> GlyphImage {
        GlyphImage {
            left: 0,
            top: 0,
            width: 1,
            height: 1,
            stride: 8,
            advance: 7.0,
            pixels: Pixels::Mask(vec![0; bytes]),
        }
    }

    #[test]
    fn a_hit_returns_the_stored_image() {
        let mut cache = GlyphCache::new(1 << 20);
        let slot = cache.insert(key(1), image(10)).unwrap();
        assert_eq!(cache.lookup(&key(1)), Some(slot));
        assert_eq!(cache.image(slot).map(|image| image.advance), Some(7.0));
        assert_eq!(cache.lookup(&key(2)), None);
    }

    #[test]
    fn the_least_recently_used_glyph_is_evicted_first() {
        let cost = 100 + ENTRY_OVERHEAD;
        let mut cache = GlyphCache::new(3 * cost);
        for id in 1..=3 {
            cache.insert(key(id), image(100)).unwrap();
        }
        assert!(cache.lookup(&key(1)).is_some(), "1 becomes the most recent");
        cache.insert(key(4), image(100)).unwrap();
        assert!(cache.lookup(&key(2)).is_none(), "2 was the least recent");
        assert!(cache.lookup(&key(1)).is_some() && cache.lookup(&key(3)).is_some());
        assert_eq!(cache.used_bytes(), 3 * cost);
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn the_cap_is_never_exceeded() {
        let cap = 10_000;
        let mut cache = GlyphCache::new(cap);
        for id in 0..2_000 {
            let _slot = cache.insert(key(id), image(usize::from(id % 97) + 1));
            assert!(cache.used_bytes() <= cap);
        }
        let freed_slots_are_reused = cache.slots.len() < 2_000;
        assert!(freed_slots_are_reused);
    }

    #[test]
    fn an_image_larger_than_the_cache_is_given_back() {
        let mut cache = GlyphCache::new(64);
        assert!(cache.insert(key(1), image(1_000)).is_err());
        assert_eq!(cache.len(), 0);
    }
}
