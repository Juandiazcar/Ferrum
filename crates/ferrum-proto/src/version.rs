//! Protocol versioning (`docs/02` §4, `docs/06` §7): a major bump is incompatible and refused;
//! minor versions only add messages, and two peers talk at the lower of their minors.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ProtoError;

/// A protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProtocolVersion {
    /// Incompatible changes.
    pub major: u16,
    /// Additive changes within a major version.
    pub minor: u16,
}

/// The version this build speaks: the first protocol.
pub const PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };

impl ProtocolVersion {
    /// The version both sides use: same major, the lower minor. A peer never sends a message
    /// that is newer than the negotiated minor.
    ///
    /// # Errors
    ///
    /// [`ProtoError::VersionMismatch`] when the majors differ: the UI refuses the daemon and
    /// offers to restart it (`docs/06` §7).
    ///
    /// ```
    /// use ferrum_proto::ProtocolVersion;
    /// let ours = ProtocolVersion { major: 1, minor: 3 };
    /// let theirs = ProtocolVersion { major: 1, minor: 1 };
    /// assert_eq!(ours.negotiate(theirs)?, theirs);
    /// assert!(ours.negotiate(ProtocolVersion { major: 2, minor: 0 }).is_err());
    /// # Ok::<(), ferrum_proto::ProtoError>(())
    /// ```
    pub fn negotiate(self, theirs: Self) -> Result<Self, ProtoError> {
        if self.major == theirs.major {
            Ok(Self { major: self.major, minor: self.minor.min(theirs.minor) })
        } else {
            Err(ProtoError::VersionMismatch { ours: self, theirs })
        }
    }
}

impl fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}
