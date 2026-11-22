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

/// First SGR code of the normal foreground colors.
const FG_BASE: u16 = 30;
/// First SGR code of the normal background colors.
const BG_BASE: u16 = 40;
/// First SGR code of the bright foreground colors.
const BRIGHT_FG_BASE: u16 = 90;
/// First SGR code of the bright background colors.
const BRIGHT_BG_BASE: u16 = 100;
/// Palette index of the first bright color.
const BRIGHT_OFFSET: u16 = 8;

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
        5 => Some(Color::indexed(u8::try_from(next()?).ok()?)),
        2 => {
            let (r, g, b) = (next()?, next()?, next()?);
            Some(Color::rgb(u8::try_from(r).ok()?, u8::try_from(g).ok()?, u8::try_from(b).ok()?))
        }
        _ => None,
    }
}

fn colon_color(sub: &[u16]) -> Option<Color> {
    match sub.first()? {
        5 => Some(Color::indexed(byte(sub.get(1))?)),
        2 => {
            // `2:r:g:b` or `2:colorspace:r:g:b`.
            let channels = if sub.len() >= 5 { sub.get(2..5)? } else { sub.get(1..4)? };
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
        None | Some(1) => Some(Underline::Single),
        Some(0) => Some(Underline::None),
        Some(2) => Some(Underline::Double),
        Some(3) => Some(Underline::Curly),
        Some(4) => Some(Underline::Dotted),
        Some(5) => Some(Underline::Dashed),
        Some(_) => None,
    }
}

fn reset(pen: &mut Pen) {
    pen.fg = Color::DEFAULT;
    pen.bg = Color::DEFAULT;
    pen.underline_color = Color::DEFAULT;
    // DECSCA protection is not an SGR attribute and survives SGR 0.
    pen.attrs &= Attrs::PROTECTED.bits();
}

fn apply_attribute(pen: &mut Pen, code: u16) -> bool {
    let flag = match code {
        1 => Some((Attrs::BOLD, true)),
        2 => Some((Attrs::DIM, true)),
        3 => Some((Attrs::ITALIC, true)),
        5 | 6 => Some((Attrs::BLINK, true)),
        7 => Some((Attrs::INVERSE, true)),
        8 => Some((Attrs::HIDDEN, true)),
        9 => Some((Attrs::STRIKE, true)),
        53 => Some((Attrs::OVERLINE, true)),
        23 => Some((Attrs::ITALIC, false)),
        25 => Some((Attrs::BLINK, false)),
        27 => Some((Attrs::INVERSE, false)),
        28 => Some((Attrs::HIDDEN, false)),
        29 => Some((Attrs::STRIKE, false)),
        55 => Some((Attrs::OVERLINE, false)),
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
        30..=37 => pen.fg = index(FG_BASE, 0),
        40..=47 => pen.bg = index(BG_BASE, 0),
        90..=97 => pen.fg = index(BRIGHT_FG_BASE, BRIGHT_OFFSET),
        100..=107 => pen.bg = index(BRIGHT_BG_BASE, BRIGHT_OFFSET),
        39 => pen.fg = Color::DEFAULT,
        49 => pen.bg = Color::DEFAULT,
        59 => pen.underline_color = Color::DEFAULT,
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
        0 => reset(pen),
        4 => {
            if let Some(style) = underline_style(param) {
                pen.set_underline(style);
            }
        }
        21 => pen.set_underline(Underline::Double),
        22 => {
            pen.remove_attr(Attrs::BOLD);
            pen.remove_attr(Attrs::DIM);
        }
        24 => pen.set_underline(Underline::None),
        38 => pen.fg = extended_color(param, rest).unwrap_or(pen.fg),
        48 => pen.bg = extended_color(param, rest).unwrap_or(pen.bg),
        58 => pen.underline_color = extended_color(param, rest).unwrap_or(pen.underline_color),
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
