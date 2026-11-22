//! `ferrum-vt`: `VT` state machine on top of `vte`: grid, modes, charsets, selection model, hyperlinks, shell marks.
//!
//! Skeleton crate created by WP-00; the implementation arrives in a later work package
//! (see `docs/spec/docs/12-build-order.md`).
//!
//! Allowed internal dependencies: ferrum-scrollback.
//! Must never depend on: ferrum-daemon, ferrum-pty, ferrum-platform, ferrum-proto, any OS or socket code.

#![forbid(unsafe_code)]
