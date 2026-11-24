//! `ferrum-pty`: the `PtyBackend` traits plus the Unix and `ConPTY` implementations and the child
//! process lifecycle (`docs/02` §2, ADR 0026).
//!
//! # Model
//!
//! A [`PtyBackend`] turns a [`SpawnSpec`] (program, arguments, explicit working directory,
//! environment changes, size) into a [`PtySession`]: four small objects, each handed to the code
//! that needs it. The [`PtyReader`] yields output and says how the caller learns it is ready
//! ([`Readiness`]: a pollable descriptor on Unix; on Windows a notification from the backend's
//! single I/O thread, maintainer decision Q-31; never a thread per session). The [`PtyWriter`]
//! may block, so the caller writes from its own thread. The [`PtyResize`] sets the size. The [`PtyChild`] observes the exit, kills, and
//! closes the session, which ends every process attached to the terminal.
//!
//! [`session_env`] is the environment every session gets (FR-SESS-007).
//!
//! Every implementation passes the shared contract suite `ferrum_testkit::pty_contract`
//! (PT-01 to PT-25 of ADR 0026); the deterministic fake lives in `ferrum-testkit` too.
//!
//! Allowed internal dependencies: none (leaf service).
//! Must never depend on: any other ferrum crate; consumers use only its traits.

#![deny(unsafe_code)] // docs/05 §4: unsafe only in audited modules, each with `// SAFETY:`

mod env;
mod error;
mod spec;
mod traits;

pub use env::{TERM, TERM_PROGRAM, session_env};
pub use error::PtyError;
pub use spec::{EnvOp, ExitSource, ExitStatus, PtySize, Readiness, SpawnSpec};
pub use traits::{
    Notify, PtyBackend, PtyChild, PtyKiller, PtyReader, PtyResize, PtySession, PtyWriter,
};
