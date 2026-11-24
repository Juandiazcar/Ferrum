//! The terminal state that the escape-sequence handlers act on.

use ferrum_scrollback::Scrollback;

use crate::codec::Encoded;

use crate::cursor::CursorStyle;
use crate::dcs::Dcs;
use crate::events::EventSink;
use crate::extras::Extras;
use crate::modes::Mode;
use crate::palette::Palette;
use crate::screen::Screen;
use crate::size::Size;
use crate::tabs::TabStops;

/// Reusable buffers: the hot paths never allocate once these have grown to their working size.
#[derive(Debug, Default)]
pub(crate) struct Scratch {
    /// The row being encoded for the scrollback.
    pub(crate) line: Encoded,
    /// Cluster buffer for grapheme joining.
    pub(crate) cluster: String,
    /// Sanitized title, link key and other short strings built from an OSC payload.
    pub(crate) short: String,
    /// Decoded OSC 52 data.
    pub(crate) bytes: Vec<u8>,
}

/// Everything except the byte-stream front end.
pub(crate) struct Core<S> {
    pub(crate) sink: S,
    pub(crate) primary: Screen,
    pub(crate) alt: Option<Screen>,
    /// The alternate screen's DECSC memory, kept while the screen itself is freed (xterm keeps
    /// one saved cursor per screen; the Alacritty reference test `saved_cursor_alt` relies on it).
    pub(crate) alt_saved: Option<crate::cursor::SavedCursor>,
    /// The alternate screen's kitty keyboard stack, kept while the screen itself is freed: the
    /// protocol gives each screen its own stack and it survives switching away and back.
    pub(crate) alt_keyboard: crate::keyboard::KeyboardStack,
    pub(crate) modes: Mode,
    /// G0 to G3 and the shift state. They belong to the terminal, not to a screen (xterm keeps
    /// one set for both buffers), and DECSC saves and restores them.
    pub(crate) charsets: crate::charset::Charsets,
    pub(crate) tabs: TabStops,
    pub(crate) scrollback: Scrollback,
    pub(crate) extras: Extras,
    pub(crate) palette: Palette,
    /// The last title and working directory the program set (for snapshots).
    pub(crate) title: String,
    pub(crate) cwd: String,
    pub(crate) scratch: Scratch,
    /// The last graphic character printed (REP repeats it).
    pub(crate) last_char: Option<char>,
    /// Raised by `unhook`: the DCS string left `vte`. A CAN or SUB right after it cancels the
    /// string (`perform.rs`); otherwise the front end answers it (`input.rs`).
    pub(crate) dcs_end: bool,
    /// DECSCUSR; `None` until a program sets it (the UI default applies).
    pub(crate) cursor_style: Option<CursorStyle>,
    /// The DCS string in progress.
    pub(crate) dcs: Dcs,
}

impl<S: EventSink> Core<S> {
    pub(crate) fn new(size: Size, scrollback: Scrollback, sink: S) -> Self {
        Self {
            sink,
            primary: Screen::new(size),
            alt: None,
            alt_saved: None,
            alt_keyboard: crate::keyboard::KeyboardStack::default(),
            modes: Mode::default(),
            charsets: crate::charset::Charsets::default(),
            tabs: TabStops::new(size.cols()),
            scrollback,
            extras: Extras::new(),
            palette: Palette::default(),
            title: String::new(),
            cwd: String::new(),
            scratch: Scratch::default(),
            last_char: None,
            dcs_end: false,
            cursor_style: None,
            dcs: Dcs::Ignore,
        }
    }

    /// True while the alternate screen is shown.
    pub(crate) fn alt_active(&self) -> bool {
        self.modes.contains(Mode::ALT_SCREEN) && self.alt.is_some()
    }

    /// The screen being shown.
    pub(crate) fn screen(&self) -> &Screen {
        match &self.alt {
            Some(alt) if self.modes.contains(Mode::ALT_SCREEN) => alt,
            _ => &self.primary,
        }
    }

    /// The screen being shown, mutable.
    pub(crate) fn screen_mut(&mut self) -> &mut Screen {
        match &mut self.alt {
            Some(alt) if self.modes.contains(Mode::ALT_SCREEN) => alt,
            _ => &mut self.primary,
        }
    }
}
