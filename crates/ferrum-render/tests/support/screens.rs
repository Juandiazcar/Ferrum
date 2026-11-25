//! Deterministic screens for the benches, the `dhat` gate and the golden images.
//!
//! - dense: every cell of the grid holds a printable ASCII glyph (the reading of NFR-PERF-03
//!   decided by the maintainer, Q-19), colored in runs like the typical mix;
//! - typical: the spike S3 mix (ADR 0025): 20–119 visible columns per row (about 70 on average,
//!   `docs/03` §1), 35 % of the rows colored in runs of 12 columns, 3 % of the cells highlighted.

#![allow(dead_code, reason = "each test binary uses part of the support module")]

use ferrum_render::{Cell, CellText, Rgb, Row};

pub(crate) const COLS: u16 = 200;
pub(crate) const ROWS: u16 = 60;
pub(crate) const BACKGROUND: Rgb = Rgb::new(0x10, 0x12, 0x14);
pub(crate) const FOREGROUND: Rgb = Rgb::new(0xD7, 0xDA, 0xE0);

const PALETTE: [Rgb; 6] = [
    Rgb::new(0xE8, 0x59, 0x0C),
    Rgb::new(0x98, 0xC3, 0x79),
    Rgb::new(0x61, 0xAF, 0xEF),
    Rgb::new(0xE5, 0xC0, 0x7B),
    Rgb::new(0xC6, 0x78, 0xDD),
    Rgb::new(0x56, 0xB6, 0xC2),
];
const HIGHLIGHT: Rgb = Rgb::new(0x2C, 0x31, 0x3A);

/// xorshift64: deterministic content without a dependency.
pub(crate) struct Rng(pub u64);

impl Rng {
    pub(crate) fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub(crate) fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Which content to make.
#[derive(Clone, Copy)]
pub(crate) enum Mix {
    Dense,
    Typical,
}

fn glyph(rng: &mut Rng) -> char {
    char::from(b'!' + u8::try_from(rng.below(94)).unwrap_or(0))
}

/// One screen of `rows` rows of `cols` cells.
pub(crate) fn screen(mix: Mix, cols: u16, rows: u16, seed: u64) -> Vec<Vec<Cell>> {
    let mut rng = Rng(seed | 1);
    (0..rows).map(|_| row(mix, cols, &mut rng)).collect()
}

fn row(mix: Mix, cols: u16, rng: &mut Rng) -> Vec<Cell> {
    let visible = match mix {
        Mix::Dense => cols,
        Mix::Typical => u16::try_from(20 + rng.below(100)).unwrap_or(cols).min(cols),
    };
    let colored = rng.below(100) < 35;
    let mut color = FOREGROUND;
    (0..cols)
        .map(|col| {
            if colored && col % 12 == 0 {
                color = PALETTE
                    .get(usize::try_from(rng.below(6)).unwrap_or(0))
                    .copied()
                    .unwrap_or(color);
            }
            let bg = if rng.below(100) < 3 { HIGHLIGHT } else { BACKGROUND };
            let text = if col < visible { CellText::Char(glyph(rng)) } else { CellText::Empty };
            Cell { text, ..Cell::blank(color, bg) }
        })
        .collect()
}

/// The rows of `screen` as the painter reads them.
pub(crate) fn rows(screen: &[Vec<Cell>]) -> Vec<Row<'_>> {
    screen.iter().map(|cells| Row { cells, clusters: "" }).collect()
}
