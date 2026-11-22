//! Allocator selection by cargo feature (one build per allocator). The `#[global_allocator]`
//! statics need no `unsafe` in our code; the crates carry their own `unsafe`.

#[cfg(all(feature = "mimalloc", feature = "jemalloc"))]
compile_error!("enable at most one of the features `mimalloc` and `jemalloc`");

#[cfg(all(feature = "jemalloc", target_os = "windows", target_env = "msvc"))]
compile_error!("tikv-jemallocator does not support Windows MSVC: not applicable");

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(all(feature = "jemalloc", not(all(target_os = "windows", target_env = "msvc"))))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

/// Name of the allocator this binary was built with.
pub(crate) const NAME: &str = if cfg!(feature = "mimalloc") {
    "mimalloc"
} else if cfg!(feature = "jemalloc") {
    "jemalloc"
} else {
    "system"
};
