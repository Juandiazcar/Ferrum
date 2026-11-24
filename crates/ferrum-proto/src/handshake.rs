//! The handshake (`docs/02` §4, `docs/10` §2): the client's first frame is a [`Hello`]; the daemon
//! answers `Welcome` with the negotiated version, or `Refused` and closes the connection.
//!
//! # Invariant across every major version
//!
//! Peers of different majors must still read each other's handshake, or the UI could not tell
//! the user that the daemon is incompatible (`docs/06` §7). So, in every major version:
//! `ClientMessage::Hello` is variant 0 and `DaemonMessage::Welcome` and `DaemonMessage::Refused`
//! are variants 0 and 1; [`Hello`], [`ProtocolVersion`], [`Nonce`], [`Refusal`] and the existing
//! variants of [`ClientKind`] and [`RefusalReason`] keep their fields and layout. A new major may
//! only append variants to those two enums. `tests/handshake.rs` pins the bytes.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::daemon::DaemonMessage;
use crate::version::{PROTOCOL_VERSION, ProtocolVersion};

/// Bytes of the per-daemon nonce. Reason: 256 random bits cannot be guessed by a local process
/// that cannot read the owner-only nonce file (`docs/10` §2).
pub const NONCE_LEN: usize = 32;

/// The random per-daemon secret a client reads from the owner-only nonce file and presents.
///
/// `Debug` never prints it, and `==` is [`Nonce::matches`]: it takes the same time wherever two
/// nonces differ.
#[derive(Clone, Serialize, Deserialize)]
pub struct Nonce(pub [u8; NONCE_LEN]);

impl Nonce {
    /// Whether `other` is the same nonce, in time independent of where they differ.
    pub fn matches(&self, other: &Self) -> bool {
        // black_box keeps the optimizer from ending the loop at the first difference.
        let diff = self.0.iter().zip(&other.0).fold(0, |diff, (a, b)| diff | (a ^ b));
        std::hint::black_box(diff) == 0
    }
}

impl PartialEq for Nonce {
    fn eq(&self, other: &Self) -> bool {
        self.matches(other)
    }
}

impl Eq for Nonce {}

impl fmt::Debug for Nonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Nonce(<redacted>)")
    }
}

/// Who connects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ClientKind {
    /// The `ferrum` UI.
    Ui,
}

/// The first frame of a connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    /// The client's protocol version.
    pub protocol_version: ProtocolVersion,
    /// Who connects.
    pub client_kind: ClientKind,
    /// The daemon's nonce as read from its file.
    pub nonce: Nonce,
}

/// Why the daemon refused a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RefusalReason {
    /// The protocol majors differ: the UI offers to restart the daemon.
    VersionMismatch,
    /// The nonce is wrong.
    BadNonce,
}

/// The daemon's answer to a [`Hello`] it does not accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Refusal {
    /// The daemon's version, for the UI's error message.
    pub daemon_version: ProtocolVersion,
    /// Why.
    pub reason: RefusalReason,
}

/// The daemon's answer to `hello` given its own `nonce`: `Welcome` with the negotiated version,
/// or `Refused` (wrong nonce first, then an incompatible major).
pub fn answer_hello(hello: &Hello, nonce: &Nonce) -> DaemonMessage {
    let refuse =
        |reason| DaemonMessage::Refused(Refusal { daemon_version: PROTOCOL_VERSION, reason });
    if !hello.nonce.matches(nonce) {
        return refuse(RefusalReason::BadNonce);
    }
    match PROTOCOL_VERSION.negotiate(hello.protocol_version) {
        Ok(protocol_version) => DaemonMessage::Welcome { protocol_version },
        Err(_) => refuse(RefusalReason::VersionMismatch),
    }
}
