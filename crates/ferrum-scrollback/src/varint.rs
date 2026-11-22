//! LEB128 varints (little-endian, 7 bits per byte), the integer encoding of `docs/02` §3.2.

use crate::error::DecodeError;

/// Longest varint of a `u64`.
pub(crate) const MAX_VARINT_LEN: usize = 10;
const PAYLOAD_MASK: u64 = 0x7F;
const CONTINUE_BIT: u8 = 0x80;
const PAYLOAD_BITS: u32 = 7;

/// Appends `value` as a varint.
pub(crate) fn put_varint(out: &mut Vec<u8>, mut value: u64) {
    while value > PAYLOAD_MASK {
        out.push(low_bits(value) | CONTINUE_BIT);
        value >>= PAYLOAD_BITS;
    }
    out.push(low_bits(value));
}

/// Appends a `usize` as a varint.
pub(crate) fn put_len(out: &mut Vec<u8>, value: usize) {
    // INVARIANT: usize is at most 64 bits on every supported target, so the fallback is dead.
    put_varint(out, u64::try_from(value).unwrap_or(u64::MAX));
}

fn low_bits(value: u64) -> u8 {
    // INVARIANT: masked to 7 bits, always fits.
    u8::try_from(value & PAYLOAD_MASK).unwrap_or(0)
}

/// Reads a varint at `*pos` and advances `*pos` past it.
pub(crate) fn get_varint(buf: &[u8], pos: &mut usize) -> Result<u64, DecodeError> {
    let rest = buf.get(*pos..).ok_or(DecodeError::Truncated)?;
    let mut value = 0u64;
    for (index, &byte) in rest.iter().enumerate().take(MAX_VARINT_LEN) {
        let payload = u64::from(byte & !CONTINUE_BIT);
        // INVARIANT: index < 10 so the shift is at most 63.
        let shift = u32::try_from(index).unwrap_or(0) * PAYLOAD_BITS;
        let shifted = payload << shift;
        if shifted >> shift != payload {
            return Err(DecodeError::BadVarint);
        }
        value |= shifted;
        if byte & CONTINUE_BIT == 0 {
            *pos += index + 1;
            return Ok(value);
        }
    }
    if rest.len() < MAX_VARINT_LEN {
        Err(DecodeError::Truncated)
    } else {
        Err(DecodeError::BadVarint)
    }
}

/// Reads a varint that must fit a `usize` and not exceed `limit`.
pub(crate) fn get_len(buf: &[u8], pos: &mut usize, limit: usize) -> Result<usize, DecodeError> {
    let value = get_varint(buf, pos)?;
    match usize::try_from(value) {
        Ok(len) if len <= limit => Ok(len),
        _ => Err(DecodeError::TooLarge(value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_boundaries() {
        for value in [0, 1, 127, 128, 300, 16_383, 16_384, u64::from(u32::MAX), u64::MAX] {
            let mut buf = Vec::new();
            put_varint(&mut buf, value);
            let mut pos = 0;
            assert_eq!(get_varint(&buf, &mut pos), Ok(value));
            assert_eq!(pos, buf.len());
        }
    }

    #[test]
    fn rejects_truncated_and_overlong_input() {
        assert_eq!(get_varint(&[0x80], &mut 0), Err(DecodeError::Truncated));
        assert_eq!(get_varint(&[0xFF; 11], &mut 0), Err(DecodeError::BadVarint));
        // The 10th byte may only carry one bit of a u64.
        let mut overflow = vec![0xFF; 9];
        overflow.push(0x02);
        assert_eq!(get_varint(&overflow, &mut 0), Err(DecodeError::BadVarint));
    }

    #[test]
    fn get_len_enforces_the_limit() {
        let mut buf = Vec::new();
        put_len(&mut buf, 500);
        assert_eq!(get_len(&buf, &mut 0, 500), Ok(500));
        assert_eq!(get_len(&buf, &mut 0, 499), Err(DecodeError::TooLarge(500)));
    }
}
