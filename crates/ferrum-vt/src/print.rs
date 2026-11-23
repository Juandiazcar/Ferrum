//! Printing text: ASCII runs in bulk, wide characters, combining marks and clusters
//! (FR-TERM-003), wrapping and insert mode.

use crate::cell::{Cell, CellFlags};
use crate::core::Core;
use crate::cursor::Pen;
use crate::events::EventSink;
use crate::limits::{MAX_CLUSTER_BYTES, MAX_UTF8_BYTES};
use crate::modes::Mode;
use crate::perform::control;
use crate::row::Row;
use crate::screen::Screen;
use crate::width::{WIDE_COLUMNS, char_width, cluster_width, joins_cluster};

/// What happens at the right margin (DECAWM).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Wrap {
    /// The next character wraps to the next row.
    Auto,
    /// The cursor stays on the last column.
    Stay,
}

impl Wrap {
    fn of(modes: Mode) -> Self {
        if modes.contains(Mode::AUTOWRAP) { Self::Auto } else { Self::Stay }
    }
}

/// Whether a scalar always belongs to the previous cluster or only when UAX #29 says so.
#[derive(Clone, Copy, PartialEq, Eq)]
enum JoinRule {
    /// Zero-width scalars: they never take a cell of their own.
    Always,
    /// Wider scalars: only when they continue the cluster.
    IfContinues,
}

/// DEL is never displayed (xterm ignores it).
const DEL: char = '\u{7f}';

/// What a glyph cell shows.
#[derive(Clone, Copy)]
enum Glyph {
    Scalar(char),
    /// An index into the grapheme table.
    Cluster(u32),
}

/// A cluster that grew by one scalar.
struct Merge {
    x: usize,
    y: usize,
    old_width: usize,
    new_width: usize,
    index: u32,
}

enum Joined {
    No,
    /// The scalar was absorbed without changing the cell (cluster cap, full table, no cell).
    Swallowed,
    Merged(Merge),
}

/// The head cell of the glyph before the cursor, if any.
fn previous_glyph(screen: &Screen) -> Option<(usize, usize)> {
    let cursor = &screen.cursor;
    let x = if cursor.wrap_pending { cursor.x } else { cursor.x.checked_sub(1)? };
    let row = screen.grid.row(cursor.y)?;
    let spacer = row.get(x)?.flags().contains(CellFlags::SPACER);
    Some((if spacer { x.checked_sub(1)? } else { x }, cursor.y))
}

/// The head cell of a glyph and, for a wide one, the spacer that follows it.
fn glyph_cells(glyph: Glyph, width: usize, pen: &Pen) -> (Cell, Cell) {
    let mut head = Cell::styled(' ', pen.fg_word(), pen.bg_word(), pen.attrs);
    match glyph {
        Glyph::Scalar(c) => head.set_scalar(c),
        Glyph::Cluster(index) => {
            head.set_raw_char(index);
            head.insert_flags(CellFlags::GRAPHEME);
        }
    }
    let mut tail = Cell::styled(' ', pen.fg_word(), pen.bg_word(), pen.attrs);
    if width == WIDE_COLUMNS {
        head.insert_flags(CellFlags::WIDE);
        tail.insert_flags(CellFlags::SPACER);
    }
    (head, tail)
}

impl<S: EventSink> Core<S> {
    /// A pending wrap happens now (or is dropped when auto-wrap was turned off meanwhile).
    fn settle_wrap(&mut self) {
        if !self.screen().cursor.wrap_pending {
            return;
        }
        if self.modes.contains(Mode::AUTOWRAP) {
            self.wrap_line();
        } else {
            self.screen_mut().cursor.wrap_pending = false;
        }
    }

    /// Marks the current row as continued and moves to the start of the next one.
    fn wrap_line(&mut self) {
        let y = self.screen().cursor.y;
        if let Some(row) = self.screen_mut().grid.row_mut(y) {
            row.insert_flag(ferrum_scrollback::LineFlags::WRAPPED);
        }
        self.carriage_return();
        self.index();
    }

