//! Terminal state to protocol values: screen rows and history rows into a [`RowBlock`], the
//! cursor and the modes (`docs/02` §4).
//!
//! Rows are encoded from a reusable [`RowScratch`]: the cell texts are copied into one buffer and
//! the cells refer to it, so encoding a row does not allocate once the scratch has grown
//! (`docs/05` §6: `proto::encode_row` is a hot path).

use ferrum_proto::{
    Attrs, CellRef, CellWidth, Color, Cursor, CursorShape, Modes, ProtoError, RowBlock, Style,
    TerminalModes,
};
use ferrum_scrollback::{DecodedLine, Style as Words, StyleTable};
use ferrum_vt::{CellFlags, CellStyle, ColorKind, EventSink, ExtraId, Extras, Mode, Terminal};

use crate::session::sink::Sink;

/// The terminal of a session.
pub(crate) type Term = Terminal<Sink>;

/// Bytes of the longest UTF-8 scalar (`Grid::cell_text` writes one into such a buffer).
const UTF8_MAX_BYTES: usize = 4;

/// One cell of the row being encoded: a slice of the scratch text, its width and its style.
#[derive(Debug, Clone, Copy)]
struct Slot {
    start: usize,
    end: usize,
    width: CellWidth,
    style: Words,
}

/// Reusable buffers for encoding rows.
#[derive(Debug, Default)]
pub(crate) struct RowScratch {
    text: String,
    slots: Vec<Slot>,
}

impl RowScratch {
    fn clear(&mut self) {
        self.text.clear();
        self.slots.clear();
    }

    fn push(&mut self, text: &str, width: CellWidth, style: Words) {
        let start = self.text.len();
        self.text.push_str(text);
        self.slots.push(Slot { start, end: self.text.len(), width, style });
    }

    fn cells<'a>(&'a self, extras: &'a Extras) -> impl Iterator<Item = CellRef<'a>> + 'a {
        self.slots.iter().map(move |slot| CellRef {
            text: self.text.get(slot.start..slot.end).unwrap_or(" "),
            width: slot.width,
            style: style(&slot.style, extras),
        })
    }
}

fn color(color: ferrum_vt::Color) -> Color {
    match color.kind() {
        ColorKind::Indexed(index) => Color::indexed(index),
        ColorKind::Rgb(red, green, blue) => Color::rgb(red, green, blue),
        _ => Color::DEFAULT,
    }
}

/// The protocol style of a cell's style words. The attribute words of `ferrum-vt` and
/// `ferrum-proto` share their layout (the underline style in bits 3 to 5 included).
fn style<'a>(words: &Words, extras: &'a Extras) -> Style<'a> {
    let cell = CellStyle::from_style(words);
    Style {
        attrs: Attrs::from_bits(words.attrs).unwrap_or(Attrs::NONE),
        fg: color(cell.fg),
        bg: color(cell.bg),
        underline_color: color(extras.underline_color(cell.extra)),
        link: link(cell.extra, extras),
    }
}

fn link(extra: ExtraId, extras: &Extras) -> Option<&str> {
    extras.uri(extras.link(extra))
}

/// Appends screen row `y` of the visible grid to `block`.
///
/// # Errors
/// [`ProtoError::BlockFull`] when the block has no room (send it and retry in an empty one), or
/// another encoding error for a row the protocol cannot carry.
pub(crate) fn push_screen_row<S: EventSink>(
    block: &mut RowBlock,
    term: &Terminal<S>,
    y: usize,
    scratch: &mut RowScratch,
) -> Result<(), ProtoError> {
    let grid = term.grid();
    scratch.clear();
    let mut utf8 = [0; UTF8_MAX_BYTES];
    if let Some(row) = grid.row(y) {
        for cell in row.cells().iter().take(row.used_len()) {
            if cell.flags().contains(CellFlags::SPACER) {
                continue;
            }
            let is_wide = cell.flags().contains(CellFlags::WIDE);
            let width = if is_wide { CellWidth::Double } else { CellWidth::Single };
            let style = Words { fg: cell.fg_word(), bg: cell.bg_word(), attrs: cell.attr_word() };
            scratch.push(grid.cell_text(cell, &mut utf8), width, style);
        }
    }
    let cols = clamp_u16(grid.cols());
    block.push_row(clamp_u16(y), cols, scratch.cells(term.extras()))
}

/// A history line and where it goes: row `y` (0 is the row just above the screen).
pub(crate) struct HistoryRow<'a> {
    pub(crate) y: u16,
    pub(crate) line: &'a DecodedLine,
}

