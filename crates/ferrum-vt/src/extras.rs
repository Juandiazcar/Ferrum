//! Interned hyperlinks and underline colors.
//!
//! A cell has no room for a URI or a second color, so both live in a per-session table and the
//! cell carries a 12-bit [`ExtraId`] (`color.rs`). The id travels into the scrollback inside the
//! style words, so ids are never reused: reusing one would re-point old rows to a different URI.
//! The table is therefore bounded by refusing new entries (`MAX_EXTRAS`, `MAX_LINK_TABLE_BYTES`),
//! counted in [`Extras::overflowed`], and not by eviction (`docs/03` §3 rule 4, `docs/10` §3
//! item 11).

use rustc_hash::FxHashMap;

use crate::color::Color;
use crate::limits::{MAX_EXTRAS, MAX_LINK_TABLE_BYTES};

/// Names a hyperlink / underline-color combination. `NONE` means "neither".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ExtraId(u16);

impl ExtraId {
    /// No hyperlink and the default underline color.
    pub const NONE: Self = Self(0);

    pub(crate) const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    pub(crate) const fn bits(self) -> u16 {
        self.0
    }
}

/// Names an interned hyperlink; `NONE` is no link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LinkId(u16);

impl LinkId {
    /// No hyperlink.
    pub const NONE: Self = Self(0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ExtraEntry {
    link: LinkId,
    underline: Color,
}

/// The per-session tables of hyperlinks and extra styles.
#[derive(Debug, Default)]
pub struct Extras {
    uris: Vec<Box<str>>,
    link_index: FxHashMap<Box<str>, LinkId>,
    link_bytes: usize,
    entries: Vec<ExtraEntry>,
    entry_index: FxHashMap<ExtraEntry, ExtraId>,
    overflowed: u64,
}

impl Extras {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Interns `uri` under the optional OSC 8 `id` (the same pair gives the same link). `None`
    /// when the table is full; the caller then shows the text without a link.
    pub fn intern_link(&mut self, id: &str, uri: &str, key: &mut String) -> Option<LinkId> {
        key.clear();
        key.push_str(id);
        key.push('\0');
        key.push_str(uri);
        if let Some(&link) = self.link_index.get(key.as_str()) {
            return Some(link);
        }
        let cost = key.len() + uri.len();
        let next =
            u16::try_from(self.uris.len() + 1).ok().filter(|&n| usize::from(n) <= MAX_EXTRAS);
        let Some(next) = next.filter(|_| self.link_bytes + cost <= MAX_LINK_TABLE_BYTES) else {
            self.overflowed += 1;
            return None;
        };
        self.link_bytes += cost;
        self.uris.push(uri.into());
        let link = LinkId(next);
        self.link_index.insert(key.as_str().into(), link);
        Some(link)
    }

    /// The id of the combination, adding it when new. A full table gives `ExtraId::NONE`.
    pub fn intern(&mut self, link: LinkId, underline: Color) -> ExtraId {
        if link == LinkId::NONE && underline == Color::DEFAULT {
            return ExtraId::NONE;
        }
        let entry = ExtraEntry { link, underline };
        if let Some(&id) = self.entry_index.get(&entry) {
            return id;
        }
        let Some(next) =
            u16::try_from(self.entries.len() + 1).ok().filter(|&n| usize::from(n) <= MAX_EXTRAS)
        else {
            self.overflowed += 1;
            return ExtraId::NONE;
        };
        self.entries.push(entry);
        let id = ExtraId(next);
        self.entry_index.insert(entry, id);
        id
    }

    fn entry(&self, id: ExtraId) -> Option<ExtraEntry> {
        let index = usize::from(id.0).checked_sub(1)?;
        self.entries.get(index).copied()
    }

    /// The hyperlink of `id`, if any.
    pub fn link(&self, id: ExtraId) -> LinkId {
        self.entry(id).map_or(LinkId::NONE, |entry| entry.link)
    }

    /// The URI behind a link.
    pub fn uri(&self, link: LinkId) -> Option<&str> {
        let index = usize::from(link.0).checked_sub(1)?;
        self.uris.get(index).map(AsRef::as_ref)
    }

    /// The underline color of `id` (the default color when none was set).
    pub fn underline_color(&self, id: ExtraId) -> Color {
        self.entry(id).map_or(Color::DEFAULT, |entry| entry.underline)
    }

    /// How many entries were refused because a table was full.
    pub fn overflowed(&self) -> u64 {
        self.overflowed
    }

    /// Heap bytes held, by capacity.
    pub fn capacity_bytes(&self) -> usize {
        self.link_bytes
            + self.entries.capacity() * size_of::<ExtraEntry>()
            + self.uris.capacity() * size_of::<Box<str>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_link_and_the_same_combination_intern_once() {
        let mut extras = Extras::new();
        let mut key = String::new();
        let first = extras.intern_link("a", "https://x.test", &mut key);
        assert_eq!(first, extras.intern_link("a", "https://x.test", &mut key));
        assert_ne!(first, extras.intern_link("b", "https://x.test", &mut key));
        let link = first.unwrap_or_default();
        let id = extras.intern(link, Color::rgb(1, 2, 3));
        assert_eq!(id, extras.intern(link, Color::rgb(1, 2, 3)));
        assert_eq!(extras.uri(extras.link(id)), Some("https://x.test"));
        assert_eq!(extras.underline_color(id), Color::rgb(1, 2, 3));
    }

    #[test]
    fn nothing_special_is_the_none_id() {
        let mut extras = Extras::new();
        assert_eq!(extras.intern(LinkId::NONE, Color::DEFAULT), ExtraId::NONE);
        assert_eq!(extras.underline_color(ExtraId::NONE), Color::DEFAULT);
        assert_eq!(extras.uri(LinkId::NONE), None);
    }

    #[test]
    fn the_table_refuses_new_entries_when_full_and_never_reuses_ids() {
        let mut extras = Extras::new();
        for n in 0..u8::MAX {
            extras.intern(LinkId::NONE, Color::indexed(n));
        }
        let mut last = ExtraId::NONE;
        for value in 0..=u32::try_from(MAX_EXTRAS).unwrap_or(0) {
            let [high, low] = u16::try_from(value).unwrap_or(0).to_be_bytes();
            last = extras.intern(LinkId::NONE, Color::rgb(high, low, 9));
        }
        assert_eq!(last, ExtraId::NONE);
        assert!(extras.overflowed() > 0);
        assert_eq!(extras.underline_color(ExtraId(1)), Color::indexed(0));
    }

    #[test]
    fn link_bytes_are_capped() {
        let mut extras = Extras::new();
        let mut key = String::new();
        let uri = "u".repeat(crate::limits::MAX_URI_BYTES);
        let mut refused = false;
        for n in 0..200 {
            refused |= extras.intern_link(&n.to_string(), &uri, &mut key).is_none();
        }
        assert!(refused);
    }
}
