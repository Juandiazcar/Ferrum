//! Character sets G0-G3 and their shifts (FR-TERM-004: line drawing charsets).

/// A designated character set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Charset {
    /// US ASCII (`B`).
    #[default]
    Ascii,
    /// DEC special graphics, the line drawing set (`0`).
    DecSpecial,
    /// United Kingdom (`A`): `#` is the pound sign.
    Uk,
}

/// DEC special graphics for 0x5F..=0x7E.
const DEC_SPECIAL: [char; 32] = [
    ' ', '◆', '▒', '␉', '␌', '␍', '␊', '°', '±', '␤', '␋', '┘', '┐', '┌', '└', '┼', '⎺', '⎻', '─',
    '⎼', '⎽', '├', '┤', '┴', '┬', '│', '≤', '≥', 'π', '≠', '£', '·',
];

/// First scalar of the DEC special graphics range.
const DEC_SPECIAL_FIRST: u32 = 0x5F;

impl Charset {
    /// The character set designated by a final byte (`ESC ( B` and friends).
    pub fn from_final(byte: u8) -> Option<Self> {
        match byte {
            b'B' => Some(Self::Ascii),
            b'0' => Some(Self::DecSpecial),
            b'A' => Some(Self::Uk),
            _ => None,
        }
    }

    fn map(self, c: char) -> char {
        match self {
            Self::Uk if c == '#' => '£',
            Self::Ascii | Self::Uk => c,
            Self::DecSpecial => u32::from(c)
                .checked_sub(DEC_SPECIAL_FIRST)
                .and_then(|offset| usize::try_from(offset).ok())
                .and_then(|offset| DEC_SPECIAL.get(offset))
                .copied()
                .unwrap_or(c),
        }
    }
}

/// The four designations and the active shift state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Charsets {
    designated: [Charset; 4],
    /// Which of G0..G3 is in GL.
    gl: usize,
    /// G2 or G3 for the next printed character only (SS2, SS3).
    single_shift: Option<usize>,
}

impl Charsets {
    /// Designates `charset` as G`slot` (0..=3; larger slots are ignored).
    pub fn designate(&mut self, slot: usize, charset: Charset) {
        if let Some(entry) = self.designated.get_mut(slot) {
            *entry = charset;
        }
    }

    /// Locking shift: G`slot` becomes GL (SI, SO, LS2, LS3).
    pub fn shift(&mut self, slot: usize) {
        if slot < self.designated.len() {
            self.gl = slot;
        }
    }

    /// Single shift: the next printed character uses G`slot` (SS2, SS3).
    pub fn single_shift(&mut self, slot: usize) {
        if slot < self.designated.len() {
            self.single_shift = Some(slot);
        }
    }

    /// True when every character prints as itself (no designation or shift is active).
    pub fn is_plain(&self) -> bool {
        self.single_shift.is_none() && self.designated.get(self.gl) == Some(&Charset::Ascii)
    }

    /// Maps a printed character through the active set, consuming a pending single shift.
    pub fn map(&mut self, c: char) -> char {
        let slot = self.single_shift.take().unwrap_or(self.gl);
        self.designated.get(slot).map_or(c, |charset| charset.map(c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_drawing_maps_letters_to_box_characters() {
        let mut sets = Charsets::default();
        sets.designate(0, Charset::DecSpecial);
        assert_eq!(sets.map('q'), '─');
        assert_eq!(sets.map('x'), '│');
        assert_eq!(sets.map('A'), 'A');
        assert!(!sets.is_plain());
    }

    #[test]
    fn shifts_select_the_set_and_single_shift_lasts_one_character() {
        let mut sets = Charsets::default();
        sets.designate(1, Charset::DecSpecial);
        sets.shift(1);
        assert_eq!(sets.map('l'), '┌');
        sets.shift(0);
        sets.designate(2, Charset::Uk);
        sets.single_shift(2);
        assert_eq!((sets.map('#'), sets.map('#')), ('£', '#'));
    }

    #[test]
    fn unknown_slots_and_finals_are_ignored() {
        let mut sets = Charsets::default();
        sets.designate(9, Charset::DecSpecial);
        sets.shift(9);
        assert!(sets.is_plain());
        assert_eq!(Charset::from_final(b'Z'), None);
    }
}
