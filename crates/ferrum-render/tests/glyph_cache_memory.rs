//! The glyph cache cap is a memory cap (`docs/02` §6, `docs/03` §3 rule 4): drawing thousands of
//! distinct glyphs grows the heap by at most the cap more than the same drawing with a cache that
//! keeps nothing (whose growth is the rasterizer's and `swash`'s own scratch). One test per
//! binary: `dhat` profiles the whole process.

#![allow(clippy::unwrap_used, reason = "test code: a failed expectation is a failed test")]

use ferrum_render::font::{BOLD_WEIGHT, Family, FontSpec, Fonts, Slant, TextStyle};
use ferrum_render::{Glyphs, Rgb, Target, TextRun};

#[cfg(not(miri))] // dhat is not supported under Miri
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// The cap under test: small enough that the workload overflows it many times over.
const CAP_BYTES: usize = 256 * 1024;
/// Font sizes drawn, in pixels: 94 printable ASCII glyphs in four styles at each.
const SIZES_PX: std::ops::Range<u16> = 8..40;
/// Width and height of the target the lines are drawn into.
const TARGET: (u32, u32) = (4_000, 64);

fn heap() -> usize {
    dhat::HeapStats::get().curr_bytes
}

/// Heap growth of drawing every printable ASCII glyph in four styles at every size.
fn growth(cache_bytes: usize) -> usize {
    let fonts = Fonts::new(Family::Embedded, None).unwrap();
    let ascii: String = (0x21_u8..0x7F).map(char::from).collect();
    let mut pixels = vec![0; usize::try_from(TARGET.0 * TARGET.1).unwrap()];
    let mut glyphs = Glyphs::new(fonts, cache_bytes);
    let before = heap();
    for size in SIZES_PX {
        let spec = FontSpec {
            size_px: f32::from(size),
            scale_factor: 1.0,
            line_height: 1.0,
            letter_spacing_px: 0.0,
        };
        let metrics = glyphs.fonts().cell_metrics(&spec).unwrap();
        let mut target = Target::new(&mut pixels, TARGET.0, TARGET.1).unwrap();
        let clip = target.bounds();
        for weight in [400, BOLD_WEIGHT] {
            for slant in [Slant::Upright, Slant::Italic] {
                let style = TextStyle { weight, slant };
                let color = Rgb::new(255, 255, 255);
                let run = TextRun { text: &ascii, x: 0, y: 0, metrics, style, color, clip };
                let _cells = glyphs.draw_text(&mut target, &run);
            }
        }
    }
    assert!(glyphs.cache_bytes() <= cache_bytes);
    heap() - before
}

fn the_glyph_cache_stays_within_its_cap() {
    let _profiler = dhat::Profiler::builder().testing().build();
    let scratch = growth(0);
    let cached = growth(CAP_BYTES);
    assert!(cached <= scratch + CAP_BYTES, "{cached} > {scratch} + {CAP_BYTES}");
    assert!(
        cached > scratch + CAP_BYTES / 2,
        "the workload fills the cache: {cached} vs {scratch}"
    );
}

fn main() {
    ferrum_testkit::single_test::single_test(
        "the_glyph_cache_stays_within_its_cap",
        the_glyph_cache_stays_within_its_cap,
    );
}
