//! Style interning: a per-session table of `(fg, bg, attrs)` triples (`docs/02` §3.2).

use rustc_hash::FxHashMap;

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
    // perf: Fx hashing; a style is hashed once per run of every pushed line (bench
    // vt_parse/sgr_heavy_8mib). Fx is not keyed, so a program can craft styles whose hashes
    // share their low bits. The table is capped at MAX_STYLES, so the worst probe of a lookup
    // is bounded: every probe group of the map holds 16 entries, so at most MAX_STYLES / 16 =
    // 512 group checks (a few microseconds), whatever the keys (test
    // `crafted_colliding_styles_stay_correct_and_bounded`).
    ids: FxHashMap<Style, StyleId>,
    /// The last style looked up and its id. perf: consecutive runs and lines mostly share a
    /// style, so most lookups skip the hash (bench `vt_parse/ascii_8mib`).
    last: (Style, StyleId),
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
        let mut table = Self {
            styles: Vec::new(),
            ids: FxHashMap::default(),
            last: (Style::default(), StyleId::DEFAULT),
            overflowed: 0,
        };
        table.styles.push(Style::default());
        table.ids.insert(Style::default(), StyleId::DEFAULT);
        table
    }

    /// Returns the id of `style`, adding it when new. When the table already holds
    /// [`MAX_STYLES`] entries a new style is stored as the default style and counted in
    /// [`overflowed`](Self::overflowed): bounded memory beats unbounded color fidelity.
    pub fn intern(&mut self, style: Style) -> StyleId {
        if style == self.last.0 {
            return self.last.1;
        }
        if let Some(&id) = self.ids.get(&style) {
            self.last = (style, id);
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
        self.last = (style, id);
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

    /// Styles whose Fx hashes share their low `bits` bits, found by search as an attacker would.
    fn colliding_styles(count: usize, bits: u32) -> Vec<Style> {
        use std::hash::BuildHasher;
        let hasher = rustc_hash::FxBuildHasher;
        let mask = (1u64 << bits) - 1;
        let mut found = Vec::new();
        let mut fg = 1u32;
        while found.len() < count {
            let style = Style { fg, bg: fg.rotate_left(7), attrs: 0 };
            if hasher.hash_one(style) & mask == 0 {
                found.push(style);
            }
            fg += 1;
        }
        found
    }

    #[test]
    fn crafted_colliding_styles_stay_correct_and_bounded() {
        const COLLIDING: usize = 1_024;
        const LOOKUPS: usize = 100;
        let styles = colliding_styles(COLLIDING, 12);
        let mut table = StyleTable::new();
        let ids: Vec<StyleId> = styles.iter().map(|&style| table.intern(style)).collect();
        let started = std::time::Instant::now();
        for _ in 0..LOOKUPS {
            for (style, id) in styles.iter().zip(&ids) {
                assert_eq!(table.intern(*style), *id);
            }
        }
        assert_eq!(table.len(), COLLIDING + 1);
        // 102 400 lookups in one probe chain: microseconds each at worst, far below a second;
        // the bound is loose so a slow CI machine never flakes.
        assert!(started.elapsed() < std::time::Duration::from_secs(5), "{:?}", started.elapsed());
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