    /// Prints a run of printable ASCII (0x20..=0x7E) in bulk.
    pub(crate) fn print_ascii(&mut self, bytes: &[u8]) {
        if !self.charsets.is_plain() || self.modes.contains(Mode::INSERT) {
            for &byte in bytes {
                self.print_char(char::from(byte));
            }
            return;
        }
        let mut rest = bytes;
        if let Some((&first, tail)) = rest.split_first().filter(|_| self.after_non_ascii()) {
            self.print_char(char::from(first));
            rest = tail;
        }
        while !rest.is_empty() {
            self.settle_wrap();
            let wrap = Wrap::of(self.modes);
            let screen = self.screen_mut();
            let (x, y, cols) = (screen.cursor.x, screen.cursor.y, screen.grid.cols());
            let count = rest.len().min(cols.saturating_sub(x));
            let Some((chunk, tail)) = rest.split_at_checked(count) else { break };
            let pen = screen.cursor.pen;
            screen.grid.clear_wide_edges(y, x, x + count, pen.blank());
            let span = screen.grid.row_mut(y).and_then(|row| row.cells_mut().get_mut(x..x + count));
            if let Some(span) = span {
                let (fg, bg, attrs) = (pen.fg_word(), pen.bg_word(), pen.attrs);
                for (cell, &byte) in span.iter_mut().zip(chunk) {
                    *cell = Cell::styled(char::from(byte), fg, bg, attrs);
                }
            }
            Self::advance_cursor(screen, count, wrap);
            rest = tail;
        }
        self.last_char = bytes.last().map(|&byte| char::from(byte));
    }

    /// True when the last printed scalar was not ASCII, so an ASCII scalar may continue its cluster.
    fn after_non_ascii(&self) -> bool {
        self.last_char.is_some_and(|previous| !previous.is_ascii())
    }

    /// Moves the cursor right by `columns` after printing; at the margin it stays on the last
    /// column and a wrap becomes pending.
    fn advance_cursor(screen: &mut Screen, columns: usize, wrap: Wrap) {
        let cols = screen.grid.cols();
        let next = screen.cursor.x + columns;
        if next >= cols {
            screen.cursor.x = cols.saturating_sub(1);
            screen.cursor.wrap_pending = wrap == Wrap::Auto;
        } else {
            screen.cursor.x = next;
        }
    }

    /// Prints one character (any width, after the active character set mapping).
    pub(crate) fn print_char(&mut self, c: char) {
        if c == DEL {
            return;
        }
        // `vte` executes C1 controls when it decodes text in one piece but prints them when a
        // character was split across reads; both end up here and are controls.
        let c1 = control::C1_FIRST..=control::C1_LAST;
        if let Some(control) = u8::try_from(u32::from(c)).ok().filter(|b| c1.contains(b)) {
            self.control(control);
            return;
        }
        let c = self.charsets.map(c);
        let after_non_ascii = self.after_non_ascii();
        self.last_char = Some(c);
        let width = char_width(c);
        if width == 0 {
            self.join_previous(c, JoinRule::Always);
            return;
        }
        // ASCII starts a cluster, except after a prepend scalar (UAX #29 GB9b), which only a
        // non-ASCII scalar can precede.
        if (!c.is_ascii() || after_non_ascii) && self.join_previous(c, JoinRule::IfContinues) {
            return;
        }
        self.put_glyph(Glyph::Scalar(c), width);
    }

    /// Appends `c` to the glyph before the cursor when it belongs to that cluster (or always,
    /// for zero-width scalars, `JoinRule::Always`). True when `c` was consumed.
    fn join_previous(&mut self, c: char, rule: JoinRule) -> bool {
        match self.plan_join(c, rule) {
            Joined::No => false,
            Joined::Swallowed => true,
            Joined::Merged(merge) => {
                self.apply_merge(&merge);
                true
            }
        }
    }

