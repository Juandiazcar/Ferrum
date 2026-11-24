//! One client connection: framing over a non-blocking stream with bounded buffers
//! (`docs/10` §2: frame cap, per-connection rate and memory limits; `docs/02` §4: the daemon
//! never queues unbounded data for a slow client).
//!
//! Incoming bytes are kept until a whole frame is there (at most one frame plus one read).
//! Outgoing frames are appended to one buffer; above [`CONGESTED_BYTES`] the client gets no damage
//! until the buffer drains (damage is coalesced to the latest state instead of queued), and a
//! client whose buffer would pass [`MAX_OUTBOX_BYTES`], or whose backlog would push all clients'
//! output past [`TOTAL_OUTBOX_BYTES`], is disconnected. Together these keep client buffers within
//! the daemon's 2 MB reserve (`docs/03` §2), plus at most one message per client.

use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use ferrum_platform::ipc::Connection;
use ferrum_proto::{
    ClientMessage, DaemonMessage, MAX_FRAME_LEN, ProtoError, ProtocolVersion, SessionId,
    decode_message, encode_frame, split_frame,
};

use crate::view::View;

/// Bytes read from a client per read call.
const READ_BYTES: usize = 64 * 1_024;
/// Capacity an empty output buffer keeps: a typical frame of damage.
const KEEP_OUTBOX_BYTES: usize = 64 * 1_024;
/// Capacity an empty input buffer keeps: two reads.
const KEEP_INBOX_BYTES: usize = 2 * READ_BYTES;
/// Pending output above which the client is congested: two row blocks as the daemon cuts them
/// (`frame::BLOCK_FLUSH_BYTES`, 128 KiB), more than a full screen of damage at 200×55.
pub(crate) const CONGESTED_BYTES: usize = 256 * 1_024;
/// Pending output above which one client is dropped: a client this far behind is not reading.
const MAX_OUTBOX_BYTES: usize = 1_024 * 1_024;
/// Pending output of all clients together above which the client being written to is dropped:
/// the daemon's reserve (`docs/03` §2: 2 MB of the 18 MB allowance).
const TOTAL_OUTBOX_BYTES: usize = 2 * 1_024 * 1_024;
/// Frames handled per client per loop turn, so one client cannot monopolize the loop.
pub(crate) const FRAMES_PER_TURN: usize = 64;
/// Time a connection has to complete the handshake (`docs/10` §2): a connection that never says
/// `Hello` cannot hold a slot, nor keep an idle daemon alive (FR-SESS-003).
pub(crate) const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
/// Mutations (sessions and projects created, changed, closed) a client may send in a burst, and
/// the interval in which one more is allowed (`docs/10` §2: per-connection rate limit). A person
/// clicking never comes near; a process creating projects in a loop is held to 20 per second.
const MUTATION_BURST: u32 = 32;
const MUTATION_INTERVAL: Duration = Duration::from_millis(50);

/// Where a client is in its connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    /// Waiting for `Hello` until the deadline.
    Handshake(Instant),
    /// Accepted at this protocol version.
    Ready(ProtocolVersion),
    /// Refused or broken: close once the output is flushed.
    Closing,
}

/// The result of reading what a client sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Received {
    /// The peer is still connected.
    Open,
    /// The peer closed or the connection failed.
    Gone,
}

/// A token bucket.
#[derive(Debug, Clone, Copy)]
struct Bucket {
    tokens: u32,
    since: Instant,
}

/// What is known about the other end.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Peer {
    /// The peer's user was checked (`docs/10` §2), after its first bytes.
    pub(crate) checked: bool,
    /// The peer closed its end; the frames it sent before are still handled.
    pub(crate) closed: bool,
}

/// One connected client.
pub(crate) struct Client {
    pub(crate) conn: Box<dyn Connection>,
    pub(crate) phase: Phase,
    /// Sessions this client shows.
    pub(crate) views: HashMap<SessionId, View>,
    /// Bytes or frames may be waiting to be read.
    pub(crate) readable: bool,
    pub(crate) peer: Peer,
    /// Frames were skipped while the client was congested.
    pub(crate) skipped_frames: bool,
    mutations: Bucket,
    inbox: Vec<u8>,
    outbox: Vec<u8>,
    sent: usize,
    encode: Vec<u8>,
    total: Arc<AtomicUsize>,
}

impl Client {
    /// A client accepted at `now`; `total` counts the pending output of every client.
    pub(crate) fn new(conn: Box<dyn Connection>, now: Instant, total: Arc<AtomicUsize>) -> Self {
        Self {
            conn,
            phase: Phase::Handshake(now + HANDSHAKE_TIMEOUT),
            views: HashMap::new(),
            readable: true,
            peer: Peer::default(),
            skipped_frames: false,
            mutations: Bucket { tokens: MUTATION_BURST, since: now },
            inbox: Vec::new(),
            outbox: Vec::new(),
            sent: 0,
            encode: Vec::new(),
            total,
        }
    }

