//! The style of a scrollback run, decoded.
//!
//! The scrollback stores `(fg, bg, attrs)` words (`ferrum_scrollback::Style`); the packing is
//! this crate's business (`color.rs`), so the decoding is too. A consumer that shows history
//! (the daemon building a snapshot, a test) uses [`CellStyle::from_style`].

use ferrum_scrollback::Style;

use crate::cell::{Attrs, Cell, Underline};
use crate::color::Color;
use crate::extras::ExtraId;

/// A decoded [`Style`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellStyle {
    /// Foreground.
    pub fg: Color,
    /// Background.
    pub bg: Color,
    /// Attribute flags.
    pub attrs: Attrs,
    /// Underline style.
    pub underline: Underline,
    /// Hyperlink and underline color; resolve with [`Extras`](crate::Extras).
    pub extra: ExtraId,
}

impl CellStyle {
    /// Decodes the words of a scrollback style.
    pub fn from_style(style: &Style) -> Self {
        let mut cell = Cell::default();
        cell.set_style(style.fg, style.bg, style.attrs);
        Self {
            fg: cell.fg(),
            bg: cell.bg(),
            attrs: cell.attrs(),
            underline: cell.underline(),
            extra: cell.extra(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor::Pen;

    #[test]
    fn a_style_round_trips_through_a_cell() {
        let mut pen = Pen { fg: Color::indexed(3), bg: Color::rgb(1, 2, 3), ..Pen::default() };
        pen.insert_attr(Attrs::BOLD);
        pen.set_underline(Underline::Curly);
        let cell = pen.cell('x');
        let style = Style { fg: cell.fg_word(), bg: cell.bg_word(), attrs: cell.attr_word() };
        let decoded = CellStyle::from_style(&style);
        assert_eq!(
            (decoded.fg, decoded.bg, decoded.attrs, decoded.underline),
            (Color::indexed(3), Color::rgb(1, 2, 3), Attrs::BOLD, Underline::Curly)
        );
    }
}
