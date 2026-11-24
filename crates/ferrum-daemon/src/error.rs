//! The error type of the daemon library.
//!
//! `Display` texts are fixed and never contain terminal content, paths or environment values
//! (`docs/05` §8, `docs/10` §4); sources carry OS or library errors whose texts are equally free
//! of user data, except [`ferrum_config::ConfigError`], which is only ever logged by kind.

/// Why the daemon could not start or had to stop.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DaemonError {
    /// The event loop's poller or waker could not be created or used.
    #[error("event loop failure: {op}")]
    Poll {
        /// The operation that failed.
        op: &'static str,
        /// The OS error.
        #[source]
        source: std::io::Error,
    },
    /// The worker thread could not be started.
    #[error("the worker thread could not start")]
    Worker(#[source] std::io::Error),
    /// The project files could not be read.
    #[error("the projects could not be loaded")]
    Projects(#[source] ferrum_config::ConfigError),
    /// A PTY facility the daemon needs failed.
    #[error("PTY failure")]
    Pty(#[from] ferrum_pty::PtyError),
}

impl DaemonError {
    pub(crate) fn poll(op: &'static str) -> impl FnOnce(std::io::Error) -> Self {
        move |source| Self::Poll { op, source }
    }
}
