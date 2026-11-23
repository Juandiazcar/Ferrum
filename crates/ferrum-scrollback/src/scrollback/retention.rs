//! Sealing, spilling and eviction: how history stays bounded.

use super::{Place, Scrollback, ScrollbackEvent, SealedChunk, SpillDisableCause, SpillState};
use crate::error::{ScrollbackError, SpillError};
use crate::spill::SpillHandle;

impl Scrollback {
    /// Seals the full hot chunk, then moves cold chunks out of RAM if the threshold is crossed.
    pub(super) fn seal_hot(&mut self) -> Result<(), ScrollbackError> {
        let blob = self.sealer.seal(&self.hot)?;
        let lines = self.hot.len();
        let id = self.next_chunk_id;
        self.next_chunk_id += 1;
        if self.chunks.is_empty() {
            // The hot lines already dropped become the skipped head of the first sealed chunk.
            self.head_skip = std::mem::take(&mut self.hot_skip);
        }
        self.chunks.push_back(SealedChunk { id, lines, place: Place::Ram(blob) });
        self.ram_sealed_lines += lines;
        self.hot.clear();
        self.enforce_ram_limit();
        Ok(())
    }

    /// Lines held in RAM: sealed chunks plus the hot chunk.
    fn ram_lines(&self) -> usize {
        self.ram_sealed_lines + self.hot.len()
    }

    /// Most lines retained under the current spill state.
    fn effective_max_lines(&self) -> usize {
        match self.spill {
            SpillState::Active => self.config.max_lines(),
            SpillState::Unavailable | SpillState::Disabled => {
                self.config.max_lines().min(self.config.spill_threshold_lines())
            }
        }
    }

    fn enforce_ram_limit(&mut self) {
        while self.spill == SpillState::Active
            && self.ram_lines() > self.config.spill_threshold_lines()
            && self.spilled_chunks < self.chunks.len()
        {
            self.spill_next_chunk();
        }
    }

    /// Spills the oldest RAM chunk. Makes progress every call: a chunk moves, the oldest spilled
    /// chunk is dropped to make room (disk cap, `docs/10` §3 item 11), or spilling is disabled.
    fn spill_next_chunk(&mut self) {
        let index = self.spilled_chunks;
        let outcome = match (self.store.as_mut(), self.chunks.get(index)) {
            (Some(store), Some(SealedChunk { place: Place::Ram(blob), .. })) => store.append(blob),
            _ => return self.disable_spill(SpillDisableCause::Io),
        };
        match outcome {
            Ok(handle) => self.mark_spilled(index, handle),
            Err(SpillError::Full { .. }) if self.spilled_chunks > 0 => self.drop_oldest_chunk(),
            Err(SpillError::Full { .. }) => self.disable_spill(SpillDisableCause::RecordTooLarge),
            Err(error) => {
                tracing::warn!(%error, "scrollback spill write failed");
                self.disable_spill(SpillDisableCause::Io);
            }
        }
    }

    fn mark_spilled(&mut self, index: usize, handle: SpillHandle) {
        if let Some(chunk) = self.chunks.get_mut(index) {
            chunk.place = Place::Spilled(handle);
            self.ram_sealed_lines -= chunk.lines;
            self.spilled_chunks += 1;
        }
    }

    fn disable_spill(&mut self, cause: SpillDisableCause) {
        tracing::warn!(
            ?cause,
            "scrollback spill disabled for this session; oldest lines are dropped"
        );
        self.spill = SpillState::Disabled;
        self.event = Some(ScrollbackEvent::SpillDisabled(cause));
    }

    /// Drops lines, oldest first, until the retention limit holds.
    pub(super) fn evict(&mut self) {
        let limit = self.effective_max_lines();
        while self.retained > limit {
            self.drop_front_line();
        }
    }

    /// Drops the oldest retained line; does nothing when there is none.
    pub(super) fn drop_front_line(&mut self) {
        let Some(retained) = self.retained.checked_sub(1) else { return };
        self.retained = retained;
        self.dropped_lines += 1;
        let Some(front) = self.chunks.front() else {
            self.hot_skip += 1;
            return;
        };
        self.head_skip += 1;
        if self.head_skip >= front.lines {
            self.pop_front_chunk();
        }
    }

    /// Drops the whole oldest chunk (the rest of it counts as dropped).
    fn drop_oldest_chunk(&mut self) {
        let Some(front) = self.chunks.front() else { return };
        let remaining = front.lines.saturating_sub(self.head_skip);
        self.retained -= remaining;
        self.dropped_lines += u64::try_from(remaining).unwrap_or(u64::MAX);
        self.pop_front_chunk();
    }

    /// Removes the oldest chunk and releases its storage. Line counters are the caller's job.
    fn pop_front_chunk(&mut self) {
        let Some(chunk) = self.chunks.pop_front() else { return };
        self.head_skip = 0;
        match chunk.place {
            Place::Ram(_) => self.ram_sealed_lines -= chunk.lines,
            Place::Spilled(_) => {
                self.spilled_chunks -= 1;
                if let Some(Err(error)) = self.store.as_mut().map(|store| store.discard_oldest()) {
                    tracing::warn!(%error, "scrollback spill discard failed");
                }
            }
        }
    }
}
