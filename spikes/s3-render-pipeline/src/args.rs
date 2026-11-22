//! Command line of the harness.

/// Parsed command line (`--mode repaint|mem|skia`, window size, grid size, switches).
pub(crate) struct Args {
    pub(crate) mode: String,
    pub(crate) size: (u32, u32),
    pub(crate) cols: usize,
    pub(crate) rows: usize,
    pub(crate) physical: bool,
    pub(crate) font: Option<String>,
    pub(crate) iters: usize,
    pub(crate) idle: u64,
    pub(crate) skia: bool,
    pub(crate) dialog: bool,
}

impl Args {
    fn defaults() -> Args {
        Args {
            mode: "mem".into(),
            size: (1440, 900),
            cols: 200,
            rows: 55,
            physical: false,
            font: None,
            iters: 300,
            idle: 5,
            skia: false,
            dialog: false,
        }
    }

    /// Parse `std::env::args()`; unknown flags are ignored and bad numbers keep the default.
    pub(crate) fn parse() -> Args {
        let mut a = Args::defaults();
        let mut it = std::env::args().skip(1);
        while let Some(k) = it.next() {
            let mut val = || it.next().unwrap_or_default();
            match k.as_str() {
                "--mode" => a.mode = val(),
                "--size" => a.size = parse_size(&val()).unwrap_or(a.size),
                "--cols" => a.cols = val().parse().unwrap_or(a.cols),
                "--rows" => a.rows = val().parse().unwrap_or(a.rows),
                "--font" => a.font = Some(val()),
                "--iters" => a.iters = val().parse().unwrap_or(a.iters),
                "--idle-secs" => a.idle = val().parse().unwrap_or(a.idle),
                "--physical" => a.physical = true,
                "--skia" => a.skia = true,
                "--dialog" => a.dialog = true,
                _ => {}
            }
        }
        a
    }
}

fn parse_size(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?))
}
