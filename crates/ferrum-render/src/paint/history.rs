//! The row hashes of the last frames, so that a buffer of age `n` (its pixels are those of the
//! frame painted `n` frames ago, `softbuffer`'s `Buffer::age`) is brought up to date by
//! repainting only the rows whose hash differs from that frame's (ADR 0025 consequences).

/// Frames remembered. `softbuffer` reported ages 0, 1 and 2 on the four backends measured in
/// spike S3 (ADR 0025); one more keeps a triple-buffered backend incremental. An older buffer is
/// repainted in full.
pub(crate) const MAX_BUFFER_AGE: u8 = 3;

const FRAMES: usize = MAX_BUFFER_AGE as usize;

/// Row hashes of the last [`MAX_BUFFER_AGE`] frames, in a ring.
#[derive(Debug, Default)]
pub(crate) struct History {
    frames: [Vec<u64>; FRAMES],
    /// The hashes of the frame being painted (swapped into the ring when it is done).
    current: Vec<u64>,
    /// Index the next frame goes to.
    next: usize,
    /// Frames painted since the last reset (layout change), capped at the ring size.
    painted: usize,
}

impl History {
    /// Forgets every frame (the layout changed) and sizes the buffers for `rows` rows.
    pub(crate) fn reset(&mut self, rows: usize) {
        for frame in self.frames.iter_mut().chain(std::iter::once(&mut self.current)) {
            frame.clear();
            frame.resize(rows, 0);
        }
        self.painted = 0;
    }

    /// The hashes of the frame painted `age` frames ago, if remembered.
    pub(crate) fn frame(&self, age: u8) -> Option<&[u64]> {
        let age = usize::from(age);
        if age == 0 || age > self.painted {
            return None;
        }
        self.frames.get((self.next + FRAMES - age) % FRAMES).map(Vec::as_slice)
    }

    /// Records `hash` for row `index` of the current frame; whether it differs from that row in
    /// the frame `age` back (always, when `age` is `None` or not remembered).
    pub(crate) fn record(&mut self, age: Option<u8>, index: usize, hash: u64) -> bool {
        if let Some(slot) = self.current.get_mut(index) {
            *slot = hash;
        }
        let previous = age.and_then(|age| self.frame(age)).and_then(|frame| frame.get(index));
        previous != Some(&hash)
    }

    /// Records [`History::current`] as the newest frame; its buffer is reused for the next one.
    pub(crate) fn commit(&mut self) {
        if let Some(slot) = self.frames.get_mut(self.next) {
            std::mem::swap(slot, &mut self.current);
        }
        self.next = (self.next + 1) % FRAMES;
        self.painted = (self.painted + 1).min(FRAMES);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_buffer_of_age_n_compares_with_the_frame_n_back() {
        let mut history = History::default();
        history.reset(1);
        assert_eq!(history.frame(1), None, "nothing painted yet");
        for hash in 1..=5 {
            let _changed = history.record(None, 0, hash);
            history.commit();
        }
        assert!(!history.record(Some(1), 0, 5), "same as one frame back");
        assert!(history.record(Some(2), 0, 5), "differs from two frames back");
        assert_eq!(history.frame(1), Some(&[5][..]));
        assert_eq!(history.frame(2), Some(&[4][..]));
        assert_eq!(history.frame(3), Some(&[3][..]));
        assert_eq!(history.frame(4), None, "older than the ring");
        assert_eq!(history.frame(0), None, "age 0: contents undefined");
        history.reset(1);
        assert_eq!(history.frame(1), None);
    }
}
