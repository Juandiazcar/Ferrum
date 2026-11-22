//! Reading sealed chunks: [`decode_chunk`] validates and decompresses a blob (the fuzz target).

use lz4_flex::block::decompress_into;

use crate::decode::decode_line;
use crate::error::DecodeError;
use crate::limits::{HOT_CHUNK_LINES, LZ4_MAX_EXPANSION, MAX_CHUNK_RAW_BYTES};
use crate::line::DecodedLine;
use crate::varint::get_len;

/// A decompressed, validated chunk: the raw encoded lines and where each one ends.
///
/// Reusable: decoding another chunk into it keeps its allocations.
#[derive(Debug, Default)]
pub struct ChunkBuf {
    raw: Vec<u8>,
    ends: Vec<u32>,
    probe: DecodedLine,
    pub(crate) id: Option<u64>,
}

impl ChunkBuf {
    /// An empty buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of lines of the last successfully decoded chunk.
    pub fn line_count(&self) -> usize {
        self.ends.len()
    }

    /// Decodes line `index` of the chunk into `out`; `None` when out of range.
    pub fn line(&self, index: usize, out: &mut DecodedLine) -> Option<Result<(), DecodeError>> {
        let end = usize::try_from(*self.ends.get(index)?).ok()?;
        let start = match index.checked_sub(1).and_then(|previous| self.ends.get(previous)) {
            Some(&previous) => usize::try_from(previous).ok()?,
            None => 0,
        };
        let bytes = self.raw.get(start..end)?;
        Some(decode_line(bytes, out))
    }

    /// Heap bytes held by the buffers.
    pub fn capacity_bytes(&self) -> usize {
        self.raw.capacity() + self.ends.capacity() * size_of::<u32>() + self.probe.capacity_bytes()
    }
}

/// Validates the sealed blob `blob` and decompresses it into `buf`.
///
/// On `Ok`, every line of the chunk has been decoded once, so [`ChunkBuf::line`] cannot fail
/// afterwards (style ids are checked against the style table by the caller). Total for all inputs:
/// it never panics, never allocates more than `MAX_CHUNK_RAW_BYTES` (64 MiB) and rejects declared sizes
/// that LZ4 cannot produce before allocating.
///
/// # Errors
/// Any [`DecodeError`] when the blob is malformed.
pub fn decode_chunk(blob: &[u8], buf: &mut ChunkBuf) -> Result<(), DecodeError> {
    buf.id = None;
    buf.ends.clear();
    let mut pos = 0;
    let lines = get_len(blob, &mut pos, HOT_CHUNK_LINES)?;
    let raw_len = get_len(blob, &mut pos, MAX_CHUNK_RAW_BYTES)?;
    let index_len = get_len(blob, &mut pos, blob.len())?;
    let index_end = pos.checked_add(index_len).ok_or(DecodeError::Truncated)?;
    let index = blob.get(pos..index_end).ok_or(DecodeError::Truncated)?;
    let lz4 = blob.get(index_end..).ok_or(DecodeError::Truncated)?;
    if lines == 0 || raw_len > lz4.len().saturating_mul(LZ4_MAX_EXPANSION) {
        return Err(DecodeError::TooLarge(u64::try_from(raw_len).unwrap_or(u64::MAX)));
    }
    buf.raw.clear();
    buf.raw.resize(raw_len, 0);
    let produced = decompress_into(lz4, &mut buf.raw).map_err(|_| DecodeError::Lz4)?;
    if produced != raw_len {
        return Err(DecodeError::Lz4);
    }
    read_index(index, lines, raw_len, &mut buf.ends)?;
    validate_lines(buf)
}

/// Turns the delta-encoded index into line end offsets and checks it covers `raw_len` exactly.
fn read_index(
    index: &[u8],
    lines: usize,
    raw_len: usize,
    ends: &mut Vec<u32>,
) -> Result<(), DecodeError> {
    let (mut pos, mut total) = (0usize, 0usize);
    for _ in 0..lines {
        let delta = get_len(index, &mut pos, raw_len)?;
        total = total.checked_add(delta).ok_or(DecodeError::BadIndex)?;
        let end = u32::try_from(total).map_err(|_| DecodeError::BadIndex)?;
        ends.push(end);
    }
    if pos != index.len() || total != raw_len {
        return Err(DecodeError::BadIndex);
    }
    Ok(())
}

fn validate_lines(buf: &mut ChunkBuf) -> Result<(), DecodeError> {
    let mut probe = std::mem::take(&mut buf.probe);
    let mut outcome = Ok(());
    for index in 0..buf.ends.len() {
        if let Some(Err(error)) = buf.line(index, &mut probe) {
            outcome = Err(error);
            break;
        }
    }
    buf.probe = probe;
    if outcome.is_err() {
        buf.ends.clear();
    }
    outcome
}

/// A blob helper for tests of this module's siblings.
#[cfg(test)]
pub(crate) fn varint_of(value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    crate::varint::put_varint(&mut out, value);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_garbage_without_panicking() {
        let mut buf = ChunkBuf::new();
        assert!(decode_chunk(&[], &mut buf).is_err());
        assert!(decode_chunk(&[0xFF; 40], &mut buf).is_err());
        // lines=1, raw_len=1 GiB-ish claim with 1 byte of LZ4: refused before allocating.
        let mut bomb = varint_of(1);
        bomb.extend(varint_of(60_000_000));
        bomb.extend(varint_of(0));
        bomb.push(0);
        assert!(matches!(decode_chunk(&bomb, &mut buf), Err(DecodeError::TooLarge(_))));
        assert_eq!(buf.line_count(), 0);
    }
}
