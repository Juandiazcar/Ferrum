//! Spike S3: winit + softbuffer window drawing a cell grid with a swash glyph cache.
//! Output is line-oriented `RESULT ...` text so CI logs and the ADR can quote it verbatim.
//!
//! Modes: `repaint` (timings), `mem` (memory phases), `skia` (needs `--features skia`).
//! Usage: s3 --mode repaint|mem|skia [--size 1440x900] [--rows 55] [--cols 200] [--physical]
//!        [--font PATH] [--iters N] [--idle-secs N] [--skia] [--dialog]

#[macro_use]
mod out;
mod app;
mod args;
mod bench;
#[cfg_attr(not(feature = "dialog"), path = "dialog_stub.rs")]
mod dialog;
mod font;
mod gfx;
mod grid;
mod mem;
mod render;
#[cfg_attr(not(feature = "skia"), path = "skia_stub.rs")]
mod skia;
mod stats;

use winit::event_loop::EventLoop;

fn main() {
    let args = args::Args::parse();
    out!(
        "RESULT env os={} arch={} features=[skia={} dialog={}] mode={}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        cfg!(feature = "skia"),
        cfg!(feature = "dialog"),
        args.mode
    );
    if args.mode == "skia" {
        skia::bench();
        return;
    }
    mem::report("0_process_start");
    let Ok(el) = EventLoop::new() else {
        out!("RESULT error no_event_loop");
        return;
    };
    let mut app = app::App::new(args);
    if let Err(e) = el.run_app(&mut app) {
        out!("RESULT error event_loop {e}");
    }
}
