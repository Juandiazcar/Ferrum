//! Window, font, glyph cache and softbuffer surface, created in the order the memory phases
//! measure them (window, then font + cache, then surface).

use std::num::NonZeroU32;
use std::sync::Arc;

use softbuffer::{Context, Surface};
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::raw_window_handle::HasWindowHandle;
use winit::window::Window;

use crate::args::Args;
use crate::font::Font;
use crate::mem;
use crate::render::{GlyphCache, Metrics};

/// Everything the measurements draw with.
pub(crate) struct Gfx {
    pub(crate) window: Arc<Window>,
    pub(crate) surface: Surface<Arc<Window>, Arc<Window>>,
    /// Kept alive: the mapped font file stays resident like in the product.
    pub(crate) font: Font,
    pub(crate) cache: GlyphCache,
    pub(crate) metrics: Metrics,
    pub(crate) w: u32,
    pub(crate) h: u32,
}

impl Gfx {
    /// Create the window, load the font, warm the glyph cache and open the surface.
    pub(crate) fn open(el: &ActiveEventLoop, args: &Args) -> Option<Gfx> {
        let window = open_window(el, args)?;
        let sz = window.inner_size();
        let (rw, rh) = args.size;
        let (w, h) = if sz.width == 0 || sz.height == 0 { (rw, rh) } else { (sz.width, sz.height) };
        report_window(&window, args, (w, h));
        mem::report("1_window_created");

        let eff = if args.physical { 1.0 } else { scale_f32(&window) };
        let (font, cache, metrics) = load_glyphs(args, 12.0 * eff)?;
        mem::report("2_font_and_glyph_cache");

        let ctx = Context::new(window.clone()).map_err(|e| out!("RESULT error ctx {e}")).ok()?;
        let mut surface = Surface::new(&ctx, window.clone())
            .map_err(|e| out!("RESULT error surface {e}"))
            .ok()?;
        surface
            .resize(NonZeroU32::new(w)?, NonZeroU32::new(h)?)
            .map_err(|e| out!("RESULT error resize {e}"))
            .ok()?;
        Some(Gfx { window, surface, font, cache, metrics, w, h })
    }
}

/// Display scale factor as `f32`.
#[allow(clippy::cast_possible_truncation)]
// INVARIANT: scale factors are small (1.0 to 8.0), exactly representable well within `f32` range.
fn scale_f32(window: &Window) -> f32 {
    window.scale_factor() as f32
}

fn open_window(el: &ActiveEventLoop, args: &Args) -> Option<Arc<Window>> {
    let (rw, rh) = args.size;
    let attrs = Window::default_attributes().with_title("ferrum-spike-s3");
    let attrs = if args.physical {
        attrs.with_inner_size(PhysicalSize::new(rw, rh))
    } else {
        attrs.with_inner_size(LogicalSize::new(rw, rh))
    };
    Some(Arc::new(el.create_window(attrs).ok()?))
}

fn report_window(window: &Window, args: &Args, (w, h): (u32, u32)) {
    let backend = window.window_handle().map(|h| format!("{:?}", h.as_raw())).unwrap_or_default();
    let backend = backend.split('(').next().unwrap_or("?");
    out!(
        "RESULT window requested={}x{} physical_mode={} scale_factor={} inner_px={w}x{h} surface_bytes_per_buffer={} backend={backend}",
        args.size.0,
        args.size.1,
        args.physical,
        window.scale_factor(),
        u64::from(w) * u64::from(h) * 4
    );
}

/// Load the font and warm the cache with the 94 printable ASCII glyphs.
fn load_glyphs(args: &Args, size_px: f32) -> Option<(Font, GlyphCache, Metrics)> {
    let font = Font::load(args.font.as_deref()).or_else(|| {
        out!("RESULT error font_load");
        None
    })?;
    let (mut cache, metrics) = GlyphCache::new(&font, size_px)?;
    for c in 0x21u8..0x7f {
        cache.warm(&font, c as char);
    }
    out!(
        "RESULT font name={:?} file_bytes={} size_px={size_px} cell={}x{} baseline={} glyphs={} cache_arena_bytes={} cache_total_bytes={} grid_px={}x{}",
        font.name,
        font.file_bytes,
        metrics.cell_w,
        metrics.cell_h,
        metrics.baseline,
        cache.glyphs(),
        cache.arena_len(),
        cache.bytes(),
        args.cols * metrics.cell_w,
        args.rows * metrics.cell_h
    );
    Some((font, cache, metrics))
}
