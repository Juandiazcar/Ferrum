//! Deterministic, seeded terminal-output fixtures: the reference content mix of `docs/03` §1
//! (35 % colored lines of 3 to 5 style runs, about 70 visible columns on average, 5 % non-ASCII
//! lines with CJK and emoji, 2 % lines of 300 to 400 columns), wrapped into rows of a given width.
//!
//! The generator is included by tests, benches and examples with `#[path]`; it depends only on the
//! public API of `ferrum-scrollback`. Same seed, same bytes, on every platform. The achieved mix is
//! counted in [`Stats`] and checked by `tests/fixture_mix.rs`, never assumed.
//!
//! Colors are fixture-local values (`FG_TAG | index`); `ferrum-vt` defines the real encoding.

#![allow(dead_code, reason = "each includer uses a different subset of the fixture API")]

mod rng;
pub(crate) mod stream;
mod text;

use ferrum_scrollback::{LineFlags, LineRef, Style, StyledRun};
use rng::Rng;

/// Columns of the reference window (`docs/03` §1: about 200 x 55).
pub(crate) const REFERENCE_COLS: usize = 200;

/// Per-mille shares of the reference mix.
const COLORED_PER_MILLE: usize = 350;
const NON_ASCII_PER_MILLE: usize = 50;
const LONG_PER_MILLE: usize = 20;
/// Visible columns of an ordinary line, and of a very long one.
const ORDINARY_COLS: (usize, usize) = (10, 118);
const LONG_COLS: (usize, usize) = (300, 400);
/// Style runs of a colored line.
const COLORED_RUNS: (usize, usize) = (3, 5);
/// Distinct colored styles drawn from (sessions use fewer than 200, `docs/02` §3.2).
const PALETTE_SIZE: usize = 24;
const FG_TAG: u32 = 0x0100_0000;
const BG_TAG: u32 = 0x0200_0000;
const PALETTE_COLORS: usize = 8;
const ATTRS_VARIANTS: usize = 3;
const BG_EVERY: usize = 5;

/// Which text the lines carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Profile {
    /// Log-like lines from a small vocabulary: the reference fixture.
    Mixed,
    /// Random printable ASCII: the lower bound for any compressor.
    Entropy,
}

/// One grid row ready to push.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) flags: LineFlags,
    pub(crate) text: String,
    pub(crate) runs: Vec<StyledRun>,
}

impl Row {
    pub(crate) fn as_line(&self) -> LineRef<'_> {
        LineRef { flags: self.flags, text: &self.text, runs: &self.runs }
    }
}

/// What was actually generated.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stats {
    pub(crate) lines: usize,
    pub(crate) rows: usize,
    pub(crate) colored: usize,
    pub(crate) non_ascii: usize,
    pub(crate) long: usize,
    pub(crate) columns: usize,
    pub(crate) colored_runs: usize,
}

/// The generator.
pub(crate) struct Fixture {
    rng: Rng,
    profile: Profile,
    cols: usize,
    stats: Stats,
}

impl Fixture {
    pub(crate) fn new(seed: u64, profile: Profile, cols: usize) -> Self {
        Self { rng: Rng::new(seed), profile, cols: cols.max(2), stats: Stats::default() }
    }

    pub(crate) fn stats(&self) -> Stats {
        self.stats
    }

    /// Generates `logical_lines` lines and returns their rows (wrapping adds rows).
    pub(crate) fn rows(
        seed: u64,
        profile: Profile,
        cols: usize,
        logical_lines: usize,
    ) -> (Vec<Row>, Stats) {
        let mut fixture = Self::new(seed, profile, cols);
        let mut rows = Vec::new();
        for _ in 0..logical_lines {
            fixture.push_logical_line(&mut rows);
        }
        (rows, fixture.stats)
    }

