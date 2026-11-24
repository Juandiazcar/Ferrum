//! OSC 133 shell integration marks and OSC 9;4 progress (FR-TERM-009).

use ferrum_scrollback::LineFlags;

use crate::core::Core;
use crate::events::{Event, EventSink, ProgressState, ShellMark};

/// Largest progress percentage.
const MAX_PROGRESS: u8 = 100;
/// `OSC 9 ; 4`: the progress form of `OSC 9`.
const PROGRESS_SUBCOMMAND: &[u8] = b"4";

/// The state numbers of `OSC 9 ; 4 ; st ; pr`.
mod progress {
    pub(super) const REMOVE: u8 = 0;
    pub(super) const NORMAL: u8 = 1;
    pub(super) const ERROR: u8 = 2;
    pub(super) const INDETERMINATE: u8 = 3;
    pub(super) const WARNING: u8 = 4;
}

fn parse_number<T: std::str::FromStr>(text: &[u8]) -> Option<T> {
    std::str::from_utf8(text).ok()?.parse().ok()
}

impl<S: EventSink> Core<S> {
    /// `OSC 133 ; A|B|C|D [; options]`. The first letter picks the mark; options (such as the
    /// exit code of `D`) follow after a `;`. The mark is also stored on the cursor row for prompt
    /// navigation (`docs/02` §3.1).
    pub(crate) fn osc_shell_mark(&mut self, rest: &[u8]) {
        let mut parts = rest.split(|&b| b == b';');
        let kind = parts.next().unwrap_or_default();
        let (flag, mark) = match kind {
            b"A" => (LineFlags::PROMPT_MARK, ShellMark::PromptStart),
            b"B" => (LineFlags::COMMAND_START, ShellMark::CommandStart),
            b"C" => (LineFlags::COMMAND_START, ShellMark::CommandExecuted),
            b"D" => {
                let exit_code = parts.next().and_then(parse_number::<i32>);
                (LineFlags::COMMAND_END, ShellMark::CommandFinished { exit_code })
            }
            _ => return,
        };
        let y = self.screen().cursor.y;
        if let Some(row) = self.screen_mut().grid.row_mut(y) {
            row.insert_flag(flag);
        }
        self.sink.event(Event::ShellMark(mark));
    }

    /// `OSC 9 ; 4 ; state ; value`. Other `OSC 9` forms (desktop notifications) are ignored.
    pub(crate) fn osc_progress(&mut self, rest: &[u8]) {
        let mut parts = rest.split(|&b| b == b';');
        if parts.next() != Some(PROGRESS_SUBCOMMAND) {
            return;
        }
        let state = match parts.next().and_then(parse_number::<u8>) {
            Some(progress::REMOVE) => ProgressState::Remove,
            Some(progress::NORMAL) => ProgressState::Normal,
            Some(progress::ERROR) => ProgressState::Error,
            Some(progress::INDETERMINATE) => ProgressState::Indeterminate,
            Some(progress::WARNING) => ProgressState::Warning,
            _ => return,
        };
        let value = parts.next().and_then(parse_number::<u32>).unwrap_or(0);
        let value = u8::try_from(value.min(u32::from(MAX_PROGRESS))).unwrap_or(MAX_PROGRESS);
        let value = match state {
            ProgressState::Remove | ProgressState::Indeterminate => 0,
            _ => value,
        };
        self.sink.event(Event::Progress { state, value });
    }
}
