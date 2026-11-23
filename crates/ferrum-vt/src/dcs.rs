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
use crate::reply::{Reply, ST};
use crate::sgr::code;
use crate::style_modes::decscusr;

/// What the DCS string in progress is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Dcs {
    /// No DCS string, or one that is discarded.
    #[default]
    Ignore,
    /// DECRQSS: the request is collected here (at most `MAX_DCS_REQUEST_BYTES`).
    Decrqss { request: [u8; MAX_DCS_REQUEST_BYTES], len: usize, overflow: bool },
    /// XTGETTCAP: answered "not available" when the string ends.
    Xtgettcap,
}

/// SGR codes that switch on each attribute, in the order DECRQSS reports them.
const SGR_ATTRIBUTES: [(crate::cell::Attrs, u16); 8] = [
    (crate::cell::Attrs::BOLD, code::BOLD),
    (crate::cell::Attrs::DIM, code::DIM),
    (crate::cell::Attrs::ITALIC, code::ITALIC),
    (crate::cell::Attrs::BLINK, code::BLINK),
    (crate::cell::Attrs::INVERSE, code::INVERSE),
    (crate::cell::Attrs::HIDDEN, code::HIDDEN),
    (crate::cell::Attrs::STRIKE, code::STRIKE),
    (crate::cell::Attrs::OVERLINE, code::OVERLINE),
];
/// SGR code of the first normal foreground color and of the first normal background color.
const SGR_FG_BASE: usize = code::FG_BASE as usize;
const SGR_BG_BASE: usize = code::BG_BASE as usize;
/// Distance from a base code to its extended-color code (38 and 48).
const SGR_EXTENDED_OFFSET: usize = 8;

/// `DCS 1 $ r`: the start of a DECRQSS answer for a setting the terminal reports.
const DECRQSS_VALID: &[u8] = b"\x1bP1$r";
/// `DCS 0 $ r ST`: the DECRQSS answer for a request the terminal does not support.
const DECRQSS_INVALID: &[u8] = b"\x1bP0$r\x1b\\";
/// `;4:`, `;5;` and `;2;`: the SGR underline style and the indexed and RGB color forms.
const SGR_UNDERLINE_STYLE: &[u8] = b";4:";
const SGR_INDEXED: &[u8] = b";5;";
const SGR_RGB: &[u8] = b";2;";
/// `DCS 0 + r ST`: XTGETTCAP answered with "not available" so that programs do not wait.
const NO_CAPABILITIES: &[u8] = b"\x1bP0+r\x1b\\";

impl<S: EventSink> Core<S> {
    /// Start of a DCS string.
    pub(crate) fn dcs_hook(&mut self, _params: &Params, intermediates: &[u8], action: char) {
        self.dcs = match (intermediates, action) {
            ([b'$'], 'q') => {
                Dcs::Decrqss { request: [0; MAX_DCS_REQUEST_BYTES], len: 0, overflow: false }
            }
            ([b'+'], 'q') => Dcs::Xtgettcap,
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

    /// `vte` left the DCS string. The front end decides whether it ended with a terminator
    /// ([`dcs_complete`](Self::dcs_complete)) or was cancelled ([`dcs_cancel`](Self::dcs_cancel));
    /// `vte` calls this for both.
    pub(crate) fn dcs_unhook(&mut self) {
        self.dcs_end = true;
    }

    /// The DCS string ended with a terminator: answers it. A string cancelled by CAN or SUB, or
    /// abandoned by the watchdog, is never answered (`docs/10` §3 items 3 and 9).
    pub(crate) fn dcs_complete(&mut self) {
        match std::mem::take(&mut self.dcs) {
            Dcs::Decrqss { request, len, overflow } => {
                let request = request.get(..len).unwrap_or_default();
                let reply = if overflow { None } else { self.status_string(request) };
                let reply = reply.unwrap_or_else(|| Reply::new().text(DECRQSS_INVALID));
                self.send(&reply);
            }
            Dcs::Xtgettcap => self.send(&Reply::new().text(NO_CAPABILITIES)),
            Dcs::Ignore => {}
        }
    }

    /// The DCS string was cancelled: forget it unanswered.
    pub(crate) fn dcs_cancel(&mut self) {
        self.dcs = Dcs::Ignore;
    }

    /// The `DCS 1 $ r ... ST` answer for the settings DECRQSS may ask about: SGR (`m`), the scroll
    /// region (`r`) and the cursor style (`SP q`).
    fn status_string(&self, request: &[u8]) -> Option<Reply> {
        let head = Reply::new().text(DECRQSS_VALID);
        let screen = self.screen();
        let body = match request {
            b"m" => self.sgr_report(head),
            b"r" => head
                .number(screen.region.top() + 1)
                .text(b";")
                .number(screen.region.bottom() + 1)
                .text(b"r"),
            b" q" => head.number(self.cursor_style_code()).text(b" q"),
            _ => return None,
        };
        Some(body.text(ST))
    }

    /// `0;...m` describing the current pen.
    fn sgr_report(&self, head: Reply) -> Reply {
        let pen = self.screen().cursor.pen;
        let mut reply = head.text(b"0");
        for (attr, sgr_code) in SGR_ATTRIBUTES {
            if pen.has(attr) {
                reply = reply.text(b";").number(usize::from(sgr_code));
            }
        }
        if pen.underline() != Underline::None {
            reply = reply.text(SGR_UNDERLINE_STYLE).number(pen.underline() as usize);
        }
        reply = push_color(reply, pen.fg, SGR_FG_BASE);
        push_color(reply, pen.bg, SGR_BG_BASE).text(b"m")
    }

    /// The DECSCUSR value that reproduces the current cursor style.
    fn cursor_style_code(&self) -> usize {
        let Some(style) = self.cursor_style else {
            let code = if self.modes.contains(Mode::CURSOR_BLINK) {
                decscusr::BLINKING_BLOCK
            } else {
                decscusr::STEADY_BLOCK
            };
            return usize::from(code);
        };
        let blinking = match style.shape {
            CursorShape::Block => decscusr::BLINKING_BLOCK,
            CursorShape::Underline => decscusr::BLINKING_UNDERLINE,
            CursorShape::Bar => decscusr::BLINKING_BAR,
        };
        // The steady variant of every shape is the next code.
        usize::from(blinking) + usize::from(!style.blinking)
    }
}

/// Appends `;38;5;n` style parameters for a non-default color; `base` is 30 (foreground) or 40.
fn push_color(reply: Reply, color: crate::color::Color, base: usize) -> Reply {
    use crate::color::ColorKind;
    match color.kind() {
        ColorKind::Default => reply,
        ColorKind::Indexed(index) => reply
            .text(b";")
            .number(base + SGR_EXTENDED_OFFSET)
            .text(SGR_INDEXED)
            .number(usize::from(index)),
        ColorKind::Rgb(r, g, b) => reply
            .text(b";")
            .number(base + SGR_EXTENDED_OFFSET)
            .text(SGR_RGB)
            .number(usize::from(r))
            .text(b";")
            .number(usize::from(g))
            .text(b";")
            .number(usize::from(b)),
    }
}
