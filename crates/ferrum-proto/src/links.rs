//! The hyperlink table of a block of rows: each distinct URI is stored once and rows refer to it
//! by index, so a screen whose cells alternate between links costs each URI once, not once per
//! run.
//!
//! ```text
//! table := { len:u16 | uri (UTF-8, len bytes) }*      little-endian; entry n is index n + 1
//! ```

use std::collections::HashMap;
use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};

use crate::error::ProtoError;

/// Bytes of an entry's length field.
const LEN_LEN: usize = 2;

/// Distinct hyperlinks of a block of rows. Rows store index 0 for "no link" and `n` for the
/// `n`-th entry, in a `u16`, so a table holds at most 65 535 entries.
///
/// `Debug` prints the sizes only: URIs are user data (`docs/05` §8).
#[derive(Clone)]
pub struct LinkTable {
    bytes: Vec<u8>,
    offsets: Vec<u32>,
    /// Hash of a URI to the index it was last stored at; a hash collision only costs a duplicate
    /// entry. Reused across `clear`, so interning does not allocate in steady state.
    by_hash: HashMap<u64, u16, BuildHasherDefault<DefaultHasher>>,
}

/// The size of a table at some point, to undo what was added after it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LinkMark {
    bytes: usize,
    entries: usize,
}

impl LinkTable {
    /// An empty table.
    pub const fn new() -> Self {
        Self {
            bytes: Vec::new(),
            offsets: Vec::new(),
            by_hash: HashMap::with_hasher(BuildHasherDefault::new()),
        }
    }

    /// Removes every entry and keeps the capacity.
    pub fn clear(&mut self) {
        self.bytes.clear();
        self.offsets.clear();
        self.by_hash.clear();
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    /// Whether the table has no entry.
    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    /// The encoded table.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The URI at `index` (1-based), or `None` when there is no such entry.
    pub fn get(&self, index: u16) -> Option<&str> {
        let at = *self.offsets.get(usize::from(index).checked_sub(1)?)?;
        let at = usize::try_from(at).ok()?;
        let len = self.bytes.get(at..at + LEN_LEN)?;
        let len = usize::from(u16::from_le_bytes([*len.first()?, *len.get(1)?]));
        std::str::from_utf8(self.bytes.get(at + LEN_LEN..at + LEN_LEN + len)?).ok()
    }

    /// Parses an encoded table received from a peer.
    ///
    /// # Errors
    ///
    /// [`ProtoError::MalformedRow`] for a truncated entry or invalid UTF-8, and
    /// [`ProtoError::TooManyLinks`] above 65 535 entries.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, ProtoError> {
        let mut offsets = Vec::new();
        let mut at = 0;
        while at < bytes.len() {
            let (len, rest) = bytes
                .get(at..)
                .and_then(<[u8]>::split_first_chunk::<LEN_LEN>)
                .ok_or(ProtoError::MalformedRow("truncated link table"))?;
            let len = usize::from(u16::from_le_bytes(*len));
            let uri = rest.get(..len).ok_or(ProtoError::MalformedRow("truncated link table"))?;
            std::str::from_utf8(uri).map_err(|_| ProtoError::MalformedRow("invalid UTF-8"))?;
            if offsets.len() == usize::from(u16::MAX) {
                return Err(ProtoError::TooManyLinks);
            }
            offsets.push(u32::try_from(at).map_err(|_| ProtoError::TooManyLinks)?);
            at += LEN_LEN + len;
        }
        Ok(Self { bytes, offsets, ..Self::new() })
    }

    /// The index of `uri`, adding it when it is new.
    pub(crate) fn intern(&mut self, uri: &str) -> Result<u16, ProtoError> {
        let hash = self.by_hash.hasher().hash_one(uri);
        if let Some(&index) = self.by_hash.get(&hash)
            && self.get(index) == Some(uri)
        {
            return Ok(index);
        }
        let len = u16::try_from(uri.len()).map_err(|_| ProtoError::LinkTooLong(uri.len()))?;
        let index = u16::try_from(self.offsets.len() + 1).map_err(|_| ProtoError::TooManyLinks)?;
        let at = u32::try_from(self.bytes.len()).map_err(|_| ProtoError::TooManyLinks)?;
        self.offsets.push(at);
        self.bytes.extend_from_slice(&len.to_le_bytes());
        self.bytes.extend_from_slice(uri.as_bytes());
        let _previous = self.by_hash.insert(hash, index); // a collision keeps the newer entry
        Ok(index)
    }

    pub(crate) fn mark(&self) -> LinkMark {
        LinkMark { bytes: self.bytes.len(), entries: self.offsets.len() }
    }

    /// Removes the entries added after `mark`.
    pub(crate) fn rollback(&mut self, mark: LinkMark) {
        if self.offsets.len() > mark.entries {
            self.bytes.truncate(mark.bytes);
            self.offsets.truncate(mark.entries);
            self.by_hash.retain(|_, index| usize::from(*index) <= mark.entries);
        }
    }
}

impl Default for LinkTable {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for LinkTable {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}

impl Eq for LinkTable {}

impl std::fmt::Debug for LinkTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LinkTable({} links, {} bytes)", self.len(), self.bytes.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_uri_is_stored_once() {
        let mut table = LinkTable::new();
        assert_eq!(table.intern("https://a.test").unwrap(), 1);
        assert_eq!(table.intern("https://b.test").unwrap(), 2);
        assert_eq!(table.intern("https://a.test").unwrap(), 1);
        assert_eq!(table.len(), 2);
        assert_eq!(table.get(2), Some("https://b.test"));
        assert_eq!(table.get(0), None);
        assert_eq!(table.get(3), None);
    }

    #[test]
    fn a_parsed_table_matches_the_original() {
        let mut table = LinkTable::new();
        table.intern("x").unwrap();
        table.intern("").unwrap();
        let parsed = LinkTable::from_bytes(table.as_bytes().to_vec()).unwrap();
        assert_eq!(parsed, table);
        assert_eq!(parsed.get(1), Some("x"));
        assert!(LinkTable::from_bytes(vec![2, 0, b'x']).is_err());
        assert!(LinkTable::from_bytes(vec![1, 0, 0xFF]).is_err());
    }

    #[test]
    fn rollback_forgets_new_entries() {
        let mut table = LinkTable::new();
        table.intern("kept").unwrap();
        let mark = table.mark();
        table.intern("dropped").unwrap();
        table.rollback(mark);
        assert_eq!(table.len(), 1);
        assert_eq!(table.intern("dropped").unwrap(), 2);
        assert!(!format!("{table:?}").contains("kept"));
    }
}
