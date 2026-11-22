//! A text rendering of the terminal state for snapshot tests and diagnostics.
//!
//! The format is stable because the fixtures under `fixtures/vt/` snapshot it: size, cursor,
//! modes, the screen as `|text|` rows with line flags, the style of every non-default cell run
//! and the history. Wide characters show once (their spacer is skipped), clusters show whole.

use std::fmt::Write as _;

use ferrum_scrollback::{LineCursor, LineFlags};

use crate::cell::{Attrs, Cell, CellFlags, Underline};
use crate::color::ColorKind;
use crate::cursor::Pen;
use crate::events::EventSink;
use crate::extras::Extras;
use crate::grid::Grid;
use crate::screen::Region;
use crate::terminal::Terminal;

/// Shown for the blank a wide character left at the end of a row.
const LEADING_SPACER_MARK: char = '»';

fn color_text(color: crate::color::Color) -> String {
    match color.kind() {
        ColorKind::Default => "default".to_owned(),
        ColorKind::Indexed(index) => format!("i{index}"),
        ColorKind::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
    }
}

fn flag_letters(flags: LineFlags) -> String {
    [
        (LineFlags::WRAPPED, 'W'),
        (LineFlags::PROMPT_MARK, 'P'),
        (LineFlags::COMMAND_START, 'C'),
        (LineFlags::COMMAND_END, 'E'),
    ]
    .iter()
    .filter(|(flag, _)| flags.contains(*flag))
    .map(|&(_, letter)| letter)
    .collect()
}

/// Describes the style of a cell, empty for the default style.
fn style_text(cell: &Cell, extras: &Extras) -> String {
    let mut parts = Vec::new();
    if cell.fg() != crate::color::Color::DEFAULT {
        parts.push(format!("fg={}", color_text(cell.fg())));
    }
    if cell.bg() != crate::color::Color::DEFAULT {
        parts.push(format!("bg={}", color_text(cell.bg())));
    }
    for (attr, name) in [
        (Attrs::BOLD, "bold"),
        (Attrs::DIM, "dim"),
        (Attrs::ITALIC, "italic"),
        (Attrs::BLINK, "blink"),
        (Attrs::INVERSE, "inverse"),
        (Attrs::HIDDEN, "hidden"),
        (Attrs::STRIKE, "strike"),
        (Attrs::OVERLINE, "overline"),
        (Attrs::PROTECTED, "protected"),
    ] {
        if cell.attrs().contains(attr) {
            parts.push(name.to_owned());
        }
    }
    if cell.underline() != Underline::None {
        parts.push(format!("underline={:?}", cell.underline()).to_lowercase());
    }
    let extra = cell.extra();
    let underline_color = extras.underline_color(extra);
    if underline_color != crate::color::Color::DEFAULT {
        parts.push(format!("ul={}", color_text(underline_color)));
    }
    if let Some(uri) = extras.uri(extras.link(extra)) {
        parts.push(format!("link={uri}"));
    }
    parts.join(" ")
}

fn row_text(grid: &Grid, y: usize) -> String {
    let mut text = String::new();
    let Some(row) = grid.row(y) else { return text };
    for cell in row.cells() {
        if cell.flags().contains(CellFlags::SPACER) {
            continue;
        }
        if cell.flags().contains(CellFlags::LEADING_SPACER) {
            text.push(LEADING_SPACER_MARK);
            continue;
        }
        let mut buffer = [0; 4];
        text.push_str(grid.cell_text(cell, &mut buffer));
    }
    text
}

fn style_runs(grid: &Grid, y: usize, extras: &Extras) -> Vec<String> {
    let Some(row) = grid.row(y) else { return Vec::new() };
    let mut runs: Vec<(usize, usize, String)> = Vec::new();
    for (x, cell) in row.cells().iter().enumerate() {
        let style = style_text(cell, extras);
        if style.is_empty() {
            continue;
        }
        match runs.last_mut() {
            Some((_, end, last)) if *end + 1 == x && *last == style => *end = x,
            _ => runs.push((x, x, style)),
        }
    }
    runs.into_iter().map(|(from, to, style)| format!("  r{y} c{from}-{to} {style}")).collect()
}

fn pen_text(pen: &Pen, extras: &Extras) -> String {
    let mut cell = pen.cell(' ');
    cell.remove_flags(CellFlags::all());
    style_text(&cell, extras)
}

impl<S: EventSink> Terminal<S> {
    /// Renders the whole state as text (see the module docs).
    pub fn dump(&self) -> String {
        let core = &self.core;
        let screen = core.screen();
        let grid = &screen.grid;
        let cursor = &screen.cursor;
        let mut out = String::new();
        let screen_name = if core.alt_active() { "alt" } else { "primary" };
        // Writing to a String cannot fail, so the results of `write!` are ignored throughout.
        let _ = writeln!(out, "size {}x{} {screen_name}", grid.cols(), grid.lines());
        let _ = writeln!(
            out,
            "cursor row={} col={} wrap_pending={}",
            cursor.y, cursor.x, cursor.wrap_pending
        );
        let modes: Vec<&str> = core.modes.iter_names().map(|(name, _)| name).collect();
        let _ = writeln!(out, "modes {}", modes.join("|"));
        if screen.region != Region::full(grid.lines()) {
            let _ = writeln!(out, "region {}..{}", screen.region.top, screen.region.bottom);
        }
        let pen = pen_text(&cursor.pen, &core.extras);
        if !pen.is_empty() {
            let _ = writeln!(out, "pen {pen}");
        }
        for y in 0..grid.lines() {
            let flags = grid.row(y).map_or(LineFlags::NONE, crate::row::Row::flags);
            let _ = writeln!(out, "{y:02} |{}|{}", row_text(grid, y), flag_letters(flags));
        }
        for y in 0..grid.lines() {
            for line in style_runs(grid, y, &core.extras) {
                let _ = writeln!(out, "{line}");
            }
        }
        out.push_str(&self.history_dump());
        out
    }

    fn history_dump(&self) -> String {
        let history = &self.core.scrollback;
        let mut out = String::new();
        let _ = writeln!(out, "history {}", history.len());
        let mut cursor = LineCursor::new();
        for index in 0..history.len() {
            if let Ok(Some(line)) = history.read_line(index, &mut cursor) {
                let _ = writeln!(out, "h{index:02} |{}|{}", line.text, flag_letters(line.flags));
            }
        }
        out
    }
}
