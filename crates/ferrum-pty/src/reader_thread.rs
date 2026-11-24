//! The blocking reader thread used with [`Readiness::Blocking`](crate::Readiness::Blocking)
//! readers (`docs/11` §4, ADR 0026 W5): one named thread per session reads into a stack buffer and
//! hands each chunk to a [`ChunkSink`], which forwards it (bounded channel plus `mio::Waker` in the
//! daemon).

use std::ops::ControlFlow;
use std::thread::JoinHandle;

use crate::{PtyError, PtyReader};

/// Stack size of a reader thread (`docs/11` §4: 256 KiB).
pub const READER_STACK_BYTES: usize = 256 * 1024;

/// Size of the read buffer, on the reader thread's stack (`docs/02` §5: chunks of at most 64 KiB).
pub const READ_BUFFER_BYTES: usize = 64 * 1024;

/// Receives the output of a reader thread.
pub trait ChunkSink: Send {
    /// Called with each chunk read, in order. `Break` stops the thread.
    fn chunk(&mut self, bytes: &[u8]) -> ControlFlow<()>;

    /// Called once when the thread stops: `Ok` at the end of the output or after `Break`, `Err`
    /// when a read failed.
    fn end(&mut self, result: Result<(), PtyError>);
}

/// Starts the reader thread of session `id`, named `pty-read-<id>`, with a
/// [`READER_STACK_BYTES`] stack. The caller owns the join handle; the thread ends when the reader
/// reaches the end of the output (after `close`), fails, or the sink returns `Break`.
///
/// # Errors
/// [`PtyError::Io`] when the thread cannot be created.
pub fn spawn_reader_thread<S: ChunkSink + 'static>(
    id: u64,
    mut reader: Box<dyn PtyReader>,
    mut sink: S,
) -> Result<JoinHandle<()>, PtyError> {
    std::thread::Builder::new()
        .name(format!("pty-read-{id}"))
        .stack_size(READER_STACK_BYTES)
        .spawn(move || pump(reader.as_mut(), &mut sink))
        .map_err(|source| PtyError::io("spawn the reader thread", source))
}

#[expect(
    clippy::large_stack_arrays,
    reason = "docs/11 §4 and PT-24: the 64 KiB buffer lives on the 256 KiB reader stack, no heap"
)]
fn pump(reader: &mut dyn PtyReader, sink: &mut dyn ChunkSink) {
    let mut buf = [0u8; READ_BUFFER_BYTES];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => return sink.end(Ok(())),
            Ok(read) => {
                // A reader that reports more than the buffer breaks its contract: stop, never
                // forward bytes that were not read.
                let Some(chunk) = buf.get(..read) else {
                    let broken = std::io::Error::other("read reported more bytes than the buffer");
                    return sink.end(Err(PtyError::io("read the terminal", broken)));
                };
                if sink.chunk(chunk).is_break() {
                    return sink.end(Ok(()));
                }
            }
            Err(error) => return sink.end(Err(error)),
        }
    }
}
