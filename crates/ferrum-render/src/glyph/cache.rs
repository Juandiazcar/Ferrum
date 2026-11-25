//! The glyph cache: rasterized glyphs in a bounded LRU (`docs/02` §6, `docs/03` §2).
//!
//! The key is (face, glyph, size, subpixel position, weight, synthesis, antialiasing), as
//! `docs/02` §6 lists it (weight and synthesis because variable faces and synthetic styles share
//! glyph ids). The cap is a memory cap: it counts the pixel bytes of every entry and the
//! capacity of every container of the cache (slots, map, free list); a container never grows
//! past what the cap allows, the least recently drawn glyphs are evicted instead. A hit allocates
//! nothing.

use rustc_hash::FxHashMap;

use super::Antialias;
use crate::font::{FaceId, Synthesis};

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
        /// Pen position within its pixel, in steps of 1/`SUBPIXEL_STEPS` px.
        subpixel_q: u8,
        weight: u16,
        synth: Synthesis,
        antialias: Antialias,
        /// Largest image of a color glyph (two cells), which depends on the cell size.
        fit: (u32, u32),
    },
    /// A glyph drawn by `crate::synth` over this many pixels and line thickness.
    Synthetic { ch: char, width: u32, height: u32, light: u32 },
}

impl GlyphKey {
    /// Whether this is a glyph of `face`.
    pub(crate) fn is_of(&self, face: FaceId) -> bool {
        matches!(self, Self::Font { face: of, .. } if *of == face)
    }
}

/// Pixels of a rasterized glyph, `stride × height`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pixels {
    /// 8-bit coverage (grayscale antialiasing).
    Mask(Vec<u8>),
    /// Coverage per channel, `0x00RRGGBB` (LCD antialiasing).
    Subpixel(Vec<u32>),
    /// Premultiplied `0xAARRGGBB` (color emoji).
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
    /// Pixels per stored row.
    pub(crate) stride: u32,
    /// Horizontal advance in physical pixels; `None` when the glyph could not be rasterized.
    pub(crate) advance: Option<f32>,
    pub(crate) pixels: Pixels,
}

impl GlyphImage {
    /// An image without pixels: a glyph without ink, or (advance `None`) one that could not be
    /// rasterized, cached too so it is not tried again every frame.
    pub(crate) fn empty(advance: Option<f32>) -> Self {
        let pixels = Pixels::Mask(Vec::new());
        Self { left: 0, top: 0, width: 0, height: 0, stride: 0, advance, pixels }
    }

