//! Sealing: LZ4-compress the hot chunk into a blob with a delta-encoded line index.

use lz4_flex::block::{CompressTable, compress_into_with_table, get_maximum_output_size};

use super::HotChunk;
use crate::error::ScrollbackError;
use crate::varint::put_len;

/// Scratch buffers shared by all seals of a session (`docs/02` §5: compression runs on one worker,
/// so the scratch is shared, not per chunk).
pub(crate) struct Sealer {
    index: Vec<u8>,
    out: Vec<u8>,
    /// LZ4 hash table, reused so a seal allocates nothing but its result (`docs/05` §6).
    table: CompressTable,
}

/// Heap size of the large LZ4 hash table (4 096 entries of `u32`), counted as shared scratch.
const LZ4_TABLE_BYTES: usize = 4_096 * size_of::<u32>();

impl Default for Sealer {
    fn default() -> Self {
        // The large table is needed from the first seal: a full chunk is above 64 KiB of input
        // only sometimes, and a table upgrade later would allocate on the hot path.
        Self { index: Vec::new(), out: Vec::new(), table: CompressTable::large() }
    }
}

impl std::fmt::Debug for Sealer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sealer").field("scratch_bytes", &self.capacity_bytes()).finish()
    }
}

impl Sealer {
    /// Builds the sealed blob of `hot` (which must hold at least one line).
    pub(crate) fn seal(&mut self, hot: &HotChunk) -> Result<Box<[u8]>, ScrollbackError> {
        self.index.clear();
        let raw = hot.data();
        let mut previous = 0usize;
        for &start in hot.starts().iter().skip(1).chain(std::iter::once(&u32_len(raw.len()))) {
            let start = usize::try_from(start).unwrap_or(raw.len());
            put_len(&mut self.index, start.saturating_sub(previous));
            previous = start;
        }
        self.out.clear();
        put_len(&mut self.out, hot.len());
        put_len(&mut self.out, raw.len());
        put_len(&mut self.out, self.index.len());
        self.out.extend_from_slice(&self.index);
        let header = self.out.len();
        let total = header + get_maximum_output_size(raw.len());
        // Exact growth: `resize` alone would double the capacity of this shared scratch.
        self.out.reserve_exact(total.saturating_sub(self.out.len()));
        self.out.resize(total, 0);
        let target = self.out.get_mut(header..).ok_or(ScrollbackError::Compress)?;
        let written = compress_into_with_table(raw, target, &mut self.table)
            .map_err(|_| ScrollbackError::Compress)?;
        self.out.truncate(header + written);
        Ok(Box::from(self.out.as_slice()))
    }

    /// Capacity of the scratch buffers, for memory accounting.
    pub(crate) fn capacity_bytes(&self) -> usize {
        self.index.capacity() + self.out.capacity() + LZ4_TABLE_BYTES
    }
}

fn u32_len(len: usize) -> u32 {
    // INVARIANT: a hot chunk is at most 64 MiB, below u32::MAX.
    u32::try_from(len).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{ChunkBuf, decode_chunk};
    use crate::encode::Encoder;
    use crate::line::{DecodedLine, LineFlags, LineRef};
    use crate::style::StyleTable;

    fn sealed_blob(texts: &[&str]) -> Box<[u8]> {
        let (mut styles, mut encoder, mut hot) =
            (StyleTable::new(), Encoder::default(), HotChunk::new());
        for text in texts {
            let line = LineRef { flags: LineFlags::NONE, text, runs: &[] };
            let planned = encoder.plan(&mut styles, &line).expect("plan");
            hot.append(&encoder, &planned);
        }
        Sealer::default().seal(&hot).expect("seal")
    }

    #[test]
    fn sealed_blob_decodes_back_to_the_same_lines() {
        let texts = ["alpha", "", "gamma gamma gamma gamma", "漢字 😀"];
        let blob = sealed_blob(&texts);
        let mut buf = ChunkBuf::new();
        decode_chunk(&blob, &mut buf).expect("decode_chunk");
        assert_eq!(buf.line_count(), texts.len());
        let mut line = DecodedLine::new();
        for (index, text) in texts.iter().enumerate() {
            buf.line(index, &mut line).expect("in range").expect("valid line");
            assert_eq!(line.text, *text);
        }
        assert!(buf.line(texts.len(), &mut line).is_none());
    }

    #[test]
    fn every_single_byte_corruption_is_handled_without_panicking() {
        let blob = sealed_blob(&["alpha beta gamma", "delta", "epsilon zeta eta theta"]);
        let mut buf = ChunkBuf::new();
        for position in 0..blob.len() {
            for flip in [0x01u8, 0x80, 0xFF] {
                let mut damaged = blob.to_vec();
                damaged[position] ^= flip;
                // Either outcome is fine, but a failure must leave no lines behind.
                if decode_chunk(&damaged, &mut buf).is_err() {
                    assert_eq!(buf.line_count(), 0, "byte {position} ^ {flip:#x}");
                }
            }
        }
        for cut in 0..blob.len() {
            assert!(decode_chunk(&blob[..cut], &mut buf).is_err(), "truncated at {cut}");
        }
    }
}
