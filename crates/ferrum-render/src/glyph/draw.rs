//! Blending a glyph image into the target, clipped.

use super::Placement;
use super::cache::{GlyphImage, Pixels};
use crate::blit::Kernel;
use crate::target::{Rect, Target};

/// Blends `image` into `target` at `place`, clipped to the clip rectangle, the target and the
/// glyph's row of cells.
pub(super) fn blit(target: &mut Target<'_>, kernel: Kernel, image: &GlyphImage, place: &Placement) {
    let band = Rect::new(0, place.y, target.width(), place.metrics.height);
    let clip = place.clip.intersect(&band).intersect(&target.bounds());
    let left = i64::from(place.x) + i64::from(image.left);
    let top = i64::from(place.y) + i64::from(place.metrics.baseline) - i64::from(image.top);
    let columns = clip_span(left, image.width, (clip.x, clip.right()));
    let lines = clip_span(top, image.height, (clip.y, clip.bottom()));
    let (Some((x, skip, len)), Some((y, first, rows))) = (columns, lines) else { return };
    let stride = usize::try_from(image.stride).unwrap_or(0);
    let color = place.color.pixel();
    for row in 0..rows {
        let Some(dst) = target.span_mut(y.saturating_add(row), x, len) else { return };
        let line = first.saturating_add(usize::try_from(row).unwrap_or(usize::MAX));
        let from = line.saturating_mul(stride).saturating_add(skip);
        let to = from.saturating_add(dst.len());
        match &image.pixels {
            Pixels::Mask(mask) => {
                if let Some(mask) = mask.get(from..to) {
                    kernel.blend_mask(dst, mask, color);
                }
            }
            Pixels::Subpixel(coverage) => {
                if let Some(coverage) = coverage.get(from..to) {
                    kernel.blend_subpixel(dst, coverage, color);
                }
            }
            Pixels::Color(pixels) => {
                if let Some(pixels) = pixels.get(from..to) {
                    kernel.blend_color(dst, pixels);
                }
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
