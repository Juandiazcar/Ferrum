//! `ferrum-pty`: `PtyBackend` traits plus Unix and `ConPTY` implementations and child process lifecycle.
//!
//! Skeleton crate created by WP-00; the implementation arrives in a later work package
//! (see `docs/spec/docs/12-build-order.md`).
//!
//! Allowed internal dependencies: none (leaf service).
//! Must never depend on: any other ferrum crate; consumers use only its traits.

#![deny(unsafe_code)] // docs/05 §4: unsafe only in audited modules, each with `// SAFETY:`
