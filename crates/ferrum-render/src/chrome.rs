//! Chrome painter helpers: rounded rectangles and shadows as cached tiles (`docs/02` §6,
//! ADR 0025 decision 1).
//!
//! A tile is drawn once with `tiny-skia` (shadows are then blurred) at the smallest size that
//! holds its corners and one middle row and column, and kept by the caller. Drawing it at any
//! size is a nine-slice composite: the corners as they are, the middle row and column repeated
//! along the edges, the center pixel over the inside. No `tiny-skia` call runs while painting a
//! frame (ADR 0025 consequences), and drawing allocates nothing. Sizes, radii, colors and blur
//! come from the caller (the theme's design tokens, `docs/09` §9).

mod blur;

use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::blit::{Kernel, Shape, blend_color_pixel};
use crate::color::Rgb;
use crate::error::RenderError;
use crate::font::metrics::{f32_from, to_px};
use crate::target::{Rect, Target};

/// Largest tile side in pixels. A tile only holds two corners and one pixel between them, so
/// 1 024 allows corner radius plus border or blur of 511 px, far beyond any widget of a
/// 1440×900 window (`docs/09` uses 6 px), while one tile stays under 4 MiB.
pub const MAX_TILE_SIDE: u32 = 1024;

/// Cubic Bézier control distance that approximates a quarter circle: 4/3 × (√2 − 1).
const KAPPA: f32 = 0.552_284_8;

/// A rounded rectangle, optionally bordered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundedRect {
    /// Corner radius in pixels.
    pub radius: f32,
    /// Fill color.
    pub fill: Rgb,
    /// Border drawn inside the edge.
    pub border: Option<Border>,
}

/// A border line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Border {
    /// Line color.
    pub color: Rgb,
    /// Line width in pixels.
    pub width: f32,
}

/// The shadow of a rounded rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shadow {
    /// Corner radius of the shadowed shape, in pixels.
    pub radius: u32,
    /// How far the shadow fades out beyond the shape, in pixels.
    pub blur: u32,
    /// Shadow color.
    pub color: Rgb,
    /// Opacity where the shadow is solid (255 opaque).
    pub alpha: u8,
}

impl Shadow {
    /// The rectangle to draw this shadow's tile into, for a shape at `shape` moved by `offset`.
    pub fn bounds(&self, shape: Rect, offset: (i32, i32)) -> Rect {
        let moved =
            |position: u32, by: i32| position.saturating_add_signed(by).saturating_sub(self.blur);
        let grow = |len: u32| len.saturating_add(self.blur.saturating_mul(2));
        Rect::new(
            moved(shape.x, offset.0),
            moved(shape.y, offset.1),
            grow(shape.width),
            grow(shape.height),
        )
    }
}

/// A premultiplied image drawn by nine-slicing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    /// Side of the square tile.
    side: u32,
    /// Width of each corner; the pixel row and column after it are the repeated middle.
    corner: u32,
    /// Premultiplied `0xAARRGGBB`, row after row.
    pixels: Vec<u32>,
}

