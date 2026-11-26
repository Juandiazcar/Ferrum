//! A direct table from printable ASCII in the four common styles to glyph cache slots, so the
//! dense screens of NFR-PERF-03 skip the font lookup and the hash of the cache key. An entry is
//! valid while the cache has evicted nothing since it was written (its generation), for the cell
//! metrics and the [`super::Glyphs::generation`] (fonts, antialiasing) it was written for.
// perf: `render_repaint/dense_200x60`.

use crate::font::{BOLD_WEIGHT, CellMetrics, REGULAR_WEIGHT, Slant, TextStyle};

/// Printable ASCII: U+0021 to U+007E (the space draws nothing).
const FIRST: u32 = 0x21;
const COUNT: usize = 94;
/// Regular and bold, upright and italic.
const STYLES: usize = 4;

#[derive(Debug, Clone, Copy, Default)]
struct Entry {
    slot: u32,
    generation: u32,
    valid: bool,
}

/// The table, for one set of cell metrics and one generation of the glyphs.
#[derive(Debug)]
pub(crate) struct AsciiSlots {
    owner: Option<(CellMetrics, u64)>,
    entries: Vec<Entry>,
}

impl AsciiSlots {
    pub(crate) fn new() -> Self {
        Self { owner: None, entries: vec![Entry::default(); COUNT * STYLES] }
    }

    /// The table index of `ch` in `style`, if the table covers them.
    pub(crate) fn index(ch: char, style: TextStyle) -> Option<usize> {
        let column =
            usize::try_from(u32::from(ch).checked_sub(FIRST)?).ok().filter(|&c| c < COUNT)?;
        let row = match (style.weight, style.slant) {
            (REGULAR_WEIGHT, Slant::Upright) => 0,
            (BOLD_WEIGHT, Slant::Upright) => 1,
            (REGULAR_WEIGHT, Slant::Italic) => 2,
            (BOLD_WEIGHT, Slant::Italic) => 3,
            _ => return None,
        };
        Some(row * COUNT + column)
    }

    /// The cached slot of entry `index` for `metrics` and the glyphs' generation, if still
    /// valid at cache `generation`.
    pub(crate) fn get(
        &mut self,
        index: usize,
        (metrics, glyphs): (&CellMetrics, u64),
        generation: u32,
    ) -> Option<u32> {
        if self.owner != Some((*metrics, glyphs)) {
            self.owner = Some((*metrics, glyphs));
            self.entries.fill(Entry::default());
            return None;
        }
        let entry = self.entries.get(index)?;
        (entry.valid && entry.generation == generation).then_some(entry.slot)
    }

    pub(crate) fn set(&mut self, index: usize, slot: u32, generation: u32) {
        if let Some(entry) = self.entries.get_mut(index) {
            *entry = Entry { slot, generation, valid: true };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_printable_ascii_in_the_four_styles_is_covered() {
        assert_eq!(AsciiSlots::index('!', TextStyle::REGULAR), Some(0));
        assert_eq!(AsciiSlots::index('~', TextStyle::REGULAR), Some(COUNT - 1));
        let bold_italic = TextStyle { weight: BOLD_WEIGHT, slant: Slant::Italic };
        assert_eq!(AsciiSlots::index('!', bold_italic), Some(3 * COUNT));
        assert_eq!(AsciiSlots::index(' ', TextStyle::REGULAR), None);
        assert_eq!(AsciiSlots::index('é', TextStyle::REGULAR), None);
        assert_eq!(AsciiSlots::index('a', TextStyle { weight: 600, slant: Slant::Upright }), None);
    }
}
