//! std::time compatibility layer for KPIO
//!
//! Provides time-related functionality via KPIO syscalls.

use crate::syscall;
use core::ops::{Add, Sub};

/// A measurement of a monotonically nondecreasing clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant {
    nanos: u64,
}

impl Instant {
    /// Returns an instant corresponding to "now".
    pub fn now() -> Instant {
        let nanos = syscall::time_monotonic().unwrap_or(0);
        Instant { nanos }
    }

    /// Returns the amount of time elapsed since this instant.
    pub fn elapsed(&self) -> Duration {
        let now = Instant::now();
        now.duration_since(*self)
    }

    /// Returns the duration since the earlier instant.
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        Duration::from_nanos(self.nanos.saturating_sub(earlier.nanos))
    }

    /// Returns `Some(t)` if `t` is after this instant.
    pub fn checked_add(&self, duration: Duration) -> Option<Instant> {
        self.nanos
            .checked_add(duration.as_nanos() as u64)
            .map(|n| Instant { nanos: n })
    }

    /// Returns `Some(t)` if `t` is before this instant.
    pub fn checked_sub(&self, duration: Duration) -> Option<Instant> {
        self.nanos
            .checked_sub(duration.as_nanos() as u64)
            .map(|n| Instant { nanos: n })
    }

    /// Saturating addition.
    pub fn saturating_duration_since(&self, earlier: Instant) -> Duration {
        self.duration_since(earlier)
    }
}

impl Add<Duration> for Instant {
    type Output = Instant;

    fn add(self, rhs: Duration) -> Instant {
        self.checked_add(rhs)
            .expect("overflow when adding duration to instant")
    }
}

impl Sub<Duration> for Instant {
    type Output = Instant;

    fn sub(self, rhs: Duration) -> Instant {
        self.checked_sub(rhs)
            .expect("overflow when subtracting duration from instant")
    }
}

impl Sub<Instant> for Instant {
    type Output = Duration;

    fn sub(self, rhs: Instant) -> Duration {
        self.duration_since(rhs)
    }
}

/// A measurement of the system clock (wall clock time).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemTime {
    secs: u64,
    nanos: u32,
}

impl SystemTime {
    /// The Unix epoch (1970-01-01 00:00:00 UTC).
    pub const UNIX_EPOCH: SystemTime = SystemTime { secs: 0, nanos: 0 };

    /// Returns the system time corresponding to "now".
    pub fn now() -> SystemTime {
        let (secs, nanos) = syscall::time_realtime().unwrap_or((0, 0));
        SystemTime { secs, nanos }
    }

    /// Returns the duration since UNIX_EPOCH.
    pub fn duration_since_epoch(&self) -> Duration {
        Duration::new(self.secs, self.nanos)
    }

    /// Returns the duration since the given time.
    pub fn duration_since(&self, earlier: SystemTime) -> Result<Duration, SystemTimeError> {
        if *self < earlier {
            return Err(SystemTimeError(
                earlier.duration_since_epoch() - self.duration_since_epoch(),
            ));
        }
        Ok(self.duration_since_epoch() - earlier.duration_since_epoch())
    }

    /// Returns the amount of time elapsed since this time.
    pub fn elapsed(&self) -> Result<Duration, SystemTimeError> {
        SystemTime::now().duration_since(*self)
    }

    /// Checked addition.
    pub fn checked_add(&self, duration: Duration) -> Option<SystemTime> {
        let total_nanos = self.nanos as u64 + duration.subsec_nanos() as u64;
        let extra_secs = total_nanos / 1_000_000_000;
        let nanos = (total_nanos % 1_000_000_000) as u32;

        self.secs
            .checked_add(duration.as_secs())
            .and_then(|s| s.checked_add(extra_secs))
            .map(|secs| SystemTime { secs, nanos })
    }

    /// Checked subtraction.
    pub fn checked_sub(&self, duration: Duration) -> Option<SystemTime> {
        let total_nanos = self.duration_since_epoch();
        total_nanos.checked_sub(duration).map(|d| SystemTime {
            secs: d.as_secs(),
            nanos: d.subsec_nanos(),
        })
    }
}

