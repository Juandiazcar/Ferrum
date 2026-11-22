//! Reports the program may ask for: device attributes, status, cursor position, the version and
//! the size in characters.
//!
//! Every reply is fixed text plus numbers the terminal itself owns; none carries text a program
//! supplied (`docs/10` §3 item 3). There is no title or icon report (`CSI 21 t` is ignored) and no
//! answerback (ENQ).

use crate::args::Args;
use crate::core::Core;
use crate::events::EventSink;
use crate::keyboard::{KeyboardFlags, SetMode};
use crate::modes::{Mode, Scope};
use crate::reply::Reply;

/// Primary device attributes: a VT220 with ANSI color (`CSI c`).
const PRIMARY_ATTRIBUTES: &[u8] = b"\x1b[?62;22c";
/// Secondary device attributes: `>Pp;Pv;Pc` with VT220 as the terminal type and version 10.
const SECONDARY_ATTRIBUTES: &[u8] = b"\x1b[>1;10;0c";
/// `CSI 5 n`: the terminal is fine.
const STATUS_OK: &[u8] = b"\x1b[0n";
/// Start of the XTVERSION reply.
const VERSION_PREFIX: &[u8] = b"\x1bP>|Ferrum ";
/// String terminator.
const ST: &[u8] = b"\x1b\\";
/// The `CSI 18 t` request (report the text area size in characters).
const REPORT_TEXT_SIZE: u16 = 18;

/// Which device attributes report is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttributeLevel {
    /// `CSI c`.
    Primary,
    /// `CSI > c`.
    Secondary,
}

impl<S: EventSink> Core<S> {
    /// `CSI c` and `CSI > c`.
    pub(crate) fn report_attributes(&mut self, level: AttributeLevel) {
        let text = match level {
            AttributeLevel::Primary => PRIMARY_ATTRIBUTES,
            AttributeLevel::Secondary => SECONDARY_ATTRIBUTES,
        };
        self.send(&Reply::new().text(text));
    }

    /// `CSI > q`: XTVERSION.
    pub(crate) fn report_version(&mut self) {
        let reply =
            Reply::new().text(VERSION_PREFIX).text(env!("CARGO_PKG_VERSION").as_bytes()).text(ST);
        self.send(&reply);
    }

    /// `CSI Ps n` and `CSI ? Ps n`.
    pub(crate) fn report_status(&mut self, code: u16, scope: Scope) {
        match (code, scope) {
            (5, Scope::Ansi) => self.send(&Reply::new().text(STATUS_OK)),
            (6, _) => self.report_cursor_position(scope),
            _ => {}
        }
    }

    /// CPR / DECXCPR: the cursor position, 1-based, relative to the scroll region in origin mode.
    fn report_cursor_position(&mut self, scope: Scope) {
        let origin = self.modes.contains(Mode::ORIGIN);
        let screen = self.screen();
        let top = if origin { screen.region.top } else { 0 };
        let (row, col) = (screen.cursor.y.saturating_sub(top) + 1, screen.cursor.x + 1);
        let head: &[u8] = match scope {
            Scope::Private => b"\x1b[?",
            Scope::Ansi => b"\x1b[",
        };
        self.send(&Reply::new().text(head).number(row).text(b";").number(col).text(b"R"));
    }

    /// `CSI Ps t`: only the text area size report (18) is answered.
    pub(crate) fn window_operation(&mut self, args: Args<'_>) {
        if args.mode(0) != REPORT_TEXT_SIZE {
            return;
        }
        let grid = &self.screen().grid;
        let (rows, cols) = (grid.lines(), grid.cols());
        self.send(&Reply::new().text(b"\x1b[8;").number(rows).text(b";").number(cols).text(b"t"));
    }

    /// `CSI > flags u`: pushes keyboard flags.
    pub(crate) fn keyboard_push(&mut self, args: Args<'_>) {
        let flags = KeyboardFlags::from_bits_truncate(u32::from(args.mode(0)));
        self.screen_mut().keyboard.push(flags);
    }

    /// `CSI < n u`: pops keyboard flags.
    pub(crate) fn keyboard_pop(&mut self, args: Args<'_>) {
        self.screen_mut().keyboard.pop(args.count(0, 1));
    }

    /// `CSI = flags ; mode u`.
    pub(crate) fn keyboard_set(&mut self, args: Args<'_>) {
        let flags = KeyboardFlags::from_bits_truncate(u32::from(args.mode(0)));
        let mode = SetMode::from_code(args.raw(1).unwrap_or(1));
        if let Some(mode) = mode {
            self.screen_mut().keyboard.set(flags, mode);
        }
    }

    /// `CSI ? u`: reports the active keyboard flags.
    pub(crate) fn keyboard_query(&mut self) {
        let flags = self.screen().keyboard.current().bits();
        let reply =
            Reply::new().text(b"\x1b[?").number(usize::try_from(flags).unwrap_or(0)).text(b"u");
        self.send(&reply);
    }
}
