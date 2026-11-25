//! Unix domain socket endpoint (see the parent module).

use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use mio::net::{UnixListener, UnixStream};
use mio::{Interest, Registry, Token};

use super::{ClientStream, Connection, Listener, SOCKET_FILE};

/// Mode of the socket file (`docs/10` §2).
const SOCKET_MODE: u32 = 0o600;

pub(super) struct SocketListener(UnixListener);

impl SocketListener {
    pub(super) fn bind(runtime_dir: &Path) -> io::Result<Self> {
        let path = runtime_dir.join(SOCKET_FILE);
        // The caller holds the instance lock, so an existing socket is a crashed daemon's.
        match std::fs::remove_file(&path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
        let listener = UnixListener::bind(&path)?;
        // The directory is owner-only, so nobody else could reach the socket before this.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(SOCKET_MODE))?;
        Ok(Self(listener))
    }
}

impl mio::event::Source for SocketListener {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.0.register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.0.reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        self.0.deregister(registry)
    }
}

impl Listener for SocketListener {
    fn accept(&mut self) -> io::Result<Option<Box<dyn Connection>>> {
        let stream = match self.0.accept() {
            Ok((stream, _address)) => stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(None),
            Err(error) => return Err(error),
        };
        Ok(Some(Box::new(SocketConnection(stream))))
    }
}

struct SocketConnection(UnixStream);

impl Connection for SocketConnection {
    fn is_peer_owner(&mut self) -> io::Result<bool> {
        Ok(peer_uid(self.0.as_fd())? == rustix::process::geteuid().as_raw())
    }
}

impl io::Read for SocketConnection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl io::Write for SocketConnection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl mio::event::Source for SocketConnection {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.0.register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.0.reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        self.0.deregister(registry)
    }
}

pub(super) fn connect(runtime_dir: &Path) -> io::Result<ClientStream> {
    let stream = std::os::unix::net::UnixStream::connect(runtime_dir.join(SOCKET_FILE))?;
    let reader = stream.try_clone()?;
    Ok(ClientStream { reader: Box::new(reader), writer: Box::new(stream) })
}

#[cfg(target_os = "linux")]
fn peer_uid(fd: BorrowedFd<'_>) -> io::Result<u32> {
    Ok(rustix::net::sockopt::socket_peercred(fd)?.uid.as_raw())
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "docs/05 §4: getpeereid has no safe wrapper; SAFETY below")]
fn peer_uid(fd: BorrowedFd<'_>) -> io::Result<u32> {
    use std::os::fd::AsRawFd;
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    // SAFETY: `fd` is a valid, open socket for the duration of the borrow, and both out pointers
    // point to live local variables of the types the function writes.
    let result = unsafe { libc::getpeereid(fd.as_raw_fd(), &raw mut uid, &raw mut gid) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(uid)
}
