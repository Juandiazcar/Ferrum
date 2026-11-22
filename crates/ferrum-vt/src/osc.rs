//! OSC dispatch (FR-TERM-009). The payload arrives whole from the front end (`input.rs`), at most
//! `MAX_OSC_BYTES` long, without its terminator.

use crate::core::Core;
use crate::events::{Event, EventSink};
use crate::limits::{MAX_CWD_BYTES, MAX_TITLE_BYTES};

/// Splits `payload` at its first `;` into the numeric command and the rest.
pub(crate) fn split_command(payload: &[u8]) -> Option<(u16, &[u8])> {
    let end = payload.iter().position(|&b| b == b';').unwrap_or(payload.len());
    let digits = payload.get(..end)?;
    if digits.is_empty() || digits.len() > 5 || !digits.iter().all(u8::is_ascii_digit) {
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
            0 | 2 => self.set_title(rest, TitleKind::Window),
            1 => self.set_title(rest, TitleKind::Icon),
            4 => self.osc_palette(rest),
            7 => self.set_cwd(rest),
            8 => self.osc_hyperlink(rest),
            10..=12 => self.osc_dynamic_colors(command, rest),
            52 => self.osc_clipboard(rest),
            104 => self.reset_palette(rest),
            110..=112 => self.reset_dynamic_color(command),
            133 => self.osc_shell_mark(rest),
            9 => self.osc_progress(rest),
            _ => {}
        }
    }

    /// OSC 0, 1, 2: stores the sanitized text in `scratch.short` and reports it. The title is
    /// only ever *set*: there is no way to read it back (`docs/10` §3 item 1).
    fn set_title(&mut self, raw: &[u8], kind: TitleKind) {
        let text = &mut self.scratch.short;
        text.clear();
        for c in String::from_utf8_lossy(raw).chars() {
            if c.is_control() {
                continue;
            }
            if text.len() + c.len_utf8() > MAX_TITLE_BYTES {
                break;
            }
            text.push(c);
        }
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
    fn a_missing_or_non_numeric_command_is_rejected() {
        assert_eq!(split_command(b""), None);
        assert_eq!(split_command(b";x"), None);
        assert_eq!(split_command(b"a;x"), None);
        assert_eq!(split_command(b"123456;x"), None);
    }
}
