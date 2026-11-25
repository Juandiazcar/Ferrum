//! A coverage mask the size of a cell, with the few drawing operations the synthesized glyphs
//! need: solid rectangles on whole pixels, and anti-aliased shapes given by a coverage function.

use crate::font::metrics::f32_from;

/// An 8-bit coverage mask, `width × height`, row after row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Canvas {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u8>,
}

impl Canvas {
    pub(crate) fn new(width: u32, height: u32) -> Self {
        let len = usize::try_from(u64::from(width) * u64::from(height)).unwrap_or(0);
        Self { width, height, pixels: vec![0; len] }
    }

    /// Sets the pixels of columns `x0..x1` and rows `y0..y1` (clipped) to at least `value`.
    pub(crate) fn fill(&mut self, (x0, x1): (u32, u32), (y0, y1): (u32, u32), value: u8) {
        let (x1, y1) = (x1.min(self.width), y1.min(self.height));
        let width = usize::try_from(self.width).unwrap_or(0);
        for y in y0..y1 {
            let row = usize::try_from(y).unwrap_or(usize::MAX).saturating_mul(width);
            let start = row.saturating_add(usize::try_from(x0).unwrap_or(usize::MAX));
            let end = row.saturating_add(usize::try_from(x1).unwrap_or(0));
            if let Some(span) = self.pixels.get_mut(start..end) {
                for pixel in span.iter_mut() {
                    *pixel = (*pixel).max(value);
                }
            }
        }
    }

    /// Adds the coverage `shape` gives (0.0 to 1.0) at the center of every pixel; pixels keep
    /// the larger of their old and new coverage, so shapes can overlap.
    pub(crate) fn cover(&mut self, shape: impl Fn(f32, f32) -> f32) {
        let width = usize::try_from(self.width).unwrap_or(1).max(1);
        for (index, pixel) in self.pixels.iter_mut().enumerate() {
            let (x, y) = (index % width, index / width);
            let center = |n: usize| f32_from(u32::try_from(n).unwrap_or(u32::MAX)) + 0.5;
            let coverage = shape(center(x), center(y)).clamp(0.0, 1.0);
            *pixel = (*pixel).max(to_byte(coverage));
        }
    }

    /// The mask mirrored left to right.
    pub(crate) fn mirrored(mut self) -> Self {
        let width = usize::try_from(self.width).unwrap_or(1).max(1);
        self.pixels.chunks_mut(width).for_each(<[u8]>::reverse);
        self
    }
}

/// Coverage 0.0–1.0 as a byte.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to 0..=1 first, so 0..=255"
)]
pub(crate) fn to_byte(coverage: f32) -> u8 {
    (coverage.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Coverage of a stroke of `thickness` at `distance` from its center line: a one-pixel ramp
/// across each edge (anti-aliasing by distance).
pub(crate) fn stroke(distance: f32, thickness: f32) -> f32 {
    thickness / 2.0 + 0.5 - distance.abs()
}

/// Distance from `point` to the segment from `from` to `to`.
pub(crate) fn segment_distance(point: (f32, f32), from: (f32, f32), to: (f32, f32)) -> f32 {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = dx * dx + dy * dy;
    let along = if length > 0.0 {
        (((point.0 - from.0) * dx + (point.1 - from.1) * dy) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (gap_x, gap_y) = (from.0 + along * dx - point.0, from.1 + along * dy - point.1);
    (gap_x * gap_x + gap_y * gap_y).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_is_clipped_and_keeps_the_larger_coverage() {
        let mut canvas = Canvas::new(4, 2);
        canvas.fill((1, 9), (0, 1), 100);
        canvas.fill((2, 3), (0, 9), 50);
        assert_eq!(canvas.pixels, [0, 100, 100, 100, 0, 0, 50, 0]);
    }

    #[test]
    fn a_stroke_ramps_over_one_pixel_at_each_edge() {
        assert!((stroke(0.0, 1.0) - 1.0).abs() < f32::EPSILON);
        assert!((stroke(0.5, 1.0) - 0.5).abs() < f32::EPSILON);
        assert!(stroke(1.0, 1.0) <= 0.0);
    }

    #[test]
    fn mirroring_reverses_every_row() {
        let mut canvas = Canvas::new(3, 2);
        canvas.fill((0, 1), (0, 2), 9);
        assert_eq!(canvas.mirrored().pixels, [0, 0, 9, 0, 0, 9]);
    }

    #[test]
    fn segment_distance_is_perpendicular_or_to_the_end() {
        assert!((segment_distance((1.0, 1.0), (0.0, 0.0), (2.0, 0.0)) - 1.0).abs() < 1e-6);
        assert!((segment_distance((3.0, 0.0), (0.0, 0.0), (2.0, 0.0)) - 1.0).abs() < 1e-6);
    }
}
