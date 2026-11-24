//! Interned hyperlinks and underline colors.
//!
//! A cell has no room for a URI or a second color, so both live in a per-session table and the
//! cell carries a 12-bit [`ExtraId`] (`color.rs`). The id travels into the scrollback inside the
//! style words, so an id may only be handed out again when no row anywhere still carries it:
//! `Core::sweep_extras` (`extras_sweep.rs`) marks the ids used by the screens, the pens and the
//! retained history (from its per-block summaries, `history_extras.rs`) and calls
//! [`Extras::release_unmarked`]. Until then a full table refuses new entries, counted in
//! [`Extras::overflowed`] (`docs/03` §3 rule 4, `docs/10` §3 item 11).

use rustc_hash::FxHashMap;

use crate::color::Color;
use crate::history_extras::HistoryExtras;
use crate::limits::{MAX_EXTRAS, MAX_LINK_TABLE_BYTES, WORK_PER_REFUSAL};
use crate::slot_set::SlotSet;

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

    pub(crate) const fn bits(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ExtraEntry {
    link: LinkId,
    underline: Color,
}

/// One interned hyperlink: `key` is `id NUL uri`, the text the index is looked up with.
#[derive(Debug)]
struct LinkEntry {
    key: Box<str>,
    /// Where the URI starts inside `key`.
    uri_at: usize,
}

impl LinkEntry {
    fn cost(&self) -> usize {
        link_cost(self.key.len(), self.uri_at)
    }
}

/// Bytes a link counts against [`MAX_LINK_TABLE_BYTES`]: its key (`id NUL uri`) plus its URI.
fn link_cost(key_len: usize, uri_at: usize) -> usize {
    key_len + key_len.saturating_sub(uri_at)
}

/// The per-session tables of hyperlinks and extra styles. Slot `n` of a table is the id `n + 1`;
/// a released slot is `None` and is handed out again by the next interning.
#[derive(Debug, Default)]
pub struct Extras {
    links: Slots<LinkEntry>,
    link_index: FxHashMap<Box<str>, LinkId>,
    link_bytes: usize,
    entries: Slots<ExtraEntry>,
    entry_index: FxHashMap<ExtraEntry, ExtraId>,
    overflowed: u64,
    /// Refusals left before the next sweep: the work of the last one, amortized.
    sweep_cooldown: usize,
    /// Work units (cells and summary words) of every sweep so far, for the tests of the bound.
    sweep_work: u64,
    /// The ids the retained history may use.
    pub(crate) history: HistoryExtras,
}

/// A table of slots that may have holes (released slots) and may still grow.
#[derive(Debug)]
struct Slots<T> {
    items: Vec<Option<T>>,
    /// Released slots inside `items`: with none, a new item is pushed without a search.
    holes: usize,
}

impl<T> Default for Slots<T> {
    fn default() -> Self {
        Self { items: Vec::new(), holes: 0 }
    }
}

impl<T> Slots<T> {
    /// The slot a new item would take: a hole, or the end while the table may still grow.
    fn free_slot(&self) -> Option<usize> {
        if self.holes > 0 {
            return self.items.iter().position(Option::is_none);
        }
        (self.items.len() < MAX_EXTRAS).then_some(self.items.len())
    }

    fn put(&mut self, slot: usize, value: T) {
        match self.items.get_mut(slot) {
            Some(place) => {
                *place = Some(value);
                self.holes = self.holes.saturating_sub(1);
            }
            None => self.items.push(Some(value)),
        }
    }

    fn get(&self, id: u16) -> Option<&T> {
        self.items.get(usize::from(id).checked_sub(1)?)?.as_ref()
    }
}

impl Extras {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Interns `uri` under the optional OSC 8 `id` (the same pair gives the same link). `None`
    /// when the table is full; the caller then shows the text without a link. A refusal is
    /// counted.
    pub fn intern_link(&mut self, id: &str, uri: &str, key: &mut String) -> Option<LinkId> {
        let link = self.try_intern_link(id, uri, key);
        if link.is_none() {
            self.overflowed += 1;
        }
        link
    }

    /// Like [`intern_link`](Self::intern_link) but a refusal is not counted: the caller may free
    /// slots and try again.
    pub(crate) fn try_intern_link(
        &mut self,
        id: &str,
        uri: &str,
        key: &mut String,
    ) -> Option<LinkId> {
        key.clear();
        key.push_str(id);
        key.push('\0');
        let uri_at = key.len();
        key.push_str(uri);
        if let Some(&link) = self.link_index.get(key.as_str()) {
            return Some(link);
        }
        // Room is checked before the key is copied: a refusal allocates nothing.
        let cost = link_cost(key.len(), uri_at);
        let slot =
            self.links.free_slot().filter(|_| self.link_bytes + cost <= MAX_LINK_TABLE_BYTES)?;
        let link = LinkId(u16::try_from(slot + 1).ok()?);
        let entry = LinkEntry { key: key.as_str().into(), uri_at };
        self.link_bytes += cost;
        self.link_index.insert(entry.key.clone(), link);
        self.links.put(slot, entry);
        Some(link)
    }

    /// The id of the combination, adding it when new. A full table gives `ExtraId::NONE`, and the
    /// refusal is counted.
    pub fn intern(&mut self, link: LinkId, underline: Color) -> ExtraId {
        self.try_intern(link, underline).unwrap_or_else(|| {
            self.overflowed += 1;
            ExtraId::NONE
        })
    }

    /// Like [`intern`](Self::intern) but `None` (not counted) when the table is full.
    pub(crate) fn try_intern(&mut self, link: LinkId, underline: Color) -> Option<ExtraId> {
        if link == LinkId::NONE && underline == Color::DEFAULT {
            return Some(ExtraId::NONE);
        }
        let entry = ExtraEntry { link, underline };
        if let Some(&id) = self.entry_index.get(&entry) {
            return Some(id);
        }
        let slot = self.entries.free_slot()?;
        let id = ExtraId(u16::try_from(slot + 1).ok()?);
        self.entries.put(slot, entry);
        self.entry_index.insert(entry, id);
        Some(id)
    }

    /// Forgets every entry (RIS); the count of refusals is kept.
    pub(crate) fn clear(&mut self) {
        let overflowed = self.overflowed;
        *self = Self::new();
        self.overflowed = overflowed;
    }

    /// Counts a refusal the caller could not avoid.
    pub(crate) fn note_refused(&mut self) {
        self.overflowed += 1;
    }

    /// True when a sweep may run now; otherwise the refusal spends one unit of the cooldown.
    pub(crate) fn sweep_due(&mut self) -> bool {
        if self.sweep_cooldown == 0 {
            return true;
        }
        self.sweep_cooldown -= 1;
        false
    }

    /// Work units of every sweep so far.
    #[cfg(test)]
    pub(crate) fn sweep_work(&self) -> u64 {
        self.sweep_work
    }

    /// Records a sweep that did `work` units (cells and summary words) and freed `freed` entries:
    /// the next `work / WORK_PER_REFUSAL - freed` refusals do not sweep, so the amortized cost of
    /// a refused or freshly stored entry is bounded by [`WORK_PER_REFUSAL`], whatever the grid
    /// and history sizes (the `freed` entries that follow are cheap successes).
    pub(crate) fn sweep_done(&mut self, work: usize, freed: usize) {
        self.sweep_work = self.sweep_work.saturating_add(u64::try_from(work).unwrap_or(u64::MAX));
        self.sweep_cooldown = (work / WORK_PER_REFUSAL).saturating_sub(freed);
    }

    /// Releases every extra style and link that is not marked. `extras` holds the ids in use by
    /// cells, pens and history; `links` the links in use by pens. A link used by a surviving
    /// extra style survives too. Returns how many entries were released. No allocation.
    pub(crate) fn release_unmarked(&mut self, extras: &SlotSet, mut links: SlotSet) -> usize {
        let mut freed = 0;
        for (slot, place) in self.entries.items.iter_mut().enumerate() {
            let Some(entry) = *place else { continue };
            if extras.contains(slot + 1) {
                links.insert(usize::from(entry.link.bits()));
            } else {
                self.entry_index.remove(&entry);
                *place = None;
                self.entries.holes += 1;
                freed += 1;
            }
        }
        for (slot, place) in self.links.items.iter_mut().enumerate() {
            if links.contains(slot + 1) {
                continue;
            }
            if let Some(entry) = place.take() {
                self.link_index.remove(&entry.key);
                self.link_bytes = self.link_bytes.saturating_sub(entry.cost());
                self.links.holes += 1;
                freed += 1;
            }
        }
        freed
    }

    /// Number of extra styles currently in use.
    pub fn len(&self) -> usize {
        self.entries.items.iter().flatten().count()
    }

    /// True when no extra style is in use.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn entry(&self, id: ExtraId) -> Option<ExtraEntry> {
        self.entries.get(id.0).copied()
    }

    /// The hyperlink of `id`, if any.
    pub fn link(&self, id: ExtraId) -> LinkId {
        self.entry(id).map_or(LinkId::NONE, |entry| entry.link)
    }

    /// The URI behind a link.
    pub fn uri(&self, link: LinkId) -> Option<&str> {
        let entry = self.links.get(link.0)?;
        entry.key.get(entry.uri_at..)
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
            + self.entries.items.capacity() * size_of::<Option<ExtraEntry>>()
            + self.links.items.capacity() * size_of::<Option<LinkEntry>>()
            + self.history.capacity_bytes()
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
