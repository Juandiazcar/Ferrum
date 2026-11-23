//! Tab stops (HTS, TBC, CHT, CBT, HT).

use crate::limits::DEFAULT_TAB_WIDTH;

/// One flag per column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabStops {
    stops: Vec<bool>,
}

impl TabStops {
    /// Default stops every [`DEFAULT_TAB_WIDTH`] columns.
    pub fn new(cols: usize) -> Self {
        let mut tabs = Self { stops: Vec::new() };
        tabs.resize(cols);
        tabs
    }

    /// Changes the width; new columns get the default stops, existing ones keep their state.
    pub fn resize(&mut self, cols: usize) {
        let old = self.stops.len();
        self.stops.resize(cols, false);
        for (col, stop) in self.stops.iter_mut().enumerate().skip(old) {
            *stop = col.is_multiple_of(usize::from(DEFAULT_TAB_WIDTH));
        }
    }

    /// Restores the default stops.
    pub fn reset(&mut self) {
        let cols = self.stops.len();
        self.stops.clear();
        self.resize(cols);
    }

    /// Sets a stop at `col` (HTS).
    pub fn set(&mut self, col: usize) {
        if let Some(stop) = self.stops.get_mut(col) {
            *stop = true;
        }
    }

    /// Clears the stop at `col` (TBC 0).
    pub fn clear(&mut self, col: usize) {
        if let Some(stop) = self.stops.get_mut(col) {
            *stop = false;
        }
    }

    /// Clears every stop (TBC 3).
    pub fn clear_all(&mut self) {
        self.stops.fill(false);
    }

    /// True when `col` has a stop.
    pub fn is_set(&self, col: usize) -> bool {
        self.stops.get(col).copied().unwrap_or(false)
    }

    /// The first stop after `col`, or the last column when there is none.
    pub fn next(&self, col: usize) -> usize {
        let last = self.stops.len().saturating_sub(1);
        (col + 1..self.stops.len()).find(|&c| self.is_set(c)).unwrap_or(last)
    }

    /// The first stop before `col`, or column 0 when there is none.
    pub fn prev(&self, col: usize) -> usize {
        (0..col.min(self.stops.len())).rev().find(|&c| self.is_set(c)).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_stops_are_every_eight_columns() {
        let tabs = TabStops::new(20);
        assert_eq!((tabs.next(0), tabs.next(7), tabs.next(8), tabs.next(16)), (8, 8, 16, 19));
        assert_eq!((tabs.prev(9), tabs.prev(8), tabs.prev(3)), (8, 0, 0));
    }

    #[test]
    fn set_clear_and_reset() {
        let mut tabs = TabStops::new(20);
        tabs.set(3);
        tabs.clear(8);
        assert_eq!(tabs.next(0), 3);
        assert_eq!(tabs.next(3), 16);
        tabs.clear_all();
        assert_eq!(tabs.next(0), 19);
        tabs.reset();
        assert_eq!(tabs.next(0), 8);
        tabs.set(99);
        tabs.clear(99);
    }

    #[test]
    fn growing_adds_default_stops_and_keeps_custom_ones() {
        let mut tabs = TabStops::new(10);
        tabs.clear(8);
        tabs.resize(20);
        assert!(!tabs.is_set(8));
        assert!(tabs.is_set(16));
    }
}
