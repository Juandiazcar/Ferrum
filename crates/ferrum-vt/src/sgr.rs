//! SGR: colors, attributes, underline styles and underline colors (FR-TERM-002).
//!
//! Supported forms: `38;5;n` and `38;2;r;g;b` (semicolons), `38:5:n`, `38:2:r:g:b` and
//! `38:2::r:g:b` (colons, with or without the color space id), the same for 48 and 58, and
//! `4:n` underline styles. Anything malformed is ignored without disturbing the rest.

use vte::ParamsIter;

use crate::args::Args;
use crate::cell::{Attrs, Underline};
use crate::color::Color;
use crate::core::Core;
use crate::cursor::Pen;
use crate::events::EventSink;

/// The SGR parameter numbers of ECMA-48 and xterm that Ferrum handles.
pub(crate) mod code {
    pub(crate) const RESET: u16 = 0;
    pub(crate) const BOLD: u16 = 1;
    pub(crate) const DIM: u16 = 2;
    pub(crate) const ITALIC: u16 = 3;
    pub(crate) const UNDERLINE: u16 = 4;
    pub(crate) const BLINK: u16 = 5;
    pub(crate) const RAPID_BLINK: u16 = 6;
    pub(crate) const INVERSE: u16 = 7;
    pub(crate) const HIDDEN: u16 = 8;
    pub(crate) const STRIKE: u16 = 9;
    /// Cancels bold: the Alacritty reference test `underline` expects it (xterm: double underline).
    pub(crate) const BOLD_OFF_ALACRITTY: u16 = 21;
    pub(crate) const NORMAL_INTENSITY: u16 = 22;
    pub(crate) const ITALIC_OFF: u16 = 23;
    pub(crate) const UNDERLINE_OFF: u16 = 24;
    pub(crate) const BLINK_OFF: u16 = 25;
    pub(crate) const INVERSE_OFF: u16 = 27;
    pub(crate) const HIDDEN_OFF: u16 = 28;
    pub(crate) const STRIKE_OFF: u16 = 29;
    /// First code of the normal foreground colors (30 to 37).
    pub(crate) const FG_BASE: u16 = 30;
    pub(crate) const FG_EXTENDED: u16 = 38;
    pub(crate) const FG_DEFAULT: u16 = 39;
    /// First code of the normal background colors (40 to 47).
    pub(crate) const BG_BASE: u16 = 40;
    pub(crate) const BG_EXTENDED: u16 = 48;
    pub(crate) const BG_DEFAULT: u16 = 49;
    pub(crate) const OVERLINE: u16 = 53;
    pub(crate) const OVERLINE_OFF: u16 = 55;
    pub(crate) const UNDERLINE_COLOR_EXTENDED: u16 = 58;
    pub(crate) const UNDERLINE_COLOR_DEFAULT: u16 = 59;
    /// First code of the bright foreground colors (90 to 97).
    pub(crate) const BRIGHT_FG_BASE: u16 = 90;
    /// First code of the bright background colors (100 to 107).
    pub(crate) const BRIGHT_BG_BASE: u16 = 100;
}

use code::{BG_BASE, BRIGHT_BG_BASE, BRIGHT_FG_BASE, FG_BASE};

/// Colors in each bank of eight (normal and bright).
const BANK_COLORS: u16 = 8;
const FG_LAST: u16 = FG_BASE + BANK_COLORS - 1;
const BG_LAST: u16 = BG_BASE + BANK_COLORS - 1;
const BRIGHT_FG_LAST: u16 = BRIGHT_FG_BASE + BANK_COLORS - 1;
const BRIGHT_BG_LAST: u16 = BRIGHT_BG_BASE + BANK_COLORS - 1;
/// Palette index of the first bright color.
const BRIGHT_OFFSET: u16 = 8;
/// What follows an extended color code: a palette index (`5`) or three channels (`2`).
const COLOR_INDEXED: u16 = 5;
const COLOR_RGB: u16 = 2;
/// Channels of an RGB color, and the length of the colon form that carries a color space id.
const RGB_CHANNELS: usize = 3;
const COLON_RGB_WITH_COLORSPACE: usize = 5;
/// The `4:n` underline styles.
mod underline {
    pub(super) const NONE: u16 = 0;
    pub(super) const SINGLE: u16 = 1;
    pub(super) const DOUBLE: u16 = 2;
    pub(super) const CURLY: u16 = 3;
    pub(super) const DOTTED: u16 = 4;
    pub(super) const DASHED: u16 = 5;
}

/// Reads one palette index or channel value; anything above 255 is invalid.
fn byte(value: Option<&u16>) -> Option<u8> {
    u8::try_from(*value?).ok()
}

/// The color after `38`, `48` or `58`, in either separator style.
fn extended_color(param: &[u16], rest: &mut ParamsIter<'_>) -> Option<Color> {
    if param.len() > 1 {
        return colon_color(param.get(1..)?);
    }
    let kind = rest.next()?.first().copied()?;
    let mut next = || rest.next().and_then(|p| p.first().copied());
    match kind {
        COLOR_INDEXED => Some(Color::indexed(u8::try_from(next()?).ok()?)),
        COLOR_RGB => {
            let (r, g, b) = (next()?, next()?, next()?);
            Some(Color::rgb(u8::try_from(r).ok()?, u8::try_from(g).ok()?, u8::try_from(b).ok()?))
        }
        _ => None,
    }
}

