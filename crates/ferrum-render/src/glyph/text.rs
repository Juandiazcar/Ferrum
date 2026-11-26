//! What a character asks of the renderer: its presentation (text or emoji), whether it has a
//! glyph at all, and how many cells its advance takes.

use crate::font::Presentation;
use crate::font::metrics::f32_from;

/// A glyph's advance may exceed the font's own advance by up to half a pixel and still take one
/// cell: the cell's advance is the font's rounded to whole pixels (`crate::font::metrics`).
const ADVANCE_ROUNDING_PX: f32 = 0.5;

/// VARIATION SELECTOR-15: text presentation.
const TEXT_SELECTOR: char = '\u{FE0E}';
/// VARIATION SELECTOR-16: emoji presentation.
const EMOJI_SELECTOR: char = '\u{FE0F}';

/// The code points with `Emoji_Presentation=Yes` (Unicode 17.0 `emoji-data.txt`, ranges
/// merged, sorted); `tests` checks the table against `fixtures/unicode/emoji-data.txt`.
const EMOJI_PRESENTATION: [(char, char); 81] = [
    ('\u{231A}', '\u{231B}'),
    ('\u{23E9}', '\u{23EC}'),
    ('\u{23F0}', '\u{23F0}'),
    ('\u{23F3}', '\u{23F3}'),
    ('\u{25FD}', '\u{25FE}'),
    ('\u{2614}', '\u{2615}'),
    ('\u{2648}', '\u{2653}'),
    ('\u{267F}', '\u{267F}'),
    ('\u{2693}', '\u{2693}'),
    ('\u{26A1}', '\u{26A1}'),
    ('\u{26AA}', '\u{26AB}'),
    ('\u{26BD}', '\u{26BE}'),
    ('\u{26C4}', '\u{26C5}'),
    ('\u{26CE}', '\u{26CE}'),
    ('\u{26D4}', '\u{26D4}'),
    ('\u{26EA}', '\u{26EA}'),
    ('\u{26F2}', '\u{26F3}'),
    ('\u{26F5}', '\u{26F5}'),
    ('\u{26FA}', '\u{26FA}'),
    ('\u{26FD}', '\u{26FD}'),
    ('\u{2705}', '\u{2705}'),
    ('\u{270A}', '\u{270B}'),
    ('\u{2728}', '\u{2728}'),
    ('\u{274C}', '\u{274C}'),
    ('\u{274E}', '\u{274E}'),
    ('\u{2753}', '\u{2755}'),
    ('\u{2757}', '\u{2757}'),
    ('\u{2795}', '\u{2797}'),
    ('\u{27B0}', '\u{27B0}'),
    ('\u{27BF}', '\u{27BF}'),
    ('\u{2B1B}', '\u{2B1C}'),
    ('\u{2B50}', '\u{2B50}'),
    ('\u{2B55}', '\u{2B55}'),
    ('\u{1F004}', '\u{1F004}'),
    ('\u{1F0CF}', '\u{1F0CF}'),
    ('\u{1F18E}', '\u{1F18E}'),
    ('\u{1F191}', '\u{1F19A}'),
    ('\u{1F1E6}', '\u{1F1FF}'),
    ('\u{1F201}', '\u{1F201}'),
    ('\u{1F21A}', '\u{1F21A}'),
    ('\u{1F22F}', '\u{1F22F}'),
    ('\u{1F232}', '\u{1F236}'),
    ('\u{1F238}', '\u{1F23A}'),
    ('\u{1F250}', '\u{1F251}'),
    ('\u{1F300}', '\u{1F320}'),
    ('\u{1F32D}', '\u{1F335}'),
    ('\u{1F337}', '\u{1F37C}'),
    ('\u{1F37E}', '\u{1F393}'),
    ('\u{1F3A0}', '\u{1F3CA}'),
    ('\u{1F3CF}', '\u{1F3D3}'),
    ('\u{1F3E0}', '\u{1F3F0}'),
    ('\u{1F3F4}', '\u{1F3F4}'),
    ('\u{1F3F8}', '\u{1F43E}'),
    ('\u{1F440}', '\u{1F440}'),
    ('\u{1F442}', '\u{1F4FC}'),
    ('\u{1F4FF}', '\u{1F53D}'),
    ('\u{1F54B}', '\u{1F54E}'),
    ('\u{1F550}', '\u{1F567}'),
    ('\u{1F57A}', '\u{1F57A}'),
    ('\u{1F595}', '\u{1F596}'),
    ('\u{1F5A4}', '\u{1F5A4}'),
    ('\u{1F5FB}', '\u{1F64F}'),
    ('\u{1F680}', '\u{1F6C5}'),
    ('\u{1F6CC}', '\u{1F6CC}'),
    ('\u{1F6D0}', '\u{1F6D2}'),
    ('\u{1F6D5}', '\u{1F6D8}'),
    ('\u{1F6DC}', '\u{1F6DF}'),
    ('\u{1F6EB}', '\u{1F6EC}'),
    ('\u{1F6F4}', '\u{1F6FC}'),
    ('\u{1F7E0}', '\u{1F7EB}'),
    ('\u{1F7F0}', '\u{1F7F0}'),
    ('\u{1F90C}', '\u{1F93A}'),
    ('\u{1F93C}', '\u{1F945}'),
    ('\u{1F947}', '\u{1F9FF}'),
    ('\u{1FA70}', '\u{1FA7C}'),
    ('\u{1FA80}', '\u{1FA8A}'),
    ('\u{1FA8E}', '\u{1FAC6}'),
    ('\u{1FAC8}', '\u{1FAC8}'),
    ('\u{1FACD}', '\u{1FADC}'),
    ('\u{1FADF}', '\u{1FAEA}'),
    ('\u{1FAEF}', '\u{1FAF8}'),
];

