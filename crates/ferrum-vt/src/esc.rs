//! ESC dispatch: DECSC/DECRC, index controls, character set designation, DECALN, RIS.

use crate::charset::Charset;
use crate::core::Core;
use crate::cursor::{Cursor, SavedCursor};
use crate::events::EventSink;
use crate::modes::Mode;
use crate::screen::Screen;

impl<S: EventSink> Core<S> {
    /// Routes `ESC <intermediates> <final>`.
    pub(crate) fn esc(&mut self, intermediates: &[u8], byte: u8) {
        match (intermediates, byte) {
            ([], b'7') => self.save_cursor(),
            ([], b'8') => self.restore_cursor(),
            ([], b'D') => self.index(),
            ([], b'E') => self.next_line(),
            ([], b'M') => self.reverse_index(),
            ([], b'H') => self.control(0x88),
            ([], b'c') => self.reset(),
            ([], b'=') => self.modes.insert(Mode::APP_KEYPAD),
            ([], b'>') => self.modes.remove(Mode::APP_KEYPAD),
            ([], b'n') => self.charsets.shift(2),
            ([], b'o') => self.charsets.shift(3),
            ([], b'N') => self.charsets.single_shift(2),
            ([], b'O') => self.charsets.single_shift(3),
            ([b'#'], b'8') => self.screen_alignment_test(),
            ([slot @ (b'(' | b')' | b'*' | b'+')], final_byte) => {
                self.designate(*slot, final_byte);
            }
            _ => {}
        }
    }

    /// ESC ( ) * + followed by a set name.
    fn designate(&mut self, slot: u8, final_byte: u8) {
        let Some(charset) = Charset::from_final(final_byte) else { return };
        let index = match slot {
            b'(' => 0,
            b')' => 1,
            b'*' => 2,
            _ => 3,
        };
        self.charsets.designate(index, charset);
    }

    /// DECSC.
    pub(crate) fn save_cursor(&mut self) {
        let origin = self.modes.contains(Mode::ORIGIN);
        let charsets = self.charsets;
        let screen = self.screen_mut();
        screen.saved = Some(SavedCursor { cursor: screen.cursor, origin, charsets });
    }

    /// DECRC. Without a saved state the cursor goes home with default attributes.
    pub(crate) fn restore_cursor(&mut self) {
        let saved = self.screen().saved.unwrap_or_default();
        self.modes.set(Mode::ORIGIN, saved.origin);
        let screen = self.screen_mut();
        let last_x = screen.grid.cols().saturating_sub(1);
        let last_y = screen.grid.lines().saturating_sub(1);
        screen.cursor =
            Cursor { x: saved.cursor.x.min(last_x), y: saved.cursor.y.min(last_y), ..saved.cursor };
        self.charsets = saved.charsets;
    }

    /// DECALN: fills the screen with `E` in the default style. The cursor and the margins stay
    /// where they are (the Alacritty reference test `decaln_reset`).
    fn screen_alignment_test(&mut self) {
        let screen = self.screen_mut();
        let mut fill = crate::cell::Cell::default();
        fill.set_scalar('E');
        for y in 0..screen.grid.lines() {
            if let Some(row) = screen.grid.row_mut(y) {
                row.fill(0, usize::MAX, fill);
                row.set_flags(ferrum_scrollback::LineFlags::NONE);
            }
        }
    }

    /// RIS: back to the power-on state; the history is discarded too (the Alacritty reference
    /// test `grid_reset`).
    pub(crate) fn reset(&mut self) {
        self.clear_scrollback();
        let size = self.primary.grid.size();
        self.alt = None;
        self.alt_saved = None;
        self.charsets = crate::charset::Charsets::default();
        self.modes = Mode::default();
        self.tabs.reset();
        self.primary = Screen::new(size);
        self.last_char = None;
    }
}
