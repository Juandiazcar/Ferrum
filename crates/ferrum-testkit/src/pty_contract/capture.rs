//! Reading a session's output in tests: [`Capture`] keeps everything (small outputs), and
//! [`LineCheck`] checks a flood line by line without keeping it (bounded memory).

use std::ops::ControlFlow;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::time::{Duration, Instant};

use ferrum_pty::{PtyChild, PtyError, PtyReader};

use super::reader::{ChunkSink, spawn_reader_thread};

/// Chunks the reader thread may queue ahead of the test (a bounded channel).
const QUEUE_CHUNKS: usize = 32;

/// How long one wait for output lasts before the caller re-checks its condition.
pub(crate) const POLL: Duration = Duration::from_millis(10);

enum Event {
    Chunk(Vec<u8>),
    End(Result<(), String>),
}

struct ChannelSink(SyncSender<Event>);

impl ChunkSink for ChannelSink {
    fn chunk(&mut self, bytes: &[u8]) -> ControlFlow<()> {
        match self.0.send(Event::Chunk(bytes.to_vec())) {
            Ok(()) => ControlFlow::Continue(()),
            Err(_) => ControlFlow::Break(()),
        }
    }

    fn end(&mut self, result: Result<(), PtyError>) {
        // The test may have stopped listening; nothing is left to report then.
        let _ignored = self.0.send(Event::End(result.map_err(|error| error.to_string())));
    }
}

/// All output of a session, read by a test reader thread.
pub struct Capture {
    rx: Receiver<Event>,
    /// Every byte received so far.
    pub data: Vec<u8>,
    /// How the reader ended, once it did.
    pub end: Option<Result<(), String>>,
}

impl Capture {
    /// Starts reading `reader` on a reader thread.
    pub fn start(reader: Box<dyn PtyReader>) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel(QUEUE_CHUNKS);
        let _thread = spawn_reader_thread(0, reader, ChannelSink(tx)).unwrap();
        Self { rx, data: Vec::new(), end: None }
    }

    /// Receives for at most `timeout`; returns `false` once the reader has ended.
    fn pump(&mut self, timeout: Duration) -> bool {
        if self.end.is_some() {
            return false;
        }
        match self.rx.recv_timeout(timeout) {
            Ok(Event::Chunk(chunk)) => self.data.extend_from_slice(&chunk),
            Ok(Event::End(result)) => self.end = Some(result),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => self.end = Some(Err("reader vanished".into())),
        }
        true
    }

    /// Waits until `done(data)` holds; `false` on timeout or when the reader ends first.
    pub fn wait_until(&mut self, timeout: Duration, mut done: impl FnMut(&[u8]) -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if done(&self.data) {
                return true;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() || !self.pump(left.min(POLL)) && !done(&self.data) {
                return false;
            }
        }
    }

    /// Waits until `needle` occurs in the output after byte offset `from`.
    pub fn wait_for_after(&mut self, from: usize, needle: &[u8], timeout: Duration) -> bool {
        self.wait_until(timeout, |output| {
            output.get(from..).is_some_and(|tail| contains(tail, needle))
        })
    }

    /// Waits until `needle` occurs anywhere in the output.
    pub fn wait_for(&mut self, needle: &[u8], timeout: Duration) -> bool {
        self.wait_for_after(0, needle, timeout)
    }

    /// Waits until the reader ends (`read` returned `Ok(0)` or failed).
    pub fn wait_end(&mut self, timeout: Duration) -> Option<Result<(), String>> {
        let deadline = Instant::now() + timeout;
        while self.end.is_none() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            self.pump(left.min(POLL));
        }
        self.end.clone()
    }

    /// Waits until the child has exited, receiving output meanwhile.
    pub fn wait_exit(
        &mut self,
        child: &mut dyn PtyChild,
        timeout: Duration,
    ) -> Option<ferrum_pty::ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                return Some(status);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            if !self.pump(left.min(POLL)) {
                // The reader ended; pace the polling on the deadline alone.
                std::thread::park_timeout(left.min(POLL));
            }
        }
    }
}

/// Whether `needle` occurs in `haystack`.
pub fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|window| window == needle)
}

/// Numbers that follow each occurrence of `prefix` in `data` (`LINE12` gives 12).
pub fn numbers_after(output: &[u8], prefix: &[u8]) -> Vec<u64> {
    let mut found = Vec::new();
    let mut rest = output;
    while let Some(at) = rest.windows(prefix.len()).position(|window| window == prefix) {
        rest = rest.get(at + prefix.len()..).unwrap_or_default();
        let digits = rest.iter().take_while(|byte| byte.is_ascii_digit()).count();
        let text = std::str::from_utf8(rest.get(..digits).unwrap_or_default()).unwrap_or("");
        if let Ok(number) = text.parse() {
            found.push(number);
        }
    }
    found
}

/// Checks a `flood` output line by line while keeping only the current partial line: sequence
/// numbers must be contiguous from 0, and the end marker must report the same count.
#[derive(Debug, Default)]
pub struct LineCheck {
    partial: Vec<u8>,
    /// Next expected sequence number.
    pub next: u64,
    /// Count reported by the end marker, once seen.
    pub end: Option<u64>,
    /// First sequence error seen.
    pub error: Option<String>,
    /// Bytes received.
    pub bytes: u64,
    /// Whether sequence numbers are checked (a resize repaint re-emits lines).
    pub sequence: Sequence,
}

/// Whether a [`LineCheck`] checks the sequence numbers of the flood lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sequence {
    /// Every line must follow the previous one (no repaint can happen).
    #[default]
    Checked,
    /// Only the end marker counts (resizes may repaint lines).
    Ignored,
}

/// Longest line kept: a flood line is [`FLOOD_LINE_BYTES`](super::FLOOD_LINE_BYTES) bytes, the
/// rest is escape sequences around it.
const MAX_LINE_BYTES: usize = 4096;

impl LineCheck {
    /// A checker of a flood whose sequence numbers are checked or not.
    pub fn new(sequence: Sequence) -> Self {
        Self { sequence, ..Self::default() }
    }

    /// Feeds one chunk.
    pub fn feed(&mut self, chunk: &[u8]) {
        self.bytes += chunk.len() as u64;
        for &byte in chunk {
            if byte == b'\n' {
                let line = std::mem::take(&mut self.partial);
                self.line(&line);
                self.partial = line;
                self.partial.clear();
            } else if self.partial.len() < MAX_LINE_BYTES {
                self.partial.push(byte);
            }
        }
    }

    fn line(&mut self, line: &[u8]) {
        if let Some(count) = numbers_after(line, b"FLOOD-END ").first() {
            self.end = Some(*count);
            return;
        }
        if self.sequence == Sequence::Ignored || self.error.is_some() {
            return;
        }
        for seq in numbers_after(line, b"F").into_iter().filter(|_| contains(line, b"xxxx")) {
            if seq != self.next {
                self.error = Some(format!("expected line {} and got {seq}", self.next));
                return;
            }
            self.next += 1;
        }
    }
}
