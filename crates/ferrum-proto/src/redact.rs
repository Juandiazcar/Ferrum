//! User content inside messages: keystrokes, queries, titles, paths, names. Their `Debug` prints
//! only the size, so logging a message never logs what the user typed or saw (`docs/05` §8,
//! `docs/10` §4). The wire form is the plain string or byte list.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Text that is user content.
#[derive(Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Text(String);

impl Text {
    /// Wraps `text`.
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The text, by value.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Text(<{} bytes>)", self.0.len())
    }
}

/// Bytes that are user content (input for a PTY, opaque payloads).
#[derive(Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Bytes(Vec<u8>);

impl Bytes {
    /// Wraps `bytes`.
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self(bytes.into())
    }

    /// The bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// The bytes, by value.
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

impl fmt::Debug for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bytes(<{} bytes>)", self.0.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_shows_the_size_only() {
        assert_eq!(format!("{:?}", Text::new("hunter2")), "Text(<7 bytes>)");
        assert_eq!(format!("{:?}", Bytes::new(*b"pw\r")), "Bytes(<3 bytes>)");
    }
}