/// Appends a history line as a row of the width of the screen.
///
/// # Errors
/// As [`push_screen_row`].
pub(crate) fn push_history_row<S: EventSink>(
    block: &mut RowBlock,
    row: &HistoryRow<'_>,
    term: &Terminal<S>,
    scratch: &mut RowScratch,
) -> Result<(), ProtoError> {
    let cols = term.grid().cols();
    fill_history(scratch, row.line, term.scrollback().styles(), cols);
    block.push_row(row.y, clamp_u16(cols), scratch.cells(term.extras()))
}

/// The cells of a history line: its grapheme clusters, each with the style of its first scalar.
fn fill_history(scratch: &mut RowScratch, line: &DecodedLine, styles: &StyleTable, cols: usize) {
    scratch.clear();
    let mut runs = line.runs.iter();
    let mut run = runs.next();
    let mut left_in_run = run.map_or(0, |run| run.len);
    let mut used = 0;
    for cluster in ferrum_vt::clusters(&line.text) {
        let wide = ferrum_vt::cluster_width(cluster) > 1;
        used += if wide { 2 } else { 1 };
        if used > cols {
            break;
        }
        let style = run.and_then(|run| styles.get(run.style)).unwrap_or_default();
        let width = if wide { CellWidth::Double } else { CellWidth::Single };
        scratch.push(cluster, width, style);
        for _ in cluster.chars() {
            while left_in_run == 0 && run.is_some() {
                run = runs.next();
                left_in_run = run.map_or(0, |run| run.len);
            }
            left_in_run = left_in_run.saturating_sub(1);
        }
    }
}

/// The cursor of the visible screen, with its DECSCUSR style (block, steady by default).
pub(crate) fn cursor<S: EventSink>(term: &Terminal<S>) -> Cursor {
    let position = term.cursor();
    let modes = term.modes();
    let (shape, is_blinking) = term.cursor_style().map_or(
        (CursorShape::Block, modes.contains(Mode::CURSOR_BLINK)),
        |style| {
            let shape = match style.shape {
                ferrum_vt::CursorShape::Underline => CursorShape::Underline,
                ferrum_vt::CursorShape::Bar => CursorShape::Bar,
                _ => CursorShape::Block,
            };
            (shape, style.blinking)
        },
    );
    Cursor {
        col: clamp_u16(position.x),
        row: clamp_u16(position.y),
        shape,
        is_blinking,
        is_visible: modes.contains(Mode::CURSOR_VISIBLE),
    }
}

/// The modes the UI needs to encode input and draw.
pub(crate) fn modes<S: EventSink>(term: &Terminal<S>) -> TerminalModes {
    const MAP: [(Mode, Modes); 13] = [
        (Mode::APP_CURSOR, Modes::APP_CURSOR),
        (Mode::APP_KEYPAD, Modes::APP_KEYPAD),
        (Mode::LINE_FEED_NEWLINE, Modes::LINE_FEED_NEWLINE),
        (Mode::BRACKETED_PASTE, Modes::BRACKETED_PASTE),
        (Mode::FOCUS_REPORTING, Modes::FOCUS_REPORTING),
        (Mode::MOUSE_X10, Modes::MOUSE_X10),
        (Mode::MOUSE_NORMAL, Modes::MOUSE_NORMAL),
        (Mode::MOUSE_BUTTON, Modes::MOUSE_BUTTON),
        (Mode::MOUSE_ANY, Modes::MOUSE_ANY),
        (Mode::MOUSE_SGR, Modes::MOUSE_SGR),
        (Mode::ALTERNATE_SCROLL, Modes::ALTERNATE_SCROLL),
        (Mode::ALT_SCREEN, Modes::ALT_SCREEN),
        (Mode::REVERSE_VIDEO, Modes::REVERSE_VIDEO),
    ];
    let mode = term.modes();
    let bits =
        MAP.iter().filter(|(vt, _)| mode.contains(*vt)).fold(0, |bits, (_, proto)| bits | proto.0);
    let kitty = u8::try_from(term.keyboard_flags().bits()).unwrap_or(u8::MAX);
    TerminalModes { modes: Modes(bits), kitty_keyboard: kitty }
}

/// A grid coordinate for the protocol; the grid is at most `MAX_COLS` by `MAX_ROWS`, far below
/// `u16::MAX`.
pub(crate) fn clamp_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}
