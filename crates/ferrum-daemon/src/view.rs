//! What one client shows of one session: the rows that changed since this client's last frame,
//! the scroll it has not seen, the rows as it last got them (one hash each), and the snapshot or
//! history rows it asked for (`docs/02` §4: only visible sessions get `Damage`; damage is
//! coalesced per frame interval and a slow client gets the latest state, not a queue of frames).
//!
//! The terminal keeps one damage set; every frame it is merged into each view of the session and
//! cleared, so two clients at different speeds each get exactly the rows they missed. A scroll
//! marks every row of its region; comparing the row hashes with the client's rows shifted by the
//! scrolled lines finds the rows the scroll already brings, which are not sent again
//! (`Damage.scroll_delta`).

use ferrum_proto::{Cursor, TerminalModes};
use ferrum_vt::Damage;

const WORD_BITS: usize = u64::BITS as usize;

/// Rows a view still has to send, and lines scrolled since its last frame.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Dirty {
    words: Vec<u64>,
    scrolled: usize,
}

impl Dirty {
    /// Adds the rows of `damage`.
    pub(crate) fn merge(&mut self, damage: &Damage, rows: usize) {
        let scrolled = usize::try_from(damage.scrolled_lines()).unwrap_or(usize::MAX);
        self.scrolled = self.scrolled.saturating_add(scrolled);
        let words = rows.div_ceil(WORD_BITS);
        if self.words.len() < words {
            self.words.resize(words, 0); // grows once per size, then reused
        }
        for row in damage.dirty_rows() {
            if let Some(word) = self.words.get_mut(row / WORD_BITS) {
                *word |= 1 << (row % WORD_BITS);
            }
        }
    }

    /// Whether row `row` must be looked at.
    pub(crate) fn is_dirty(&self, row: usize) -> bool {
        self.words.get(row / WORD_BITS).is_some_and(|word| word >> (row % WORD_BITS) & 1 == 1)
    }

    /// Lines scrolled since the last frame (both directions add up).
    pub(crate) fn scrolled(&self) -> usize {
        self.scrolled
    }

    pub(crate) fn is_clean(&self) -> bool {
        self.scrolled == 0 && self.words.iter().all(|&word| word == 0)
    }

    pub(crate) fn clear(&mut self) {
        self.scrolled = 0;
        self.words.fill(0);
    }
}

/// One session as one client shows it.
#[derive(Debug, Default)]
pub struct View {
    pub(crate) dirty: Dirty,
    /// A hash of each screen row as the client has it; empty until its first snapshot.
    pub(crate) hashes: Vec<u64>,
    /// What the client last got, so unchanged values are not resent.
    pub(crate) cursor: Option<Cursor>,
    pub(crate) modes: Option<TerminalModes>,
    /// A snapshot is owed, with this many history rows.
    pub(crate) snapshot: Option<u16>,
    /// History rows still owed (0 is the row just above the screen), sent a step at a time while
    /// the client keeps up (`docs/02` §4: nothing unbounded is queued for a client).
    pub(crate) history: Option<std::ops::Range<u32>>,
}

impl View {
    /// A view that owes a snapshot with `history` history rows.
    pub fn attached(history: u16) -> Self {
        Self { snapshot: Some(history), ..Self::default() }
    }

    /// Adds a frame's damage of a screen of `rows` rows.
    pub fn merge(&mut self, damage: &Damage, rows: usize) {
        self.dirty.merge(damage, rows);
    }
}

/// The scroll that explains the most rows of `now` from the client's rows `before`, among no
/// scroll and `scrolled` lines up or down; returned as the protocol's `scroll_delta` (positive:
/// up).
pub(crate) fn best_shift(before: &[u64], now: &[u64], scrolled: usize) -> i32 {
    let lines = now.len();
    if scrolled == 0 || before.len() != lines {
        return 0;
    }
    let shift = i32::try_from(scrolled.min(lines.saturating_sub(1))).unwrap_or(0);
    [0, shift, -shift]
        .into_iter()
        .max_by_key(|&delta| {
            (0..lines).filter(|&y| predicted(before, y, delta) == now.get(y)).count()
        })
        .unwrap_or(0)
}

/// The client's row at `y` after it applied `delta`; `None` for a row the scroll blanked.
pub(crate) fn predicted(before: &[u64], y: usize, delta: i32) -> Option<&u64> {
    let from = i64::try_from(y).ok()? + i64::from(delta);
    before.get(usize::try_from(from).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_collect_dirty_rows_and_scroll_until_sent() {
        let mut damage = Damage::new(4);
        damage.take();
        damage.mark(1);
        damage.mark(3);
        let mut dirty = Dirty::default();
        dirty.merge(&damage, 4);
        assert_eq!((0..4).filter(|&row| dirty.is_dirty(row)).collect::<Vec<_>>(), [1, 3]);
        dirty.clear();
        assert!(dirty.is_clean());
        damage.add_scroll(1);
        dirty.merge(&damage, 4);
        assert_eq!(dirty.scrolled(), 1);
    }

    #[test]
    fn the_shift_that_explains_the_rows_is_found() {
        let before = [1, 2, 3, 4];
        assert_eq!(best_shift(&before, &[2, 3, 4, 9], 1), 1, "scrolled up one line");
        assert_eq!(best_shift(&before, &[9, 1, 2, 3], 1), -1, "scrolled down one line");
        assert_eq!(best_shift(&before, &[1, 2, 3, 9], 1), 0, "a region below kept its place");
        assert_eq!(predicted(&before, 3, 1), None, "the bottom row is new after a scroll up");
    }
}
