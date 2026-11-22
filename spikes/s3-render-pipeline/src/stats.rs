//! Timing samples and their summary (min / median / p95 / max, microseconds).

use std::time::{Duration, Instant};

/// Microseconds in a duration.
pub(crate) fn micros(d: Duration) -> f64 {
    d.as_secs_f64() * 1e6
}

/// Emit `RESULT <label> min_us=.. med_us=.. p95_us=.. max_us=..[ <extra>]` for `v` (sorted in place).
pub(crate) fn emit(label: &str, v: &mut [f64], extra: &str) {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    let at = |i: usize| v.get(i.min(n.saturating_sub(1))).copied().unwrap_or(f64::NAN);
    let (min, med, p95, max) = (at(0), at(n / 2), at(n * 95 / 100), at(n.saturating_sub(1)));
    let sep = if extra.is_empty() { "" } else { " " };
    out!(
        "RESULT {label} min_us={min:.1} med_us={med:.1} p95_us={p95:.1} max_us={max:.1}{sep}{extra}"
    );
}

/// Per-iteration timings of the three parts of a frame: acquire, draw, present.
#[derive(Default)]
pub(crate) struct Frames {
    pub(crate) acquire: Vec<f64>,
    pub(crate) draw: Vec<f64>,
    pub(crate) present: Vec<f64>,
}

impl Frames {
    /// Record one frame from its four time stamps (start, buffer acquired, drawn, presented).
    pub(crate) fn push(&mut self, t: [Instant; 4]) {
        self.acquire.push(micros(t[1] - t[0]));
        self.draw.push(micros(t[2] - t[1]));
        self.present.push(micros(t[3] - t[2]));
    }
}
