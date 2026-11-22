//! The public terminal: bytes in, grid and events out.

use ferrum_scrollback::Scrollback;

use crate::core::Core;
use crate::cursor::{Cursor, CursorStyle};
use crate::damage::Damage;
use crate::error::VtError;
use crate::events::EventSink;
use crate::grid::Grid;
use crate::input::Input;
use crate::keyboard::KeyboardFlags;
use crate::modes::Mode;
use crate::palette::Palette;
use crate::screen::Screen;
use crate::size::Size;

/// One terminal session's state machine (`docs/02` §3).
///
/// Feed it the bytes the program writes with [`parse`](Self::parse); read the screen with
/// [`grid`](Self::grid), the history with [`scrollback`](Self::scrollback) and learn about
/// everything else (title, bell, replies for the PTY, ...) through the [`EventSink`].
pub struct Terminal<S: EventSink> {
    input: Input,
    pub(crate) core: Core<S>,
}

impl<S: EventSink> Terminal<S> {
    /// A blank terminal. The scrollback is created by the caller so that the daemon decides on
    /// the spill store.
    pub fn new(size: Size, scrollback: Scrollback, sink: S) -> Self {
        Self { input: Input::new(), core: Core::new(size, scrollback, sink) }
    }

    /// Feeds bytes from the program. Never panics and never fails: malformed input is dropped
    /// (`docs/10` §3 item 9).
    pub fn parse(&mut self, bytes: &[u8]) {
        self.input.advance(&mut self.core, bytes);
    }

    /// Resizes the terminal. A change of width reflows the primary screen and the scrollback
    /// (FR-TERM-010); the alternate screen is cut or padded. The size is clamped to the supported
    /// range.
    ///
    /// # Errors
    /// [`VtError::Scrollback`] when the history cannot be read back; the terminal is consistent
    /// afterwards but the history may be shorter.
    pub fn resize(&mut self, size: Size) -> Result<(), VtError> {
        self.core.resize(size)
    }

    /// The size of the terminal.
    pub fn size(&self) -> Size {
        self.core.primary.grid.size()
    }

    /// The visible screen (primary or alternate).
    pub fn screen(&self) -> &Screen {
        self.core.screen()
    }

    /// The visible grid.
    pub fn grid(&self) -> &Grid {
        &self.core.screen().grid
    }

    /// The cursor of the visible screen.
    pub fn cursor(&self) -> &Cursor {
        &self.core.screen().cursor
    }

    /// The terminal modes.
    pub fn modes(&self) -> Mode {
        self.core.modes
    }

    /// The history of rows that scrolled off the primary screen.
    pub fn scrollback(&self) -> &Scrollback {
        &self.core.scrollback
    }

    /// The damage of the visible screen since the last [`take_damage`](Self::take_damage).
    pub fn damage(&self) -> &Damage {
        self.core.screen().grid.damage()
    }

    /// Clears the damage of the visible screen.
    pub fn take_damage(&mut self) {
        self.core.screen_mut().grid.damage_mut().take();
    }

    /// The last title a program set (already sanitized).
    pub fn title(&self) -> &str {
        &self.core.title
    }

    /// The last working directory a program reported with OSC 7, as a `file://` URI.
    pub fn cwd(&self) -> &str {
        &self.core.cwd
    }

    /// The cursor style a program chose with DECSCUSR; `None` until it chooses one.
    pub fn cursor_style(&self) -> Option<CursorStyle> {
        self.core.cursor_style
    }

    /// The active kitty keyboard flags of the visible screen (0 when none were pushed).
    pub fn keyboard_flags(&self) -> KeyboardFlags {
        self.core.screen().keyboard.current()
    }

    /// The color palette and default colors.
    pub fn palette(&self) -> &Palette {
        &self.core.palette
    }

    /// Replaces the palette (a new theme). Marks the whole screen as damaged.
    pub fn set_palette(&mut self, palette: Palette) {
        self.core.palette = palette;
        self.core.screen_mut().grid.damage_mut().mark_all();
    }

    /// Answers a [`Event::ClipboardLoad`](crate::Event::ClipboardLoad) that the owner approved:
    /// writes the `OSC 52` reply carrying `clipboard` for `selection` to the sink. A denied
    /// request is simply never answered.
    pub fn answer_clipboard_request(&mut self, selection: &str, clipboard: &[u8]) {
        self.core.answer_clipboard(selection, clipboard);
    }

    /// The event sink.
    pub fn sink(&self) -> &S {
        &self.core.sink
    }

    /// The event sink, mutable.
    pub fn sink_mut(&mut self) -> &mut S {
        &mut self.core.sink
    }
}
