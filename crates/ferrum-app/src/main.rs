//! `ferrum` (crate `ferrum-app`): composition root of the `ferrum` binary: `winit` glue, event loop and wiring of all crates.
//!
//! Skeleton crate created by WP-00; the implementation arrives in a later work package
//! (see `docs/spec/docs/12-build-order.md`).
//!
//! Allowed internal dependencies: ferrum-vt, ferrum-scrollback, ferrum-proto, ferrum-pty, ferrum-daemon, ferrum-render, ferrum-ui, ferrum-config, ferrum-project, ferrum-platform.
//! Must never depend on: ferrum-testkit (except as a dev-dependency).

#![forbid(unsafe_code)]

fn main() {}
