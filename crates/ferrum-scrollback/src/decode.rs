//! Line decoder: the inverse of [`encode`](crate::encode), strict on every byte.

use crate::error::DecodeError;
use crate::limits::{MAX_ENCODED_LINE_BYTES, MAX_LINE_TEXT_BYTES};
use crate::line::{DecodedLine, LineFlags, Run};
use crate::style::StyleId;
use crate::varint::{get_len, get_varint};

/// Decodes one encoded line into `out`, reusing its buffers. The whole slice must be consumed.
pub(crate) fn decode_line(bytes: &[u8], out: &mut DecodedLine) -> Result<(), DecodeError> {
    if bytes.len() > MAX_ENCODED_LINE_BYTES {
        return Err(DecodeError::TooLarge(u64::try_from(bytes.len()).unwrap_or(u64::MAX)));
    }
    let flags_byte = *bytes.first().ok_or(DecodeError::Truncated)?;
    let flags = LineFlags::from_bits(flags_byte).ok_or(DecodeError::BadFlags(flags_byte))?;
    let mut pos = 1;
    let text_len = get_len(bytes, &mut pos, MAX_LINE_TEXT_BYTES)?;
    let end = pos.checked_add(text_len).ok_or(DecodeError::Truncated)?;
    let text = bytes.get(pos..end).ok_or(DecodeError::Truncated)?;
    let text = std::str::from_utf8(text).map_err(|_| DecodeError::BadUtf8)?;
    pos = end;
    out.flags = flags;
    out.text.clear();
    out.text.push_str(text);
    out.runs.clear();
    let run_count = get_len(bytes, &mut pos, text.len())?;
    let covered = decode_runs(bytes, &mut pos, run_count, &mut out.runs)?;
    if covered != text.chars().count() {
        return Err(DecodeError::BadRuns);
    }
    if pos != bytes.len() {
        return Err(DecodeError::TrailingBytes);
    }
    Ok(())
}

/// Reads `count` runs into `runs`; returns the number of scalars they cover.
fn decode_runs(
    bytes: &[u8],
    pos: &mut usize,
    count: usize,
    runs: &mut Vec<Run>,
) -> Result<usize, DecodeError> {
    let mut covered = 0usize;
    for _ in 0..count {
        let id = get_varint(bytes, pos)?;
        let id = u32::try_from(id).map_err(|_| DecodeError::TooLarge(id))?;
        let len = get_len(bytes, pos, MAX_LINE_TEXT_BYTES)?;
        if len == 0 {
            return Err(DecodeError::BadRuns);
        }
        covered = covered.checked_add(len).ok_or(DecodeError::BadRuns)?;
        // INVARIANT: len <= MAX_LINE_TEXT_BYTES (16 KiB) was checked above.
        runs.push(Run {
            style: StyleId::from_raw(id),
            len: u32::try_from(len).unwrap_or(u32::MAX),
        });
    }
    Ok(covered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_documented_layout() {
        let mut line = DecodedLine::new();
        assert_eq!(decode_line(&[1, 2, b'a', b'b', 1, 0, 2], &mut line), Ok(()));
        assert_eq!(line.text, "ab");
        assert!(line.flags.contains(LineFlags::WRAPPED));
        assert_eq!(line.runs, [Run { style: StyleId::DEFAULT, len: 2 }]);
    }

    #[test]
    fn rejects_malformed_lines() {
        let mut line = DecodedLine::new();
        assert_eq!(decode_line(&[], &mut line), Err(DecodeError::Truncated));
        assert_eq!(decode_line(&[0x80, 0, 0], &mut line), Err(DecodeError::BadFlags(0x80)));
        assert_eq!(decode_line(&[0, 1, 0xFF, 0], &mut line), Err(DecodeError::BadUtf8));
        assert_eq!(decode_line(&[0, 2, b'a', b'b', 1, 0, 1], &mut line), Err(DecodeError::BadRuns));
        assert_eq!(decode_line(&[0, 0, 0, 9], &mut line), Err(DecodeError::TrailingBytes));
        assert_eq!(decode_line(&[0, 1, b'a', 1, 0, 0], &mut line), Err(DecodeError::BadRuns));
    }
}
