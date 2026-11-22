//! Glyph cache (8-bit coverage masks) and the cell blitter that draws straight into a
//! `&mut [u32]` (0x00RRGGBB), as docs/02 §6 prescribes.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use crate::font::Font;
use swash::scale::{Render, ScaleContext, Source, image::Content};

#[derive(Default)]
pub(crate) struct IdHasher(u64);
impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, _: &[u8]) {}
    fn write_u32(&mut self, v: u32) {
        self.0 = u64::from(v).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Glyph {
    w: u16,
    h: u16,
    left: i16,
    top: i16,
    off: u32,
}

/// Non-negative font metric (pixels) as `usize`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
// INVARIANT: only metrics of a 12 to 48 px font are passed (values below 100), and the value is
// clamped to zero from below first, so the cast can neither truncate nor lose a sign.
fn px(v: f32) -> usize {
    v.max(0.0) as usize
}

/// Cell geometry in pixels.
pub(crate) struct Metrics {
    pub(crate) cell_w: usize,
    pub(crate) cell_h: usize,
    pub(crate) baseline: usize,
}

/// Glyph cache keyed (glyph id, size). Spike: unbounded map (no LRU); bytes are reported.
pub(crate) struct GlyphCache {
    size_px: f32,
    size_q: u32,
    ctx: ScaleContext,
    map: HashMap<u32, Option<Glyph>, BuildHasherDefault<IdHasher>>,
    arena: Vec<u8>,
    ascii: [u16; 128],
}

impl GlyphCache {
    pub(crate) fn new(font: &Font, size_px: f32) -> Option<(GlyphCache, Metrics)> {
        let f = font.font_ref()?;
        let cm = f.charmap();
        let mut ascii = [0u16; 128];
        for (byte, slot) in (0u8..128).zip(ascii.iter_mut()) {
            *slot = cm.map(char::from(byte));
        }
        let m = f.metrics(&[]).scale(size_px);
        let adv = f.glyph_metrics(&[]).scale(size_px).advance_width(cm.map('M'));
        let metrics = Metrics {
            cell_w: px(adv.round()),
            cell_h: px((m.ascent + m.descent + m.leading).ceil()),
            baseline: px(m.ascent.round()),
        };
        let cache = GlyphCache {
            size_px,
            size_q: u32::try_from(px(size_px * 4.0)).unwrap_or(0),
            ctx: ScaleContext::new(),
            map: HashMap::default(),
            arena: Vec::new(),
            ascii,
        };
        Some((cache, metrics))
    }

    /// Bytes held by the cache: mask arena + map entries (`ScaleContext` is transient, not counted).
    pub(crate) fn bytes(&self) -> usize {
        self.arena.capacity()
            + self.map.capacity() * (size_of::<u32>() + size_of::<Option<Glyph>>() + 1)
    }
    pub(crate) fn glyphs(&self) -> usize {
        self.map.len()
    }
    pub(crate) fn arena_len(&self) -> usize {
        self.arena.len()
    }

    fn gid(&self, ch: char) -> u16 {
        self.ascii.get((ch as usize) & 127).copied().unwrap_or(0)
    }

    fn key(&self, gid: u16) -> u32 {
        (u32::from(gid) << 16) ^ self.size_q
    }

    /// Rasterize the glyph for `ch` into the cache (no-op when already cached).
    pub(crate) fn warm(&mut self, font: &Font, ch: char) {
        let gid = self.gid(ch);
        let key = self.key(gid);
        if self.map.contains_key(&key) {
            return;
        }
        let Some(f) = font.font_ref() else { return };
        let mut sc = self
            .ctx
            .builder(f)
            .size(self.size_px)
            .hint(true)
            .variations(&[("wght", 400.0)])
            .build();
        let img =
            Render::new(&[Source::Outline]).format(swash::zeno::Format::Alpha).render(&mut sc, gid);
        let glyph = img.filter(|i| i.content == Content::Mask && !i.data.is_empty()).map(|i| {
            let off = u32::try_from(self.arena.len()).unwrap_or(u32::MAX);
            self.arena.extend_from_slice(&i.data);
            Glyph {
                w: u16::try_from(i.placement.width).unwrap_or(0),
                h: u16::try_from(i.placement.height).unwrap_or(0),
                left: i16::try_from(i.placement.left).unwrap_or(0),
                top: i16::try_from(i.placement.top).unwrap_or(0),
                off,
            }
        });
        self.map.insert(key, glyph);
    }

    /// Cached glyph and its mask for `ch`.
    #[inline]
    pub(crate) fn get(&self, ch: char) -> Option<(Glyph, &[u8])> {
        let g = (*self.map.get(&self.key(self.gid(ch)))?)?;
        let n = usize::from(g.w) * usize::from(g.h);
        Some((g, self.arena.get(g.off as usize..g.off as usize + n)?))
    }
}

#[inline]
fn blend(bg: u32, fg: u32, a: u32) -> u32 {
    let inv = 255 - a;
    let rb = (bg & 0x00FF_00FF) * inv + (fg & 0x00FF_00FF) * a;
    let g = (bg & 0x0000_FF00) * inv + (fg & 0x0000_FF00) * a;
    let rb = ((rb + 0x0080_0080 + ((rb >> 8) & 0x00FF_00FF)) >> 8) & 0x00FF_00FF;
    let g = ((g + 0x8000 + ((g >> 8) & 0xFF00)) >> 8) & 0xFF00;
    rb | g
}

/// A framebuffer: `width` x `height` pixels, tightly packed.
pub(crate) struct Target<'a> {
    pub(crate) buf: &'a mut [u32],
    pub(crate) width: usize,
    pub(crate) height: usize,
}

