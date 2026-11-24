//! Input to a session's child, written by the I/O thread without blocking (`docs/02` §5: one I/O
//! thread; `docs/11` §5).
//!
//! Bytes go straight to the PTY with [`PtyWriter::try_write`]; what the child does not take now
//! waits in a bounded buffer and is written when the PTY reports room (its descriptor becomes
//! writable on Unix, a notification on Windows). When the buffer is full, further input for that
//! session is dropped (the child is not reading), so one stuck child can neither stall the loop
//! nor grow the daemon (FR-SESS-009).

use ferrum_pty::{Notify, PtyError, PtyWriter};

/// Bytes waiting for a child that does not read: one 4 MiB paste (the frame limit) fits.
const QUEUE_BYTES: usize = 4 * 1_024 * 1_024;
/// Capacity an empty buffer keeps (`docs/03` §3 rule 7: a paste does not keep its buffer).
const KEEP_BYTES: usize = 4 * 1_024;

/// Why input was not taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputRefused {
    /// The child is not reading and the buffer is full.
    Full,
    /// The session is closed.
    Closed,
}

/// The input side of a session.
pub(crate) struct Input {
    writer: Box<dyn PtyWriter>,
    pending: Vec<u8>,
    written: usize,
}

impl Input {
    pub(crate) fn new(writer: Box<dyn PtyWriter>) -> Self {
        Self { writer, pending: Vec::new(), written: 0 }
    }

    /// For a notifying writer: `notify` runs when room frees up.
    pub(crate) fn set_notify(&mut self, notify: Notify) {
        self.writer.set_notify(notify);
    }

    /// Bytes are waiting for room in the PTY.
    pub(crate) fn is_waiting(&self) -> bool {
        self.written < self.pending.len()
    }

    /// Writes `bytes` now, keeping what the child does not take yet.
    pub(crate) fn send(&mut self, bytes: &[u8]) -> Result<(), InputRefused> {
        if bytes.is_empty() {
            return Ok(());
        }
        let mut rest = bytes;
        if !self.is_waiting() {
            rest = rest.get(write_some(&mut *self.writer, rest)?..).unwrap_or_default();
        }
        if rest.is_empty() {
            return Ok(());
        }
        // The written prefix goes first, so the buffer itself never holds more than the limit.
        self.pending.drain(..self.written);
        self.written = 0;
        if self.pending.len() + rest.len() > QUEUE_BYTES {
            return Err(InputRefused::Full);
        }
        self.pending.extend_from_slice(rest);
        Ok(())
    }

    /// Writes waiting bytes until the PTY is full.
    pub(crate) fn flush(&mut self) -> Result<(), InputRefused> {
        while self.is_waiting() {
            let rest = self.pending.get(self.written..).unwrap_or_default();
            let written = write_some(&mut *self.writer, rest)?;
            if written == 0 {
                return Ok(());
            }
            self.written += written;
        }
        self.pending.clear();
        self.written = 0;
        if self.pending.capacity() > KEEP_BYTES {
            self.pending.shrink_to(KEEP_BYTES);
        }
        Ok(())
    }
}

/// Bytes the PTY took (0 when it is full).
fn write_some(writer: &mut dyn PtyWriter, bytes: &[u8]) -> Result<usize, InputRefused> {
    match writer.try_write(bytes) {
        Ok(written) => Ok(written.unwrap_or(0)),
        Err(PtyError::Closed) => Err(InputRefused::Closed),
        Err(error) => {
            tracing::warn!(%error, "writing to a session failed");
            Err(InputRefused::Closed)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, PoisonError};

    use super::*;

    /// A PTY that takes at most `room` bytes until refilled; what it took is shared.
    #[derive(Default)]
    struct Pipe {
        room: usize,
        got: Vec<u8>,
    }

    struct Slow(Arc<Mutex<Pipe>>);

    impl PtyWriter for Slow {
        fn write_all(&mut self, _bytes: &[u8]) -> Result<(), PtyError> {
            Err(PtyError::Closed)
        }

        fn try_write(&mut self, bytes: &[u8]) -> Result<Option<usize>, PtyError> {
            let mut pipe = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            let taken = bytes.len().min(pipe.room);
            pipe.room -= taken;
            pipe.got.extend_from_slice(bytes.get(..taken).unwrap_or_default());
            Ok((taken > 0).then_some(taken))
        }
    }

    #[test]
    fn the_buffer_of_a_slow_child_never_holds_more_than_the_limit() {
        let pipe = Arc::new(Mutex::new(Pipe { room: 0, got: Vec::new() }));
        let mut input = Input::new(Box::new(Slow(Arc::clone(&pipe))));
        let chunk = vec![b'x'; QUEUE_BYTES / 4];
        for _ in 0..64 {
            // The child takes a little now and then; the sender keeps pasting.
            pipe.lock().unwrap_or_else(PoisonError::into_inner).room += chunk.len() / 3;
            assert_eq!(input.flush(), Ok(()));
            let _refused = input.send(&chunk);
            assert!(input.pending.len() <= QUEUE_BYTES, "{} bytes held", input.pending.len());
        }
    }

    #[test]
    fn input_waits_for_room_in_order_and_is_bounded() {
        let pipe = Arc::new(Mutex::new(Pipe { room: 3, got: Vec::new() }));
        let mut input = Input::new(Box::new(Slow(Arc::clone(&pipe))));
        assert_eq!(input.send(b"hello"), Ok(()));
        assert!(input.is_waiting());
        assert_eq!(input.send(b" world"), Ok(()));
        assert_eq!(input.send(&vec![b'x'; QUEUE_BYTES]), Err(InputRefused::Full));
        assert_eq!(input.flush(), Ok(()));
        assert!(input.is_waiting(), "no room yet");
        pipe.lock().unwrap_or_else(PoisonError::into_inner).room = usize::MAX;
        assert_eq!(input.flush(), Ok(()));
        assert!(!input.is_waiting());
        assert_eq!(pipe.lock().unwrap_or_else(PoisonError::into_inner).got, b"hello world");
    }
}
