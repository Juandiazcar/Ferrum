//! SM/RM and DECSET/DECRST (xterm ctlseqs "Set Mode", "DEC Private Mode Set"), DECRQM, and the
//! alternate screen (FR-TERM-004, FR-TERM-005, FR-TERM-006, FR-TERM-007).

use crate::args::Args;
use crate::core::Core;
use crate::events::EventSink;
use crate::modes::{Mode, Scope, Switch};
use crate::reply::{self, Reply};
use crate::screen::{Region, Screen};

/// What `DECRQM` answers for a mode (the `Pm` of `CSI ? Ps ; Pm $ y`).
#[derive(Clone, Copy)]
enum ModeState {
    NotRecognized = 0,
    Set = 1,
    Reset = 2,
}

/// Mode numbers of SM/RM (ANSI) and DECSET/DECRST (private), as xterm numbers them.
mod number {
    pub(super) const INSERT: u16 = 4;
    pub(super) const LINE_FEED_NEWLINE: u16 = 20;
    pub(super) const APP_CURSOR: u16 = 1;
    pub(super) const COLUMN_132: u16 = 3;
    pub(super) const REVERSE_VIDEO: u16 = 5;
    pub(super) const ORIGIN: u16 = 6;
    pub(super) const AUTOWRAP: u16 = 7;
    pub(super) const MOUSE_X10: u16 = 9;
    pub(super) const CURSOR_BLINK: u16 = 12;
    pub(super) const CURSOR_VISIBLE: u16 = 25;
    pub(super) const ALLOW_COLUMN_SWITCH: u16 = 40;
    pub(super) const ALT_SCREEN_OLD: u16 = 47;
    pub(super) const MOUSE_NORMAL: u16 = 1000;
    pub(super) const MOUSE_BUTTON: u16 = 1002;
    pub(super) const MOUSE_ANY: u16 = 1003;
    pub(super) const FOCUS_REPORTING: u16 = 1004;
    pub(super) const MOUSE_SGR: u16 = 1006;
    pub(super) const ALTERNATE_SCROLL: u16 = 1007;
    pub(super) const ALT_SCREEN_CLEAR: u16 = 1047;
    pub(super) const SAVE_CURSOR: u16 = 1048;
    pub(super) const ALT_SCREEN_SAVE_CURSOR: u16 = 1049;
    pub(super) const BRACKETED_PASTE: u16 = 2004;
    pub(super) const SYNC_UPDATE: u16 = 2026;
}

/// The flag behind an ANSI mode number.
fn ansi_flag(code: u16) -> Option<Mode> {
    match code {
        number::INSERT => Some(Mode::INSERT),
        number::LINE_FEED_NEWLINE => Some(Mode::LINE_FEED_NEWLINE),
        _ => None,
    }
}

/// The flag behind a DEC private mode number.
fn private_flag(code: u16) -> Option<Mode> {
    Some(match code {
        number::APP_CURSOR => Mode::APP_CURSOR,
        number::COLUMN_132 => Mode::COLUMN_132,
        number::REVERSE_VIDEO => Mode::REVERSE_VIDEO,
        number::ORIGIN => Mode::ORIGIN,
        number::AUTOWRAP => Mode::AUTOWRAP,
        number::MOUSE_X10 => Mode::MOUSE_X10,
        number::CURSOR_BLINK => Mode::CURSOR_BLINK,
        number::CURSOR_VISIBLE => Mode::CURSOR_VISIBLE,
        number::ALLOW_COLUMN_SWITCH => Mode::ALLOW_COLUMN_SWITCH,
        number::ALT_SCREEN_OLD | number::ALT_SCREEN_CLEAR | number::ALT_SCREEN_SAVE_CURSOR => {
            Mode::ALT_SCREEN
        }
        number::MOUSE_NORMAL => Mode::MOUSE_NORMAL,
        number::MOUSE_BUTTON => Mode::MOUSE_BUTTON,
        number::MOUSE_ANY => Mode::MOUSE_ANY,
        number::FOCUS_REPORTING => Mode::FOCUS_REPORTING,
        number::MOUSE_SGR => Mode::MOUSE_SGR,
        number::ALTERNATE_SCROLL => Mode::ALTERNATE_SCROLL,
        number::BRACKETED_PASTE => Mode::BRACKETED_PASTE,
        number::SYNC_UPDATE => Mode::SYNC_UPDATE,
        _ => return None,
    })
}

