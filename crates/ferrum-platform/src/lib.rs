//! `ferrum-platform`: clipboard, directories, single-instance, own-process memory metric and daemon autostart behind traits.
//!
//! Skeleton crate created by WP-00; the implementation arrives in a later work package
//! (see `docs/spec/docs/12-build-order.md`).
//!
//! Allowed internal dependencies: none (leaf service).
//! Must never depend on: any other ferrum crate; pure crates never import it.

#![deny(unsafe_code)] // docs/05 §4: unsafe only in audited modules, each with `// SAFETY:`