    fn plan_join(&mut self, c: char, rule: JoinRule) -> Joined {
        let force = rule == JoinRule::Always;
        let alt = self.alt_active();
        let screen =
            if alt { self.alt.as_mut().unwrap_or(&mut self.primary) } else { &mut self.primary };
        let Some((x, y)) = previous_glyph(screen) else {
            return if force { Joined::Swallowed } else { Joined::No };
        };
        let Some(cell) = screen.grid.row(y).and_then(|row| row.get(x)).copied() else {
            return Joined::No;
        };
        let mut buffer = [0; MAX_UTF8_BYTES];
        let previous = screen.grid.cell_text(&cell, &mut buffer);
        let joins = force || joins_cluster(previous, c, &mut self.scratch.cluster);
        if !joins {
            return Joined::No;
        }
        if force {
            self.scratch.cluster.clear();
            self.scratch.cluster.push_str(previous);
            self.scratch.cluster.push(c);
        }
        if self.scratch.cluster.len() > MAX_CLUSTER_BYTES {
            return Joined::Swallowed;
        }
        let new_width = cluster_width(&self.scratch.cluster).max(1);
        match screen.grid.intern_cluster(&self.scratch.cluster) {
            Some(index) => {
                Joined::Merged(Merge { x, y, old_width: cell.columns(), new_width, index })
            }
            None => Joined::Swallowed,
        }
    }

    fn apply_merge(&mut self, merge: &Merge) {
        let Merge { x, y, old_width, new_width, index } = *merge;
        if old_width == new_width {
            let cell = self.screen_mut().grid.row_mut(y).and_then(|row| row.get_mut(x));
            if let Some(cell) = cell {
                cell.set_raw_char(index);
                cell.insert_flags(CellFlags::GRAPHEME);
            }
            return;
        }
        // The width changed (a variation selector made it wide or narrow): print the cluster
        // again from where it started, so wrapping and spacers are right.
        let screen = self.screen_mut();
        let blank = screen.cursor.pen.blank();
        let wrapped = screen.grid.row(y).is_some_and(Row::is_wrapped);
        screen.grid.erase_cells(y, x, x + old_width, blank);
        if wrapped && let Some(row) = screen.grid.row_mut(y) {
            // Erasing up to the last column clears the flag, but the row still continues.
            row.insert_flag(ferrum_scrollback::LineFlags::WRAPPED);
        }
        screen.cursor.x = x;
        screen.cursor.y = y;
        screen.cursor.wrap_pending = false;
        self.put_glyph(Glyph::Cluster(index), new_width);
    }

    /// Writes a glyph of `width` 1 or 2 at the cursor and advances.
    fn put_glyph(&mut self, glyph: Glyph, width: usize) {
        self.settle_wrap();
        let wrap = Wrap::of(self.modes);
        let cols = self.screen().grid.cols();
        if width == WIDE_COLUMNS && cols < WIDE_COLUMNS {
            return;
        }
        if width == WIDE_COLUMNS && self.screen().cursor.x + 1 >= cols {
            if wrap == Wrap::Auto {
                self.wrap_with_leading_spacer();
            } else {
                self.screen_mut().cursor.x = cols - WIDE_COLUMNS;
            }
        }
        let insert = self.modes.contains(Mode::INSERT);
        let screen = self.screen_mut();
        let (x, y, pen) = (screen.cursor.x, screen.cursor.y, screen.cursor.pen);
        if insert {
            screen.grid.insert_blank(y, x, width, pen.blank());
        }
        screen.grid.clear_wide_edges(y, x, x + width, pen.blank());
        let (head, tail) = glyph_cells(glyph, width, &pen);
        if let Some(row) = screen.grid.row_mut(y) {
            if let Some(cell) = row.get_mut(x) {
                *cell = head;
            }
            if let Some(cell) = row.get_mut(x + 1).filter(|_| width == WIDE_COLUMNS) {
                *cell = tail;
            }
        }
        Self::advance_cursor(screen, width, wrap);
    }

    /// A wide character does not fit in the last column: leave a spacer there and wrap.
    fn wrap_with_leading_spacer(&mut self) {
        let screen = self.screen_mut();
        let (x, y, blank) = (screen.cursor.x, screen.cursor.y, screen.cursor.pen.blank());
        if let Some(cell) = screen.grid.row_mut(y).and_then(|row| row.get_mut(x)) {
            *cell = blank;
            cell.insert_flags(CellFlags::LEADING_SPACER);
        }
        self.wrap_line();
    }

    /// REP: repeats the last printed character `n` times.
    pub(crate) fn repeat_last(&mut self, n: usize) {
        let Some(c) = self.last_char else { return };
        for _ in 0..n.min(usize::from(crate::limits::MAX_REPEAT)) {
            self.print_char(c);
        }
    }
}