impl Tile {
    /// The tile of `style`.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidTileSize`] for a negative or non-finite radius or border width, or
    /// one that makes the tile larger than [`MAX_TILE_SIDE`].
    pub fn rounded_rect(style: &RoundedRect) -> Result<Self, RenderError> {
        let border = style.border.map_or(0.0, |border| border.width);
        let valid = |value: f32| value.is_finite() && value >= 0.0;
        if !valid(style.radius) || !valid(border) {
            return Err(RenderError::InvalidTileSize);
        }
        let corner = to_px((style.radius + border).ceil());
        let side = tile_side(corner)?;
        let mut pixmap = Pixmap::new(side, side).ok_or(RenderError::InvalidTileSize)?;
        let extent = f32_from(side);
        let outer = rounded_path(0.0, extent, style.radius).ok_or(RenderError::InvalidTileSize)?;
        pixmap.fill_path(
            &outer,
            &paint(style.fill),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        if let Some(line) = style.border.filter(|line| line.width > 0.0) {
            stroke_border(&mut pixmap, line, style.radius);
        }
        Ok(Self {
            side,
            corner,
            pixels: pixmap
                .pixels()
                .iter()
                .map(|pixel| argb(pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()))
                .collect(),
        })
    }

    /// The tile of `shadow`: its rounded shape inset by the blur, blurred, in the shadow color.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidTileSize`] when radius and blur make the tile larger than
    /// [`MAX_TILE_SIDE`].
    pub fn shadow(shadow: &Shadow) -> Result<Self, RenderError> {
        // The shape's straight middle is two blurs long, so the repeated middle is solid.
        let corner = shadow.radius.saturating_add(shadow.blur.saturating_mul(2));
        let side = tile_side(corner)?;
        let mut pixmap = Pixmap::new(side, side).ok_or(RenderError::InvalidTileSize)?;
        let (inset, extent) = (f32_from(shadow.blur), f32_from(side - shadow.blur));
        let shape = rounded_path(inset, extent, f32_from(shadow.radius))
            .ok_or(RenderError::InvalidTileSize)?;
        pixmap.fill_path(
            &shape,
            &paint(Rgb::BLACK),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        let mut alpha: Vec<u8> = pixmap.pixels().iter().map(|pixel| pixel.alpha()).collect();
        blur::blur(&mut alpha, side, shadow.blur);
        let [_, red, green, blue] = shadow.color.pixel().to_be_bytes();
        let pixels = alpha
            .iter()
            .map(|&coverage| {
                let a = scale(coverage, shadow.alpha);
                argb(scale(red, a), scale(green, a), scale(blue, a), a)
            })
            .collect();
        Ok(Self { side, corner, pixels })
    }

    /// Bytes of pixels the tile holds.
    pub fn bytes(&self) -> usize {
        self.pixels.len() * size_of::<u32>()
    }

    /// Composites the tile over `rect` of `target` (clipped), stretched by nine-slicing. A
    /// rectangle smaller than the two corners shows the corners cut at the middle.
    pub fn draw(&self, target: &mut Target<'_>, rect: Rect) {
        let kernel = Kernel::detect();
        let clip = rect.intersect(&target.bounds());
        let side = usize::try_from(self.side).unwrap_or(0);
        let columns = self.spans(rect.x, rect.width, (clip.x, clip.right()));
        for y in clip.y..clip.bottom() {
            let row = usize::try_from(self.source(y - rect.y, rect.height)).unwrap_or(0);
            for &(x, len, column) in &columns {
                let Some(dst) = target.span_mut(y, x, len) else { continue };
                if let Some(column) = column {
                    let start = row * side + column;
                    if let Some(src) = self.pixels.get(start..start + dst.len()) {
                        let width = dst.len();
                        let shape = Shape { width, rows: 1, dst_stride: width, src_stride: width };
                        kernel.blend_color_rect(dst, src, shape);
                    }
                } else {
                    let middle = usize::try_from(self.corner).unwrap_or(0);
                    let color = self.pixels.get(row * side + middle).copied().unwrap_or(0);
                    for pixel in dst.iter_mut() {
                        *pixel = blend_color_pixel(*pixel, color);
                    }
                }
            }
        }
    }

    /// How many pixels of a `len`-long side come from the start corner and from the end one.
    fn ends(&self, len: u32) -> (u32, u32) {
        let head = self.corner.min(len.div_ceil(2));
        (head, self.corner.min(len - head))
    }

    /// The tile row (or column) drawn at `offset` of a side `len` long.
    fn source(&self, offset: u32, len: u32) -> u32 {
        let (head, tail) = self.ends(len);
        if offset < head {
            offset
        } else if offset >= len - tail {
            self.side - (len - offset)
        } else {
            self.corner
        }
    }

    /// The three column spans of a row (start corner, stretched middle, end corner) clipped to
    /// `lo..hi`: target x, length, and the tile column of the first pixel (`None` for the
    /// middle, one repeated color).
    fn spans(
        &self,
        start: u32,
        total: u32,
        (lo, hi): (u32, u32),
    ) -> [(u32, u32, Option<usize>); 3] {
        let (head, tail) = self.ends(total);
        let parts = [(0, head, true), (head, total - tail, false), (total - tail, total, true)];
        parts.map(|(from, to, copied)| {
            let (x0, x1) = ((start + from).max(lo), (start + to).min(hi));
            let column =
                copied.then(|| usize::try_from(self.source(x0 - start, total)).unwrap_or(0));
            (x0, x1.saturating_sub(x0), column)
        })
    }
}

/// Strokes `line` inside the edge of the square `pixmap`, following corners of `radius`.
fn stroke_border(pixmap: &mut Pixmap, line: Border, radius: f32) {
    let half = line.width / 2.0;
    let extent = f32_from(pixmap.width()) - half;
    let stroke = Stroke { width: line.width, ..Stroke::default() };
    if let Some(inner) = rounded_path(half, extent, (radius - half).max(0.0)) {
        pixmap.stroke_path(&inner, &paint(line.color), &stroke, Transform::identity(), None);
    }
}

/// The side of a tile with corners of `corner` pixels: two corners and the middle pixel.
fn tile_side(corner: u32) -> Result<u32, RenderError> {
    corner
        .checked_mul(2)
        .and_then(|both| both.checked_add(1))
        .filter(|&side| side <= MAX_TILE_SIDE)
        .ok_or(RenderError::InvalidTileSize)
}

/// A rounded square from `from` to `to` on both axes with corners of `radius`.
fn rounded_path(from: f32, to: f32, radius: f32) -> Option<tiny_skia::Path> {
    let radius = radius.min((to - from) / 2.0).max(0.0);
    let handle = radius * (1.0 - KAPPA);
    let mut path = PathBuilder::new();
    path.move_to(from + radius, from);
    path.line_to(to - radius, from);
    path.cubic_to(to - handle, from, to, from + handle, to, from + radius);
    path.line_to(to, to - radius);
    path.cubic_to(to, to - handle, to - handle, to, to - radius, to);
    path.line_to(from + radius, to);
    path.cubic_to(from + handle, to, from, to - handle, from, to - radius);
    path.line_to(from, from + radius);
    path.cubic_to(from, from + handle, from + handle, from, from + radius, from);
    path.close();
    path.finish()
}

fn paint(color: Rgb) -> Paint<'static> {
    let [_, red, green, blue] = color.pixel().to_be_bytes();
    let mut paint = Paint::default();
    paint.set_color_rgba8(red, green, blue, u8::MAX);
    paint.anti_alias = true;
    paint
}

/// Premultiplied channels as `0xAARRGGBB`.
fn argb(red: u8, green: u8, blue: u8, alpha: u8) -> u32 {
    u32::from_be_bytes([alpha, red, green, blue])
}

/// `value × factor / 255`, rounded.
fn scale(value: u8, factor: u8) -> u8 {
    let t = u32::from(value) * u32::from(factor) + 128;
    u8::try_from((t + (t >> 8)) >> 8).unwrap_or(u8::MAX)
}

// Not under Miri: there, and only there, tiny-skia's anti-aliased fill indexes past a small tile
// (an out-of-range slice in its `lowp` pipeline); the native runs on the three OS check the tiles.
#[cfg(test)]
#[cfg(not(miri))]
mod tests {
    use super::*;

