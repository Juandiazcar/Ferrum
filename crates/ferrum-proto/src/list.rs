//! A list whose length is capped while it is decoded.

use std::fmt;
use std::marker::PhantomData;

use serde::de::{Deserialize, Deserializer, Error as _, SeqAccess, Visitor};
use serde::{Serialize, Serializer};

use crate::error::ProtoError;

/// Most items in any list of a message.
///
/// Reason: a 4 MiB frame of one-byte items would otherwise decode into millions of items, each
/// taking far more memory than its byte on the wire (a `String` is 24 bytes), so a peer could make
/// the decoder allocate about 100 MB per frame (`docs/10` §2: per-connection memory limits).
/// 1 024 is far above any real list (project roots, layout nodes) and bounds that cost to a few
/// tens of KiB. Semantic limits (for example the roots of a project) belong to the model crates.
pub const MAX_LIST_ITEMS: usize = 1_024;

/// A list of at most [`MAX_LIST_ITEMS`] items; longer input fails to decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List<T>(Vec<T>);

impl<T> Default for List<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T> List<T> {
    /// Wraps `items`.
    ///
    /// # Errors
    ///
    /// [`ProtoError::TooManyItems`] when there are more than [`MAX_LIST_ITEMS`].
    pub fn new(items: Vec<T>) -> Result<Self, ProtoError> {
        if items.len() <= MAX_LIST_ITEMS {
            Ok(Self(items))
        } else {
            Err(ProtoError::TooManyItems(items.len()))
        }
    }

    /// The items.
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// The items, by value.
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }
}

impl<T: Serialize> Serialize for List<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for List<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(ListVisitor(PhantomData))
    }
}

struct ListVisitor<T>(PhantomData<T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for ListVisitor<T> {
    type Value = List<T>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a list of at most {MAX_LIST_ITEMS} items")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let declared = seq.size_hint().unwrap_or(0);
        if declared > MAX_LIST_ITEMS {
            return Err(A::Error::invalid_length(declared, &self));
        }
        let mut items = Vec::with_capacity(declared);
        while let Some(item) = seq.next_element()? {
            if items.len() == MAX_LIST_ITEMS {
                return Err(A::Error::invalid_length(MAX_LIST_ITEMS + 1, &self));
            }
            items.push(item);
        }
        Ok(List(items))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode_message;

    #[test]
    fn a_list_longer_than_the_cap_does_not_decode() {
        let at_cap = postcard::to_stdvec(&vec![0_u8; MAX_LIST_ITEMS]).unwrap();
        assert_eq!(decode_message::<List<u8>>(&at_cap).unwrap().as_slice().len(), MAX_LIST_ITEMS);
        let over = postcard::to_stdvec(&vec![0_u8; MAX_LIST_ITEMS + 1]).unwrap();
        assert!(decode_message::<List<u8>>(&over).is_err());
        assert!(List::new(vec![0_u8; MAX_LIST_ITEMS + 1]).is_err());
    }
}
