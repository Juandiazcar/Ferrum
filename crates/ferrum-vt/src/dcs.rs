//! DCS strings (FR-TERM-001): DECRQSS with a safe subset, XTGETTCAP answered as unavailable, and
//! everything else discarded without storing a byte.
//!
//! `vte` hands a DCS string to the handlers one byte at a time and keeps nothing itself, so the
//! only buffer is the few bytes of a DECRQSS request (`docs/10` §3 item 9).

use vte::Params;

use crate::cell::Underline;
use crate::core::Core;
use crate::cursor::CursorShape;
use crate::events::EventSink;
use crate::limits::MAX_DCS_REQUEST_BYTES;
use crate::modes::Mode;
use crate::reply::Reply;

/// What the DCS string in progress is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Dcs {
    /// No DCS string, or one that is discarded.
    #[default]
    Ignore,
    /// DECRQSS: the request is collected here (at most `MAX_DCS_REQUEST_BYTES`).
    Decrqss { request: [u8; MAX_DCS_REQUEST_BYTES], len: usize, overflow: bool },
}

/// `DCS 0 + r ST`: XTGETTCAP answered with "not available" so that programs do not wait.
const NO_CAPABILITIES: &[u8] = b"\x1bP0+r\x1b\\";

impl<S: EventSink> Core<S> {
    /// Start of a DCS string.
    pub(crate) fn dcs_hook(&mut self, _params: &Params, intermediates: &[u8], action: char) {
        self.dcs = match (intermediates, action) {
            ([b'$'], 'q') => {
                Dcs::Decrqss { request: [0; MAX_DCS_REQUEST_BYTES], len: 0, overflow: false }
            }
            ([b'+'], 'q') => {
                self.send(&Reply::new().text(NO_CAPABILITIES));
                Dcs::Ignore
            }
            _ => Dcs::Ignore,
        };
    }

    /// One byte of a DCS string.
    pub(crate) fn dcs_put(&mut self, byte: u8) {
        if let Dcs::Decrqss { request, len, overflow } = &mut self.dcs {
            match request.get_mut(*len) {
                Some(slot) => {
                    *slot = byte;
                    *len += 1;
                }
                None => *overflow = true,
            }
        }
    }

    /// End of a DCS string: answers a DECRQSS request.
    pub(crate) fn dcs_unhook(&mut self) {
        let dcs = std::mem::take(&mut self.dcs);
        if let Dcs::Decrqss { request, len, overflow } = dcs {
            let request = request.get(..len).unwrap_or_default();
            let reply = if overflow { None } else { self.status_string(request) };
            let reply = reply.unwrap_or_else(|| Reply::new().text(b"\x1bP0$r\x1b\\"));
            self.send(&reply);
        }
    }

    /// The `DCS 1 $ r ... ST` answer for the settings DECRQSS may ask about: SGR (`m`), the scroll
    /// region (`r`), the cursor style (`SP q`) and the protection attribute (`" q`).
    fn status_string(&self, request: &[u8]) -> Option<Reply> {
        let head = Reply::new().text(b"\x1bP1$r");
        let screen = self.screen();
        let body = match request {
            b"m" => self.sgr_report(head),
            b"r" => head
                .number(screen.region.top + 1)
                .text(b";")
                .number(screen.region.bottom + 1)
                .text(b"r"),
            b" q" => head.number(self.cursor_style_code()).text(b" q"),
            b"\"q" => head
                .number(usize::from(screen.cursor.pen.has(crate::cell::Attrs::PROTECTED)))
                .text(b"\"q"),
            _ => return None,
        };
        Some(body.text(b"\x1b\\"))
    }

    /// `0;...m` describing the current pen.
    fn sgr_report(&self, head: Reply) -> Reply {
        use crate::cell::Attrs;
        let pen = self.screen().cursor.pen;
        let mut reply = head.text(b"0");
        for (attr, code) in [
            (Attrs::BOLD, 1),
            (Attrs::DIM, 2),
            (Attrs::ITALIC, 3),
            (Attrs::BLINK, 5),
            (Attrs::INVERSE, 7),
            (Attrs::HIDDEN, 8),
            (Attrs::STRIKE, 9),
            (Attrs::OVERLINE, 53),
        ] {
            if pen.has(attr) {
                reply = reply.text(b";").number(code);
            }
        }
        if pen.underline() != Underline::None {
            reply = reply.text(b";4:").number(pen.underline() as usize);
        }
        reply = push_color(reply, pen.fg, 30);
        push_color(reply, pen.bg, 40).text(b"m")
    }

    /// The DECSCUSR value that reproduces the current cursor style.
    fn cursor_style_code(&self) -> usize {
        let Some(style) = self.cursor_style else {
            return if self.modes.contains(Mode::CURSOR_BLINK) { 1 } else { 2 };
        };
        let base = match style.shape {
            CursorShape::Block => 1,
            CursorShape::Underline => 3,
            CursorShape::Bar => 5,
        };
        base + usize::from(!style.blinking)
    }
}

/// Appends `;38;5;n` style parameters for a non-default color; `base` is 30 (foreground) or 40.
fn push_color(reply: Reply, color: crate::color::Color, base: usize) -> Reply {
    use crate::color::ColorKind;
    match color.kind() {
        ColorKind::Default => reply,
        ColorKind::Indexed(index) => {
            reply.text(b";").number(base + 8).text(b";5;").number(usize::from(index))
        }
        ColorKind::Rgb(r, g, b) => reply
            .text(b";")
            .number(base + 8)
            .text(b";2;")
            .number(usize::from(r))
            .text(b";")
            .number(usize::from(g))
            .text(b";")
            .number(usize::from(b)),
    }
}
