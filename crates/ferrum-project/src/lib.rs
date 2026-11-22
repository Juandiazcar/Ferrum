//! `ferrum-project`: project model, `.git/HEAD` reader, `cwd` tracking logic and startup recipes.
//!
//! Skeleton crate created by WP-00; the implementation arrives in a later work package
//! (see `docs/spec/docs/12-build-order.md`).
//!
//! Allowed internal dependencies: ferrum-config (types only, if needed).
//! Must never depend on: ferrum-daemon, ferrum-ui, OS-specific code.

#![forbid(unsafe_code)]
