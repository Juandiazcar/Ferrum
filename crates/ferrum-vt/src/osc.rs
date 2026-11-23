//! OSC dispatch (FR-TERM-009). The payload arrives whole from the front end (`input.rs`), at most
//! `MAX_OSC_BYTES` long, without its terminator.

use crate::core::Core;
use crate::events::{Event, EventSink};
use crate::limits::{MAX_CWD_BYTES, MAX_TITLE_BYTES};

/// Most digits of the numeric command of an OSC (`OSC 133 ; ...` has three).
const MAX_COMMAND_DIGITS: usize = 5;

/// The OSC commands Ferrum handles.
pub(crate) mod command {
    pub(crate) const TITLE_AND_ICON: u16 = 0;
    pub(crate) const ICON: u16 = 1;
    pub(crate) const TITLE: u16 = 2;
    pub(crate) const PALETTE: u16 = 4;
    pub(crate) const CWD: u16 = 7;
    pub(crate) const HYPERLINK: u16 = 8;
    pub(crate) const PROGRESS: u16 = 9;
    pub(crate) const FOREGROUND: u16 = 10;
    pub(crate) const BACKGROUND: u16 = 11;
    pub(crate) const CURSOR_COLOR: u16 = 12;
    pub(crate) const CLIPBOARD: u16 = 52;
    pub(crate) const RESET_PALETTE: u16 = 104;
    pub(crate) const RESET_FOREGROUND: u16 = 110;
    pub(crate) const RESET_BACKGROUND: u16 = 111;
    pub(crate) const RESET_CURSOR_COLOR: u16 = 112;
    pub(crate) const SHELL_MARKS: u16 = 133;
}

/// Splits `payload` at its first `;` into the numeric command and the rest.
pub(crate) fn split_command(payload: &[u8]) -> Option<(u16, &[u8])> {
    let end = payload.iter().position(|&b| b == b';').unwrap_or(payload.len());
    let digits = payload.get(..end)?;
    if digits.is_empty()
        || digits.len() > MAX_COMMAND_DIGITS
        || !digits.iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    let number = std::str::from_utf8(digits).ok()?.parse().ok()?;
    Some((number, payload.get(end + 1..).unwrap_or_default()))
}

impl<S: EventSink> Core<S> {
    /// Handles one OSC string.
    pub(crate) fn osc(&mut self, payload: &[u8]) {
        let Some((command, rest)) = split_command(payload) else { return };
        match command {
            command::TITLE_AND_ICON | command::TITLE => self.set_title(rest, TitleKind::Window),
            command::ICON => self.set_title(rest, TitleKind::Icon),
            command::PALETTE => self.osc_palette(rest),
            command::CWD => self.set_cwd(rest),
            command::HYPERLINK => self.osc_hyperlink(rest),
            command::FOREGROUND..=command::CURSOR_COLOR => self.osc_dynamic_colors(command, rest),
            command::CLIPBOARD => self.osc_clipboard(rest),
            command::RESET_PALETTE => self.reset_palette(rest),
            command::RESET_FOREGROUND..=command::RESET_CURSOR_COLOR => {
                self.reset_dynamic_color(command);
            }
            command::SHELL_MARKS => self.osc_shell_mark(rest),
            command::PROGRESS => self.osc_progress(rest),
            _ => {}
        }
    }

    /// OSC 0, 1, 2: stores the sanitized text in `scratch.short` and reports it. The title is
    /// only ever *set*: there is no way to read it back (`docs/10` §3 item 1).
    fn set_title(&mut self, raw: &[u8], kind: TitleKind) {
        sanitize_title(raw, &mut self.scratch.short);
        match kind {
            TitleKind::Window => {
                self.title.clear();
                self.title.push_str(&self.scratch.short);
                self.sink.event(Event::Title(&self.scratch.short));
            }
            TitleKind::Icon => self.sink.event(Event::IconName(&self.scratch.short)),
        }
    }

    /// OSC 7: the shell's working directory as a `file://` URI. Anything else, a URI with control
    /// characters or an over-long one is ignored.
    fn set_cwd(&mut self, raw: &[u8]) {
        let Ok(uri) = std::str::from_utf8(raw) else { return };
        if !uri.starts_with("file://")
            || uri.len() > MAX_CWD_BYTES
            || uri.chars().any(char::is_control)
        {
            return;
        }
        self.cwd.clear();
        self.cwd.push_str(uri);
        self.sink.event(Event::Cwd(uri));
    }
}

/// Writes `raw` into `text` without control characters and cut at `MAX_TITLE_BYTES`; bytes that
/// are not UTF-8 become U+FFFD, as `String::from_utf8_lossy` does, but without allocating.
fn sanitize_title(raw: &[u8], text: &mut String) {
    text.clear();
    for chunk in raw.utf8_chunks() {
        let broken = (!chunk.invalid().is_empty()).then_some(char::REPLACEMENT_CHARACTER);
        for c in chunk.valid().chars().chain(broken).filter(|c| !c.is_control()) {
            if text.len() + c.len_utf8() > MAX_TITLE_BYTES {
                return;
            }
            text.push(c);
        }
    }
}

#[derive(Clone, Copy)]
enum TitleKind {
    Window,
    Icon,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_split_at_the_first_semicolon() {
        assert_eq!(split_command(b"0;title"), Some((0, &b"title"[..])));
        assert_eq!(split_command(b"8;;https://x;y"), Some((8, &b";https://x;y"[..])));
        assert_eq!(split_command(b"104"), Some((104, &b""[..])));
    }

    #[test]
    fn titles_drop_controls_replace_invalid_utf8_and_stop_at_the_limit() {
        let mut text = String::new();
        sanitize_title(b"a\x07b\xffc\xe2\x82", &mut text);
        assert_eq!(text, "ab\u{FFFD}c\u{FFFD}");
        sanitize_title(&vec![b'x'; MAX_TITLE_BYTES * 2], &mut text);
        assert_eq!(text.len(), MAX_TITLE_BYTES);
        assert_eq!(String::from_utf8_lossy(b"a\xffc\xe2\x82"), "a\u{FFFD}c\u{FFFD}");
    }

    #[test]
    fn a_missing_or_non_numeric_command_is_rejected() {
        assert_eq!(split_command(b""), None);
        assert_eq!(split_command(b";x"), None);
        assert_eq!(split_command(b"a;x"), None);
        assert_eq!(split_command(b"123456;x"), None);
    }
}
