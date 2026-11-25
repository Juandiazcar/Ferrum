//! Time as the daemon sees it (`docs/05` §2: `ferrum-daemon` defines `Clock`), so timers (frame
//! pacing, the idle exit) are testable with a fake clock.

use std::time::Instant;

/// A monotonic clock.
pub trait Clock: Send {
    /// The current instant.
    fn now(&self) -> Instant;
}

/// The OS monotonic clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}
