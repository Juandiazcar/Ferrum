//! The event sink: how the terminal tells its owner about things that are not grid state.
//!
//! Events borrow from the terminal's buffers, so reporting one never allocates; a sink that wants
//! to keep the data copies it. Policy (what to do with a clipboard request, whether to ring a
//! bell) belongs to the owner, never to `ferrum-vt` (`docs/02` §3, WP-11).

use ferrum_scrollback::ScrollbackEvent;

/// Progress state of `OSC 9;4` (`ConEmu` and Windows Terminal progress).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProgressState {
    /// No progress shown.
    Remove,
    /// Normal progress with a value.
    Normal,
    /// Error state with a value.
    Error,
    /// Busy, no value.
    Indeterminate,
    /// Warning state with a value.
    Warning,
}

/// A shell integration mark (`OSC 133`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ShellMark {
    /// `A`: a prompt starts.
    PromptStart,
    /// `B`: the command line starts (the prompt ended).
    CommandStart,
    /// `C`: the command is executing (its output starts).
    CommandExecuted,
    /// `D`: the command finished, with its exit code when the shell reported one.
    CommandFinished {
        /// Exit status.
        exit_code: Option<i32>,
    },
}

/// Something the owner should know about.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event<'a> {
    /// Bytes the terminal answers to the program (DA, DSR, ...): write them to the PTY. They are
    /// fixed and never contain program-supplied text (`docs/10` §3 item 3).
    Reply(&'a [u8]),
    /// The title changed (OSC 0/2), sanitized and at most `MAX_TITLE_BYTES`.
    Title(&'a str),
    /// The icon name changed (OSC 1), sanitized.
    IconName(&'a str),
    /// BEL.
    Bell,
    /// The shell reported its working directory (OSC 7), as the `file://` URI it sent.
    Cwd(&'a str),
    /// A program asks to set the clipboard (OSC 52); `bytes` are already decoded and at most
    /// `MAX_CLIPBOARD_BYTES` of base64 long. Policy decides.
    ClipboardStore {
        /// The selection letters of the request (`c`, `p`, `s`, digits).
        selection: &'a str,
        /// The decoded bytes.
        bytes: &'a [u8],
    },
    /// A program asks to read the clipboard (OSC 52 `?`). Denied unless the owner approves and
    /// answers with `Terminal::answer_clipboard_request`.
    ClipboardLoad {
        /// The selection letters of the request.
        selection: &'a str,
    },
    /// Progress (OSC 9;4); `value` is 0 to 100.
    Progress {
        /// The state.
        state: ProgressState,
        /// Percentage, 0 for states without one.
        value: u8,
    },
    /// A shell mark (OSC 133).
    ShellMark(ShellMark),
    /// The color palette or the default colors changed (OSC 4/10/11/12 and resets).
    PaletteChanged,
    /// The scrollback reports something (for example that spilling was disabled).
    Scrollback(ScrollbackEvent),
}

/// Receives the events of one terminal.
pub trait EventSink {
    /// Handles one event.
    fn event(&mut self, event: Event<'_>);
}

/// A sink that drops everything (benches, fuzzing).
#[derive(Debug, Clone, Copy, Default)]
pub struct NullSink;

impl EventSink for NullSink {
    fn event(&mut self, _event: Event<'_>) {}
}
