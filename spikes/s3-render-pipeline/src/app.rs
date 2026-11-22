//! Event loop: opens the window on `resumed`, measures on the first redraw, then idles and
//! (with `--dialog`) opens the native folder dialog while memory is sampled.

use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::WindowId;

use crate::args::Args;
use crate::bench;
use crate::dialog;
use crate::gfx::Gfx;
use crate::mem;
use crate::skia::Tiles;

#[derive(PartialEq)]
enum Stage {
    Init,
    Idle,
    Dialog(usize),
    Done,
}

/// Labels and waits (ms before the next sample) of the dialog phases. The external sampler closes
/// the dialog after phase 6.
const DIALOG_PHASES: [(&str, u64); 4] = [
    ("5_dialog_open_1.5s", 1000),
    ("6_dialog_open_4s", 2000),
    ("7_dialog_closed_2s", 4000),
    ("8_dialog_closed_6s", 0),
];

pub(crate) struct App {
    args: Args,
    gfx: Option<Gfx>,
    stage: Stage,
    tiles: Option<Tiles>,
}

impl App {
    pub(crate) fn new(args: Args) -> App {
        App { args, gfx: None, stage: Stage::Init, tiles: None }
    }

    fn wait_for(&mut self, el: &ActiveEventLoop, stage: Stage, ms: u64) {
        self.stage = stage;
        el.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(ms.max(1)),
        ));
    }

    fn finish(&mut self, el: &ActiveEventLoop) {
        self.stage = Stage::Done;
        el.exit();
    }

    fn on_idle(&mut self, el: &ActiveEventLoop) {
        mem::report(&format!("4_idle_{}s", self.args.idle));
        if self.args.dialog {
            dialog::spawn();
            self.wait_for(el, Stage::Dialog(0), 1500);
        } else {
            self.finish(el);
        }
    }

    fn on_dialog(&mut self, el: &ActiveEventLoop, n: usize) {
        let Some(&(label, wait)) = DIALOG_PHASES.get(n) else { return self.finish(el) };
        mem::report(label);
        if n + 1 == DIALOG_PHASES.len() {
            self.finish(el);
        } else {
            self.wait_for(el, Stage::Dialog(n + 1), wait);
        }
    }

    /// First redraw: run the selected measurement.
    fn on_first_redraw(&mut self, el: &ActiveEventLoop) {
        let Some(g) = self.gfx.as_mut() else { return };
        let ok = if self.args.mode == "repaint" {
            self.stage = Stage::Done;
            bench::run_repaint(&self.args, g)
        } else {
            self.tiles = self.args.skia.then(Tiles::build);
            let r = bench::present_frames(&self.args, g, self.tiles.as_ref());
            mem::report("3_after_4_presents");
            self.wait_for(el, Stage::Idle, self.args.idle * 1000);
            r
        };
        if ok.is_none() {
            out!("RESULT error measurement_failed");
            self.finish(el);
        } else if self.stage == Stage::Done {
            el.exit();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        if let Some(g) = Gfx::open(el, &self.args) {
            g.window.request_redraw();
            self.gfx = Some(g);
        } else {
            out!("RESULT error setup_failed");
            el.exit();
        }
    }

    fn new_events(&mut self, el: &ActiveEventLoop, cause: StartCause) {
        if !matches!(cause, StartCause::ResumeTimeReached { .. }) {
            return;
        }
        match self.stage {
            Stage::Idle => self.on_idle(el),
            Stage::Dialog(n) => self.on_dialog(el, n),
            Stage::Init | Stage::Done => {}
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested if self.stage == Stage::Init => self.on_first_redraw(el),
            _ => {}
        }
    }
}
