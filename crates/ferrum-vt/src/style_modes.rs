//! DECSCUSR, DECSCA and DECSTR: cursor style, character protection and the soft reset.

use crate::cell::Attrs;
use crate::charset::Charsets;
use crate::core::Core;
use crate::cursor::{Cursor, CursorShape, CursorStyle, Pen};
use crate::events::EventSink;
use crate::modes::Mode;
use crate::screen::Region;

/// DECSCUSR 0 and 1 are the default (blinking block); even values are steady.
const fn style_for(code: u16) -> Option<CursorStyle> {
    let (shape, blinking) = match code {
        0 | 1 => (CursorShape::Block, true),
        2 => (CursorShape::Block, false),
        3 => (CursorShape::Underline, true),
        4 => (CursorShape::Underline, false),
        5 => (CursorShape::Bar, true),
        6 => (CursorShape::Bar, false),
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

    /// `CSI Ps " q`: 1 protects the cells printed from now on, 0 and 2 stop doing so.
    pub(crate) fn set_pen_protection(&mut self, code: u16) {
        let mut pen = self.screen().cursor.pen;
        match code {
            1 => pen.set_attr(Attrs::PROTECTED, true),
            0 | 2 => pen.set_attr(Attrs::PROTECTED, false),
            _ => return,
        }
        self.set_pen(pen);
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
