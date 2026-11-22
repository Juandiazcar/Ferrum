//! CSI dispatch: cursor movement, erase, insert/delete, scroll region, tabs (xterm ctlseqs).

use crate::args::Args;
use crate::core::Core;
use crate::erase::Protection;
use crate::events::EventSink;

impl<S: EventSink> Core<S> {
    /// Routes a CSI sequence. Unknown sequences are ignored (and never reported back).
    pub(crate) fn csi(&mut self, args: Args<'_>, intermediates: &[u8], action: char) {
        match (intermediates, action) {
            ([], _) => self.csi_plain(args, action),
            ([b'?'], _) => self.csi_private(args, action),
            ([b' '], 'q') => self.set_cursor_style(args.mode(0)),
            ([b'"'], 'q') => self.set_pen_protection(args.mode(0)),
            ([b'!'], 'p') => self.soft_reset(),
            ([b'$'], 'p') => self.report_mode(args, false),
            ([b'?', b'$'], 'p') => self.report_mode(args, true),
            _ => {}
        }
    }

    fn csi_plain(&mut self, args: Args<'_>, action: char) {
        if self.csi_cursor(args, action) || self.csi_edit(args, action) {
            return;
        }
        match action {
            'm' => self.sgr(args),
            'h' => self.set_modes(args, false, true),
            'l' => self.set_modes(args, false, false),
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
            'J' => self.erase_display(args.mode(0), Protection::Ignore),
            'K' => self.erase_line(args.mode(0), Protection::Ignore),
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
            'J' => self.erase_display(args.mode(0), Protection::Respect),
            'K' => self.erase_line(args.mode(0), Protection::Respect),
            'h' => self.set_modes(args, true, true),
            'l' => self.set_modes(args, true, false),
            _ => {}
        }
    }

    /// TBC.
    fn clear_tab_stops(&mut self, mode: u16) {
        match mode {
            0 => {
                let x = self.screen().cursor.x;
                self.tabs.clear(x);
            }
            3 => self.tabs.clear_all(),
            _ => {}
        }
    }

    /// DECSTBM: top and bottom margins, 1-based. An empty or inverted region is ignored.
    fn set_scroll_region(&mut self, args: Args<'_>) {
        let lines = self.screen().grid.lines();
        let top = args.count(0, 1) - 1;
        let bottom = args.count(1, lines).min(lines) - 1;
        if top >= bottom {
            return;
        }
        self.screen_mut().region = crate::screen::Region { top, bottom };
        self.goto(Some(0), Some(0));
    }
}
