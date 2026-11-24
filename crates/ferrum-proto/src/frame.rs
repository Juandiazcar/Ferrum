//! Framing (`docs/02` §4): `len:u32` little-endian, then `len` bytes of one `postcard`-encoded
//! message. The length counts the payload only; the whole frame, header included, is at most
//! [`MAX_FRAME_LEN`], so the payload is at most [`MAX_PAYLOAD_LEN`].

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::ProtoError;

/// Largest frame, length header included: 4 MiB (`docs/02` §4 "Max frame 4 MiB", `docs/10` §2
/// "Frame size cap 4 MiB"). A peer that declares more is malformed and its connection is closed.
pub const MAX_FRAME_LEN: usize = 4 * 1_024 * 1_024;

/// Bytes of the length prefix.
pub const FRAME_HEADER_LEN: usize = 4;

/// Largest payload: the frame cap minus the header.
pub const MAX_PAYLOAD_LEN: usize = MAX_FRAME_LEN - FRAME_HEADER_LEN;

/// Appends one frame holding `message` to `out`.
///
/// Allocation-free once `out` has the capacity.
///
/// # Errors
///
/// [`ProtoError::FrameTooLarge`] when the frame would be over [`MAX_FRAME_LEN`] (`out` is left
/// as it was) and [`ProtoError::Postcard`] when `message` cannot be serialized.
pub fn encode_frame<T: Serialize>(message: &T, out: &mut Vec<u8>) -> Result<(), ProtoError> {
    let start = out.len();
    out.extend_from_slice(&[0; FRAME_HEADER_LEN]);
    let written = postcard::to_io(message, &mut *out).map(|_| ());
    let len = out.len() - start - FRAME_HEADER_LEN;
    let header = u32::try_from(len).ok().filter(|_| len <= MAX_PAYLOAD_LEN);
    match (written, header) {
        (Ok(()), Some(header)) => {
            if let Some(slot) = out.get_mut(start..start + FRAME_HEADER_LEN) {
                slot.copy_from_slice(&header.to_le_bytes());
            }
            Ok(())
        }
        (written, _) => {
            out.truncate(start);
            written?;
            Err(ProtoError::FrameTooLarge { len: len + FRAME_HEADER_LEN, max: MAX_FRAME_LEN })
        }
    }
}

/// A complete frame found by [`split_frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame<'a> {
    /// The message bytes (see [`decode_message`]).
    pub payload: &'a [u8],
    /// Bytes the frame takes in the buffer: the header plus the payload.
    pub consumed: usize,
}

/// Finds the first complete frame at the start of `buf`; `Ok(None)` while more bytes are needed.
///
/// # Errors
///
/// [`ProtoError::FrameTooLarge`] as soon as the header declares a frame over [`MAX_FRAME_LEN`],
/// so a reader never buffers an oversized frame.
pub fn split_frame(buf: &[u8]) -> Result<Option<Frame<'_>>, ProtoError> {
    let Some((header, rest)) = buf.split_first_chunk::<FRAME_HEADER_LEN>() else {
        return Ok(None);
    };
    let len = usize::try_from(u32::from_le_bytes(*header)).unwrap_or(usize::MAX);
    if len > MAX_PAYLOAD_LEN {
        let len = len.saturating_add(FRAME_HEADER_LEN); // only reported
        return Err(ProtoError::FrameTooLarge { len, max: MAX_FRAME_LEN });
    }
    Ok(rest.get(..len).map(|payload| Frame { payload, consumed: FRAME_HEADER_LEN + len }))
}

/// Decodes one message from a frame payload (see [`split_frame`]).
///
/// # Errors
///
/// [`ProtoError::Postcard`] for a malformed payload and [`ProtoError::TrailingBytes`] when bytes
/// remain after the message.
pub fn decode_message<T: DeserializeOwned>(payload: &[u8]) -> Result<T, ProtoError> {
    let (message, rest) = postcard::take_from_bytes(payload)?;
    if rest.is_empty() { Ok(message) } else { Err(ProtoError::TrailingBytes(rest.len())) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_split_only_when_complete() {
        let mut out = Vec::new();
        encode_frame(&(7_u8, "hi"), &mut out).unwrap();
        assert_eq!(out, [4, 0, 0, 0, 7, 2, b'h', b'i']);
        for len in 0..out.len() {
            assert!(split_frame(&out[..len]).unwrap().is_none());
        }
        out.push(1);
        let frame = split_frame(&out).unwrap().unwrap();
        assert_eq!(frame.consumed, 8);
        assert_eq!(decode_message::<(u8, String)>(frame.payload).unwrap(), (7, "hi".to_owned()));
    }

    #[test]
    fn an_oversized_header_is_refused_before_the_payload_arrives() {
        let header = u32::try_from(MAX_PAYLOAD_LEN + 1).unwrap().to_le_bytes();
        let error = split_frame(&header).unwrap_err();
        assert!(matches!(error, ProtoError::FrameTooLarge { len, .. } if len == MAX_FRAME_LEN + 1));
        let max = u32::try_from(MAX_PAYLOAD_LEN).unwrap().to_le_bytes();
        assert!(split_frame(&max).unwrap().is_none());
    }

    #[test]
    fn the_largest_frame_including_its_header_is_exactly_the_cap() {
        // A byte vector of n bytes is a 4-byte varint length plus n bytes for 2^21 <= n < 2^28.
        let mut out = Vec::new();
        let fits = vec![0_u8; MAX_PAYLOAD_LEN - 4];
        encode_frame(&fits, &mut out).unwrap();
        assert_eq!(out.len(), MAX_FRAME_LEN);
        let frame = split_frame(&out).unwrap().unwrap();
        assert_eq!(frame.consumed, MAX_FRAME_LEN);

        let mut out = vec![1];
        let big = vec![0_u8; MAX_PAYLOAD_LEN - 3];
        let error = encode_frame(&big, &mut out).unwrap_err();
        assert!(matches!(error, ProtoError::FrameTooLarge { len, .. } if len == MAX_FRAME_LEN + 1));
        assert_eq!(out, [1]);
    }

    #[test]
    fn trailing_bytes_are_refused() {
        assert!(matches!(decode_message::<u8>(&[1, 2]), Err(ProtoError::TrailingBytes(1))));
    }
}
