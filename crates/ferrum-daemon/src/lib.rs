//! `ferrum-daemon`: session manager, event loop, client server and layout persistence (library behind the `ferrumd` binary).
//!
//! Skeleton crate created by WP-00; the implementation arrives in a later work package
//! (see `docs/spec/docs/12-build-order.md`).
//!
//! Allowed internal dependencies: ferrum-vt, ferrum-scrollback, ferrum-proto, ferrum-config, ferrum-project, ferrum-pty.
//! Must never depend on: ferrum-ui, ferrum-render, ferrum-app, winit.

#![forbid(unsafe_code)]
