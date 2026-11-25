//! Glyphs the renderer draws itself, bypassing the font: box drawing (U+2500–U+257F), block
//! elements (U+2580–U+259F) and the powerline separators (U+E0B0–U+E0B3). They are drawn on the
//! cell's own pixel grid, so lines and blocks meet exactly across cells whatever the font.

mod blocks;
mod box_drawing;
mod canvas;
mod powerline;

pub(crate) use canvas::Canvas;

/// Whether `ch` is drawn by [`draw`] instead of the font.
pub(crate) fn is_synthesized(ch: char) -> bool {
    matches!(ch, '\u{2500}'..='\u{259F}' | '\u{E0B0}'..='\u{E0B3}')
}

/// The coverage of `ch` on a `width × height` cell whose light lines are `light` pixels thick.
pub(crate) fn draw(ch: char, width: u32, height: u32, light: u32) -> Option<Canvas> {
    match ch {
        '\u{2500}'..='\u{257F}' => box_drawing::draw(ch, width, height, light),
        '\u{2580}'..='\u{259F}' => blocks::draw(ch, width, height),
        _ => powerline::draw(ch, width, height, light),
    }
}