    /// Appends the rows of the next logical line.
    pub(crate) fn push_logical_line(&mut self, rows: &mut Vec<Row>) {
        let long = self.rng.chance(LONG_PER_MILLE);
        let (low, high) = if long { LONG_COLS } else { ORDINARY_COLS };
        let target = self.rng.between(low, high);
        let base = match self.profile {
            Profile::Mixed => text::mixed_line(&mut self.rng, target),
            Profile::Entropy => text::entropy_line(&mut self.rng, target),
        };
        let non_ascii = self.rng.chance(NON_ASCII_PER_MILLE);
        let line = if non_ascii { text::inject_non_ascii(&mut self.rng, &base) } else { base };
        let styles = self.styles_for(line.chars().count());
        let before = rows.len();
        wrap(&line, &styles, self.cols, rows);
        let columns = line.chars().map(text::char_width).sum::<usize>();
        let stats = &mut self.stats;
        stats.lines += 1;
        stats.rows += rows.len() - before;
        stats.columns += columns;
        stats.long += usize::from(long);
        stats.non_ascii += usize::from(non_ascii);
    }

    /// One style per scalar: all default, or 3 to 5 runs of palette styles for a colored line.
    fn styles_for(&mut self, scalars: usize) -> Vec<Style> {
        let mut styles = vec![Style::default(); scalars];
        if scalars < COLORED_RUNS.1 || !self.rng.chance(COLORED_PER_MILLE) {
            return styles;
        }
        let runs = self.rng.between(COLORED_RUNS.0, COLORED_RUNS.1);
        // Distinct cut points and neighbors of different styles: exactly `runs` runs.
        let mut cuts: Vec<usize> = Vec::new();
        while cuts.len() + 1 < runs {
            let cut = self.rng.between(1, scalars - 1);
            if !cuts.contains(&cut) {
                cuts.push(cut);
            }
        }
        cuts.sort_unstable();
        cuts.push(scalars);
        let (mut start, mut previous) = (0, Style::default());
        for end in cuts {
            let mut style = palette(self.rng.below(PALETTE_SIZE));
            while style == previous {
                style = palette(self.rng.below(PALETTE_SIZE));
            }
            for slot in styles.get_mut(start..end).unwrap_or_default() {
                *slot = style;
            }
            (start, previous) = (end, style);
        }
        self.stats.colored += 1;
        self.stats.colored_runs += runs;
        styles
    }
}

fn palette(index: usize) -> Style {
    let color = u32::try_from(index % PALETTE_COLORS).unwrap_or(0);
    let variant = index / PALETTE_COLORS;
    let background = if index.is_multiple_of(BG_EVERY) { BG_TAG | color } else { 0 };
    Style {
        fg: FG_TAG | color,
        bg: background,
        attrs: u16::try_from(variant % ATTRS_VARIANTS).unwrap_or(0),
    }
}

/// Splits `line` into rows of at most `cols` columns; every row but the last is soft-wrapped.
fn wrap(line: &str, styles: &[Style], cols: usize, rows: &mut Vec<Row>) {
    let mut row = new_row();
    let (mut used, mut row_styles): (usize, Vec<Style>) = (0, Vec::new());
    for (c, &style) in line.chars().zip(styles) {
        let width = text::char_width(c);
        if used + width > cols {
            finish(&mut row, &row_styles, LineFlags::WRAPPED);
            rows.push(std::mem::replace(&mut row, new_row()));
            (used, row_styles) = (0, Vec::new());
        }
        row.text.push(c);
        row_styles.push(style);
        used += width;
    }
    finish(&mut row, &row_styles, LineFlags::NONE);
    rows.push(row);
}

fn new_row() -> Row {
    Row { flags: LineFlags::NONE, text: String::new(), runs: Vec::new() }
}

/// Sets the flags and groups the per-scalar styles into runs.
fn finish(row: &mut Row, styles: &[Style], flags: LineFlags) {
    row.flags = flags;
    for &style in styles {
        match row.runs.last_mut() {
            Some(last) if last.style == style => last.len += 1,
            _ => row.runs.push(StyledRun { style, len: 1 }),
        }
    }
}