impl Add<Duration> for SystemTime {
    type Output = SystemTime;

    fn add(self, rhs: Duration) -> SystemTime {
        self.checked_add(rhs)
            .expect("overflow when adding duration to system time")
    }
}

impl Sub<Duration> for SystemTime {
    type Output = SystemTime;

    fn sub(self, rhs: Duration) -> SystemTime {
        self.checked_sub(rhs)
            .expect("overflow when subtracting duration from system time")
    }
}

/// Error returned from SystemTime operations.
#[derive(Debug, Clone)]
pub struct SystemTimeError(Duration);

impl SystemTimeError {
    /// Returns the positive duration representing how far the time is.
    pub fn duration(&self) -> Duration {
        self.0
    }
}

/// A duration of time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Duration {
    secs: u64,
    nanos: u32,
}

impl Duration {
    /// Zero duration.
    pub const ZERO: Duration = Duration { secs: 0, nanos: 0 };

    /// Maximum duration.
    pub const MAX: Duration = Duration {
        secs: u64::MAX,
        nanos: 999_999_999,
    };

    /// One second.
    pub const SECOND: Duration = Duration { secs: 1, nanos: 0 };

    /// One millisecond.
    pub const MILLISECOND: Duration = Duration {
        secs: 0,
        nanos: 1_000_000,
    };

    /// One microsecond.
    pub const MICROSECOND: Duration = Duration {
        secs: 0,
        nanos: 1_000,
    };

    /// One nanosecond.
    pub const NANOSECOND: Duration = Duration { secs: 0, nanos: 1 };

    /// Creates a new duration.
    pub const fn new(secs: u64, nanos: u32) -> Duration {
        let extra_secs = (nanos / 1_000_000_000) as u64;
        let nanos = nanos % 1_000_000_000;
        Duration {
            secs: secs + extra_secs,
            nanos,
        }
    }

    /// Creates a duration from seconds.
    pub const fn from_secs(secs: u64) -> Duration {
        Duration { secs, nanos: 0 }
    }

    /// Creates a duration from milliseconds.
    pub const fn from_millis(millis: u64) -> Duration {
        Duration {
            secs: millis / 1_000,
            nanos: ((millis % 1_000) * 1_000_000) as u32,
        }
    }

    /// Creates a duration from microseconds.
    pub const fn from_micros(micros: u64) -> Duration {
        Duration {
            secs: micros / 1_000_000,
            nanos: ((micros % 1_000_000) * 1_000) as u32,
        }
    }

    /// Creates a duration from nanoseconds.
    pub const fn from_nanos(nanos: u64) -> Duration {
        Duration {
            secs: nanos / 1_000_000_000,
            nanos: (nanos % 1_000_000_000) as u32,
        }
    }

    /// Creates a duration from seconds (floating point).
    pub fn from_secs_f64(secs: f64) -> Duration {
        let whole_secs = secs as u64;
        let nanos = ((secs - whole_secs as f64) * 1_000_000_000.0) as u32;
        Duration {
            secs: whole_secs,
            nanos,
        }
    }

    /// Creates a duration from seconds (f32).
    pub fn from_secs_f32(secs: f32) -> Duration {
        Self::from_secs_f64(secs as f64)
    }

    /// Returns true if duration is zero.
    pub const fn is_zero(&self) -> bool {
        self.secs == 0 && self.nanos == 0
    }

    /// Returns the whole seconds.
    pub const fn as_secs(&self) -> u64 {
        self.secs
    }

    /// Returns the sub-second nanoseconds.
    pub const fn subsec_nanos(&self) -> u32 {
        self.nanos
    }

    /// Returns the sub-second microseconds.
    pub const fn subsec_micros(&self) -> u32 {
        self.nanos / 1_000
    }

    /// Returns the sub-second milliseconds.
    pub const fn subsec_millis(&self) -> u32 {
        self.nanos / 1_000_000
    }

