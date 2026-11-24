//! The `ConPTY` reader, which answers the startup cursor query (ADR 0026 W1, `docs/11` §4 point 1).
//!
//! With `PSEUDOCONSOLE_INHERIT_CURSOR` the first bytes of the output are `ESC[6n` and nothing else
//! arrives until the terminal answers. The reader reads those bytes alone (never past them),
//! writes `ESC[1;1R` (a new session's cursor is at row 1, column 1) and passes on everything after
//! them. Output that does not start with the query is passed on untouched.

use std::io::{PipeReader, PipeWriter, Read, Write};

use crate::{PtyError, PtyReader, Readiness};

/// The cursor position query the pseudoconsole sends first.
const CURSOR_QUERY: &[u8] = b"\x1b[6n";
/// The answer for a new session: the cursor is at row 1, column 1.
const CURSOR_REPLY: &[u8] = b"\x1b[1;1R";

/// Reads the pseudoconsole's output pipe.
pub(super) struct ConptyReader {
    pipe: PipeReader,
    /// Where the answer goes; `None` once the startup is over.
    reply: Option<PipeWriter>,
    /// Bytes read during the startup that turned out not to be the query, still to be returned.
    carry: Vec<u8>,
}

impl ConptyReader {
    pub(super) fn new(pipe: PipeReader, reply: PipeWriter) -> Self {
        Self { pipe, reply: Some(reply), carry: Vec::new() }
    }

    /// Reads up to the length of the query, answers it if that is what arrived, and keeps any
    /// other bytes in `carry`.
    fn startup(&mut self, reply: &mut PipeWriter) -> Result<(), PtyError> {
        let mut seen = Vec::with_capacity(CURSOR_QUERY.len());
        while CURSOR_QUERY.starts_with(&seen) && seen.len() < CURSOR_QUERY.len() {
            let mut byte = [0u8; 1];
            match self.pipe.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => seen.extend_from_slice(&byte),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(PtyError::io("read the pseudoconsole", error)),
            }
        }
        if seen == CURSOR_QUERY {
            // The session may already be closed; the output then simply ends.
            let _answered = reply.write_all(CURSOR_REPLY);
        } else {
            self.carry = seen;
        }
        Ok(())
    }
}

impl PtyReader for ConptyReader {
    fn readiness(&self) -> Readiness {
        Readiness::Blocking
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PtyError> {
        if let Some(mut reply) = self.reply.take() {
            self.startup(&mut reply)?;
        }
        if !self.carry.is_empty() {
            let count = buf.len().min(self.carry.len());
            for (slot, byte) in buf.iter_mut().zip(self.carry.drain(..count)) {
                *slot = byte;
            }
            return Ok(count);
        }
        // `std` reports the broken pipe after the console closed as `Ok(0)`.
        self.pipe.read(buf).map_err(|error| PtyError::io("read the pseudoconsole", error))
    }
}
