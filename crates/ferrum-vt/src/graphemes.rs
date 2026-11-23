//! The per-grid table of multi-scalar grapheme clusters (`docs/02` §3.1).
//!
//! A cell holding a cluster of more than one scalar (a base plus combining marks, a ZWJ emoji)
//! stores an index into this table. The table is bounded; the grid compacts it when it is full
//! (`Grid::intern_cluster`), which is possible because only grid cells refer to it: the
//! scrollback stores the cluster's scalars as text.

use rustc_hash::FxHashMap;

use crate::limits::MAX_GRAPHEMES;
use crate::slot_set::SlotSet;

/// Interned clusters of one grid.
#[derive(Debug, Default, Clone)]
pub struct GraphemeTable {
    clusters: Vec<Box<str>>,
    index: FxHashMap<Box<str>, u32>,
}

impl GraphemeTable {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// The cluster behind `index`.
    pub fn get(&self, index: u32) -> Option<&str> {
        let slot = usize::try_from(index).ok()?;
        self.clusters.get(slot).map(AsRef::as_ref)
    }

    /// True when no more clusters can be added.
    pub fn is_full(&self) -> bool {
        self.clusters.len() >= MAX_GRAPHEMES
    }

    /// Number of clusters held.
    pub fn len(&self) -> usize {
        self.clusters.len()
    }

    /// True when the table holds no cluster.
    pub fn is_empty(&self) -> bool {
        self.clusters.is_empty()
    }

    /// The index of `cluster`, adding it when new; `None` when the table is full.
    pub fn intern(&mut self, cluster: &str) -> Option<u32> {
        if let Some(&index) = self.index.get(cluster) {
            return Some(index);
        }
        if self.is_full() {
            return None;
        }
        let index = u32::try_from(self.clusters.len()).ok()?;
        self.clusters.push(cluster.into());
        self.index.insert(cluster.into(), index);
        Some(index)
    }

    /// Keeps the clusters in `used` and renumbers them in order, without allocating: the cells
    /// that refer to them get their new index from [`SlotSet::rank`].
    pub(crate) fn compact(&mut self, used: &SlotSet) {
        let mut write = 0;
        for read in 0..self.clusters.len() {
            if used.contains(read) {
                // INVARIANT: write <= read < len, so both indices are in bounds.
                self.clusters.swap(write, read);
                write += 1;
            }
        }
        self.clusters.truncate(write);
        self.index.retain(|_, slot| usize::try_from(*slot).is_ok_and(|s| used.contains(s)));
        for slot in self.index.values_mut() {
            *slot = used.rank(usize::try_from(*slot).unwrap_or(0));
        }
    }

    /// Heap bytes held, by capacity.
    pub fn capacity_bytes(&self) -> usize {
        self.clusters.iter().map(|cluster| cluster.len() * 2 + 2 * size_of::<Box<str>>()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interning_is_idempotent_and_bounded() {
        let mut table = GraphemeTable::new();
        let first = table.intern("e\u{0301}");
        assert_eq!(first, table.intern("e\u{0301}"));
        assert_eq!(table.get(first.unwrap_or(u32::MAX)), Some("e\u{0301}"));
        for n in 0..MAX_GRAPHEMES {
            table.intern(&format!("a{n}"));
        }
        assert!(table.is_full());
        assert_eq!(table.intern("new"), None);
    }

    #[test]
    fn compact_keeps_the_used_clusters_in_order_and_renumbers_them() {
        let mut table = GraphemeTable::new();
        table.intern("a\u{0301}");
        table.intern("b\u{0301}");
        table.intern("c\u{0301}");
        let mut used = SlotSet::new();
        used.insert(0);
        used.insert(2);
        used.seal();
        table.compact(&used);
        assert_eq!(table.len(), 2);
        assert_eq!(used.rank(2), 1);
        assert_eq!(table.get(1), Some("c\u{0301}"));
        assert_eq!(table.intern("c\u{0301}"), Some(1));
        assert_eq!(table.intern("a\u{0301}"), Some(0));
    }
}
