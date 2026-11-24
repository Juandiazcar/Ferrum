//! `ferrum-pty`: the `PtyBackend` traits plus the Unix and `ConPTY` implementations and the child
//! process lifecycle (`docs/02` §2, ADR 0026).
//!
//! # Model
//!
//! A [`PtyBackend`] turns a [`SpawnSpec`] (program, arguments, explicit working directory,
//! environment changes, size) into a [`PtySession`]: four small objects, each handed to the code
//! that needs it. The [`PtyReader`] yields output and says how the caller learns it is ready
//! ([`Readiness`]: a pollable descriptor on Unix, a blocking reader thread from
//! [`spawn_reader_thread`] on Windows). The [`PtyWriter`] may block, so the caller writes from its
//! own thread. The [`PtyResize`] sets the size. The [`PtyChild`] observes the exit, kills, and
//! closes the session, which ends every process attached to the terminal.
//!
//! [`session_env`] is the environment every session gets (FR-SESS-007);
//! [`shell_integration`] makes bash, zsh, fish and `PowerShell` report the working directory and
//! prompt marks (FR-SESS-005).
//!
//! Every implementation passes the shared contract suite `ferrum_testkit::pty_contract`
//! (PT-01 to PT-25 of ADR 0026); the deterministic fake lives in `ferrum-testkit` too.
//!
//! Allowed internal dependencies: none (leaf service).
//! Must never depend on: any other ferrum crate; consumers use only its traits.

#![deny(unsafe_code)] // docs/05 §4: unsafe only in audited modules, each with `// SAFETY:`

#[cfg(windows)]
mod conpty;
mod env;
mod error;
#[cfg(any(unix, windows))]
mod program;
mod reader_thread;
pub mod shell_integration;
mod spec;
mod traits;
#[cfg(unix)]
mod unix;
#[cfg(any(windows, test))]
mod windows_command;

#[cfg(windows)]
pub use conpty::ConptyBackend;
pub use env::{TERM, TERM_PROGRAM, session_env};
pub use error::PtyError;
pub use reader_thread::{ChunkSink, READ_BUFFER_BYTES, READER_STACK_BYTES, spawn_reader_thread};
pub use spec::{EnvOp, ExitSource, ExitStatus, PtySize, Readiness, SpawnSpec};
pub use traits::{PtyBackend, PtyChild, PtyKiller, PtyReader, PtyResize, PtySession, PtyWriter};
#[cfg(unix)]
pub use unix::UnixBackend;

/// The backend of the running OS: `UnixBackend` on Linux and macOS, `ConptyBackend` on Windows.
pub fn native_backend() -> Box<dyn PtyBackend> {
    #[cfg(unix)]
    let backend = UnixBackend;
    #[cfg(windows)]
    let backend = ConptyBackend;
    Box::new(backend)
}
