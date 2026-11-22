//! `ferrum-testkit`: dev-only helpers: fixtures, golden harness, `PTY` test driver, fake clock,
//! and shared contract suites.
//!
//! Contents so far: [`spill_contract`], the suite every `SpillStore` implementation must pass.
//! The rest arrives with the work packages that need it (see `docs/spec/docs/12-build-order.md`).
//!
//! Allowed internal dependencies: any (from dev-dependencies only).
//! Must never depend on: being a normal dependency of any crate; it is publish = false.

#![forbid(unsafe_code)]

pub mod spill_contract;
