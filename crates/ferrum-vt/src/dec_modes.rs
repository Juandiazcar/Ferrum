//! SM/RM and DECSET/DECRST (xterm ctlseqs "Set Mode", "DEC Private Mode Set"), DECRQM, and the
//! alternate screen (FR-TERM-004, FR-TERM-005, FR-TERM-006, FR-TERM-007).

use crate::args::Args;
use crate::core::Core;
use crate::erase::Protection;
use crate::events::EventSink;
use crate::modes::Mode;
use crate::reply::Reply;
use crate::screen::{Region, Screen};

/// What `DECRQM` answers for a mode (the `Pm` of `CSI ? Ps ; Pm $ y`).
#[derive(Clone, Copy)]
enum ModeState {
    NotRecognized = 0,
    Set = 1,
    Reset = 2,
}

/// The flag behind an ANSI mode number.
fn ansi_flag(code: u16) -> Option<Mode> {
    match code {
        4 => Some(Mode::INSERT),
        20 => Some(Mode::LINE_FEED_NEWLINE),
        _ => None,
    }
}

/// The flag behind a DEC private mode number.
fn private_flag(code: u16) -> Option<Mode> {
    Some(match code {
        1 => Mode::APP_CURSOR,
        3 => Mode::COLUMN_132,
        5 => Mode::REVERSE_VIDEO,
        6 => Mode::ORIGIN,
        7 => Mode::AUTOWRAP,
        9 => Mode::MOUSE_X10,
        12 => Mode::CURSOR_BLINK,
        25 => Mode::CURSOR_VISIBLE,
        40 => Mode::ALLOW_COLUMN_SWITCH,
        47 | 1047 | 1049 => Mode::ALT_SCREEN,
        1000 => Mode::MOUSE_NORMAL,
        1002 => Mode::MOUSE_BUTTON,
        1003 => Mode::MOUSE_ANY,
        1004 => Mode::FOCUS_REPORTING,
        1006 => Mode::MOUSE_SGR,
        1007 => Mode::ALTERNATE_SCROLL,
        2004 => Mode::BRACKETED_PASTE,
        2026 => Mode::SYNC_UPDATE,
        _ => return None,
    })
}

impl<S: EventSink> Core<S> {
    /// `CSI Pm h` and `CSI Pm l` (ANSI modes) and their `?` forms (DEC private modes).
    pub(crate) fn set_modes(&mut self, args: Args<'_>, private: bool, on: bool) {
        for param in args.iter() {
            let code = param.first().copied().unwrap_or(0);
            if private {
                self.set_private_mode(code, on);
            } else if let Some(flag) = ansi_flag(code) {
                self.modes.set(flag, on);
            }
        }
    }

    fn set_private_mode(&mut self, code: u16, on: bool) {
        match code {
            3 => self.column_mode(on),
            6 => {
                self.modes.set(Mode::ORIGIN, on);
                self.goto(Some(0), Some(0));
            }
            9 | 1000 | 1002 | 1003 => self.set_mouse_tracking(code, on),
            47 | 1047 => self.set_alt_screen(on, CursorHandling::Keep),
            1048 => {
                if on {
                    self.save_cursor();
                } else {
                    self.restore_cursor();
                }
            }
            1049 => self.set_alt_screen(on, CursorHandling::SaveAndRestore),
            _ => {
                if let Some(flag) = private_flag(code) {
                    self.modes.set(flag, on);
                    self.screen_mut().grid.damage_mut().mark_all();
                }
            }
        }
    }

    /// Only one mouse tracking level is active at a time: the last one enabled.
    fn set_mouse_tracking(&mut self, code: u16, on: bool) {
        let Some(flag) = private_flag(code) else { return };
        if on {
            self.modes.remove(Mode::MOUSE_TRACKING);
        }
        self.modes.set(flag, on);
    }

    /// DECCOLM: clears the screen and homes the cursor. The terminal never resizes itself: the
    /// UI owns the size, so only the mode flag and the clear are applied (as Alacritty does).
    fn column_mode(&mut self, on: bool) {
        self.modes.set(Mode::COLUMN_132, on);
        let screen = self.screen_mut();
        screen.region = Region::full(screen.grid.lines());
        self.erase_display(2, Protection::Ignore);
        self.goto(Some(0), Some(0));
    }

    fn set_alt_screen(&mut self, on: bool, cursor: CursorHandling) {
        if on == self.alt_active() {
            return;
        }
        if on {
            if cursor == CursorHandling::SaveAndRestore {
                self.save_cursor();
            }
            let mut alt = Screen::new(self.primary.grid.size());
            alt.cursor = self.primary.cursor;
            self.alt = Some(alt);
            self.modes.insert(Mode::ALT_SCREEN);
        } else {
            self.alt = None;
            self.modes.remove(Mode::ALT_SCREEN);
            self.primary.grid.damage_mut().mark_all();
            if cursor == CursorHandling::SaveAndRestore {
                self.restore_cursor();
            }
        }
    }

    /// DECRQM: reports whether a mode is set.
    pub(crate) fn report_mode(&mut self, args: Args<'_>, private: bool) {
        let code = args.mode(0);
        let flag = if private { private_flag(code) } else { ansi_flag(code) };
        let state = match flag {
            Some(flag) if self.modes.contains(flag) => ModeState::Set,
            Some(_) => ModeState::Reset,
            None => ModeState::NotRecognized,
        };
        let reply = Reply::new()
            .text(if private { b"\x1b[?" } else { b"\x1b[" })
            .number(usize::from(code))
            .text(b";")
            .number(state as usize)
            .text(b"$y");
        self.send(&reply);
    }
}

/// Whether entering or leaving the alternate screen also saves and restores the cursor.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CursorHandling {
    Keep,
    SaveAndRestore,
}
