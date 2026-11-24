//! The error type of every `ferrum-pty` operation.
//!
//! The `Display` text of every variant is fixed and never contains the program, arguments,
//! environment values or paths (`docs/10` §4). Sources are OS errors (`std::io::Error`), whose text
//! is the operating system's message for an error code, also without user data.

/// Why a PTY operation failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PtyError {
    /// The spawn request is malformed (empty program, NUL byte, bad variable name, too long).
    #[error("invalid spawn request: {0}")]
    InvalidSpec(&'static str),
    /// The requested working directory is not an existing directory.
    #[error("the working directory does not exist or is not a directory")]
    InvalidCwd,
    /// The program was not found or cannot be executed.
    #[error("the program was not found or is not executable")]
    ProgramNotFound,
    /// The session was closed; no further I/O is possible.
    #[error("the session is closed")]
    Closed,
    /// An operating system call failed.
    #[error("{op} failed")]
    Io {
        /// The operation that failed.
        op: &'static str,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
}

impl PtyError {
    /// Wraps an I/O error with the name of the operation that produced it.
    pub fn io(op: &'static str, source: std::io::Error) -> Self {
        Self::Io { op, source }
    }
}
