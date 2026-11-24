//! Registration of the PTY crate's readiness sources with the `mio` loop.
//!
//! `ferrum-pty` reports a pollable descriptor on Unix ([`Readiness::Fd`], which only exists
//! there) and a notification on Windows ([`Readiness::Notify`]); this is the one place that turns
//! either into a wake-up of the loop, so the rest of the daemon is the same on every OS
//! (NFR-PORT-01).

use std::io;

use ferrum_pty::{Notify, Readiness};
use mio::{Interest, Registry, Token};

/// Where a source reports: its token and what it waits for (a descriptor only).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Slot {
    pub(crate) token: Token,
    pub(crate) interest: Interest,
}

/// Registers the source that reports `readiness` at `slot`. A notifying source gets nothing
/// registered: `notify` is given back, for the caller to hand to the source.
///
/// # Errors
/// [`io::ErrorKind::Unsupported`] for a source that can only block ([`Readiness::Blocking`]), or
/// the registry's error.
pub(crate) fn register(
    registry: &Registry,
    readiness: Readiness,
    slot: Slot,
    notify: Notify,
) -> io::Result<Option<Notify>> {
    #[cfg(not(unix))]
    let _unused = (registry, slot.token, slot.interest);
    match readiness {
        Readiness::Notify => Ok(Some(notify)),
        #[cfg(unix)]
        Readiness::Fd(fd) => registry
            .register(&mut mio::unix::SourceFd(&fd), slot.token, slot.interest)
            .map(|()| None),
        _ => Err(io::Error::new(io::ErrorKind::Unsupported, "a blocking source cannot be polled")),
    }
}

/// Changes what a registered descriptor waits for (a notifying source needs nothing).
pub(crate) fn reregister(registry: &Registry, readiness: Readiness, slot: Slot) {
    #[cfg(unix)]
    if let Readiness::Fd(fd) = readiness
        && let Err(error) =
            registry.reregister(&mut mio::unix::SourceFd(&fd), slot.token, slot.interest)
    {
        tracing::warn!(%error, "a session's descriptor could not be re-registered");
    }
    #[cfg(not(unix))]
    let _unused = (registry, readiness, slot.token, slot.interest);
}

/// Removes a source registered with [`register`] (a notifying source needs nothing).
pub(crate) fn deregister(registry: &Registry, readiness: Readiness) {
    #[cfg(unix)]
    if let Readiness::Fd(fd) = readiness {
        // The descriptor is still open: it is closed after this, with its reader.
        let _gone = registry.deregister(&mut mio::unix::SourceFd(&fd));
    }
    #[cfg(not(unix))]
    let _unused = (registry, readiness);
}