    pub(crate) fn bytes(&self) -> usize {
        match &self.pixels {
            Pixels::Mask(mask) => mask.capacity(),
            Pixels::Subpixel(pixels) | Pixels::Color(pixels) => {
                pixels.capacity() * size_of::<u32>()
            }
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
    /// Bytes of pixels of the cached images.
    pixel_bytes: usize,
    capacity_bytes: usize,
}

impl GlyphCache {
    pub(crate) fn new(capacity_bytes: usize) -> Self {
        Self {
            map: FxHashMap::default(),
            slots: Vec::new(),
            free: Vec::new(),
            head: NIL,
            tail: NIL,
            pixel_bytes: 0,
            capacity_bytes,
        }
    }

    /// Bytes the cache holds: pixels plus the capacity of its containers. Never above the cap.
    pub(crate) fn used_bytes(&self) -> usize {
        self.pixel_bytes + Self::container_bytes(self.slots.capacity(), self.map.capacity())
    }

    /// The memory of the containers at these capacities: the slots, the free list (kept as
    /// long as the slots) and the map's buckets (key, value and one control byte each).
    fn container_bytes(slots: usize, map_capacity: usize) -> usize {
        let buckets = map_buckets(map_capacity);
        let control = if buckets == 0 { 0 } else { buckets + MAP_GROUP_WIDTH };
        slots * (size_of::<Slot>() + size_of::<u32>())
            + buckets * size_of::<(GlyphKey, u32)>()
            + control
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

    /// Marks `slot` as the most recently used.
    pub(crate) fn touch(&mut self, slot: u32) {
        self.unlink(slot);
        self.push_front(slot);
    }

    pub(crate) fn image(&self, slot: u32) -> Option<&GlyphImage> {
        self.slots.get(usize::try_from(slot).ok()?).map(|slot| &slot.image)
    }

    /// Stores `image`, evicting the least recently used glyphs until it and any container growth
    /// it needs fit. `Err` gives the image back when it cannot fit even in an empty cache (the
    /// caller draws it uncached).
    pub(crate) fn insert(&mut self, key: GlyphKey, image: GlyphImage) -> Result<u32, GlyphImage> {
        let cost = image.bytes();
        loop {
            let (slots, map) = self.capacities_after_insert();
            let containers = Self::container_bytes(slots, map);
            if self.pixel_bytes + cost + containers <= self.capacity_bytes {
                break;
            }
            if self.tail == NIL {
                return Err(image);
            }
            self.evict(self.tail);
        }
        let index =
            self.store(Slot { key, image, prev: NIL, next: NIL }).map_err(|slot| slot.image)?;
        self.push_front(index);
        let _previous = self.map.insert(key, index);
        self.pixel_bytes += cost;
        self.enforce_cap(index)?;
        Ok(index)
    }

    /// Evicts the least recently used glyphs other than `kept` while the cache is above its cap:
    /// the map may have grown more than projected (it also grows to clear the tombstones of
    /// removed entries). When `kept` alone is too much, it goes too and is given back.
    fn enforce_cap(&mut self, kept: u32) -> Result<(), GlyphImage> {
        while self.used_bytes() > self.capacity_bytes {
            if self.tail != kept && self.tail != NIL {
                self.evict(self.tail);
                continue;
            }
            let image = self
                .slot_mut(kept)
                .map(|slot| std::mem::replace(&mut slot.image, GlyphImage::empty(None)));
            let bytes = image.as_ref().map_or(0, GlyphImage::bytes);
            self.pixel_bytes = self.pixel_bytes.saturating_sub(bytes);
            self.evict(kept);
            self.map.shrink_to_fit();
            return Err(image.unwrap_or_else(|| GlyphImage::empty(None)));
        }
        Ok(())
    }

    /// Container capacities once one more entry is in: grown only when full (slots double,
    /// the map grows to its next bucket count).
    fn capacities_after_insert(&self) -> (usize, usize) {
        let slots = if self.free.is_empty() && self.slots.len() == self.slots.capacity() {
            (self.slots.capacity() * 2).max(MIN_SLOTS)
        } else {
            self.slots.capacity()
        };
        let map = if self.map.len() == self.map.capacity() {
            map_capacity((map_buckets(self.map.capacity()) * 2).max(4))
        } else {
            self.map.capacity()
        };
        (slots, map)
    }

    /// Removes every entry whose key `stale` matches (glyphs of an evicted face).
    pub(crate) fn evict_where(&mut self, stale: impl Fn(&GlyphKey) -> bool) {
        let doomed: Vec<u32> =
            self.map.iter().filter(|(key, _)| stale(key)).map(|(_, &slot)| slot).collect();
        for slot in doomed {
            self.evict(slot);
        }
    }

    /// Puts `slot` in a free slot or a new one; the slots and the free list grow together and
    /// exactly as [`Self::capacities_after_insert`] counted.
    fn store(&mut self, slot: Slot) -> Result<u32, Slot> {
        if let Some(index) = self.free.pop() {
            let Some(free) = self.slot_mut(index) else { return Err(slot) };
            *free = slot;
            return Ok(index);
        }
        if self.slots.len() == self.slots.capacity() {
            let grown = (self.slots.capacity() * 2).max(MIN_SLOTS);
            self.slots.reserve_exact(grown - self.slots.len());
            self.free.reserve_exact(grown.saturating_sub(self.free.len()));
        }
        let Ok(index) = u32::try_from(self.slots.len()) else { return Err(slot) };
        self.slots.push(slot);
        Ok(index)
    }

    fn evict(&mut self, index: u32) {
        self.unlink(index);
        let Some(slot) = self.slot_mut(index) else { return };
        let (key, bytes) = (slot.key, slot.image.bytes());
        slot.image = GlyphImage::empty(None);
        let _removed = self.map.remove(&key);
        self.pixel_bytes = self.pixel_bytes.saturating_sub(bytes);
        self.free.push(index);
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

/// Slots allocated the first time: the smallest growth step.
const MIN_SLOTS: usize = 4;
/// Extra control bytes `hashbrown` keeps after its buckets (one SIMD group, at most 16).
const MAP_GROUP_WIDTH: usize = 16;

/// Buckets behind a `hashbrown` map of `capacity` (capacity is 7/8 of a power-of-two bucket
/// count, or the bucket count minus one below 8 buckets).
fn map_buckets(capacity: usize) -> usize {
    match capacity {
        0 => 0,
        1..=3 => 4,
        4..=7 => 8,
        _ => (capacity * 8).div_ceil(7).next_power_of_two(),
    }
}

/// The capacity `hashbrown` gives `buckets` buckets.
fn map_capacity(buckets: usize) -> usize {
    if buckets < 8 { buckets.saturating_sub(1) } else { buckets / 8 * 7 }
}

#[cfg(test)]
mod tests;