fn colon_color(sub: &[u16]) -> Option<Color> {
    match *sub.first()? {
        COLOR_INDEXED => Some(Color::indexed(byte(sub.get(1))?)),
        COLOR_RGB => {
            // `2:r:g:b` or `2:colorspace:r:g:b`.
            let channels = if sub.len() >= COLON_RGB_WITH_COLORSPACE {
                sub.get(2..2 + RGB_CHANNELS)?
            } else {
                sub.get(1..1 + RGB_CHANNELS)?
            };
            Some(Color::rgb(
                byte(channels.first())?,
                byte(channels.get(1))?,
                byte(channels.get(2))?,
            ))
        }
        _ => None,
    }
}

fn underline_style(param: &[u16]) -> Option<Underline> {
    match param.get(1) {
        None | Some(&underline::SINGLE) => Some(Underline::Single),
        Some(&underline::NONE) => Some(Underline::None),
        Some(&underline::DOUBLE) => Some(Underline::Double),
        Some(&underline::CURLY) => Some(Underline::Curly),
        Some(&underline::DOTTED) => Some(Underline::Dotted),
        Some(&underline::DASHED) => Some(Underline::Dashed),
        Some(_) => None,
    }
}

fn reset(pen: &mut Pen) {
    pen.fg = Color::DEFAULT;
    pen.bg = Color::DEFAULT;
    pen.underline_color = Color::DEFAULT;
    pen.attrs = 0;
}

fn apply_attribute(pen: &mut Pen, code: u16) -> bool {
    let flag = match code {
        code::BOLD => Some((Attrs::BOLD, true)),
        code::DIM => Some((Attrs::DIM, true)),
        code::ITALIC => Some((Attrs::ITALIC, true)),
        code::BLINK | code::RAPID_BLINK => Some((Attrs::BLINK, true)),
        code::INVERSE => Some((Attrs::INVERSE, true)),
        code::HIDDEN => Some((Attrs::HIDDEN, true)),
        code::STRIKE => Some((Attrs::STRIKE, true)),
        code::OVERLINE => Some((Attrs::OVERLINE, true)),
        code::ITALIC_OFF => Some((Attrs::ITALIC, false)),
        code::BLINK_OFF => Some((Attrs::BLINK, false)),
        code::INVERSE_OFF => Some((Attrs::INVERSE, false)),
        code::HIDDEN_OFF => Some((Attrs::HIDDEN, false)),
        code::STRIKE_OFF => Some((Attrs::STRIKE, false)),
        code::OVERLINE_OFF => Some((Attrs::OVERLINE, false)),
        _ => None,
    };
    let Some((attr, on)) = flag else { return false };
    if on {
        pen.insert_attr(attr);
    } else {
        pen.remove_attr(attr);
    }
    true
}

fn apply_color_code(pen: &mut Pen, code: u16) -> bool {
    let index =
        |base: u16, offset: u16| Color::indexed(u8::try_from(code - base + offset).unwrap_or(0));
    match code {
        FG_BASE..=FG_LAST => pen.fg = index(FG_BASE, 0),
        BG_BASE..=BG_LAST => pen.bg = index(BG_BASE, 0),
        BRIGHT_FG_BASE..=BRIGHT_FG_LAST => pen.fg = index(BRIGHT_FG_BASE, BRIGHT_OFFSET),
        BRIGHT_BG_BASE..=BRIGHT_BG_LAST => pen.bg = index(BRIGHT_BG_BASE, BRIGHT_OFFSET),
        code::FG_DEFAULT => pen.fg = Color::DEFAULT,
        code::BG_DEFAULT => pen.bg = Color::DEFAULT,
        code::UNDERLINE_COLOR_DEFAULT => pen.underline_color = Color::DEFAULT,
        _ => return false,
    }
    true
}

/// Applies one SGR parameter (with its sub-parameters) to `pen`.
fn apply(pen: &mut Pen, param: &[u16], rest: &mut ParamsIter<'_>) {
    let code = param.first().copied().unwrap_or(0);
    if apply_attribute(pen, code) || apply_color_code(pen, code) {
        return;
    }
    match code {
        code::RESET => reset(pen),
        code::UNDERLINE => {
            if let Some(style) = underline_style(param) {
                pen.set_underline(style);
            }
        }
        // SGR 21 cancels bold, as the Alacritty reference test `underline` expects (maintainer
        // decision Q-23); xterm reads it as a double underline (`4:2` is the portable form).
        code::BOLD_OFF_ALACRITTY => pen.remove_attr(Attrs::BOLD),
        code::NORMAL_INTENSITY => {
            pen.remove_attr(Attrs::BOLD);
            pen.remove_attr(Attrs::DIM);
        }
        code::UNDERLINE_OFF => pen.set_underline(Underline::None),
        code::FG_EXTENDED => pen.fg = extended_color(param, rest).unwrap_or(pen.fg),
        code::BG_EXTENDED => pen.bg = extended_color(param, rest).unwrap_or(pen.bg),
        code::UNDERLINE_COLOR_EXTENDED => {
            pen.underline_color = extended_color(param, rest).unwrap_or(pen.underline_color);
        }
        _ => {}
    }
}

impl<S: EventSink> Core<S> {
    /// `CSI Pm m`.
    pub(crate) fn sgr(&mut self, args: Args<'_>) {
        let mut pen = self.screen().cursor.pen;
        let mut params = args.iter();
        let mut seen = false;
        while let Some(param) = params.next() {
            seen = true;
            apply(&mut pen, param, &mut params);
        }
        if !seen {
            reset(&mut pen);
        }
        self.set_pen(pen);
    }

    /// Stores `pen`, interning its hyperlink and underline color.
    pub(crate) fn set_pen(&mut self, mut pen: Pen) {
        pen.extra = self.extras.intern(pen.link, pen.underline_color);
        self.screen_mut().cursor.pen = pen;
    }
}
