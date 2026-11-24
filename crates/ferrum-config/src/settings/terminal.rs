//! Settings → Terminal (`docs/09` §8, `docs/10` §3).
//!
//! Every default and bound below is either fixed by the spec (cited) or chosen and recorded in
//! `docs/14` Q-30 with the reason given here; all of them are editable in the settings UI.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::value::bounded;

/// Characters that count as part of a word for double-click selection, besides letters and
/// digits (`FR-TERM-014`). Default `-_.~/:@`: keeps paths, URLs, `user@host` and `a-b_c.d`
/// identifiers in one word.
pub const DEFAULT_WORD_CHARACTERS: &str = "-_.~/:@";

/// URL schemes that open on Ctrl/Cmd+click by default (`docs/10` §3 item 6).
pub const DEFAULT_URL_SCHEMES: [&str; 4] = ["http", "https", "mailto", "file"];

/// Placeholder of [`EditorCommand`] replaced by the value of `$EDITOR`.
pub const EDITOR_PLACEHOLDER: &str = "{editor}";

/// Placeholder of [`EditorCommand`] replaced by the clicked file path (required).
pub const FILE_PLACEHOLDER: &str = "{file}";

/// Placeholder of [`EditorCommand`] replaced by the line number (`:line:col` links).
pub const LINE_PLACEHOLDER: &str = "{line}";

/// Placeholder of [`EditorCommand`] replaced by the column number.
pub const COLUMN_PLACEHOLDER: &str = "{col}";

bounded!(
    /// Scrollback length in lines (`FR-TERM-011`: default 10 000, min 0, max 1 000 000).
    ScrollbackLines(u32),
    min = 0,
    max = 1_000_000,
    default = 10_000
);
bounded!(
    /// Lines scrolled per mouse-wheel step. Default 3: the usual desktop wheel step. Bounds: at
    /// least one line; 20 is a third of a 60-row screen per step, beyond which wheel scrolling
    /// skips content the eye cannot follow.
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

/// Which macOS Option keys act as Alt (`FR-TERM-008`). Default none: Option keeps composing
/// characters (accents, `@` on many layouts), the macOS convention.
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

/// Title reporting (`docs/09` §8 lists it as "title reporting (off)"). Reporting the title back
/// to programs (`CSI 21 t`, OSC title queries) is not implemented, to prevent title injection
/// (`docs/10` §3 item 1), so `off` is the only value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleReporting {
    /// Title reports are never sent.
    #[default]
    Off,
}

/// The argv template that opens a clicked `path:line:col` (`FR-TERM-016`). It is run as an argv
/// array, never through a shell (`docs/10` §3 item 7). It must have a program and contain
/// [`FILE_PLACEHOLDER`]. Default `{editor} {file}`: `$EDITOR` with the file, which every editor
/// accepts; line and column flags differ per editor, so the user adds them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<String>", into = "Vec<String>")]
pub struct EditorCommand(Vec<String>);

impl EditorCommand {
    /// The template arguments; the first is the program.
    pub fn args(&self) -> &[String] {
        &self.0
    }
}

impl Default for EditorCommand {
    fn default() -> Self {
        Self(vec![EDITOR_PLACEHOLDER.to_owned(), FILE_PLACEHOLDER.to_owned()])
    }
}

impl TryFrom<Vec<String>> for EditorCommand {
    type Error = String;

    fn try_from(args: Vec<String>) -> Result<Self, Self::Error> {
        let has_program = args.first().is_some_and(|program| !program.trim().is_empty());
        let has_file = args.iter().any(|arg| arg.contains(FILE_PLACEHOLDER));
        let clean = args.iter().all(|arg| !arg.chars().any(char::is_control));
        if has_program && has_file && clean {
            Ok(Self(args))
        } else {
            Err(format!("editor command needs a program and a {FILE_PLACEHOLDER} argument"))
        }
    }
}

impl From<EditorCommand> for Vec<String> {
    fn from(command: EditorCommand) -> Self {
        command.0
    }
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
    /// Copy the selection as soon as it is made (`FR-UI-009`: an option). Default off: the
    /// clipboard only changes on an explicit copy.
    pub copy_on_select: bool,
    /// Ctrl+C copies when a selection exists, else sends SIGINT (`docs/09` §5: configurable).
    /// Default off: Ctrl+C always reaches the program, as in every terminal.
    pub smart_copy: bool,
    /// Confirm multi-line pastes into programs without bracketed paste (`docs/09` §5: on).
    pub paste_confirmation: bool,
    /// Honor bracketed paste mode 2004 (`FR-TERM-006`; default on, it is the safe mode).
    pub bracketed_paste: bool,
    /// Extra word characters for double-click selection.
    pub word_characters: String,
    /// URL schemes allowed to open on click.
    pub url_schemes: Vec<String>,
    /// Editor command template for detected file paths.
    pub editor_command: EditorCommand,
    /// Bell behavior.
    pub bell: Bell,
    /// Set the window urgency hint on a bell in an unfocused window (`FR-TERM-018`: optional).
    /// Default off: the user opts in.
    pub bell_urgency_hint: bool,
    /// macOS Option-as-Alt.
    pub option_as_alt: OptionAsAlt,
    /// Kitty keyboard protocol (`FR-TERM-008`). Default on: it is progressive, so programs that
    /// do not ask for it get the legacy encoding anyway.
    pub kitty_keyboard: bool,
    /// OSC 52 policy.
    pub osc52: Osc52Policy,
    /// Title reporting (always off).
    pub title_reporting: TitleReporting,
    /// Inline images (`FR-TERM-012`/`FR-TERM-013`); the cache cap is in Performance. Default on:
    /// the cache is allocated on first use, so it costs nothing until an image arrives
    /// (`docs/03` §3 rule 11).
    pub images: bool,
    /// Paste a clipboard image as a saved file path (`FR-UI-011`, `docs/09` §5: on, with a
    /// setting to disable it).
    pub paste_images: bool,
    /// Directory for pasted images; `None` uses `runtime_dir/clips` (`docs/09` §5).
    pub clips_dir: Option<PathBuf>,
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
            editor_command: EditorCommand::default(),
            bell: Bell::default(),
            bell_urgency_hint: false,
            option_as_alt: OptionAsAlt::default(),
            kitty_keyboard: true,
            osc52: Osc52Policy::default(),
            title_reporting: TitleReporting::Off,
            images: true,
            paste_images: true,
            clips_dir: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_reporting_is_off_and_cannot_be_turned_on() {
        assert_eq!(Terminal::default().title_reporting, TitleReporting::Off);
        assert!(toml::from_str::<Terminal>("title_reporting = \"off\"").is_ok());
        assert!(toml::from_str::<Terminal>("title_reporting = \"on\"").is_err());
    }

    #[test]
    fn editor_command_needs_a_program_and_the_file_placeholder() {
        let ok = vec!["code".to_owned(), "--goto".to_owned(), "{file}:{line}".to_owned()];
        assert!(EditorCommand::try_from(ok).is_ok());
        for bad in [vec![], vec!["vim".to_owned()], vec![" ".to_owned(), "{file}".to_owned()]] {
            assert!(EditorCommand::try_from(bad).is_err());
        }
        let control = vec!["vim".to_owned(), "{file}\n".to_owned()];
        assert!(EditorCommand::try_from(control).is_err());
    }
}
