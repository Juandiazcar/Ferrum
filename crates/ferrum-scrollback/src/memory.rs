//! Memory accounting by category (`docs/03` §2 lines), for `cargo xtask memcheck`.

/// Heap bytes held by one [`Scrollback`](crate::Scrollback), by the categories of `docs/03` §2.
///
/// Capacity-based (what the allocator was asked for), not length-based; it reconciles with `dhat`
/// requested bytes (test `memory_accounting`). Caller-owned read buffers
/// ([`LineCursor`](crate::LineCursor)) are not included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryReport {
    /// Hot chunk: data buffer capacity plus its 4 KiB line-offset index.
    pub hot_chunk_bytes: usize,
    /// Sealed chunks in RAM: LZ4 blobs (index included) plus the chunk list.
    pub sealed_bytes: usize,
    /// Style table and the line encoder scratch ("misc" line).
    pub misc_bytes: usize,
    /// LZ4 output scratch; shared by all sessions in the design (compression runs on one worker),
    /// so it is reported separately from the per-session total.
    pub shared_scratch_bytes: usize,
    /// Bytes of live records in the spill store (disk, not RAM).
    pub spill_disk_bytes: u64,
}

impl MemoryReport {
    /// Per-session RAM: hot chunk, sealed chunks and misc (the `docs/03` §2 per-session lines).
    pub const fn session_ram_bytes(&self) -> usize {
        self.hot_chunk_bytes + self.sealed_bytes + self.misc_bytes
    }

    /// Per-session RAM plus the shared scratch.
    pub const fn total_ram_bytes(&self) -> usize {
        self.session_ram_bytes() + self.shared_scratch_bytes
    }
}
