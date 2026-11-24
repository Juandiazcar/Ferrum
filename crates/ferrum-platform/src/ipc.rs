//! The daemon's local IPC endpoint (`docs/02` §4, `docs/10` §2), behind traits so the daemon
//! library never names an OS type (ADR 0023).
//!
//! - Unix: a domain socket `ferrumd.sock` in the owner-only run directory, mode 0600; the peer
//!   must have the daemon's effective user id (`SO_PEERCRED` on Linux, `getpeereid` on macOS).
//! - Windows: the named pipe `\\.\pipe\ferrum-<SID-hash>`, created with a DACL granting only the
//!   current user and `SYSTEM`, first instance, remote clients rejected; the peer's pipe client
//!   token (impersonation at identification level) must carry the daemon's user SID.
//!
//! The server side is non-blocking and served by the daemon's `mio` loop ([`Listener`],
//! [`Connection`]; maintainer decision Q-32). Every OS checks the peer the same way: the daemon
//! calls [`Connection::is_peer_owner`] once the first bytes arrived (Windows can impersonate a
//! pipe client only after reading from it) and closes the connection when it is `false`.
//! [`connect`] is the blocking client the tests and the measurement tools use; it verifies the
//! server too (Windows: the pipe server process runs as the current user). The UI's client is
//! `interprocess` (Q-32, WP-08).

use std::io::{self, Read, Write};
use std::path::Path;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

/// File name of the socket in the run directory (Unix).
pub const SOCKET_FILE: &str = "ferrumd.sock";

/// An accepted client connection: non-blocking, registered by the daemon with its `mio` registry.
pub trait Connection: mio::event::Source + Read + Write + Send {
    /// Whether the peer runs as the daemon's user (`docs/10` §2). Call it once the first bytes were
    /// read from the connection.
    ///
    /// # Errors
    /// The OS error of querying the peer; treat it as a refusal.
    fn is_peer_owner(&mut self) -> io::Result<bool>;
}

/// The listening endpoint. Register it for readability; on each event call
/// [`accept`](Self::accept) until it returns `Ok(None)`.
pub trait Listener: mio::event::Source + Send {
    /// The next pending connection, without blocking; check its peer with
    /// [`Connection::is_peer_owner`].
    ///
    /// # Errors
    /// The OS error.
    fn accept(&mut self) -> io::Result<Option<Box<dyn Connection>>>;
}

/// Creates the endpoint for the run directory `runtime_dir` (prepared with
/// [`Dirs::prepare_runtime`](crate::Dirs::prepare_runtime)). Call it only while holding the
/// [`InstanceLock`](crate::InstanceLock): a socket file left by a crashed daemon is replaced.
///
/// # Errors
/// The OS error of creating the endpoint.
pub fn listen(runtime_dir: &Path) -> io::Result<Box<dyn Listener>> {
    #[cfg(unix)]
    let listener = unix::SocketListener::bind(runtime_dir)?;
    #[cfg(windows)]
    let listener = windows::PipeListener::create(runtime_dir)?;
    Ok(Box::new(listener))
}

/// A blocking client connection, split so one thread can read while another writes.
pub struct ClientStream {
    /// Bytes from the daemon.
    pub reader: Box<dyn Read + Send>,
    /// Bytes to the daemon.
    pub writer: Box<dyn Write + Send>,
}

/// Connects to the daemon of the run directory `runtime_dir` (test support and tools; the UI uses
/// `interprocess`, Q-32).
///
/// # Errors
/// [`io::ErrorKind::NotFound`] (Windows) or `ConnectionRefused`/`NotFound` (Unix) when no daemon
/// listens, or the OS error.
pub fn connect(runtime_dir: &Path) -> io::Result<ClientStream> {
    #[cfg(unix)]
    let stream = unix::connect(runtime_dir)?;
    #[cfg(windows)]
    let stream = windows::connect(runtime_dir)?;
    Ok(stream)
}
