//! `ferrum-daemon`: session manager, event loop, client server and layout persistence (library
//! behind the `ferrumd` binary).
//!
//! # Model
//!
//! [`Daemon`] owns every session (a [`ferrum_vt::Terminal`] fed by a PTY), every project and
//! every client connection, on one I/O thread polled with `mio` (`docs/02` §5). Output is parsed
//! inline with a fairness quantum per session; input is written without blocking when the PTY
//! has room; damage is coalesced per frame interval and sent only to the clients that show the
//! session; a slow client gets the latest state, never a queue. One worker thread compresses
//! history chunks, reflows lent terminals (interruptibly), searches history snapshots, closes
//! sessions and writes project files. A panic in one session's terminal fails that session only.
//! Session events go through an internal [`EventBus`] (`docs/02` §9). The daemon exits after a
//! grace period with no session and no client (FR-SESS-003).
//!
//! Platform behavior arrives through [`Ports`]: the PTY backend and exit watcher of `ferrum-pty`,
//! the IPC endpoint and memory release of `ferrum-platform`, the spill stores and a [`Clock`],
//! all as traits; the real implementations are wired only in `src/main.rs` (ADR 0023).
//!
//! Allowed internal dependencies: ferrum-vt, ferrum-scrollback, ferrum-proto, ferrum-config, ferrum-project, ferrum-pty, ferrum-platform (traits only in the library; real implementations are wired only in `src/main.rs`, ADR 0023).
//! Must never depend on: ferrum-ui, ferrum-render, ferrum-app, winit.

#![forbid(unsafe_code)]

mod bus;
mod client;
mod clock;
mod config;
mod cwd;
mod encode;
mod error;
pub mod frame;
mod layout;
pub mod logging;
mod os_source;
mod projects;
pub mod search;
mod server;
mod session;
mod view;
mod worker;

pub use bus::{EventBus, EventSubscriber};
pub use clock::{Clock, SystemClock};
pub use config::DaemonConfig;
pub use error::DaemonError;
pub use server::{Daemon, Ports, SpillFactory};
pub use view::View;