    const FILL: Rgb = Rgb::new(40, 44, 52);
    const LINE: Rgb = Rgb::new(232, 89, 12);
    const BG: Rgb = Rgb::new(16, 18, 20);

    fn draw(tile: &Tile, width: u32, height: u32) -> Vec<u32> {
        let mut pixels = vec![BG.pixel(); usize::try_from(width * height).unwrap()];
        let mut target = Target::new(&mut pixels, width, height).unwrap();
        tile.draw(&mut target, Rect::new(0, 0, width, height));
        pixels
    }

    #[test]
    fn a_rounded_rect_tile_holds_two_corners_and_a_middle() {
        let tile =
            Tile::rounded_rect(&RoundedRect { radius: 6.0, fill: FILL, border: None }).unwrap();
        assert_eq!((tile.side, tile.corner), (13, 6));
        assert_eq!(tile.bytes(), 13 * 13 * 4);
    }

    #[test]
    fn a_stretched_rounded_rect_is_filled_inside_and_round_at_the_corners() {
        let border = Border { color: LINE, width: 1.0 };
        let tile =
            Tile::rounded_rect(&RoundedRect { radius: 6.0, fill: FILL, border: Some(border) })
                .unwrap();
        let (width, height) = (120, 32);
        let pixels = draw(&tile, width, height);
        let at = |x: u32, y: u32| pixels[(y * width + x) as usize];
        assert_eq!(at(60, 16), FILL.pixel(), "inside");
        assert_eq!(at(60, 0), LINE.pixel(), "the border along the edge");
        assert_eq!(at(0, 16), LINE.pixel());
        assert_eq!(at(0, 0), BG.pixel(), "outside the rounded corner");
        assert_eq!(at(width - 1, height - 1), BG.pixel());
        assert!(pixels.iter().all(|pixel| pixel >> 24 == 0));
    }

