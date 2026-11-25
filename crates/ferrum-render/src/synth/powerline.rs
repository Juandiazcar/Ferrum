//! The powerline separators U+E0B0–U+E0B3, drawn to the exact cell so they align with the
//! backgrounds of the cells next to them (a font's own separators rarely match the cell height).

use super::canvas::{Canvas, segment_distance, stroke};
use crate::font::metrics::f32_from;

/// Draws `ch`: `` solid right-pointing triangle, `` its outline, `` and `` the left-pointing
/// pair, with light lines of `light` pixels.
pub(crate) fn draw(ch: char, width: u32, height: u32, light: u32) -> Option<Canvas> {
    let mut canvas = Canvas::new(width, height);
    let (w, h) = (f32_from(width), f32_from(height));
    let thickness = f32_from(light.max(1));
    // Drawn pointing right; the left-pointing glyphs are mirrored, which keeps the base on the
    // cell edge it touches.
    let tip = (w, h / 2.0);
    match ch {
        '\u{E0B0}' | '\u{E0B2}' => canvas.cover(|x, y| {
            // Inside both slanted edges, anti-aliased by the distance to the nearer one.
            let slope = h / 2.0 / w;
            let half = (h / 2.0 - (y - h / 2.0).abs()).max(0.0);
            let edge_x = half / slope;
            let distance = (edge_x - x) * slope / (1.0 + slope * slope).sqrt();
            distance + 0.5
        }),
        '\u{E0B1}' | '\u{E0B3}' => canvas.cover(|x, y| {
            let upper = segment_distance((x, y), (0.0, 0.0), tip);
            let lower = segment_distance((x, y), (0.0, h), tip);
            stroke(upper.min(lower), thickness)
        }),
        _ => return None,
    }
    Some(if ch >= '\u{E0B2}' { canvas.mirrored() } else { canvas })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_solid_triangle_fills_the_left_edge_and_points_right() {
        let canvas = draw('\u{E0B0}', 8, 16, 1).unwrap();
        let at = |x: usize, y: usize| canvas.pixels[y * 8 + x];
        assert_eq!(at(0, 8), 255, "the base spans the whole left edge");
        assert_eq!(at(0, 1), 255);
        assert_eq!(at(6, 8), 255);
        assert!(at(7, 8) >= 128, "the tip reaches the right edge at the middle");
        assert_eq!(at(7, 0), 0);
        assert_eq!(at(7, 15), 0);
    }

    #[test]
    fn left_glyphs_mirror_the_right_ones() {
        let right = draw('\u{E0B0}', 8, 16, 1).unwrap();
        assert_eq!(draw('\u{E0B2}', 8, 16, 1).unwrap(), right.mirrored());
        let outline = draw('\u{E0B1}', 8, 16, 1).unwrap();
        assert!(outline.pixels.contains(&0), "an outline, not a fill");
        assert_eq!(draw('\u{E0B3}', 8, 16, 1).unwrap(), outline.mirrored());
    }
}
