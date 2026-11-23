//! Sealing, spilling and eviction: how history stays bounded.

use super::{Place, Scrollback, ScrollbackEvent, SealedChunk, SpillDisableCause, SpillState};
use crate::error::{ScrollbackError, SpillError};
use crate::spill::SpillHandle;
use crate::style::StyleTable;

impl Scrollback {
    /// Retires the full hot chunk: it becomes a pending chunk for the worker, or, when
    /// `MAX_PENDING_CHUNKS` already wait, it is sealed here (backpressure). Then moves cold chunks
    /// out of RAM if the threshold is crossed. The hot chunk is always retired, so the next line
    /// always has room: a seal that fails keeps the chunk raw and readable as a pending chunk
    /// (over the cap; the worker or the spill frontier seals it later) and the error is returned.
    pub(super) fn seal_hot(&mut self) -> Result<(), ScrollbackError> {
        let lines = self.hot.len();
        let mut failed = Ok(());
        let place = if let Some(place) = self.defer_hot() {
            place
        } else {
            match self.sealer.seal_hot(&self.hot) {
                Ok(blob) => {
                    self.inline_seals += 1;
                    Place::Ram(blob)
                }
                Err(error) => {
                    failed = Err(error);
                    self.keep_hot_raw()
                }
            }
        };
        let id = self.next_chunk_id;
        self.next_chunk_id += 1;
        if self.chunks.is_empty() {
            // The hot lines already dropped become the skipped head of the first sealed chunk.
            self.head_skip = std::mem::take(&mut self.hot_skip);
        }
        self.chunks.push_back(SealedChunk { id, lines, place });
        self.ram_sealed_lines += lines;
        self.hot.clear();
        self.enforce_ram_limit();
        failed
    }

    /// Lines held in RAM: sealed and pending chunks plus the hot chunk.
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

    pub(super) fn enforce_ram_limit(&mut self) {
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
        // A chunk the worker has not sealed yet is sealed here: spilling must not wait for it.
        if let Err(error) = self.seal_in_place(index) {
            tracing::warn!(%error, "scrollback seal before spilling failed");
            return self.disable_spill(SpillDisableCause::Compress);
        }
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
            self.ram_sealed_lines = self.ram_sealed_lines.saturating_sub(chunk.lines);
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

    /// Drops every retained line without decoding it: chunks and spilled records are released
    /// as they are. Lines already pushed still count in [`pushed_lines`](Self::pushed_lines).
    pub fn clear(&mut self) {
        while !self.chunks.is_empty() {
            self.pop_front_chunk();
        }
        self.hot.clear();
        self.head_skip = 0;
        self.hot_skip = 0;
        self.dropped_lines += u64::try_from(self.retained).unwrap_or(u64::MAX);
        self.retained = 0;
        self.styles = StyleTable::new();
    }

    /// Drops the whole oldest chunk (the rest of it counts as dropped).
    fn drop_oldest_chunk(&mut self) {
        let Some(front) = self.chunks.front() else { return };
        let remaining = front.lines.saturating_sub(self.head_skip);
        self.retained = self.retained.saturating_sub(remaining);
        self.dropped_lines += u64::try_from(remaining).unwrap_or(u64::MAX);
        self.pop_front_chunk();
    }

    /// Removes the oldest chunk and releases its storage. Line counters are the caller's job.
    fn pop_front_chunk(&mut self) {
        let Some(chunk) = self.chunks.pop_front() else { return };
        self.head_skip = 0;
        match chunk.place {
            Place::Ram(_) | Place::Pending { .. } => {
                self.ram_sealed_lines = self.ram_sealed_lines.saturating_sub(chunk.lines);
                self.release(chunk.place);
            }
            Place::Spilled(_) => {
                self.spilled_chunks = self.spilled_chunks.saturating_sub(1);
                if let Some(Err(error)) = self.store.as_mut().map(|store| store.discard_oldest()) {
                    tracing::warn!(%error, "scrollback spill discard failed");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

    use super::*;
    use crate::config::ScrollbackConfig;
    use crate::limits::HOT_CHUNK_LINES;
    use crate::line::{LineFlags, LineRef};
    use crate::spill::MemorySpill;

    const BIG_DISK: u64 = 64 * 1_024 * 1_024;

    #[test]
    fn a_line_is_stored_and_counted_when_the_inline_seal_fails() {
        let mut store = Scrollback::new(ScrollbackConfig::default());
        let line = LineRef { flags: LineFlags::NONE, text: "a line", runs: &[] };
        let total = HOT_CHUNK_LINES * (crate::MAX_PENDING_CHUNKS + 2);
        store.sealer.failing_after = Some(0);
        let mut failures = 0;
        for _ in 0..total {
            failures += usize::from(store.push_line(&line).is_err());
        }
        assert!(failures > 0, "the failed seals are reported");
        assert_eq!((store.len(), store.pushed_lines()), (total, total as u64));
        let mut cursor = crate::LineCursor::new();
        for index in [0, HOT_CHUNK_LINES, total - 1] {
            let read = store.read_line(index, &mut cursor).expect("read").expect("line");
            assert_eq!(read.text, "a line");
        }
    }

    #[test]
    fn a_failed_seal_before_spilling_disables_the_spill_as_a_compression_failure() {
        let config = ScrollbackConfig::new(HOT_CHUNK_LINES * 4, HOT_CHUNK_LINES).expect("config");
        let mut store = Scrollback::with_spill(config, Box::new(MemorySpill::new(BIG_DISK)));
        let line = LineRef { flags: LineFlags::NONE, text: "a line", runs: &[] };
        for _ in 0..HOT_CHUNK_LINES {
            store.push_line(&line).expect("push");
        }
        let _handed = store.take_pending().expect("full chunk");
        // The next full chunk waits too (or, with a single slot, is sealed inline), then spilling
        // reaches the first pending chunk and must seal it: that seal fails.
        store.sealer.failing_after = Some(usize::from(crate::MAX_PENDING_CHUNKS < 2));
        for _ in 0..HOT_CHUNK_LINES {
            store.push_line(&line).expect("push");
        }
        assert_eq!(store.spill_state(), SpillState::Disabled);
        let event = Some(ScrollbackEvent::SpillDisabled(SpillDisableCause::Compress));
        assert_eq!(store.take_event(), event);
    }
}
