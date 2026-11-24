//! Settings → Terminal (`docs/09` §8, `docs/10` §3).

use serde::{Deserialize, Serialize};

use crate::value::bounded;

/// Characters that count as part of a word for double-click selection, besides letters and
/// digits (`FR-TERM-014`).
pub const DEFAULT_WORD_CHARACTERS: &str = "-_.~/:@";

/// URL schemes that open on Ctrl/Cmd+click by default (`docs/10` §3 item 6).
pub const DEFAULT_URL_SCHEMES: [&str; 4] = ["http", "https", "mailto", "file"];

bounded!(
    /// Scrollback length in lines (`FR-TERM-011`: default 10 000, min 0, max 1 000 000).
    ScrollbackLines(u32),
    min = 0,
    max = 1_000_000,
    default = 10_000
);
bounded!(
    /// Lines scrolled per mouse-wheel step.
    ScrollSpeed(u16),
    min = 1,
    max = 20,
    default = 3
);

/// Bell behavior (`FR-TERM-018`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Bell {
    /// Tab indicator and border pulse (default; no sound, `docs/09` §5).
    #[default]
    Visual,
    /// Visual plus system sound.
    Audible,
    /// Nothing.
    Off,
}

/// Which macOS Option keys act as Alt (`FR-TERM-008`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OptionAsAlt {
    /// Neither (Option composes characters).
    #[default]
    None,
    /// Left Option only.
    Left,
    /// Right Option only.
    Right,
    /// Both.
    Both,
}

/// OSC 52 clipboard policy (`docs/10` §3 item 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Osc52Policy {
    /// Ignore OSC 52.
    Deny,
    /// Programs may write the clipboard, with a notification; reads are denied (default).
    #[default]
    WriteOnly,
    /// Writes allowed; each read asks the user.
    Ask,
}

/// Terminal section.
#[allow(
    clippy::struct_excessive_bools,
    reason = "each bool is an independent on/off setting shown as a toggle in the UI (docs/09 §8)"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Terminal {
    /// Scrollback length.
    pub scrollback_lines: ScrollbackLines,
    /// Wheel scroll speed.
    pub scroll_speed: ScrollSpeed,
    /// Copy the selection as soon as it is made (`FR-UI-009`).
    pub copy_on_select: bool,
    /// Ctrl+C copies when a selection exists, else sends SIGINT (`docs/09` §5).
    pub smart_copy: bool,
    /// Confirm multi-line pastes into programs without bracketed paste (`docs/09` §5: on).
    pub paste_confirmation: bool,
    /// Honor bracketed paste mode 2004 (`FR-TERM-006`).
    pub bracketed_paste: bool,
    /// Extra word characters for double-click selection.
    pub word_characters: String,
    /// URL schemes allowed to open on click.
    pub url_schemes: Vec<String>,
    /// Bell behavior.
    pub bell: Bell,
    /// macOS Option-as-Alt.
    pub option_as_alt: OptionAsAlt,
    /// Kitty keyboard protocol (`FR-TERM-008`).
    pub kitty_keyboard: bool,
    /// OSC 52 policy.
    pub osc52: Osc52Policy,
    /// Inline images (`FR-TERM-012`/`FR-TERM-013`); the cache cap is in Performance.
    pub images: bool,
}

impl Default for Terminal {
    fn default() -> Self {
        Self {
            scrollback_lines: ScrollbackLines::default(),
            scroll_speed: ScrollSpeed::default(),
            copy_on_select: false,
            smart_copy: false,
            paste_confirmation: true,
            bracketed_paste: true,
            word_characters: DEFAULT_WORD_CHARACTERS.to_owned(),
            url_schemes: DEFAULT_URL_SCHEMES.map(str::to_owned).to_vec(),
            bell: Bell::default(),
            option_as_alt: OptionAsAlt::default(),
            kitty_keyboard: true,
            osc52: Osc52Policy::default(),
            images: true,
        }
    }
}
