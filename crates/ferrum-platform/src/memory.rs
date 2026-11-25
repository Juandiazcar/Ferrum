//! Returning freed heap memory to the OS (`docs/03` §3 rule 7, NFR-MEM-03): after large
//! operations the allocator keeps freed pages; an idle daemon gives them back.
//!
//! Each OS uses its allocator's own call (ADR 0016): `malloc_trim(0)` for glibc on Linux,
//! `mi_collect(true)` for `mimalloc` on macOS, and `HeapCompact` for the process heap the system
//! allocator uses on Windows. The caller decides when (at most once per 30 s while idle).

#![allow(unsafe_code, reason = "docs/05 §4: the allocators' release calls, each with SAFETY")]

/// Gives freed heap memory back to the OS.
pub trait MemoryRelease: Send {
    /// Releases what the allocator can; never fails (a call that frees nothing is harmless).
    fn release(&self);
}

/// The allocator of this process (ADR 0016).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemMemory;

impl MemoryRelease for SystemMemory {
    fn release(&self) {
        release_free_memory();
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn release_free_memory() {
    // SAFETY: `malloc_trim` only walks glibc's own arenas; 0 keeps no extra padding. Its result
    // (whether memory was released) needs no handling.
    let _released = unsafe { libc::malloc_trim(0) };
}

#[cfg(target_os = "macos")]
fn release_free_memory() {
    // SAFETY: `mi_collect` only walks mimalloc's own heaps; `true` also returns abandoned pages.
    unsafe { libmimalloc_sys::mi_collect(true) };
}

#[cfg(windows)]
fn release_free_memory() {
    use windows_sys::Win32::System::Memory::{GetProcessHeap, HeapCompact};
    // SAFETY: the process heap handle is valid for the life of the process; compacting it only
    // coalesces and decommits free blocks. The returned size needs no handling.
    let _largest = unsafe { HeapCompact(GetProcessHeap(), 0) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releasing_memory_is_harmless() {
        let freed = vec![0_u8; 1 << 20];
        drop(freed);
        SystemMemory.release();
        SystemMemory.release();
    }
}
