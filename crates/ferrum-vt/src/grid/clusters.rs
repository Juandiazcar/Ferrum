//! Interning grapheme clusters into the grid's bounded table, and compacting it when full.

use super::Grid;
use crate::cell::CellFlags;
use crate::limits::WORK_PER_REFUSAL;
use crate::row::Row;
use crate::slot_set::SlotSet;

impl Grid {
    /// Interns a multi-scalar cluster. When the table is full it is compacted first, keeping the
    /// clusters the cells still use. A compaction that scanned `C` cells and freed `f` clusters is
    /// not tried again for `C / WORK_PER_REFUSAL - f` refusals (a refused mark is dropped, as with
    /// a full table), so a stream that leaves one stale cluster per mark costs at most
    /// `WORK_PER_REFUSAL` cells per mark, amortized, not a scan of the whole grid (`docs/05`
    /// §6.1).
    pub fn intern_cluster(&mut self, cluster: &str) -> Option<u32> {
        if let Some(index) = self.graphemes.intern(cluster) {
            return Some(index);
        }
        if self.compact_cooldown > 0 {
            self.compact_cooldown -= 1;
            return None;
        }
        let cells = self.cells();
        let freed = self.compact_graphemes();
        self.compact_work = self.compact_work.saturating_add(u64::try_from(cells).unwrap_or(0));
        self.compact_cooldown = (cells / WORK_PER_REFUSAL).saturating_sub(freed);
        self.graphemes.intern(cluster)
    }

    /// Drops the clusters no cell uses and renumbers the rest. Returns how many were freed. No
    /// allocation: the used set is a fixed bitset.
    fn compact_graphemes(&mut self) -> usize {
        let mut used = SlotSet::new();
        for cell in self.rows.iter().flat_map(Row::cells) {
            if cell.flags().contains(CellFlags::GRAPHEME) {
                used.insert(usize::try_from(cell.raw_char()).unwrap_or(usize::MAX));
            }
        }
        // A cell pointing outside the table is corrupt state; it is blanked below.
        used.seal();
        let before = self.graphemes.len();
        let kept = used.count().min(before);
        if kept == before {
            return 0;
        }
        self.graphemes.compact(&used);
        for cell in self.rows.iter_mut().flat_map(Row::cells_mut) {
            if !cell.flags().contains(CellFlags::GRAPHEME) {
                continue;
            }
            match usize::try_from(cell.raw_char()).ok().filter(|&slot| used.contains(slot)) {
                Some(slot) => cell.set_raw_char(used.rank(slot)),
                None => cell.set_scalar(crate::cell::BLANK_CHAR),
            }
        }
        before - kept
    }

    /// Cells of the grid: the work of one compaction scan.
    fn cells(&self) -> usize {
        self.rows.iter().map(Row::len).sum()
    }
}

#[cfg(test)]
mod tests {
    use crate::cell::{Cell, CellFlags};
    use crate::grid::Grid;
    use crate::limits::{MAX_GRAPHEMES, WORK_PER_REFUSAL};
    use crate::size::Size;

    /// Columns and rows of a grid with exactly one cell per table slot.
    const SIDE: usize = 64;
    /// Interns of the hostile stream.
    const STEPS: usize = 2_000;
    /// Cells per intern allowed above `WORK_PER_REFUSAL`: the last compaction is not amortized.
    const SLACK: u64 = 4;

    /// A `SIDE` x `SIDE` grid whose every cell holds its own cluster: the table is full.
    fn full_grid() -> Grid {
        let side = u16::try_from(SIDE).unwrap_or(1);
        let mut grid = Grid::new(Size::new(side, side));
        for n in 0..MAX_GRAPHEMES {
            let index = grid.intern_cluster(&format!("x{n}\u{0301}"));
            put(&mut grid, n, index);
        }
        grid
    }

    /// Stores cluster `index` in cell `n`, or a plain character when it was refused.
    fn put(grid: &mut Grid, n: usize, index: Option<u32>) {
        if let Some(cell) = grid.row_mut(n / SIDE).and_then(|row| row.get_mut(n % SIDE)) {
            *cell = Cell::default();
            match index {
                Some(index) => {
                    cell.set_raw_char(index);
                    cell.insert_flags(CellFlags::GRAPHEME);
                }
                None => cell.set_scalar('?'),
            }
        }
    }

    #[test]
    fn a_compaction_that_frees_nothing_is_not_retried_until_the_cooldown_ends() {
        let mut grid = full_grid();
        let cooldown = SIDE * SIDE / WORK_PER_REFUSAL;
        assert_eq!(grid.intern_cluster("fresh\u{0301}"), None);
        assert_eq!(grid.compact_cooldown, cooldown);
        assert_eq!(grid.intern_cluster("fresher\u{0301}"), None);
        assert_eq!(grid.compact_cooldown, cooldown - 1);
    }

    #[test]
    fn one_stale_cluster_per_print_costs_bounded_work_per_intern() {
        let mut grid = full_grid();
        let before = grid.compact_work;
        for n in 0..STEPS {
            // A new base character over cell 0 leaves its cluster stale, then a combining mark
            // makes a new cluster there: one stale cluster per step.
            put(&mut grid, 0, None);
            let index = grid.intern_cluster(&format!("y{n}\u{0301}"));
            put(&mut grid, 0, index);
        }
        let per_intern = (grid.compact_work - before) / u64::try_from(STEPS).unwrap_or(1);
        assert!(per_intern <= WORK_PER_REFUSAL as u64 + SLACK, "{per_intern} cells per intern");
    }

    #[test]
    fn a_full_grapheme_table_is_compacted_to_the_clusters_in_use() {
        let mut grid = Grid::new(Size::new(2, 1));
        let keep = grid.intern_cluster("k\u{0301}");
        for n in 0..MAX_GRAPHEMES {
            grid.graphemes.intern(&format!("x{n}\u{0301}"));
        }
        if let (Some(index), Some(cell)) = (keep, grid.row_mut(0).and_then(|row| row.get_mut(0))) {
            cell.set_raw_char(index);
            cell.insert_flags(CellFlags::GRAPHEME);
        }
        let fresh = grid.intern_cluster("fresh\u{0301}");
        assert!(fresh.is_some());
        let cell = grid.row(0).and_then(|row| row.get(0)).copied().unwrap_or_default();
        assert_eq!(grid.graphemes().get(cell.raw_char()), Some("k\u{0301}"));
    }
}
