//! Damage tracking: which visible rows changed since the last [`Damage::take`]
//! (`docs/12` WP-02 "damage tracking API (dirty rows)").

const WORD_BITS: usize = u64::BITS as usize;

/// A set of dirty rows plus a scroll counter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Damage {
    words: Vec<u64>,
    rows: usize,
    all: bool,
    scrolled: u32,
}

impl Damage {
    /// A tracker for `rows` rows, everything dirty.
    pub fn new(rows: usize) -> Self {
        let mut damage = Self::default();
        damage.resize(rows);
        damage
    }

    /// Changes the number of rows; the whole screen is dirty afterwards.
    pub fn resize(&mut self, rows: usize) {
        self.rows = rows;
        self.words.clear();
        self.words.resize(rows.div_ceil(WORD_BITS), 0);
        self.all = true;
    }

    /// Marks one row.
    pub fn mark(&mut self, row: usize) {
        if row >= self.rows {
            return;
        }
        if let Some(word) = self.words.get_mut(row / WORD_BITS) {
            *word |= 1 << (row % WORD_BITS);
        }
    }

    /// Marks `from..to` (clamped).
    pub fn mark_range(&mut self, from: usize, to: usize) {
        let to = to.min(self.rows);
        if from == 0 && to == self.rows {
            // perf: scrolling the whole screen marks everything (bench vt_parse/ascii_8mib).
            self.all = true;
            return;
        }
        for row in from..to {
            self.mark(row);
        }
    }

    /// Marks every row.
    pub fn mark_all(&mut self) {
        self.all = true;
    }

    /// Records that content scrolled by `lines` (saturating); the rows it touched are marked by
    /// the caller.
    pub fn add_scroll(&mut self, lines: usize) {
        self.scrolled = self.scrolled.saturating_add(u32::try_from(lines).unwrap_or(u32::MAX));
    }

    /// True when `row` changed.
    pub fn is_dirty(&self, row: usize) -> bool {
        row < self.rows
            && (self.all
                || self
                    .words
                    .get(row / WORD_BITS)
                    .is_some_and(|word| word >> (row % WORD_BITS) & 1 == 1))
    }

    /// True when nothing changed.
    pub fn is_clean(&self) -> bool {
        !self.all && self.scrolled == 0 && self.words.iter().all(|&word| word == 0)
    }

    /// The dirty rows in order.
    pub fn dirty_rows(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.rows).filter(|&row| self.is_dirty(row))
    }

    /// Lines scrolled since the last [`take`](Self::take).
    pub const fn scrolled_lines(&self) -> u32 {
        self.scrolled
    }

    /// Clears the tracker.
    pub fn take(&mut self) {
        self.words.fill(0);
        self.all = false;
        self.scrolled = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_are_reported_in_order_and_cleared_by_take() {
        let mut damage = Damage::new(130);
        damage.take();
        assert!(damage.is_clean());
        damage.mark(129);
        damage.mark(3);
        damage.mark(500);
        assert_eq!(damage.dirty_rows().collect::<Vec<_>>(), vec![3, 129]);
        damage.take();
        assert!(damage.is_clean());
    }

    #[test]
    fn a_new_tracker_and_a_resize_are_fully_dirty() {
        let mut damage = Damage::new(5);
        assert_eq!(damage.dirty_rows().count(), 5);
        damage.take();
        damage.resize(7);
        assert_eq!(damage.dirty_rows().count(), 7);
    }

    #[test]
    fn scrolling_counts_and_saturates() {
        let mut damage = Damage::new(2);
        damage.take();
        damage.add_scroll(3);
        damage.add_scroll(usize::MAX);
        assert_eq!(damage.scrolled_lines(), u32::MAX);
        assert!(!damage.is_clean());
    }
}
