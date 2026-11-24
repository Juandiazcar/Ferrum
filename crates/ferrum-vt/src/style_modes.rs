//! DECSCUSR and DECSTR: cursor style and the soft reset.

use crate::charset::Charsets;
use crate::core::Core;
use crate::cursor::{Cursor, CursorShape, CursorStyle, Pen};
use crate::events::EventSink;
use crate::modes::Mode;
use crate::screen::Region;

/// DECSCUSR 0 and 1 are the default (blinking block); even values are steady.
pub(crate) mod decscusr {
    pub(crate) const DEFAULT: u16 = 0;
    pub(crate) const BLINKING_BLOCK: u16 = 1;
    pub(crate) const STEADY_BLOCK: u16 = 2;
    pub(crate) const BLINKING_UNDERLINE: u16 = 3;
    pub(crate) const STEADY_UNDERLINE: u16 = 4;
    pub(crate) const BLINKING_BAR: u16 = 5;
    pub(crate) const STEADY_BAR: u16 = 6;
}

const fn style_for(code: u16) -> Option<CursorStyle> {
    let (shape, blinking) = match code {
        decscusr::DEFAULT | decscusr::BLINKING_BLOCK => (CursorShape::Block, true),
        decscusr::STEADY_BLOCK => (CursorShape::Block, false),
        decscusr::BLINKING_UNDERLINE => (CursorShape::Underline, true),
        decscusr::STEADY_UNDERLINE => (CursorShape::Underline, false),
        decscusr::BLINKING_BAR => (CursorShape::Bar, true),
        decscusr::STEADY_BAR => (CursorShape::Bar, false),
        _ => return None,
    };
    Some(CursorStyle { shape, blinking })
}

impl<S: EventSink> Core<S> {
    /// `CSI Ps SP q`. An unknown style is ignored.
    pub(crate) fn set_cursor_style(&mut self, code: u16) {
        if let Some(style) = style_for(code) {
            self.cursor_style = Some(style);
            self.modes.set(Mode::CURSOR_BLINK, style.blinking);
        }
    }

    /// DECSTR: resets modes, attributes, margins and charsets, but not the screen contents.
    pub(crate) fn soft_reset(&mut self) {
        self.modes.remove(
            Mode::INSERT
                | Mode::ORIGIN
                | Mode::APP_CURSOR
                | Mode::APP_KEYPAD
                | Mode::LINE_FEED_NEWLINE,
        );
        self.modes.insert(Mode::AUTOWRAP | Mode::CURSOR_VISIBLE);
        self.cursor_style = None;
        let screen = self.screen_mut();
        screen.region = Region::full(screen.grid.lines());
        screen.charsets = Charsets::default();
        screen.saved = None;
        screen.cursor = Cursor {
            x: screen.cursor.x,
            y: screen.cursor.y,
            pen: Pen::default(),
            wrap_pending: false,
        };
    }
}
