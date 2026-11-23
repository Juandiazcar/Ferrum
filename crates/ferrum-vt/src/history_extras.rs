//! Which extra ids the retained history may use, known without reading it back (ADR 0028
//! decision 3).
//!
//! Every line pushed into the scrollback goes through [`HistoryExtras::push`], which ORs the ids
//! of its runs into the bitset of its block of [`LINES_PER_BLOCK_USIZE`] pushed lines. The retained
//! history is always the pushed lines `[pushed - len, pushed)` (lines only leave from the front:
//! eviction, `pop_front`, `skip_front`, `clear`), so after each push the blocks entirely below the
//! oldest retained line are dropped. The sweep marks the union of the blocks. That is
//! conservative (an evicted line keeps its ids alive while its block holds a retained line) and
//! never short: every retained line was recorded in a block that is still there.

use std::collections::VecDeque;

use ferrum_scrollback::{LineRef, Scrollback, ScrollbackError};

use crate::cell::extra_of;
use crate::color::EXTRA_MASK;
use crate::extras::ExtraId;
use crate::slot_set::{SET_WORDS, SlotSet};

/// Pushed lines summarized by one block: the scrollback chunk size (`HOT_CHUNK_LINES`), so a
/// block goes about when its chunk is evicted. A history of 10 000 lines holds at most 11 blocks of
/// 512 bytes (5 632 bytes) plus the 512-byte union.
const LINES_PER_BLOCK_USIZE: usize = ferrum_scrollback::HOT_CHUNK_LINES;

/// [`LINES_PER_BLOCK_USIZE`] for pushed-line numbers (a `usize` always fits a `u64` here).
const LINES_PER_BLOCK: u64 = LINES_PER_BLOCK_USIZE as u64;
/// Blocks the ring first grows by (the `VecDeque` default for elements of this size).
const MIN_GROWTH_BLOCKS: usize = 4;

/// One bit per extra id, the layout of [`SlotSet`].
type Bits = [u64; SET_WORDS];

/// Per-block summaries of the extra ids of the retained history.
#[derive(Debug)]
pub(crate) struct HistoryExtras {
    /// Block `first + i` covers the pushed lines `[(first + i) * LINES_PER_BLOCK, ..)`. Dropped
    /// blocks leave their room in the ring, so steady state allocates nothing.
    blocks: VecDeque<Bits>,
    first: u64,
    /// OR of every block; recomputed at the next sweep after a block was dropped (`stale`).
    union: Bits,
    stale: bool,
}

impl Default for HistoryExtras {
    fn default() -> Self {
        Self { blocks: VecDeque::new(), first: 0, union: [0; SET_WORDS], stale: false }
    }
}

impl HistoryExtras {
    /// Pushes `line` into `scrollback`: the single entry of every history line (scrolling and
    /// reflow). The ids are recorded first, so a line the scrollback refuses is only counted
    /// conservatively.
    ///
    /// # Errors
    /// The scrollback's refusal of the line, unchanged.
    pub(crate) fn push(
        &mut self,
        scrollback: &mut Scrollback,
        line: &LineRef<'_>,
    ) -> Result<(), ScrollbackError> {
        if line.runs.iter().any(|run| (run.style.fg | run.style.bg) & EXTRA_MASK != 0) {
            let bound = blocks_for(scrollback.config().max_lines());
            self.record(scrollback.pushed_lines(), line, bound);
        }
        let pushed = scrollback.push_line(line);
        // Nothing to drop while no block is held: a history without links or underline colors
        // pays one test per run and this one.
        if !self.blocks.is_empty() {
            self.prune(scrollback);
        }
        pushed
    }

    /// ORs the ids of `line`, pushed as line `number`, into its block.
    fn record(&mut self, number: u64, line: &LineRef<'_>, bound: usize) {
        let index = self.block_index(number / LINES_PER_BLOCK, bound);
        for run in line.runs {
            let id = extra_of(run.style.fg, run.style.bg);
            if id == ExtraId::NONE {
                continue;
            }
            let slot = usize::from(id.bits());
            set_bit(&mut self.union, slot);
            if let Some(block) = self.blocks.get_mut(index) {
                set_bit(block, slot);
            }
        }
    }

    /// The position of block `block`, adding empty blocks up to it. After a prune the front block
    /// ends above the oldest retained line, so at most `bound` blocks are ever held, and the ring
    /// grows by doubling up to exactly `bound`.
    fn block_index(&mut self, block: u64, bound: usize) -> usize {
        if self.blocks.is_empty() {
            self.first = block;
        }
        // A block below `first` cannot happen (line numbers only grow); it would go to the front
        // block, which is kept at least as long.
        let index = usize::try_from(block.saturating_sub(self.first)).unwrap_or(usize::MAX);
        while self.blocks.len() <= index {
            if self.blocks.len() == self.blocks.capacity() {
                let room = self.blocks.capacity().max(MIN_GROWTH_BLOCKS);
                self.blocks.reserve_exact(room.min(bound.saturating_sub(self.blocks.len()).max(1)));
            }
            self.blocks.push_back([0; SET_WORDS]);
        }
        index
    }

