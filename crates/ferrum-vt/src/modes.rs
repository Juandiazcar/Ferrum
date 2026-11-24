//! Terminal modes (`docs/01` FR-TERM-004 to FR-TERM-007): the state the UI needs to encode input
//! and the state the grid needs to behave.

use bitflags::bitflags;

bitflags! {
    /// Boolean terminal modes.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Mode: u32 {
        /// DECAWM (7): wrap at the right margin.
        const AUTOWRAP = 1;
        /// DECOM (6): cursor addressing is relative to the scroll region.
        const ORIGIN = 1 << 1;
        /// IRM (4): printing inserts instead of replacing.
        const INSERT = 1 << 2;
        /// LNM (20): line feed also returns the carriage.
        const LINE_FEED_NEWLINE = 1 << 3;
        /// DECCKM (1): cursor keys send application sequences.
        const APP_CURSOR = 1 << 4;
        /// DECKPAM: the keypad sends application sequences.
        const APP_KEYPAD = 1 << 5;
        /// DECTCEM (25): the cursor is shown.
        const CURSOR_VISIBLE = 1 << 6;
        /// Mode 12: the cursor blinks (the UI default applies until a program sets it).
        const CURSOR_BLINK = 1 << 7;
        /// DECSCNM (5): reverse video for the whole screen.
        const REVERSE_VIDEO = 1 << 8;
        /// The alternate screen is active (47, 1047, 1049).
        const ALT_SCREEN = 1 << 9;
        /// Bracketed paste (2004).
        const BRACKETED_PASTE = 1 << 10;
        /// Focus in/out reports (1004).
        const FOCUS_REPORTING = 1 << 11;
        /// X10 mouse reports (9).
        const MOUSE_X10 = 1 << 12;
        /// Normal mouse tracking (1000).
        const MOUSE_NORMAL = 1 << 13;
        /// Button-event mouse tracking (1002).
        const MOUSE_BUTTON = 1 << 14;
        /// Any-event mouse tracking (1003).
        const MOUSE_ANY = 1 << 15;
        /// SGR mouse encoding (1006).
        const MOUSE_SGR = 1 << 16;
        /// Alternate scroll (1007): wheel sends arrow keys on the alternate screen.
        const ALTERNATE_SCROLL = 1 << 17;
        /// Synchronized output (2026): the daemon holds back damage while set.
        const SYNC_UPDATE = 1 << 18;
        /// Mode 40: DECCOLM is allowed.
        const ALLOW_COLUMN_SWITCH = 1 << 19;
        /// DECCOLM (3): 132-column mode.
        const COLUMN_132 = 1 << 20;
    }
}

impl Default for Mode {
    fn default() -> Self {
        Self::AUTOWRAP | Self::CURSOR_VISIBLE
    }
}

/// Which mouse events the program wants (the highest tracking mode that is set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MouseTracking {
    /// No mouse reports.
    Off,
    /// Press only (9).
    X10,
    /// Press and release (1000).
    Normal,
    /// Plus motion while a button is held (1002).
    Button,
    /// Plus all motion (1003).
    Any,
}

impl Mode {
    /// The active mouse tracking level.
    pub fn mouse_tracking(self) -> MouseTracking {
        if self.contains(Self::MOUSE_ANY) {
            MouseTracking::Any
        } else if self.contains(Self::MOUSE_BUTTON) {
            MouseTracking::Button
        } else if self.contains(Self::MOUSE_NORMAL) {
            MouseTracking::Normal
        } else if self.contains(Self::MOUSE_X10) {
            MouseTracking::X10
        } else {
            MouseTracking::Off
        }
    }

    /// The mouse tracking bits, to be cleared together.
    pub const MOUSE_TRACKING: Self =
        Self::MOUSE_X10.union(Self::MOUSE_NORMAL).union(Self::MOUSE_BUTTON).union(Self::MOUSE_ANY);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_wrap_and_show_the_cursor() {
        assert_eq!(Mode::default(), Mode::AUTOWRAP | Mode::CURSOR_VISIBLE);
    }

    #[test]
    fn the_highest_tracking_level_wins() {
        assert_eq!(Mode::default().mouse_tracking(), MouseTracking::Off);
        assert_eq!((Mode::MOUSE_X10 | Mode::MOUSE_NORMAL).mouse_tracking(), MouseTracking::Normal);
        assert_eq!((Mode::MOUSE_NORMAL | Mode::MOUSE_ANY).mouse_tracking(), MouseTracking::Any);
    }
}
