//! Time sources (a fake one for tests).

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// A monotonic clock.
pub trait Clock: Send + Sync {
    /// Now.
    fn now(&self) -> Instant;
}

/// The real clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// A clock that only moves when told to.
#[derive(Debug)]
pub struct FakeClock {
    base: Instant,
    offset: Mutex<Duration>,
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeClock {
    /// Starts at an arbitrary instant.
    pub fn new() -> Self {
        Self {
            base: Instant::now(),
            offset: Mutex::new(Duration::ZERO),
        }
    }

    /// Move forward.
    pub fn advance(&self, d: Duration) {
        let mut o = self.offset.lock().unwrap_or_else(PoisonError::into_inner);
        *o += d;
    }

    /// Time elapsed since creation.
    pub fn elapsed(&self) -> Duration {
        *self.offset.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        self.base + self.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_moves_only_when_told() {
        let c = FakeClock::new();
        let t0 = c.now();
        assert_eq!(c.now(), t0);
        c.advance(Duration::from_millis(5));
        assert_eq!(c.now() - t0, Duration::from_millis(5));
        assert!(SystemClock.now() >= t0);
    }
}
