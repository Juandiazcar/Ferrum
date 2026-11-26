//! Blending a glyph image into the target, clipped.

use super::Placement;
use super::cache::{GlyphImage, Pixels};
use crate::blit::{Kernel, Shape};
use crate::target::{Rect, Target};

/// Blends `image` into `target` at `place`, clipped to the clip rectangle, the target and the
/// glyph's row of cells. Rows are blended with their transparent padding when it is visible, so
/// narrow glyphs take whole SIMD steps.
// perf: `render_repaint/*` — one call per glyph, not per row.
pub(super) fn blit(target: &mut Target<'_>, kernel: Kernel, image: &GlyphImage, place: &Placement) {
    let band = Rect::new(0, place.y, target.width(), place.metrics.height);
    let clip = place.clip.intersect(&band).intersect(&target.bounds());
    let left = i64::from(place.x) + i64::from(image.left) + i64::from(place.shift.0);
    let top = i64::from(place.y) + i64::from(place.metrics.baseline) - i64::from(image.top)
        + i64::from(place.shift.1);
    let columns = clip_span(left, image.stride, (clip.x, clip.right()));
    let lines = clip_span(top, image.height, (clip.y, clip.bottom()));
    let (Some((x, skip, width)), Some((y, first, rows))) = (columns, lines) else { return };
    let (Ok(src_stride), Ok(width), Ok(rows), Ok(dst_stride)) = (
        usize::try_from(image.stride),
        usize::try_from(width),
        usize::try_from(rows),
        usize::try_from(target.width()),
    ) else {
        return;
    };
    let shape = Shape { width, rows, dst_stride, src_stride };
    let from = first.saturating_mul(src_stride).saturating_add(skip);
    let color = place.color.pixel();
    let Some(dst) = target.tail_mut(x, y) else { return };
    match &image.pixels {
        Pixels::Mask(mask) => {
            if let Some(mask) = mask.get(from..) {
                kernel.blend_mask_rect(dst, mask, color, shape);
            }
        }
        Pixels::Subpixel(coverage) => {
            if let Some(coverage) = coverage.get(from..) {
                kernel.blend_subpixel_rect(dst, coverage, color, shape);
            }
        }
        Pixels::Color(pixels) => {
            if let Some(pixels) = pixels.get(from..) {
                kernel.blend_color_rect(dst, pixels, shape);
            }
        }
    }
}

/// The visible part of `len` pixels starting at `start` inside `lo..hi`: where it starts on the
/// target, how many pixels are skipped at its start, how many remain.
fn clip_span(start: i64, len: u32, (lo, hi): (u32, u32)) -> Option<(u32, usize, u32)> {
    let from = start.max(i64::from(lo));
    let to = (start + i64::from(len)).min(i64::from(hi));
    if to <= from {
        return None;
    }
    let skip = usize::try_from(from - start).ok()?;
    Some((u32::try_from(from).ok()?, skip, u32::try_from(to - from).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_are_clipped_on_both_sides() {
        assert_eq!(clip_span(-3, 10, (0, 100)), Some((0, 3, 7)));
        assert_eq!(clip_span(95, 10, (0, 100)), Some((95, 0, 5)));
        assert_eq!(clip_span(100, 10, (0, 100)), None);
        assert_eq!(clip_span(i64::from(u32::MAX), 10, (0, u32::MAX)), None);
    }
}
