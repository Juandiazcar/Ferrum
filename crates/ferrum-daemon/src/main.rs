//! `ferrumd`: the Ferrum daemon executable (composition root for the daemon side).
//!
//! Skeleton from WP-00. It deliberately does nothing yet; WP-06 wires the real PTY and platform
//! implementations into `ferrum_daemon` here, and only here: the library depends on
//! `ferrum-platform` for its traits alone (ADR 0023).

#![forbid(unsafe_code)]

fn main() {}
