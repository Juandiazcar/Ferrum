//! `ferrum-render`: font loading, glyph cache and damage-limited `CPU` painting into a pixel buffer.
//!
//! Skeleton crate created by WP-00; the implementation arrives in a later work package
//! (see `docs/spec/docs/12-build-order.md`).
//!
//! Allowed internal dependencies: none (no windowing; takes plain values, not config types).
//! Must never depend on: winit, softbuffer, ferrum-config and any other ferrum crate.

#![deny(unsafe_code)] // docs/05 §4: unsafe only in audited modules, each with `// SAFETY:`
