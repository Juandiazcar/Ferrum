//! A test-side reader thread: the cases read a session's output on a thread of their own and
//! receive it as chunks, whatever the backend's [`Readiness`](ferrum_pty::Readiness). It reads
//! with the daemon's chunk size ([`READ_BUFFER_BYTES`]).

use std::ops::ControlFlow;
use std::thread::JoinHandle;

use ferrum_pty::{PtyError, PtyReader};

/// The read size of the daemon (`docs/02` §5: chunks of at most 64 KiB).
pub const READ_BUFFER_BYTES: usize = 64 * 1024;

/// Receives the output of a test reader thread.
pub trait ChunkSink: Send {
    /// Called with each chunk read, in order. `Break` stops the thread.
    fn chunk(&mut self, bytes: &[u8]) -> ControlFlow<()>;

    /// Called once when the thread stops: `Ok` at the end of the output or after `Break`, `Err`
    /// when a read failed.
    fn end(&mut self, result: Result<(), PtyError>);
}

/// Starts a thread named `test-read-<id>` that reads `reader` into `sink` until the end of the
/// output, a read error, or `Break`.
pub fn spawn_reader_thread<S: ChunkSink + 'static>(
    id: u64,
    mut reader: Box<dyn PtyReader>,
    mut sink: S,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name(format!("test-read-{id}"))
        .spawn(move || pump(reader.as_mut(), &mut sink))
}

fn pump(reader: &mut dyn PtyReader, sink: &mut dyn ChunkSink) {
    let mut buf = vec![0u8; READ_BUFFER_BYTES];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => return sink.end(Ok(())),
            Ok(read) => {
                let Some(chunk) = buf.get(..read) else {
                    let broken = std::io::Error::other("read reported more bytes than the buffer");
                    return sink.end(Err(PtyError::io("read", broken)));
                };
                if sink.chunk(chunk).is_break() {
                    return sink.end(Ok(()));
                }
            }
            Err(error) => return sink.end(Err(error)),
        }
    }
}
