//! Minimal live grid driven by `vte` (parser only): printable text, CR/LF/BS/HT, SGR, autowrap.
//! Rows that scroll off the top are handed to a [`Sink`] (encoder, naive cells, or hash checker).

use unicode_width::UnicodeWidthChar;
use vte::{Params, Perform};

pub(crate) const COLS: usize = 200;
pub(crate) const ROWS: usize = 55;

/// Right half of a wide character.
pub(crate) const FLAG_SPACER: u16 = 1;

/// docs/02 §3.1: 16 bytes.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Cell {
    pub(crate) ch: u32,
    pub(crate) fg: u32,
    pub(crate) bg: u32,
    pub(crate) attrs: u16,
    pub(crate) flags: u16,
}
const _: () = assert!(size_of::<Cell>() == 16);

/// Receives rows as they scroll off the top of the live grid.
pub(crate) trait Sink {
    fn push_row(&mut self, cells: &[Cell], wrapped: bool) -> anyhow::Result<()>;
}

const COLOR_INDEXED: u32 = 0x0100_0000;
const COLOR_RGB: u32 = 0x0200_0000;

#[derive(Clone, Copy, Default)]
struct Pen {
    fg: u32,
    bg: u32,
    attrs: u16,
}

/// The live grid: a ring of `ROWS` rows of `COLS` 16-byte cells.
pub(crate) struct Grid<S: Sink> {
    cells: Vec<Cell>,
    top: usize,
    row: usize,
    col: usize,
    wrapped: [bool; ROWS],
    pen: Pen,
    pub(crate) sink: S,
    pub(crate) error: Option<anyhow::Error>,
    pub(crate) rows_pushed: usize,
}

impl<S: Sink> Grid<S> {
    pub(crate) fn new(sink: S) -> Self {
        Self {
            cells: vec![Cell::default(); ROWS * COLS],
            top: 0,
            row: 0,
            col: 0,
            wrapped: [false; ROWS],
            pen: Pen::default(),
            sink,
            error: None,
            rows_pushed: 0,
        }
    }

    /// Heap capacity of the live grid in bytes.
    pub(crate) fn capacity_bytes(&self) -> usize {
        self.cells.capacity() * size_of::<Cell>()
    }

    fn phys(&self, r: usize) -> usize {
        (self.top + r) % ROWS
    }

    fn row_mut(&mut self, p: usize) -> &mut [Cell] {
        self.cells.get_mut(p * COLS..(p + 1) * COLS).unwrap_or_default()
    }

    fn mark_wrapped(&mut self) {
        let p = self.phys(self.row);
        if let Some(w) = self.wrapped.get_mut(p) {
            *w = true;
        }
    }

    fn linefeed(&mut self) {
        if self.row + 1 < ROWS {
            self.row += 1;
            return;
        }
        let p = self.top;
        let wrapped = self.wrapped.get(p).copied().unwrap_or(false);
        let start = p * COLS;
        if let Some(Err(e)) =
            self.cells.get(start..start + COLS).map(|c| self.sink.push_row(c, wrapped))
        {
            self.error.get_or_insert(e);
        }
        self.rows_pushed += 1;
        self.row_mut(p).fill(Cell::default());
        if let Some(w) = self.wrapped.get_mut(p) {
            *w = false;
        }
        self.top = (self.top + 1) % ROWS;
    }

    fn sgr(&mut self, params: &Params) {
        let mut it = params.iter().flat_map(|p| p.iter().copied());
        while let Some(n) = it.next() {
            let pen = &mut self.pen;
            match n {
                0 => *pen = Pen::default(),
                1 => pen.attrs |= 1,
                2 => pen.attrs |= 2,
                3 => pen.attrs |= 4,
                4 => pen.attrs |= 8,
                5 => pen.attrs |= 16,
                7 => pen.attrs |= 32,
                8 => pen.attrs |= 64,
                9 => pen.attrs |= 128,
                22 => pen.attrs &= !3,
                23 => pen.attrs &= !4,
                24 => pen.attrs &= !8,
                25 => pen.attrs &= !16,
                27 => pen.attrs &= !32,
                28 => pen.attrs &= !64,
                29 => pen.attrs &= !128,
                30..=37 => pen.fg = COLOR_INDEXED | u32::from(n - 30),
                39 => pen.fg = 0,
                40..=47 => pen.bg = COLOR_INDEXED | u32::from(n - 40),
                49 => pen.bg = 0,
                90..=97 => pen.fg = COLOR_INDEXED | u32::from(n - 90 + 8),
                100..=107 => pen.bg = COLOR_INDEXED | u32::from(n - 100 + 8),
                38 | 48 => {
                    let color = match it.next() {
                        Some(5) => it.next().map(|i| COLOR_INDEXED | u32::from(i)),
                        Some(2) => {
                            let r = u32::from(it.next().unwrap_or(0));
                            let g = u32::from(it.next().unwrap_or(0));
                            let b = u32::from(it.next().unwrap_or(0));
                            Some(COLOR_RGB | (r & 255) << 16 | (g & 255) << 8 | (b & 255))
                        }
                        _ => None,
                    };
                    if let Some(c) = color {
                        if n == 38 { pen.fg = c } else { pen.bg = c }
                    }
                }
                _ => {}
            }
        }
    }
}

impl<S: Sink> Perform for Grid<S> {
    fn print(&mut self, c: char) {
        let w = c.width().unwrap_or(0);
        if w == 0 {
            return; // combining marks are out of scope for the spike
        }
        if self.col + w > COLS {
            self.mark_wrapped();
            self.col = 0;
            self.linefeed();
        }
        let (p, col, pen) = (self.phys(self.row), self.col, self.pen);
        let row = self.row_mut(p);
        if let Some(cell) = row.get_mut(col) {
            *cell = Cell { ch: u32::from(c), fg: pen.fg, bg: pen.bg, attrs: pen.attrs, flags: 0 };
        }
        if let (2, Some(cell)) = (w, row.get_mut(col + 1)) {
            *cell = Cell { ch: 0, fg: pen.fg, bg: pen.bg, attrs: pen.attrs, flags: FLAG_SPACER };
        }
        self.col += w;
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x0A..=0x0C => self.linefeed(),
            0x0D => self.col = 0,
            0x08 => self.col = self.col.saturating_sub(1),
            0x09 => self.col = ((self.col / 8 + 1) * 8).min(COLS - 1),
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, _: &[u8], _: bool, action: char) {
        if action == 'm' {
            self.sgr(params);
        }
    }
}