    /// Returns total milliseconds.
    pub const fn as_millis(&self) -> u128 {
        self.secs as u128 * 1_000 + self.nanos as u128 / 1_000_000
    }

    /// Returns total microseconds.
    pub const fn as_micros(&self) -> u128 {
        self.secs as u128 * 1_000_000 + self.nanos as u128 / 1_000
    }

    /// Returns total nanoseconds.
    pub const fn as_nanos(&self) -> u128 {
        self.secs as u128 * 1_000_000_000 + self.nanos as u128
    }

    /// Returns as f64 seconds.
    pub fn as_secs_f64(&self) -> f64 {
        self.secs as f64 + self.nanos as f64 / 1_000_000_000.0
    }

    /// Returns as f32 seconds.
    pub fn as_secs_f32(&self) -> f32 {
        self.as_secs_f64() as f32
    }

    /// Checked addition.
    pub fn checked_add(self, rhs: Duration) -> Option<Duration> {
        let nanos = self.nanos + rhs.nanos;
        let extra_secs = (nanos / 1_000_000_000) as u64;
        let nanos = nanos % 1_000_000_000;

        self.secs
            .checked_add(rhs.secs)
            .and_then(|s| s.checked_add(extra_secs))
            .map(|secs| Duration { secs, nanos })
    }

    /// Checked subtraction.
    pub fn checked_sub(self, rhs: Duration) -> Option<Duration> {
        let mut secs = self.secs.checked_sub(rhs.secs)?;
        let nanos = if self.nanos >= rhs.nanos {
            self.nanos - rhs.nanos
        } else {
            secs = secs.checked_sub(1)?;
            self.nanos + 1_000_000_000 - rhs.nanos
        };
        Some(Duration { secs, nanos })
    }

    /// Saturating addition.
    pub fn saturating_add(self, rhs: Duration) -> Duration {
        self.checked_add(rhs).unwrap_or(Duration::MAX)
    }

    /// Saturating subtraction.
    pub fn saturating_sub(self, rhs: Duration) -> Duration {
        self.checked_sub(rhs).unwrap_or(Duration::ZERO)
    }

    /// Checked multiplication.
    pub fn checked_mul(self, rhs: u32) -> Option<Duration> {
        let total_nanos = self.nanos as u64 * rhs as u64;
        let extra_secs = total_nanos / 1_000_000_000;
        let nanos = (total_nanos % 1_000_000_000) as u32;

        self.secs
            .checked_mul(rhs as u64)
            .and_then(|s| s.checked_add(extra_secs))
            .map(|secs| Duration { secs, nanos })
    }

    /// Checked division.
    pub fn checked_div(self, rhs: u32) -> Option<Duration> {
        if rhs == 0 {
            return None;
        }

        let secs = self.secs / rhs as u64;
        let carry = self.secs % rhs as u64;
        let total_nanos = carry * 1_000_000_000 + self.nanos as u64;
        let nanos = (total_nanos / rhs as u64) as u32;

        Some(Duration { secs, nanos })
    }
}

impl Add for Duration {
    type Output = Duration;

    fn add(self, rhs: Duration) -> Duration {
        self.checked_add(rhs)
            .expect("overflow when adding durations")
    }
}

impl Sub for Duration {
    type Output = Duration;

    fn sub(self, rhs: Duration) -> Duration {
        self.checked_sub(rhs)
            .expect("overflow when subtracting durations")
    }
}

impl core::ops::Mul<u32> for Duration {
    type Output = Duration;

    fn mul(self, rhs: u32) -> Duration {
        self.checked_mul(rhs)
            .expect("overflow when multiplying duration")
    }
}

impl core::ops::Div<u32> for Duration {
    type Output = Duration;

    fn div(self, rhs: u32) -> Duration {
        self.checked_div(rhs).expect("division by zero or overflow")
    }
}

// ============================================
// Free functions
// ============================================

/// Put the current thread to sleep for the specified duration.
pub fn sleep(duration: Duration) {
    let _ = syscall::sleep_ns(duration.as_nanos() as u64);
}