    #[test]
    fn a_rect_smaller_than_the_corners_cuts_them_at_the_middle() {
        let tile =
            Tile::rounded_rect(&RoundedRect { radius: 6.0, fill: FILL, border: None }).unwrap();
        let pixels = draw(&tile, 8, 8);
        assert_eq!(pixels[0], BG.pixel(), "still round");
        assert_eq!(pixels[4 * 8 + 4], FILL.pixel());
        assert_eq!(draw(&tile, 13, 13), draw(&tile, 13, 13), "deterministic");
    }

    #[test]
    fn a_shadow_fades_out_and_is_solid_under_the_shape() {
        let shadow = Shadow { radius: 6, blur: 9, color: Rgb::BLACK, alpha: 200 };
        let tile = Tile::shadow(&shadow).unwrap();
        let shape = Rect::new(20, 20, 100, 40);
        let bounds = shadow.bounds(shape, (0, 4));
        assert_eq!(bounds, Rect::new(11, 15, 118, 58));
        let (width, height) = (160, 100);
        let mut pixels = vec![0x00FF_FFFF; usize::try_from(width * height).unwrap()];
        let mut target = Target::new(&mut pixels, width, height).unwrap();
        tile.draw(&mut target, bounds);
        let at = |x: u32, y: u32| pixels[(y * width + x) as usize] & 0xFF;
        let solid = 255 - u32::from(scale(255, 200));
        assert_eq!(at(70, 44), solid, "under the shape the shadow is at full opacity");
        assert_eq!(at(2, 2), 255, "outside the bounds nothing");
        let fading: Vec<u32> = (bounds.x..shape.x + 4).map(|x| at(x, 44)).collect();
        assert!(fading.windows(2).all(|w| w[0] >= w[1]), "darker toward the shape: {fading:?}");
    }

    #[test]
    fn unusable_sizes_are_refused() {
        let bad =
            |radius: f32| Tile::rounded_rect(&RoundedRect { radius, fill: FILL, border: None });
        assert_eq!(bad(-1.0).err(), Some(RenderError::InvalidTileSize));
        assert_eq!(bad(f32::NAN).err(), Some(RenderError::InvalidTileSize));
        assert_eq!(bad(600.0).err(), Some(RenderError::InvalidTileSize));
        let shadow = Shadow { radius: 6, blur: 300, color: Rgb::BLACK, alpha: 255 };
        assert_eq!(Tile::shadow(&shadow).err(), Some(RenderError::InvalidTileSize));
        assert!(bad(0.0).is_ok(), "a square tile");
    }
}