    /// Reads until the socket is empty or a whole frame is buffered.
    pub(crate) fn receive(&mut self) -> Received {
        while !self.peer.closed && self.inbox.len() <= MAX_FRAME_LEN {
            let start = self.inbox.len();
            self.inbox.resize(start + READ_BYTES, 0);
            let read = self.conn.read(self.inbox.get_mut(start..).unwrap_or_default());
            self.inbox.truncate(start + *read.as_ref().unwrap_or(&0));
            match read {
                Ok(0) => self.peer.closed = true,
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::WouldBlock => return Received::Open,
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(_) => return Received::Gone,
            }
        }
        Received::Open
    }

    /// Whether bytes arrived (the peer can be checked).
    pub(crate) fn has_input(&self) -> bool {
        !self.inbox.is_empty()
    }

    /// The next whole frame, decoded. A malformed frame is an error: the caller drops the client.
    pub(crate) fn next_message(&mut self) -> Result<Option<ClientMessage>, ProtoError> {
        let Some(frame) = split_frame(&self.inbox)? else { return Ok(None) };
        let used = frame.consumed;
        let message = decode_message(frame.payload)?;
        self.inbox.drain(..used);
        if self.inbox.is_empty() && self.inbox.capacity() > KEEP_INBOX_BYTES {
            // docs/03 §3 rule 7: a large frame does not keep its buffer.
            self.inbox.shrink_to(KEEP_INBOX_BYTES);
        }
        Ok(Some(message))
    }

    /// Whether one more mutation is allowed at `now` (and counts it).
    pub(crate) fn allow_mutation(&mut self, now: Instant) -> bool {
        let interval = MUTATION_INTERVAL.as_nanos().max(1);
        let earned = now.saturating_duration_since(self.mutations.since).as_nanos() / interval;
        if earned > 0 {
            let earned = u32::try_from(earned).unwrap_or(u32::MAX);
            self.mutations.tokens =
                self.mutations.tokens.saturating_add(earned).min(MUTATION_BURST);
            self.mutations.since = now;
        }
        let allowed = self.mutations.tokens > 0;
        self.mutations.tokens = self.mutations.tokens.saturating_sub(1);
        allowed
    }

    /// Queues `message`; a client that fell too far behind is marked closing.
    pub(crate) fn send(&mut self, message: &DaemonMessage) {
        if self.phase == Phase::Closing {
            return;
        }
        self.encode.clear();
        if let Err(error) = encode_frame(message, &mut self.encode) {
            tracing::error!(%error, "a message could not be encoded");
            return;
        }
        let len = self.encode.len();
        let pending = self.pending();
        let total = self.total.load(Ordering::Relaxed);
        if pending > 0 && (pending + len > MAX_OUTBOX_BYTES || total + len > TOTAL_OUTBOX_BYTES) {
            tracing::warn!("a client stopped reading; it is disconnected");
            self.phase = Phase::Closing;
            return;
        }
        self.outbox.extend_from_slice(&self.encode);
        self.total.fetch_add(len, Ordering::Relaxed);
    }

    /// Writes queued output until the socket is full.
    pub(crate) fn flush(&mut self) -> Received {
        let before = self.sent;
        let mut result = Received::Open;
        while self.sent < self.outbox.len() {
            match self.conn.write(self.outbox.get(self.sent..).unwrap_or_default()) {
                Ok(0) => result = Received::Gone,
                Ok(written) => self.sent += written,
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(_) => result = Received::Gone,
            }
            if result == Received::Gone {
                break;
            }
        }
        self.total.fetch_sub(self.sent - before, Ordering::Relaxed);
        if self.sent == self.outbox.len() {
            self.outbox.clear();
            self.sent = 0;
            if self.outbox.capacity() > KEEP_OUTBOX_BYTES {
                // docs/03 §3 rule 7: a snapshot burst does not keep its buffer.
                self.outbox.shrink_to(KEEP_OUTBOX_BYTES);
                self.encode.shrink_to(KEEP_OUTBOX_BYTES);
            }
        }
        result
    }

    /// Bytes queued and not yet written.
    pub(crate) fn pending(&self) -> usize {
        self.outbox.len() - self.sent
    }

    /// Too far behind to get more damage now.
    pub(crate) fn is_congested(&self) -> bool {
        self.pending() > CONGESTED_BYTES
    }

    /// The negotiated version, once accepted.
    pub(crate) fn version(&self) -> Option<ProtocolVersion> {
        match self.phase {
            Phase::Ready(version) => Some(version),
            _ => None,
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.total.fetch_sub(self.pending(), Ordering::Relaxed);
    }
}
