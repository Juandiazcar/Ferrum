//! `ferrum-testkit`: dev-only helpers: fixtures, golden harness, `PTY` test driver, fake clock.
//!
//! Skeleton crate created by WP-00; the implementation arrives in a later work package
//! (see `docs/spec/docs/12-build-order.md`).
//!
//! Allowed internal dependencies: any (from dev-dependencies only).
//! Must never depend on: being a normal dependency of any crate; it is publish = false.

#![forbid(unsafe_code)]
