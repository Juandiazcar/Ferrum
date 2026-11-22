//! Repaint timings (`--mode repaint`) and the frames of the memory run (`--mode mem`).

use std::num::NonZeroU32;
use std::time::Instant;

use softbuffer::Rect;

use crate::args::Args;
use crate::gfx::Gfx;
use crate::grid::{BG, Grid};
use crate::render::{Target, draw_rows};
use crate::skia::Tiles;
use crate::stats::{self, Frames, micros};

/// Warm-up iterations discarded before the measured ones.
const WARMUP: usize = 30;
/// Grid rows repainted in the damage-limited case.
const DAMAGE_ROWS: usize = 4;

/// All repaint measurements for the typical and the dense grid.
pub(crate) fn run_repaint(args: &Args, g: &mut Gfx) -> Option<()> {
    let typical = Grid::generate(args.cols, args.rows, false);
    let dense = Grid::generate(args.cols, args.rows, true);
    // The window may be smaller than requested (compositor or screen limits): cells beyond the
    // buffer are clipped, so report how many cells were really drawn.
    let drawn_cols = args.cols.min(g.w as usize / g.metrics.cell_w);
    let drawn_rows = args.rows.min(g.h as usize / g.metrics.cell_h);
    out!(
        "RESULT repaint_setup cells={}x{} drawn_cells={drawn_cols}x{drawn_rows} iters={} buffer={}x{}",
        args.cols,
        args.rows,
        args.iters,
        g.w,
        g.h
    );
    for (name, grid) in [("typical", &typical), ("dense", &dense)] {
        cpu_only(args, g, name, grid)?;
        full_frame(args, g, name, grid)?;
    }
    damage_frame(args, g, &typical)?;
    full_rect_present(args, g)?;
    out!(
        "RESULT glyph_cache glyphs={} total_bytes={} font_file_bytes={}",
        g.cache.glyphs(),
        g.cache.bytes(),
        g.font.file_bytes
    );
    Some(())
}

/// CPU time of a full repaint into the real softbuffer buffer (acquired, never presented).
fn cpu_only(args: &Args, gfx: &mut Gfx, name: &str, grid: &Grid) -> Option<()> {
    let (width, height) = (gfx.w as usize, gfx.h as usize);
    let mut samples = Vec::new();
    let mut age = 0;
    for iter in 0..args.iters + WARMUP {
        let mut buf = gfx.surface.buffer_mut().ok()?;
        age = buf.age();
        let start = Instant::now();
        draw_rows(
            &mut Target { buf: &mut buf, width, height },
            grid,
            &gfx.cache,
            &gfx.metrics,
            0..grid.rows,
        );
        let elapsed = micros(start.elapsed());
        std::hint::black_box(&buf);
        if iter >= WARMUP {
            samples.push(elapsed);
        }
    }
    stats::emit(
        &format!("full_repaint kind={name} cpu_only"),
        &mut samples,
        &format!("buffer_age={age}"),
    );
    Some(())
}

/// A whole frame: acquire the buffer, repaint everything, present it all.
fn full_frame(args: &Args, g: &mut Gfx, name: &str, grid: &Grid) -> Option<()> {
    let (w, h) = (g.w as usize, g.h as usize);
    let mut f = Frames::default();
    for i in 0..args.iters + WARMUP {
        let t0 = Instant::now();
        let mut buf = g.surface.buffer_mut().ok()?;
        let t1 = Instant::now();
        draw_rows(
            &mut Target { buf: &mut buf, width: w, height: h },
            grid,
            &g.cache,
            &g.metrics,
            0..grid.rows,
        );
        let t2 = Instant::now();
        buf.present().ok()?;
        if i >= WARMUP {
            f.push([t0, t1, t2, Instant::now()]);
        }
    }
    stats::emit(&format!("full_frame kind={name} part=acquire"), &mut f.acquire, "");
    stats::emit(&format!("full_frame kind={name} part=draw"), &mut f.draw, "");
    stats::emit(&format!("full_frame kind={name} part=present_full"), &mut f.present, "");
    Some(())
}

/// Damage-limited: repaint 4 rows then `present_with_damage` with that rectangle.
fn damage_frame(args: &Args, g: &mut Gfx, grid: &Grid) -> Option<()> {
    let (w, h) = (g.w as usize, g.h as usize);
    let rows = 10..10 + DAMAGE_ROWS;
    let px = |n: usize| u32::try_from(n).ok();
    let rect = Rect {
        x: 0,
        y: px(rows.start * g.metrics.cell_h)?,
        width: NonZeroU32::new(px(args.cols * g.metrics.cell_w)?)?,
        height: NonZeroU32::new(px(DAMAGE_ROWS * g.metrics.cell_h)?)?,
    };
    let mut f = Frames::default();
    let mut age = 0;
    for i in 0..args.iters + WARMUP {
        let t0 = Instant::now();
        let mut buf = g.surface.buffer_mut().ok()?;
        age = buf.age();
        let t1 = Instant::now();
        draw_rows(
            &mut Target { buf: &mut buf, width: w, height: h },
            grid,
            &g.cache,
            &g.metrics,
            rows.clone(),
        );
        let t2 = Instant::now();
        buf.present_with_damage(&[rect]).ok()?;
        if i >= WARMUP {
            f.push([t0, t1, t2, Instant::now()]);
        }
    }
    let label = |part: &str| {
        format!("damage_frame rows={DAMAGE_ROWS} rect={}x{} part={part}", rect.width, rect.height)
    };
    let extra = format!("buffer_age={age}");
    stats::emit(&label("acquire"), &mut f.acquire, &extra);
    stats::emit(&label("draw_4_rows"), &mut f.draw, &extra);
    stats::emit(&label("present_with_damage"), &mut f.present, &extra);
    Some(())
}

/// Reference: `present_with_damage` over the whole frame (is damage honored or is it a full copy?).
fn full_rect_present(args: &Args, g: &mut Gfx) -> Option<()> {
    let full = Rect { x: 0, y: 0, width: NonZeroU32::new(g.w)?, height: NonZeroU32::new(g.h)? };
    let mut v = Vec::new();
    for i in 0..args.iters + WARMUP {
        let buf = g.surface.buffer_mut().ok()?;
        let t = Instant::now();
        buf.present_with_damage(&[full]).ok()?;
        if i >= WARMUP {
            v.push(micros(t.elapsed()));
        }
    }
    stats::emit("damage_frame rows=full part=present_with_damage_full_rect", &mut v, "");
    Some(())
}

/// Four full presents: a double-buffered backend (`wl_shm`) only allocates its second buffer on the
/// second present, so one present would understate the steady state. Every pixel is touched, like
/// a real window (untouched pages are not resident).
pub(crate) fn present_frames(args: &Args, g: &mut Gfx, tiles: Option<&Tiles>) -> Option<()> {
    let grid = Grid::generate(args.cols, args.rows, false);
    let (w, h) = (g.w as usize, g.h as usize);
    for n in 0..4 {
        let mut buf = g.surface.buffer_mut().ok()?;
        out!("RESULT present n={n} buffer_age={}", buf.age());
        buf.fill(BG);
        draw_rows(
            &mut Target { buf: &mut buf, width: w, height: h },
            &grid,
            &g.cache,
            &g.metrics,
            0..grid.rows,
        );
        if let Some(t) = tiles {
            t.blit_all(&mut buf, w);
            if n == 0 {
                out!("RESULT skia_tiles bytes={}", t.bytes());
            }
        }
        buf.present().ok()?;
    }
    Some(())
}