    /// Drops the blocks that hold no retained line any more. Call after any change to the front
    /// of the history (`CSI 3 J`); [`push`](Self::push) calls it itself.
    pub(crate) fn prune(&mut self, scrollback: &Scrollback) {
        if scrollback.is_empty() {
            self.stale |= !self.blocks.is_empty();
            self.blocks.clear();
            return;
        }
        let retained = u64::try_from(scrollback.len()).unwrap_or(u64::MAX);
        let oldest = scrollback.pushed_lines().saturating_sub(retained);
        while self.first < oldest / LINES_PER_BLOCK && self.blocks.pop_front().is_some() {
            self.first += 1;
            self.stale = true;
        }
    }

    /// Marks into `marks` every id a retained line may use. Returns the words of work done.
    pub(crate) fn mark(&mut self, marks: &mut SlotSet) -> usize {
        let mut work = SET_WORDS;
        if self.stale {
            self.union = [0; SET_WORDS];
            for block in &self.blocks {
                for (word, bits) in self.union.iter_mut().zip(block) {
                    *word |= bits;
                }
            }
            work += self.blocks.len() * SET_WORDS;
            self.stale = false;
        }
        marks.insert_all(&self.union);
        work
    }

    /// Heap bytes held, by capacity (the union lives inline).
    pub(crate) fn capacity_bytes(&self) -> usize {
        self.blocks.capacity() * size_of::<Bits>()
    }
}

/// Most blocks a history of `max_lines` retained lines can touch: its span may start inside one
/// block and end inside another.
fn blocks_for(max_lines: usize) -> usize {
    max_lines.div_ceil(LINES_PER_BLOCK_USIZE) + 1
}

fn set_bit(bits: &mut Bits, slot: usize) {
    let width = u64::BITS as usize;
    if let Some(word) = bits.get_mut(slot / width) {
        *word |= 1 << (slot % width);
    }
}

#[cfg(test)]
mod tests {
    use ferrum_scrollback::{LineFlags, ScrollbackConfig, Style, StyledRun};

    use super::*;
    use crate::cell::Cell;

    fn line_with(id: u16, runs: &mut Vec<StyledRun>) -> LineRef<'_> {
        let mut cell = Cell::default();
        cell.set_extra(ExtraId::from_bits(id));
        let (fg, bg) = (cell.fg_word(), cell.bg_word());
        runs.clear();
        runs.push(StyledRun { style: Style { fg, bg, attrs: 0 }, len: 1 });
        LineRef { flags: LineFlags::NONE, text: "x", runs }
    }

    fn marked(summary: &mut HistoryExtras, id: usize) -> bool {
        let mut marks = SlotSet::new();
        summary.mark(&mut marks);
        marks.contains(id)
    }

    #[test]
    fn ids_live_while_their_block_holds_a_retained_line_and_go_with_it() {
        let config = ScrollbackConfig::new(2_000, 50_000).unwrap_or_default();
        let mut scrollback = Scrollback::new(config);
        let mut summary = HistoryExtras::default();
        let mut runs = Vec::new();
        assert!(summary.push(&mut scrollback, &line_with(7, &mut runs)).is_ok());
        let plain = LineRef { flags: LineFlags::NONE, text: "x", runs: &[] };
        for _ in 0..2_000 {
            assert!(summary.push(&mut scrollback, &plain).is_ok());
        }
        // Line 0 is evicted, but block 0 still holds retained lines.
        assert!(marked(&mut summary, 7));
        for _ in 0..1_100 {
            assert!(summary.push(&mut scrollback, &plain).is_ok());
        }
        assert!(!marked(&mut summary, 7));
        assert!(summary.push(&mut scrollback, &line_with(9, &mut runs)).is_ok());
        scrollback.clear();
        summary.prune(&scrollback);
        assert!(!marked(&mut summary, 9));
    }

    #[test]
    fn a_full_history_of_ten_thousand_lines_holds_eleven_blocks() {
        let config = ScrollbackConfig::new(10_000, 50_000).unwrap_or_default();
        let mut scrollback = Scrollback::new(config);
        let mut summary = HistoryExtras::default();
        let mut runs = Vec::new();
        for n in 0..30_000_u16 {
            let line = line_with(n % 4_000 + 1, &mut runs);
            assert!(summary.push(&mut scrollback, &line).is_ok());
        }
        // 10 000 retained lines straddle at most 11 blocks; the ring's capacity is the
        // allocator's business, only its length is asserted.
        assert_eq!(summary.blocks.len(), 11);
        assert!(summary.capacity_bytes() >= 11 * 512);
    }
}
