//! Deterministic cell grid used as the repaint workload.

/// One terminal cell.
#[derive(Clone, Copy)]
pub(crate) struct Cell {
    pub(crate) ch: char,
    pub(crate) fg: u32,
    pub(crate) bg: u32,
}

const PALETTE: [u32; 8] = [
    0x00E6_E6E6,
    0x00F3_8BA8,
    0x00A6_E3A1,
    0x00F9_E2AF,
    0x0089_B4FA,
    0x00CB_A6F7,
    0x0094_E2D5,
    0x00FA_B387,
];
/// Default background colour.
pub(crate) const BG: u32 = 0x001E_1E2E;

/// A `cols` x `rows` grid, row-major.
pub(crate) struct Grid {
    pub(crate) cols: usize,
    pub(crate) rows: usize,
    pub(crate) cells: Vec<Cell>,
}

fn xorshift(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

impl Grid {
    /// Deterministic content. `dense` = every cell holds a glyph (worst case); otherwise the
    /// memcheck-like mix: average ~70 visible columns per line, 35 % of lines colored in runs.
    pub(crate) fn generate(cols: usize, rows: usize, dense: bool) -> Grid {
        let mut s = 0x1234_5678_9abc_def0u64;
        let mut cells = Vec::with_capacity(cols * rows);
        for _ in 0..rows {
            let visible = if dense { cols } else { 20 + (xorshift(&mut s) % 100) as usize };
            let colored = xorshift(&mut s) % 100 < 35;
            let mut fg = PALETTE[0];
            for c in 0..cols {
                if c % 12 == 0 && colored {
                    fg = PALETTE.get((xorshift(&mut s) % 8) as usize).copied().unwrap_or(fg);
                }
                let ch =
                    if c < visible { (0x21 + (xorshift(&mut s) % 94) as u8) as char } else { ' ' };
                let bg = if xorshift(&mut s) % 100 < 3 { 0x0045_475A } else { BG };
                cells.push(Cell { ch, fg, bg });
            }
        }
        Grid { cols, rows, cells }
    }
}
