//! Sealing: LZ4-compress a full chunk into a blob with a delta-encoded line index.

use std::sync::Arc;

use lz4_flex::block::{CompressTable, compress_into_with_table, get_maximum_output_size};

use super::hot::free_if_above_keep;
use super::{HotChunk, PendingChunk, SealedBlob};
use crate::error::ScrollbackError;
use crate::varint::put_len;

/// Compresses full chunks; owns the scratch buffers that every seal reuses.
///
/// The daemon worker owns one and seals the [`PendingChunk`]s of every session with it
/// (`docs/02` §5: compression runs on one worker, so the scratch is shared, not per chunk). Each
/// [`Scrollback`](crate::Scrollback) also owns one for the inline fallback.
pub struct ChunkSealer {
    index: Vec<u8>,
    out: Vec<u8>,
    /// LZ4 hash table, reused so a seal allocates nothing but its result (`docs/05` §6).
    table: CompressTable,
    /// Test hook: how many seals succeed before every later one fails as a failed LZ4 call would
    /// (compression cannot fail on real input).
    #[cfg(test)]
    pub(crate) failing_after: Option<usize>,
}

/// Heap size of the large LZ4 hash table (4 096 entries of `u32`), counted as shared scratch.
const LZ4_TABLE_BYTES: usize = 4_096 * size_of::<u32>();

impl Default for ChunkSealer {
    fn default() -> Self {
        // The large table is needed from the first seal: a full chunk is above 64 KiB of input
        // only sometimes, and a table upgrade later would allocate on the hot path.
        Self {
            index: Vec::new(),
            out: Vec::new(),
            table: CompressTable::large(),
            #[cfg(test)]
            failing_after: None,
        }
    }
}

impl std::fmt::Debug for ChunkSealer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkSealer").field("scratch_bytes", &self.capacity_bytes()).finish()
    }
}

impl ChunkSealer {
    /// A sealer with its LZ4 table allocated (16 KiB); the output scratch grows on the first seal.
    pub fn new() -> Self {
        Self::default()
    }

    /// Compresses `pending` into the blob that [`Scrollback::install`](crate::Scrollback::install)
    /// puts in its place. Allocates the blob (the retained data itself) and nothing else once the
    /// scratch has its working size. `pending` is consumed and dropped before the blob is
    /// returned, so the scrollback can reuse its buffers when the blob is installed.
    ///
    /// # Errors
    /// [`ScrollbackError::Compress`] if LZ4 fails; the chunk then stays pending (readable) until
    /// it is evicted or spilled (spilling seals it inline).
    pub fn seal(&mut self, pending: PendingChunk) -> Result<SealedBlob, ScrollbackError> {
        let PendingChunk { origin, id, lines } = pending;
        let blob = self.seal_hot(&lines)?;
        // Released here so the owner finds the buffer unshared when it installs the blob.
        drop(lines);
        Ok(SealedBlob { origin, id, blob })
    }

    /// Builds the sealed blob of `hot` (which must hold at least one line).
    pub(crate) fn seal_hot(&mut self, hot: &HotChunk) -> Result<Arc<[u8]>, ScrollbackError> {
        #[cfg(test)]
        if let Some(left) = self.failing_after.as_mut() {
            *left = left.checked_sub(1).ok_or(ScrollbackError::Compress)?;
        }
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
        // Shared, so a history snapshot can hold it while the session goes on (one allocation).
        let blob = Arc::from(self.out.as_slice());
        free_if_above_keep(&mut self.out);
        free_if_above_keep(&mut self.index);
        Ok(blob)
    }

    /// Capacity of the scratch buffers, for memory accounting.
    pub fn capacity_bytes(&self) -> usize {
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

    fn sealed_blob(texts: &[&str]) -> Arc<[u8]> {
        let (mut styles, mut encoder, mut hot) =
            (StyleTable::new(), Encoder::default(), HotChunk::new());
        for text in texts {
            let line = LineRef { flags: LineFlags::NONE, text, runs: &[] };
            let planned = encoder.plan(&mut styles, &line).expect("plan");
            hot.append(&encoder, &planned);
        }
        ChunkSealer::default().seal_hot(&hot).expect("seal")
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

    /// Writes valid sealed blobs as the initial fuzz corpus:
    /// `cargo test -p ferrum-scrollback seeds -- --ignored` (the fuzzer grows it from there).
    #[test]
    #[ignore = "regenerates crates/ferrum-scrollback/fuzz/corpus/decode_chunk"]
    fn write_fuzz_seeds() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fuzz")
            .join("corpus")
            .join("decode_chunk");
        std::fs::create_dir_all(&dir).expect("corpus dir");
        let seeds: [&[&str]; 4] = [
            &["one line"],
            &["alpha", "", "gamma gamma gamma gamma", "漢字 😀 é\u{301}"],
            &["", "", ""],
            &["wide 漢字漢字漢字漢字漢字漢字", "\t tab", "error: warning: error: warning: error"],
        ];
        for (index, texts) in seeds.iter().enumerate() {
            std::fs::write(dir.join(format!("seed-{index}")), sealed_blob(texts)).expect("seed");
        }
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
