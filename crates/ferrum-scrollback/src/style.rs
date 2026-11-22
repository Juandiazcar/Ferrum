//! Style interning: a per-session table of `(fg, bg, attrs)` triples (`docs/02` §3.2).

use std::collections::HashMap;

use crate::limits::MAX_STYLES;

/// A cell style as the scrollback stores it. The meaning of the bits (color kinds, attribute
/// flags) belongs to `ferrum-vt`; this crate only compares and stores them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Style {
    /// Foreground color value.
    pub fg: u32,
    /// Background color value.
    pub bg: u32,
    /// Attribute bits.
    pub attrs: u16,
}

/// Index of a [`Style`] in the session [`StyleTable`]. Id 0 is always the default style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct StyleId(u32);

impl StyleId {
    /// The default style, present in every table.
    pub const DEFAULT: Self = Self(0);

    /// The raw id as stored in encoded lines.
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Wraps an id read from encoded data (it is validated against the table when used).
    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
}

/// Interned styles of one session.
#[derive(Debug)]
pub struct StyleTable {
    styles: Vec<Style>,
    ids: HashMap<Style, StyleId>,
    overflowed: u64,
}

impl Default for StyleTable {
    fn default() -> Self {
        Self::new()
    }
}

impl StyleTable {
    /// A table holding only the default style.
    pub fn new() -> Self {
        let mut table = Self { styles: Vec::new(), ids: HashMap::new(), overflowed: 0 };
        table.styles.push(Style::default());
        table.ids.insert(Style::default(), StyleId::DEFAULT);
        table
    }

    /// Returns the id of `style`, adding it when new. When the table already holds
    /// [`MAX_STYLES`] entries a new style is stored as the default style and counted in
    /// [`overflowed`](Self::overflowed): bounded memory beats unbounded color fidelity.
    pub fn intern(&mut self, style: Style) -> StyleId {
        if let Some(&id) = self.ids.get(&style) {
            return id;
        }
        let Ok(raw) = u32::try_from(self.styles.len()) else {
            self.overflowed += 1;
            return StyleId::DEFAULT;
        };
        if self.styles.len() >= MAX_STYLES {
            self.overflowed += 1;
            return StyleId::DEFAULT;
        }
        let id = StyleId(raw);
        self.styles.push(style);
        self.ids.insert(style, id);
        id
    }

    /// The style behind `id`, or `None` when the id is not in the table.
    pub fn get(&self, id: StyleId) -> Option<Style> {
        self.styles.get(usize::try_from(id.0).ok()?).copied()
    }

    /// Number of interned styles (at least 1).
    pub fn len(&self) -> usize {
        self.styles.len()
    }

    /// Always false: the default style is always present.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// How many styles were not interned because the table was full.
    pub fn overflowed(&self) -> u64 {
        self.overflowed
    }

    /// Heap bytes held by the table (capacities, not lengths).
    pub fn capacity_bytes(&self) -> usize {
        // hashbrown stores one (key, value) pair plus one control byte per bucket.
        let entry = size_of::<(Style, StyleId)>() + 1;
        self.styles.capacity() * size_of::<Style>() + self.ids.capacity() * entry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_style_is_id_zero_and_interning_is_idempotent() {
        let mut table = StyleTable::new();
        assert_eq!(table.intern(Style::default()), StyleId::DEFAULT);
        let red = Style { fg: 1, bg: 0, attrs: 0 };
        let first = table.intern(red);
        assert_eq!(table.intern(red), first);
        assert_eq!(table.get(first), Some(red));
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn table_is_bounded() {
        let mut table = StyleTable::new();
        for fg in 0..u32::try_from(MAX_STYLES + 10).unwrap_or(0) {
            table.intern(Style { fg, bg: 7, attrs: 0 });
        }
        assert_eq!(table.len(), MAX_STYLES);
        assert!(table.overflowed() >= 10);
        assert_eq!(table.intern(Style { fg: u32::MAX, bg: 7, attrs: 0 }), StyleId::DEFAULT);
    }
}
