//! `ferrum-testkit`: dev-only helpers: fixtures, golden harness, `PTY` test driver, fake clock,
//! and shared contract suites.
//!
//! Contents so far: [`spill_contract`], the suite every `SpillStore` implementation must pass;
//! [`temp`], self-removing temporary directories for file-system tests; [`pty_contract`], the
//! suite every `PtyBackend` implementation must pass (PT-01 to PT-25), with its helper child
//! `ferrum-pty-helper`; [`fake_pty`], the deterministic fake `PtyBackend`; and [`single_test`],
//! the `main` of the single-threaded `dhat` test binaries.
//! The rest arrives with the work packages that need it (see `docs/spec/docs/12-build-order.md`).
//!
//! Allowed internal dependencies: any (from dev-dependencies only).
//! Must never depend on: being a normal dependency of any crate; it is publish = false.

#![forbid(unsafe_code)]

pub mod fake_pty;
pub mod pty_contract;
pub mod single_test;
pub mod spill_contract;
pub mod temp;
