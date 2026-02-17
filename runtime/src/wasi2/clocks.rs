// WASI Preview 2 — Clocks (wasi:clocks/monotonic-clock + wall-clock)
//
// This module wraps the existing clock functionality into the
// WASI P2 clock interfaces.

extern crate alloc;

// ---------------------------------------------------------------------------
// Monotonic Clock
// ---------------------------------------------------------------------------

/// Monotonic clock state.
pub struct MonotonicClock {
    /// Counter incremented on each `now()` call.
    /// In a real kernel, this would read TSC / HPET.
    counter: u64,
    /// Resolution in nanoseconds.
    resolution_ns: u64,
}

impl MonotonicClock {
    /// Create a new monotonic clock with nanosecond resolution.
    pub fn new() -> Self {
        Self {
            counter: 0,
            resolution_ns: 1_000, // 1 microsecond resolution
        }
    }

    /// Get the current time in nanoseconds.
    ///
    /// Returns a monotonically increasing value.
    pub fn now(&mut self) -> u64 {
        self.counter += self.resolution_ns;
        self.counter
    }

    /// Get the clock resolution in nanoseconds.
    pub fn resolution(&self) -> u64 {
        self.resolution_ns
    }

    /// Subscribe to a specific instant (returns immediately in our impl).
    pub fn subscribe_instant(&self, _instant: u64) -> bool {
        true // Always ready
    }

    /// Subscribe to a duration from now (returns immediately).
    pub fn subscribe_duration(&self, _duration: u64) -> bool {
        true // Always ready
    }
}

// ---------------------------------------------------------------------------
// Wall Clock
// ---------------------------------------------------------------------------

/// A date-time value as returned by the wall clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallDatetime {
    /// Whole seconds since Unix epoch.
    pub seconds: u64,
    /// Additional nanoseconds (0..999_999_999).
    pub nanoseconds: u32,
}

/// Wall clock (real-world time).
pub struct WallClock {
    /// Base seconds (simulated epoch time — Jan 1, 2025 00:00:00 UTC).
    base_seconds: u64,
    /// Monotonic counter for advancing time.
    tick_counter: u64,
}

impl WallClock {
    /// Create a new wall clock with a default base time.
    pub fn new() -> Self {
        Self {
            // Approx Jan 1, 2025 00:00:00 UTC
            base_seconds: 1_735_689_600,
            tick_counter: 0,
        }
    }

    /// Create a wall clock with a specific base time.
    pub fn with_base(seconds: u64) -> Self {
        Self {
            base_seconds: seconds,
            tick_counter: 0,
        }
    }

    /// Get the current wall-clock time.
    pub fn now(&mut self) -> WallDatetime {
        self.tick_counter += 1;
        WallDatetime {
            seconds: self.base_seconds + self.tick_counter,
            nanoseconds: 0,
        }
    }

    /// Get the wall clock resolution.
    pub fn resolution(&self) -> WallDatetime {
        WallDatetime {
            seconds: 1,
            nanoseconds: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_clock_increases() {
        let mut clock = MonotonicClock::new();
        let t1 = clock.now();
        let t2 = clock.now();
        let t3 = clock.now();
        assert!(t2 > t1);
        assert!(t3 > t2);
    }

    #[test]
    fn monotonic_clock_resolution() {
        let clock = MonotonicClock::new();
        assert!(clock.resolution() > 0);
    }

    #[test]
    fn monotonic_subscribe_always_ready() {
        let clock = MonotonicClock::new();
        assert!(clock.subscribe_instant(1_000_000));
        assert!(clock.subscribe_duration(500_000));
    }

    #[test]
    fn wall_clock_returns_valid_time() {
        let mut clock = WallClock::new();
        let dt = clock.now();
        assert!(dt.seconds > 1_700_000_000); // After ~2023
        assert!(dt.nanoseconds < 1_000_000_000);
    }

    #[test]
    fn wall_clock_advances() {
        let mut clock = WallClock::new();
        let t1 = clock.now();
        let t2 = clock.now();
        assert!(t2.seconds > t1.seconds);
    }

    #[test]
    fn wall_clock_resolution() {
        let clock = WallClock::new();
        let res = clock.resolution();
        assert_eq!(res.seconds, 1);
    }

    #[test]
    fn wall_clock_custom_base() {
        let mut clock = WallClock::with_base(1_000_000);
        let dt = clock.now();
        assert_eq!(dt.seconds, 1_000_001);
    }
}
