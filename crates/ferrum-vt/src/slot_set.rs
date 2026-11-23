//! A fixed bitset over small table slots (grapheme clusters, hyperlink and style records).
//!
//! Compacting or sweeping a table needs "which slots are in use" without allocating, so the set
//! lives on the stack: 4 096 bits are 512 bytes.

use crate::limits::{MAX_EXTRAS, MAX_GRAPHEMES};

/// Slots a set can hold: enough for every table that uses it.
const SLOT_CAPACITY: usize = 4_096;
const _: () = assert!(MAX_GRAPHEMES <= SLOT_CAPACITY && MAX_EXTRAS < SLOT_CAPACITY);

/// Words of a [`SlotSet`].
pub(crate) const SET_WORDS: usize = SLOT_CAPACITY.div_ceil(u64::BITS as usize);

/// Which slots something uses: a fixed bitset, so that compaction never allocates.
pub(crate) struct SlotSet {
    words: [u64; SET_WORDS],
    /// Set bits in the words before each word, valid after [`seal`](Self::seal).
    before: [u16; SET_WORDS],
}

impl SlotSet {
    pub(crate) const fn new() -> Self {
        Self { words: [0; SET_WORDS], before: [0; SET_WORDS] }
    }

    /// Marks `slot` as used; false when it is outside the table's range.
    pub(crate) fn insert(&mut self, slot: usize) -> bool {
        let bit = u64::BITS as usize;
        match self.words.get_mut(slot / bit) {
            Some(word) => {
                *word |= 1 << (slot % bit);
                true
            }
            None => false,
        }
    }

    /// Marks every slot set in `words` (a bitset of the same layout).
    pub(crate) fn insert_all(&mut self, words: &[u64; SET_WORDS]) {
        for (word, other) in self.words.iter_mut().zip(words) {
            *word |= other;
        }
    }

    pub(crate) fn contains(&self, slot: usize) -> bool {
        let bit = u64::BITS as usize;
        self.words.get(slot / bit).is_some_and(|word| word >> (slot % bit) & 1 == 1)
    }

    /// Computes the prefix counts that [`rank`](Self::rank) needs; call after the last insert.
    pub(crate) fn seal(&mut self) {
        let mut total = 0u16;
        for (word, before) in self.words.iter().zip(self.before.iter_mut()) {
            *before = total;
            // INVARIANT: at most MAX_GRAPHEMES (4 096) bits are set, which fits in u16.
            total = total.saturating_add(u16::try_from(word.count_ones()).unwrap_or(0));
        }
    }

    /// Number of used slots below `slot`: the new index of `slot` after compaction.
    pub(crate) fn rank(&self, slot: usize) -> u32 {
        let bit = u64::BITS as usize;
        let below = self
            .words
            .get(slot / bit)
            .map_or(0, |word| (word & ((1u64 << (slot % bit)) - 1)).count_ones());
        u32::from(self.before.get(slot / bit).copied().unwrap_or(0)) + below
    }

    /// Number of used slots.
    pub(crate) fn count(&self) -> usize {
        self.words.iter().map(|word| word.count_ones() as usize).sum()
    }
}
