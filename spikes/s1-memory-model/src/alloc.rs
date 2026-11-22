//! Counting global allocator: sums the sizes of every live allocation (requested `Layout` sizes).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicUsize = AtomicUsize::new(0);

/// Wraps the system allocator and tracks live and peak requested bytes.
pub(crate) struct Counting;

fn add(n: usize) {
    let now = LIVE.fetch_add(n, Relaxed) + n;
    PEAK.fetch_max(now, Relaxed);
    CALLS.fetch_add(1, Relaxed);
}

#[allow(unsafe_code)] // The only unsafe in the spike: a GlobalAlloc must be an unsafe impl.
// SAFETY: every method forwards to `System` with the caller's arguments unchanged, so all
// `GlobalAlloc` contracts are upheld by `System`; the counters are plain atomics.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: caller upholds the `alloc` contract; forwarded unchanged.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            add(layout.size());
        }
        p
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: caller upholds the `alloc_zeroed` contract; forwarded unchanged.
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            add(layout.size());
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr`/`layout` come from a prior allocation by this allocator (i.e. `System`).
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size(), Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: `ptr`/`layout` come from a prior allocation by this allocator; forwarded as is.
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            LIVE.fetch_sub(layout.size(), Relaxed);
            add(new_size);
        }
        p
    }
}

/// Bytes currently allocated.
pub(crate) fn live() -> usize {
    LIVE.load(Relaxed)
}

/// Highest value of [`live`] since the last [`reset_peak`].
pub(crate) fn peak() -> usize {
    PEAK.load(Relaxed)
}

/// Restarts peak tracking from the current live value.
pub(crate) fn reset_peak() {
    PEAK.store(LIVE.load(Relaxed), Relaxed);
}

/// Number of allocation calls (`alloc`, `alloc_zeroed`, `realloc`) so far.
pub(crate) fn calls() -> usize {
    CALLS.load(Relaxed)
}