impl<S: EventSink> Core<S> {
    /// `CSI Pm h` and `CSI Pm l` (ANSI modes) and their `?` forms (DEC private modes).
    pub(crate) fn set_modes(&mut self, args: Args<'_>, scope: Scope, switch: Switch) {
        for param in args.iter() {
            let code = param.first().copied().unwrap_or(0);
            match scope {
                Scope::Private => self.set_private_mode(code, switch),
                Scope::Ansi => {
                    if let Some(flag) = ansi_flag(code) {
                        self.modes.set(flag, switch.is_set());
                    }
                }
            }
        }
    }

    fn set_private_mode(&mut self, code: u16, switch: Switch) {
        match code {
            number::COLUMN_132 => self.column_mode(switch),
            number::ORIGIN => {
                self.modes.set(Mode::ORIGIN, switch.is_set());
                self.goto(Some(0), Some(0));
            }
            number::MOUSE_X10 | number::MOUSE_NORMAL | number::MOUSE_BUTTON | number::MOUSE_ANY => {
                self.set_mouse_tracking(code, switch);
            }
            number::ALT_SCREEN_OLD | number::ALT_SCREEN_CLEAR => {
                self.set_alt_screen(switch, CursorHandling::Keep);
            }
            number::SAVE_CURSOR => match switch {
                Switch::Set => self.save_cursor(),
                Switch::Reset => self.restore_cursor(),
            },
            number::ALT_SCREEN_SAVE_CURSOR => {
                self.set_alt_screen(switch, CursorHandling::SaveAndRestore);
            }
            _ => {
                if let Some(flag) = private_flag(code) {
                    self.modes.set(flag, switch.is_set());
                    self.screen_mut().grid.damage_mut().mark_all();
                }
            }
        }
    }

    /// Only one mouse tracking level is active at a time: the last one enabled.
    fn set_mouse_tracking(&mut self, code: u16, switch: Switch) {
        let Some(flag) = private_flag(code) else { return };
        if switch.is_set() {
            self.modes.remove(Mode::MOUSE_TRACKING);
        }
        self.modes.set(flag, switch.is_set());
    }

    /// DECCOLM: clears the screen and homes the cursor. The terminal never resizes itself: the
    /// UI owns the size, so only the mode flag and the clear are applied (as Alacritty does).
    fn column_mode(&mut self, switch: Switch) {
        self.modes.set(Mode::COLUMN_132, switch.is_set());
        let screen = self.screen_mut();
        screen.region = Region::full(screen.grid.lines());
        self.erase_display(crate::erase::ED_ALL);
        self.goto(Some(0), Some(0));
    }

    fn set_alt_screen(&mut self, switch: Switch, cursor: CursorHandling) {
        if switch.is_set() == self.alt_active() {
            return;
        }
        if switch.is_set() {
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
    pub(crate) fn report_mode(&mut self, args: Args<'_>, scope: Scope) {
        let code = args.mode(0);
        let flag = match scope {
            Scope::Private => private_flag(code),
            Scope::Ansi => ansi_flag(code),
        };
        let state = match flag {
            Some(flag) if self.modes.contains(flag) => ModeState::Set,
            Some(_) => ModeState::Reset,
            None => ModeState::NotRecognized,
        };
        let reply = Reply::new()
            .text(match scope {
                Scope::Private => reply::CSI_PRIVATE,
                Scope::Ansi => reply::CSI,
            })
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
