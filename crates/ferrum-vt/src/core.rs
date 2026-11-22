//! The terminal state that the escape-sequence handlers act on.

use ferrum_scrollback::{Scrollback, StyledRun};

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
    /// Row text while encoding for the scrollback.
    pub(crate) text: String,
    /// Style runs while encoding for the scrollback.
    pub(crate) runs: Vec<StyledRun>,
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
    pub(crate) modes: Mode,
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
    /// Raised by the handlers when `vte` is certain to be back in its ground state
    /// (see `input.rs`).
    pub(crate) stop: bool,
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
            modes: Mode::default(),
            tabs: TabStops::new(size.cols()),
            scrollback,
            extras: Extras::new(),
            palette: Palette::default(),
            title: String::new(),
            cwd: String::new(),
            scratch: Scratch::default(),
            last_char: None,
            stop: false,
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
