//! Chunks: the newest lines live in one uncompressed [`HotChunk`]; when it holds
//! [`HOT_CHUNK_LINES`](crate::HOT_CHUNK_LINES) lines it becomes a [`PendingChunk`] that a
//! [`ChunkSealer`] compresses, off the push path, into a self-describing LZ4 blob
//! (`docs/02` §3.2, §5).
//!
//! Sealed blob layout (also the spill record):
//!
//! ```text
//! blob := lines:varint | raw_len:varint | index_len:varint
//!         | index (index_len bytes: one varint per line, the encoded length of that line,
//!                  i.e. the delta between consecutive line offsets)
//!         | lz4_block (decompresses to raw_len bytes: the encoded lines back to back)
//! ```

mod hot;
mod pending;
mod read;
mod seal;

pub(crate) use hot::HotChunk;
pub use pending::{PendingChunk, SealedBlob};
pub use read::{ChunkBuf, decode_chunk};
pub use seal::ChunkSealer;