/// How `ch` is presented: VS15 or VS16 after it decide; otherwise its Unicode
/// `Emoji_Presentation` property (so ☕ and ✅ are emoji, ❤ and ☺ text). The same on every OS.
pub(crate) fn presentation(ch: char, next: Option<char>) -> Presentation {
    match next {
        Some(TEXT_SELECTOR) => Presentation::Text,
        Some(EMOJI_SELECTOR) => Presentation::Emoji,
        _ if is_emoji_presentation(ch) => Presentation::Emoji,
        _ => Presentation::Text,
    }
}

fn is_emoji_presentation(ch: char) -> bool {
    EMOJI_PRESENTATION
        .binary_search_by(|&(first, last)| {
            if last < ch {
                std::cmp::Ordering::Less
            } else if first > ch {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// Default-ignorable characters that have no glyph of their own: joiners, variation selectors,
/// zero-width spaces and marks, word joiner and invisible operators, tag characters (Unicode
/// `DerivedCoreProperties.txt`, `Default_Ignorable_Code_Point`, the ranges terminals meet).
pub(crate) fn is_ignorable(ch: char) -> bool {
    matches!(ch,
        '\u{00AD}' | '\u{034F}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}' | '\u{FE00}'..='\u{FE0F}' | '\u{FEFF}'
        | '\u{E0000}'..='\u{E007F}' | '\u{E0100}'..='\u{E01EF}')
}

/// Cells a glyph of `advance` pixels takes, measured against the font's own advance
/// `font_advance` (letter spacing does not count): none for a combining mark, at least one
/// otherwise. An unknown advance (the glyph could not be rasterized) takes one cell.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "small, rounded up, clamped at 1"
)]
pub(crate) fn cells_for(advance: Option<f32>, font_advance: u32) -> u32 {
    let Some(advance) = advance else { return 1 };
    if advance <= ADVANCE_ROUNDING_PX {
        return 0;
    }
    ((advance - ADVANCE_ROUNDING_PX) / f32_from(font_advance.max(1))).ceil().max(1.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_matches_the_unicode_data_file() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/unicode/emoji-data.txt");
        let data = std::fs::read_to_string(path).unwrap();
        let mut expected = Vec::new();
        for line in data.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((points, property)) = line.split_once(';') else { continue };
            if property.trim() != "Emoji_Presentation" {
                continue;
            }
            let (first, last) =
                points.trim().split_once("..").unwrap_or((points.trim(), points.trim()));
            let parse = |hex: &str| u32::from_str_radix(hex, 16).unwrap();
            expected.extend(parse(first)..=parse(last));
        }
        let ours: Vec<u32> = EMOJI_PRESENTATION
            .iter()
            .flat_map(|&(first, last)| u32::from(first)..=u32::from(last))
            .collect();
        expected.sort_unstable();
        assert_eq!(ours, expected);
    }

    #[test]
    fn presentation_follows_the_property_and_the_selectors() {
        for emoji in ['😀', '☕', '⚡', '✅', '⌚', '🇨'] {
            assert_eq!(presentation(emoji, None), Presentation::Emoji, "{emoji}");
        }
        for text in ['a', '❤', '☺', '©', '1', '─'] {
            assert_eq!(presentation(text, None), Presentation::Text, "{text}");
        }
        assert_eq!(presentation('❤', Some(EMOJI_SELECTOR)), Presentation::Emoji);
        assert_eq!(presentation('☕', Some(TEXT_SELECTOR)), Presentation::Text);
    }

    #[test]
    fn advances_map_to_cells_against_the_font_advance() {
        assert_eq!(cells_for(Some(7.2), 7), 1);
        assert_eq!(cells_for(Some(7.5), 7), 1);
        assert_eq!(cells_for(Some(13.0), 8), 2);
        assert_eq!(cells_for(Some(0.0), 7), 0, "a combining mark");
        assert_eq!(cells_for(None, 7), 1, "a glyph that could not be rasterized");
    }
}