/// Draw grid rows `rows` into `t`.
pub(crate) fn draw_rows(
    t: &mut Target<'_>,
    grid: &crate::grid::Grid,
    cache: &GlyphCache,
    m: &Metrics,
    rows: std::ops::Range<usize>,
) {
    for r in rows {
        let y0 = r * m.cell_h;
        let cols = grid.cols.min(t.width / m.cell_w);
        let Some(line) = grid.cells.get(r * grid.cols..r * grid.cols + cols) else { continue };
        fill_backgrounds(t, line, y0, m);
        for (c, cell) in line.iter().enumerate() {
            if cell.ch == ' ' {
                continue;
            }
            if let Some(glyph) = cache.get(cell.ch) {
                blit_glyph(t, glyph, cell.fg, (c * m.cell_w, y0), m);
            }
        }
    }
}

/// One fill per run of equal background colour and scanline.
fn fill_backgrounds(t: &mut Target<'_>, line: &[crate::grid::Cell], y0: usize, m: &Metrics) {
    let mut c = 0;
    while c < line.len() {
        let Some(bg) = line.get(c).map(|x| x.bg) else { return };
        let mut e = c + 1;
        while line.get(e).is_some_and(|x| x.bg == bg) {
            e += 1;
        }
        for y in y0..(y0 + m.cell_h).min(t.height) {
            let base = y * t.width;
            if let Some(run) = t.buf.get_mut(base + c * m.cell_w..base + e * m.cell_w) {
                run.fill(bg);
            }
        }
        c = e;
    }
}

/// Blend one glyph mask over the framebuffer at cell origin `org` (x, y).
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::cast_sign_loss)]
// INVARIANT: coordinates are cell origins inside a framebuffer (width and height below 2^31) plus
// glyph offsets of a few pixels, so every `usize` to `i32` cast is exact; each `i32` to `usize`
// cast happens only after the value was checked to be non-negative (`y < 0`, `gx0 < 0` branches).
fn blit_glyph(
    target: &mut Target<'_>,
    (glyph, mask): (Glyph, &[u8]),
    fg: u32,
    org: (usize, usize),
    metrics: &Metrics,
) {
    let gx0 = org.0 as i32 + i32::from(glyph.left);
    let gy0 = (org.1 + metrics.baseline) as i32 - i32::from(glyph.top);
    for (gy, mrow) in mask.chunks_exact(usize::from(glyph.w).max(1)).enumerate() {
        let y = gy0 + gy as i32;
        if y < 0 || y as usize >= target.height {
            continue;
        }
        // Clip the glyph row to the framebuffer once, then blend over a plain slice.
        let (skip, x_start) = if gx0 < 0 { ((-gx0) as usize, 0usize) } else { (0, gx0 as usize) };
        let Some(mrow) = mrow.get(skip..) else { continue };
        let n = mrow.len().min(target.width.saturating_sub(x_start));
        let base = y as usize * target.width + x_start;
        let Some(dst) = target.buf.get_mut(base..base + n) else { continue };
        for (pixel, &a) in dst.iter_mut().zip(mrow) {
            if a != 0 {
                *pixel = if a == 255 { fg } else { blend(*pixel, fg, u32::from(a)) };
            }
        }
    }
}
