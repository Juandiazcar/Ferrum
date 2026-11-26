use super::*;
use crate::font::{BOLD_WEIGHT, Family, FontSpec, Presentation, Slant, SystemFonts};

const WHITE: Rgb = Rgb::new(255, 255, 255);
const CACHE_BYTES: usize = 3 << 20;

fn spec() -> FontSpec {
    FontSpec { size_px: 16.0, scale_factor: 1.0, line_height: 1.0, letter_spacing_px: 0.0 }
}

fn glyphs(system: Option<SystemFonts>) -> (Glyphs, CellMetrics) {
    let fonts = Fonts::new(Family::Embedded, system).unwrap();
    let metrics = fonts.cell_metrics(&spec()).unwrap();
    (Glyphs::new(fonts, CACHE_BYTES), metrics)
}

fn run(text: &str, metrics: CellMetrics, clip: Rect) -> TextRun<'_> {
    TextRun { text, x: 0, y: 0, metrics, style: TextStyle::REGULAR, color: WHITE, clip }
}

/// Draws `text` on a black buffer of `cells` cells and returns the pixels and the cell count.
fn draw(
    glyphs: &mut Glyphs,
    metrics: CellMetrics,
    text: &str,
    style: TextStyle,
) -> (Vec<u32>, u32) {
    let (width, height) = (metrics.width * 4, metrics.height);
    let mut pixels = vec![0; usize::try_from(width * height).unwrap()];
    let mut target = Target::new(&mut pixels, width, height).unwrap();
    let mut line = run(text, metrics, target.bounds());
    line.style = style;
    let cells = glyphs.draw_text(&mut target, &line);
    (pixels, cells)
}

fn ink(pixels: &[u32]) -> u64 {
    pixels.iter().map(|&pixel| u64::from(pixel & 0xFF)).sum()
}

#[test]
fn a_glyph_inks_its_own_cell_only() {
    let (mut glyphs, metrics) = glyphs(None);
    let (pixels, cells) = draw(&mut glyphs, metrics, "A", TextStyle::REGULAR);
    assert_eq!(cells, 1);
    let width = usize::try_from(metrics.width * 4).unwrap();
    let cell = usize::try_from(metrics.width).unwrap();
    let inside: Vec<u32> = pixels.chunks(width).flat_map(|row| row[..cell].to_vec()).collect();
    assert!(ink(&inside) > 0, "the glyph is drawn");
    assert_eq!(ink(&pixels), ink(&inside), "nothing outside its cell");
}

#[test]
fn nothing_is_drawn_outside_the_clip() {
    let (mut glyphs, metrics) = glyphs(None);
    let mut pixels = vec![0; usize::try_from(metrics.width * metrics.height).unwrap()];
    let mut target = Target::new(&mut pixels, metrics.width, metrics.height).unwrap();
    let clip = Rect::new(0, 0, metrics.width, metrics.baseline / 2);
    let _cells = glyphs.draw_text(&mut target, &run("A", metrics, clip));
    let (above, below) = pixels.split_at(usize::try_from(metrics.width * clip.height).unwrap());
    assert!(ink(above) > 0);
    assert_eq!(ink(below), 0);
}

#[test]
fn combining_marks_and_ignorables_take_no_cell() {
    let (mut glyphs, metrics) = glyphs(None);
    assert_eq!(draw(&mut glyphs, metrics, "e\u{301}x", TextStyle::REGULAR).1, 2);
    assert_eq!(draw(&mut glyphs, metrics, "a\u{200D}b\u{FE0F}", TextStyle::REGULAR).1, 2);
}

#[test]
fn bold_and_italic_draw_differently() {
    let (mut glyphs, metrics) = glyphs(None);
    let (regular, _) = draw(&mut glyphs, metrics, "H", TextStyle::REGULAR);
    let (bold, _) =
        draw(&mut glyphs, metrics, "H", TextStyle { weight: BOLD_WEIGHT, slant: Slant::Upright });
    let (italic, _) =
        draw(&mut glyphs, metrics, "H", TextStyle { slant: Slant::Italic, ..TextStyle::REGULAR });
    assert!(ink(&bold) > ink(&regular), "bold has more ink");
    assert_ne!(italic, regular, "italic is slanted");
}

#[test]
fn drawing_again_hits_the_cache() {
    let (mut glyphs, metrics) = glyphs(None);
    let first = draw(&mut glyphs, metrics, "abc", TextStyle::REGULAR);
    let (cached, bytes) = (glyphs.cached_glyphs(), glyphs.cache_bytes());
    assert_eq!(cached, 3);
    assert_eq!(draw(&mut glyphs, metrics, "abc", TextStyle::REGULAR), first);
    assert_eq!((glyphs.cached_glyphs(), glyphs.cache_bytes()), (cached, bytes));
}

