//! CSI dispatch: cursor movement, erase, insert/delete, scroll region, tabs (xterm ctlseqs).

use crate::args::Args;
use crate::core::Core;
use crate::events::EventSink;
use crate::modes::{Scope, Switch};

/// TBC parameter that clears the tab stop at the cursor (the default).
const TBC_CURRENT: u16 = 0;
/// TBC parameter that clears every tab stop.
const TBC_ALL: u16 = 3;

impl<S: EventSink> Core<S> {
    /// Routes a CSI sequence. Unknown sequences are ignored (and never reported back).
    pub(crate) fn csi(&mut self, args: Args<'_>, intermediates: &[u8], action: char) {
        match (intermediates, action) {
            ([], _) => self.csi_plain(args, action),
            ([b'?'], _) => self.csi_private(args, action),
            ([b' '], 'q') => self.set_cursor_style(args.mode(0)),
            ([b'!'], 'p') => self.soft_reset(),
            ([b'$'], 'p') => self.report_mode(args, Scope::Ansi),
            ([b'?', b'$'], 'p') => self.report_mode(args, Scope::Private),
            _ => {}
        }
    }

    fn csi_plain(&mut self, args: Args<'_>, action: char) {
        if self.csi_cursor(args, action) || self.csi_edit(args, action) {
            return;
        }
        match action {
            'm' => self.sgr(args),
            'h' => self.set_modes(args, Scope::Ansi, Switch::Set),
            'l' => self.set_modes(args, Scope::Ansi, Switch::Reset),
            'g' => self.clear_tab_stops(args.mode(0)),
            'r' => self.set_scroll_region(args),
            's' => self.save_cursor(),
            'u' => self.restore_cursor(),
            _ => {}
        }
    }

    /// Cursor movement; true when `action` was one.
    fn csi_cursor(&mut self, args: Args<'_>, action: char) -> bool {
        let n = args.count(0, 1);
        match action {
            'A' => self.cursor_up(n),
            'B' | 'e' => self.cursor_down(n),
            'C' | 'a' => self.cursor_forward(n),
            'D' => self.cursor_back(n),
            'E' => {
                self.cursor_down(n);
                self.carriage_return();
            }
            'F' => {
                self.cursor_up(n);
                self.carriage_return();
            }
            'G' | '`' => self.goto(None, Some(n - 1)),
            'H' | 'f' => self.goto(Some(n - 1), Some(args.count(1, 1) - 1)),
            'd' => self.goto(Some(n - 1), None),
            'I' => self.tab_forward(n),
            'Z' => self.tab_back(n),
            _ => return false,
        }
        true
    }

    /// Erase, insert, delete and scroll; true when `action` was one.
    fn csi_edit(&mut self, args: Args<'_>, action: char) -> bool {
        let n = args.count(0, 1);
        match action {
            'J' => self.erase_display(args.mode(0)),
            'K' => self.erase_line(args.mode(0)),
            'X' => self.erase_chars(args),
            '@' => self.insert_chars(n),
            'P' => self.delete_chars(n),
            'L' => self.insert_lines(n),
            'M' => self.delete_lines(n),
            'S' => self.scroll_up(n),
            'T' => self.scroll_down(n),
            'b' => self.repeat_last(n),
            _ => return false,
        }
        true
    }

    fn csi_private(&mut self, args: Args<'_>, action: char) {
        match action {
            'h' => self.set_modes(args, Scope::Private, Switch::Set),
            'l' => self.set_modes(args, Scope::Private, Switch::Reset),
            _ => {}
        }
    }

    /// TBC.
    fn clear_tab_stops(&mut self, mode: u16) {
        match mode {
            TBC_CURRENT => {
                let x = self.screen().cursor.x;
                self.tabs.clear(x);
            }
            TBC_ALL => self.tabs.clear_all(),
            _ => {}
        }
    }

    /// DECSTBM: top and bottom margins, 1-based. An empty or inverted region is ignored.
    fn set_scroll_region(&mut self, args: Args<'_>) {
        let lines = self.screen().grid.lines();
        let top = args.count(0, 1) - 1;
        let bottom = args.count(1, lines).min(lines) - 1;
        // A one-row region (`top == bottom`) is ignored too, as xterm does.
        let Some(region) = crate::screen::Region::new(top, bottom).filter(|_| top < bottom) else {
            return;
        };
        self.screen_mut().region = region;
        self.goto(Some(0), Some(0));
    }
}
