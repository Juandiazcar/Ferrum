//! `tiny-skia` chrome primitives drawn on small tiles (docs/02 §6), timed individually (feature `skia`).

use std::time::Instant;

use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, Path, PathBuilder, Pixmap, Point,
    SpreadMode, Stroke, Transform,
};

use crate::stats::{self, micros};

/// Rounded rectangle path from its left, top, width, height and corner radius.
fn rrect(left: f32, top: f32, width: f32, height: f32, radius: f32) -> Option<Path> {
    let (right, bottom) = (left + width, top + height);
    let kappa = 0.552_284_7 * radius;
    let mut path = PathBuilder::new();
    path.move_to(left + radius, top);
    path.line_to(right - radius, top);
    path.cubic_to(right - radius + kappa, top, right, top + radius - kappa, right, top + radius);
    path.line_to(right, bottom - radius);
    path.cubic_to(
        right,
        bottom - radius + kappa,
        right - radius + kappa,
        bottom,
        right - radius,
        bottom,
    );
    path.line_to(left + radius, bottom);
    path.cubic_to(
        left + radius - kappa,
        bottom,
        left,
        bottom - radius + kappa,
        left,
        bottom - radius,
    );
    path.line_to(left, top + radius);
    path.cubic_to(left, top + radius - kappa, left + radius - kappa, top, left + radius, top);
    path.close();
    path.finish()
}

fn solid(red: u8, green: u8, blue: u8, alpha: u8) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(red, green, blue, alpha));
    paint.anti_alias = true;
    paint
}

fn button(px: &mut Pixmap) {
    if let Some(p) = rrect(1.0, 1.0, 238.0, 30.0, 8.0) {
        px.fill_path(
            &p,
            &solid(0x31, 0x32, 0x44, 255),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn border(px: &mut Pixmap) {
    if let Some(p) = rrect(1.5, 1.5, 237.0, 29.0, 8.0) {
        px.stroke_path(
            &p,
            &solid(0x89, 0xB4, 0xFA, 255),
            &Stroke::default(),
            Transform::identity(),
            None,
        );
    }
}

fn gradient(px: &mut Pixmap) {
    let stops = vec![
        GradientStop::new(0.0, Color::from_rgba8(0x45, 0x47, 0x5A, 255)),
        GradientStop::new(1.0, Color::from_rgba8(0x31, 0x32, 0x44, 255)),
    ];
    let sh = LinearGradient::new(
        Point::from_xy(0.0, 0.0),
        Point::from_xy(0.0, 32.0),
        stops,
        SpreadMode::Pad,
        Transform::identity(),
    );
    if let (Some(sh), Some(p)) = (sh, rrect(1.0, 1.0, 238.0, 30.0, 8.0)) {
        let paint = Paint { shader: sh, anti_alias: true, ..Paint::default() };
        px.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

/// Shadow tile: tiny-skia has no blur, so stacked rounded rects with falling alpha.
fn shadow(px: &mut Pixmap) {
    for step in 0..12u8 {
        let inset = f32::from(step);
        let size = 96.0 - 2.0 * inset;
        if let Some(path) = rrect(inset, inset, size, size, 14.0 - inset.min(10.0)) {
            px.fill_path(&path, &solid(0, 0, 0, 7), FillRule::Winding, Transform::identity(), None);
        }
    }
}

fn time(name: &str, (width, height): (u32, u32), draw: fn(&mut Pixmap)) {
    let mut samples = Vec::new();
    for _ in 0..2000 {
        let Some(mut pixmap) = Pixmap::new(width, height) else { return };
        let start = Instant::now();
        draw(&mut pixmap);
        samples.push(micros(start.elapsed()));
        std::hint::black_box(&pixmap);
    }
    let bytes = width * height * 4;
    stats::emit(&format!("skia op={name} tile={width}x{height} bytes={bytes}"), &mut samples, "");
}

/// Per-primitive cost.
pub(crate) fn bench() {
    time("rrect_fill", (240, 32), button);
    time("rrect_stroke", (240, 32), border);
    time("rrect_gradient", (240, 32), gradient);
    time("shadow_tile", (96, 96), shadow);
}

/// Resident "chrome" tile set (kept alive for the memory measurement).
pub(crate) struct Tiles(Vec<Pixmap>);

impl Tiles {
    /// 24 button-size tiles + 4 shadow tiles: a generous chrome set.
    pub(crate) fn build() -> Tiles {
        let mut v = Vec::new();
        for _ in 0..24 {
            if let Some(mut p) = Pixmap::new(240, 32) {
                button(&mut p);
                border(&mut p);
                v.push(p);
            }
        }
        for _ in 0..4 {
            if let Some(mut p) = Pixmap::new(96, 96) {
                shadow(&mut p);
                v.push(p);
            }
        }
        Tiles(v)
    }

    /// Pixel bytes held by the tiles.
    pub(crate) fn bytes(&self) -> usize {
        self.0.iter().map(|p| p.data().len()).sum()
    }

    /// Composite every tile into the framebuffer (premultiplied src-over onto 0x00RRGGBB).
    pub(crate) fn blit_all(&self, buf: &mut [u32], width: usize) {
        for (i, t) in self.0.iter().enumerate() {
            let (tw, th) = (t.width() as usize, t.height() as usize);
            let (ox, oy) = ((i % 6) * 240, 40 * (i / 6));
            for (y, src) in t.data().chunks_exact(tw * 4).enumerate().take(th) {
                let base = (oy + y) * width + ox;
                let Some(dst) = buf.get_mut(base..base + tw) else { break };
                for (d, s) in dst.iter_mut().zip(src.as_chunks::<4>().0) {
                    *d = over(*d, *s);
                }
            }
        }
    }
}

/// Premultiplied RGBA source `s` over an opaque 0x00RRGGBB destination.
fn over(d: u32, s: [u8; 4]) -> u32 {
    let inv = 255 - u32::from(s[3]);
    let ch =
        |shift: u32, sv: u8| (((d >> shift) & 0xFF) * inv / 255 + u32::from(sv)).min(255) << shift;
    ch(16, s[0]) | ch(8, s[1]) | ch(0, s[2])
}
