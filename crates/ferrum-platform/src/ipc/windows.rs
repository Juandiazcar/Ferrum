//! Windows named-pipe endpoint (see the parent module). The `unsafe` Win32 calls live in [`sys`]
//! and [`crate::win32`]; this file only sees safe wrappers.

mod client;
mod sys;

use std::io;
use std::path::Path;
use std::sync::Arc;

use mio::event::Source;
use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_OVERLAPPED, SECURITY_IDENTIFICATION};

use mio::windows::NamedPipe;
use mio::{Interest, Registry, Token};

use super::{ClientStream, Connection, Listener};
use crate::win32::{Inherit, Security, User};

/// Prefix of the pipe name (`docs/10` §2).
const PIPE_PREFIX: &str = r"\\.\pipe\ferrum-";
/// FNV-1a 64-bit offset basis and prime (the name hash; not a security boundary, the DACL is).
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
/// Tries of a client against a pipe whose instances are all busy: each waits up to a quarter second
/// for the next instance, so a client gives up after two seconds of a daemon that stopped accepting.
const CONNECT_TRIES: u32 = 8;

/// The pipe name of this user (`docs/10` §2: `ferrum-<SID-hash>`), NUL-terminated UTF-16.
fn pipe_name(user: &User) -> io::Result<Vec<u16>> {
    let hash = user
        .sid_string()?
        .bytes()
        .fold(FNV_OFFSET, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME));
    Ok(format!("{PIPE_PREFIX}{hash:016x}").encode_utf16().chain([0]).collect())
}

pub(super) struct PipeListener {
    user: Arc<User>,
    security: Security,
    name: Vec<u16>,
    pending: NamedPipe,
    registration: Option<(Registry, Token)>,
}

impl PipeListener {
    /// The run directory does not name the pipe (`docs/10` §2); one daemon per user, as the instance
    /// lock also ensures.
    pub(super) fn create(_runtime_dir: &Path) -> io::Result<Self> {
        let user = Arc::new(User::current()?);
        let security = Security::owner_only(&user, Inherit::No)?;
        let name = pipe_name(&user)?;
        let pending = sys::create_pipe(&name, &security, sys::Instance::First)?;
        Ok(Self { user, security, name, pending, registration: None })
    }

    /// Starts listening on the pending instance; `Ok(true)` when a client is already connected.
    fn start_connect(&self) -> io::Result<bool> {
        match self.pending.connect() {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(false),
            Err(error) => Err(error),
        }
    }
}

impl Source for PipeListener {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.pending.register(registry, token, interests)?;
        self.registration = Some((registry.try_clone()?, token));
        // A client that connected before registration is picked up by the next `accept`.
        self.start_connect().map(drop)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.pending.reregister(registry, token, interests)?;
        self.registration = Some((registry.try_clone()?, token));
        Ok(())
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        self.registration = None;
        self.pending.deregister(registry)
    }
}

impl Listener for PipeListener {
    fn accept(&mut self) -> io::Result<Option<Box<dyn Connection>>> {
        let Some((registry, token)) = &self.registration else { return Ok(None) };
        if !self.start_connect()? {
            return Ok(None);
        }
        let next = sys::create_pipe(&self.name, &self.security, sys::Instance::Next)?;
        let connected = std::mem::replace(&mut self.pending, next);
        self.pending.register(registry, *token, Interest::READABLE | Interest::WRITABLE)?;
        // The next client may already wait; its event arrives on the new instance.
        self.start_connect()?;
        let user = Arc::clone(&self.user);
        Ok(Some(Box::new(PipeConnection { pipe: connected, on_listener: true, user })))
    }
}

/// A connected instance, still registered under the listener's token until the daemon
/// registers it under its own.
struct PipeConnection {
    pipe: NamedPipe,
    on_listener: bool,
    user: Arc<User>,
}

impl Connection for PipeConnection {
    fn is_peer_owner(&mut self) -> io::Result<bool> {
        Ok(User::of_pipe_client(&self.pipe)?.is(&self.user))
    }
}

impl io::Read for PipeConnection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        io::Read::read(&mut self.pipe, buf)
    }
}

impl io::Write for PipeConnection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        io::Write::write(&mut self.pipe, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(&mut self.pipe)
    }
}

impl Source for PipeConnection {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        if std::mem::take(&mut self.on_listener) {
            return self.pipe.reregister(registry, token, interests);
        }
        self.pipe.register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.on_listener = false;
        self.pipe.reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        self.on_listener = false;
        self.pipe.deregister(registry)
    }
}

pub(super) fn connect(_runtime_dir: &Path) -> io::Result<ClientStream> {
    let user = User::current()?;
    let name = pipe_name(&user)?;
    let path = String::from_utf16_lossy(name.split_last().map_or(&[][..], |(_, rest)| rest));
    let mut tries = 0;
    let file = loop {
        let mut options = std::fs::OpenOptions::new();
        let _same = options.read(true).write(true);
        // Overlapped, so a blocked read never holds up a write (see `client`); the server may
        // only identify this client, never act as it (`SECURITY_IDENTIFICATION`).
        std::os::windows::fs::OpenOptionsExt::custom_flags(&mut options, FILE_FLAG_OVERLAPPED);
        std::os::windows::fs::OpenOptionsExt::security_qos_flags(
            &mut options,
            SECURITY_IDENTIFICATION,
        );
        match options.open(&path) {
            Ok(file) => break file,
            Err(error) if sys::is_pipe_busy(&error) && tries < CONNECT_TRIES => {
                tries += 1;
                sys::wait_for_instance(&name);
            }
            Err(error) => return Err(error),
        }
    };
    // A pipe of this name created by another user before the daemon would get the nonce and the
    // keystrokes: the server must run as this user (`docs/10` §2), as the owner-only run
    // directory guarantees on Unix.
    if !User::of_pipe_server(&file)?.is(&user) {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "pipe server of another user"));
    }
    let file = Arc::new(file);
    let reader = client::Half::new(Arc::clone(&file))?;
    let writer = client::Half::new(file)?;
    Ok(ClientStream { reader: Box::new(reader), writer: Box::new(writer) })
}
