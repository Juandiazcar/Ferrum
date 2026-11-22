//! Heap accounting through `dhat` 0.3.3 (docs/07 §4), a safe audited crate: no `unsafe` in the
//! spike. `dhat` counts the requested size of every allocation. With the `dhat-heap` feature off
//! (throughput runs) every figure is 0 and [`ENABLED`] is false, because `dhat`'s
//! per-allocation backtraces distort timing.

/// True when heap figures are real.
pub(crate) const ENABLED: bool = cfg!(feature = "dhat-heap");

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static GLOBAL: dhat::Alloc = dhat::Alloc;

/// Keeps the `dhat` profiler alive; heap statistics are valid only while it exists.
pub(crate) struct Guard {
    #[cfg(feature = "dhat-heap")]
    _profiler: dhat::Profiler,
}

/// Starts profiling in testing mode (no output file).
pub(crate) fn start() -> Guard {
    Guard {
        #[cfg(feature = "dhat-heap")]
        _profiler: dhat::Profiler::builder().testing().build(),
    }
}

/// Bytes currently allocated.
#[cfg(feature = "dhat-heap")]
pub(crate) fn live() -> usize {
    usize::try_from(dhat::HeapStats::get().curr_bytes as u64).unwrap_or(usize::MAX)
}

/// Highest live bytes of the whole process since [`start`] (`dhat` has no reset).
#[cfg(feature = "dhat-heap")]
pub(crate) fn peak() -> usize {
    usize::try_from(dhat::HeapStats::get().max_bytes as u64).unwrap_or(usize::MAX)
}

/// Number of allocation blocks created so far.
#[cfg(feature = "dhat-heap")]
pub(crate) fn blocks() -> u64 {
    dhat::HeapStats::get().total_blocks
}

/// Bytes currently allocated (heap accounting disabled).
#[cfg(not(feature = "dhat-heap"))]
pub(crate) fn live() -> usize {
    0
}

/// Peak bytes (heap accounting disabled).
#[cfg(not(feature = "dhat-heap"))]
pub(crate) fn peak() -> usize {
    0
}

/// Allocation blocks (heap accounting disabled).
#[cfg(not(feature = "dhat-heap"))]
pub(crate) fn blocks() -> u64 {
    0
}