#[test]
fn a_tiny_cache_still_draws_within_its_cap() {
    let fonts = Fonts::new(Family::Embedded, None).unwrap();
    let metrics = fonts.cell_metrics(&spec()).unwrap();
    let mut glyphs = Glyphs::new(fonts, 2_000);
    for text in ["abcd", "efgh", "ijkl"] {
        let (pixels, _) = draw(&mut glyphs, metrics, text, TextStyle::REGULAR);
        assert!(ink(&pixels) > 0);
        assert!(glyphs.cache_bytes() <= 2_000);
    }
}

#[test]
fn a_glyph_larger_than_the_cache_is_drawn_uncached() {
    let fonts = Fonts::new(Family::Embedded, None).unwrap();
    let metrics = fonts.cell_metrics(&spec()).unwrap();
    let mut glyphs = Glyphs::new(fonts, 16);
    let (pixels, cells) = draw(&mut glyphs, metrics, "W", TextStyle::REGULAR);
    assert_eq!(cells, 1);
    assert!(ink(&pixels) > 0, "drawn although it is never kept");
    assert_eq!(glyphs.cached_glyphs(), 0);
}

#[test]
fn letter_spacing_does_not_change_the_cells_a_glyph_takes() {
    let fonts = Fonts::new(Family::Embedded, None).unwrap();
    let narrow = fonts.cell_metrics(&FontSpec { letter_spacing_px: -2.0, ..spec() }).unwrap();
    let mut glyphs = Glyphs::new(fonts, CACHE_BYTES);
    assert!(narrow.width < narrow.advance);
    assert_eq!(draw(&mut glyphs, narrow, "abc", TextStyle::REGULAR).1, 3);
}

#[test]
#[cfg_attr(miri, ignore = "fontdb maps the font files; Miri cannot map memory")]
fn a_glyph_that_cannot_be_rasterized_still_takes_a_cell() {
    let (dir, system) = crate::test_fonts::fixture_fonts();
    let (mut glyphs, metrics) = glyphs(Some(system));
    let greek = glyphs.fonts.glyph('λ', TextStyle::REGULAR, Presentation::Text);
    assert_ne!(greek.face, crate::font::FaceId::EMBEDDED);
    drop(dir); // the face's file is gone: mapping it fails
    let (pixels, cells) = draw(&mut glyphs, metrics, "λx", TextStyle::REGULAR);
    assert_eq!(cells, 2, "the missing glyph keeps its cell");
    assert!(ink(&pixels) > 0, "x is drawn");
}

#[test]
#[cfg_attr(miri, ignore = "fontdb maps the font files; Miri cannot map memory")]
fn emoji_are_drawn_in_color_over_two_cells() {
    let (_dir, system) = crate::test_fonts::fixture_fonts();
    let (mut glyphs, metrics) = glyphs(Some(system));
    for emoji in ["😀", "☕"] {
        let (pixels, cells) = draw(&mut glyphs, metrics, emoji, TextStyle::REGULAR);
        assert_eq!(cells, 2, "{emoji}");
        let colored = pixels.iter().any(|&pixel| {
            let [blue, green, red, _] = pixel.to_le_bytes();
            red != green || green != blue
        });
        assert!(colored, "{emoji} is a color glyph: a white mask has equal channels");
        let width = usize::try_from(metrics.width * 4).unwrap();
        let two = usize::try_from(metrics.width * 2).unwrap();
        assert!(pixels.chunks(width).all(|row| row[two..].iter().all(|&pixel| pixel == 0)));
    }
}

#[test]
#[cfg_attr(miri, ignore = "fontdb maps the font files; Miri cannot map memory")]
fn glyphs_of_dropped_faces_leave_the_cache() {
    let (_dir, system) = crate::test_fonts::fixture_fonts();
    let (mut glyphs, metrics) = glyphs(Some(system));
    glyphs.fonts.set_face_cap(1);
    let _greek = draw(&mut glyphs, metrics, "λ", TextStyle::REGULAR);
    let with_greek = glyphs.cached_glyphs();
    let _cyrillic = draw(&mut glyphs, metrics, "Ж", TextStyle::REGULAR);
    let (pixels, _) = draw(&mut glyphs, metrics, "x", TextStyle::REGULAR);
    assert!(ink(&pixels) > 0);
    assert_eq!(glyphs.cached_glyphs(), with_greek + 1, "λ left, Ж and x came");
}

#[test]
fn lcd_text_blends_each_channel() {
    let (mut glyphs, metrics) = glyphs(None);
    glyphs.set_antialias(Antialias::Lcd);
    let (pixels, _) = draw(&mut glyphs, metrics, "W", TextStyle::REGULAR);
    let fringed = pixels.iter().any(|&pixel| {
        let [blue, green, red, _] = pixel.to_le_bytes();
        red != green || green != blue
    });
    assert!(fringed, "white text on black gets colored edges with LCD antialiasing");
}
