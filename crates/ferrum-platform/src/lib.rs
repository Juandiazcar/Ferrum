//! `ferrum-platform`: clipboard, directories, single-instance, own-process memory metric and daemon autostart behind traits.
//!
//! Contents so far: [`private_fs`], owner-only file and directory creation (WP-04); [`Dirs`], the
//! per-user directories; [`memory`], freed heap memory back to the OS; [`InstanceLock`] and the handshake nonce file ([`instance`]); and
//! [`default_shell`], the shell without a profile; and [`ipc`], the daemon's local endpoint (Unix socket or Windows named pipe) with peer checks
//! behind the [`ipc::Listener`] and [`ipc::Connection`] traits (WP-06). The rest arrives with the
//! work packages that need it (see `docs/spec/docs/12-build-order.md`).
//!
//! Allowed internal dependencies: none (leaf service).
//! Must never depend on: any other ferrum crate; pure crates never import it.

#![deny(unsafe_code)] // docs/05 §4: unsafe only in audited modules, each with `// SAFETY:`

mod dirs;
pub mod instance;
pub mod ipc;
pub mod memory;
pub mod private_fs;
mod shells;
#[cfg(windows)]
mod win32;

pub use dirs::{
    CONFIG_DIR_VAR, DATA_DIR_VAR, Dirs, PORTABLE_MARKER, RUNTIME_DIR_VAR, STATE_DIR_VAR,
    prepare_private_dir,
};
pub use instance::{InstanceLock, LockError};
pub use shells::default_shell;
