//! The per-grid table of multi-scalar grapheme clusters (`docs/02` §3.1).
//!
//! A cell holding a cluster of more than one scalar (a base plus combining marks, a ZWJ emoji)
//! stores an index into this table. The table is bounded; the grid compacts it when it is full
//! (`Grid::intern_cluster`), which is possible because only grid cells refer to it: the
//! scrollback stores the cluster's scalars as text.

use rustc_hash::FxHashMap;

use crate::limits::MAX_GRAPHEMES;

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

    /// Keeps the clusters for which `keep(index)` is true and returns the old-to-new index map.
    /// Indices of dropped clusters are absent from the map.
    pub fn retain(&mut self, keep: impl Fn(u32) -> bool) -> FxHashMap<u32, u32> {
        let old = std::mem::take(&mut self.clusters);
        self.index.clear();
        let mut remap = FxHashMap::default();
        for (position, cluster) in old.into_iter().enumerate() {
            let Ok(old_index) = u32::try_from(position) else { continue };
            if !keep(old_index) {
                continue;
            }
            let Ok(new_index) = u32::try_from(self.clusters.len()) else { continue };
            self.index.insert(cluster.clone(), new_index);
            self.clusters.push(cluster);
            remap.insert(old_index, new_index);
        }
        remap
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
    fn retain_compacts_and_remaps() {
        let mut table = GraphemeTable::new();
        table.intern("a\u{0301}");
        table.intern("b\u{0301}");
        table.intern("c\u{0301}");
        let remap = table.retain(|index| index != 1);
        assert_eq!(table.len(), 2);
        assert_eq!(remap.get(&2), Some(&1));
        assert_eq!(table.get(1), Some("c\u{0301}"));
        assert_eq!(table.intern("c\u{0301}"), Some(1));
    }
}
