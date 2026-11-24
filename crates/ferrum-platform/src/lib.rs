//! `ferrum-platform`: clipboard, directories, single-instance, own-process memory metric and daemon autostart behind traits.
//!
//! Contents so far: [`private_fs`], owner-only file and directory creation (WP-04). The rest
//! arrives with the work packages that need it (see `docs/spec/docs/12-build-order.md`).
//!
//! Allowed internal dependencies: none (leaf service).
//! Must never depend on: any other ferrum crate; pure crates never import it.

#![deny(unsafe_code)] // docs/05 §4: unsafe only in audited modules, each with `// SAFETY:`

pub mod private_fs;
