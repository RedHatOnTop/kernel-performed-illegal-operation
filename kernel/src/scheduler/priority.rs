//! Task priority levels.
//!
//! This module defines the priority levels used for task scheduling.

/// Task priority level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Idle priority (lowest).
    Idle = 0,
    /// Low priority.
    Low = 8,
    /// Below normal priority.
    BelowNormal = 12,
    /// Normal priority (default).
    Normal = 16,
    /// Above normal priority.
    AboveNormal = 20,
    /// High priority.
    High = 24,
    /// Realtime priority (highest).
    Realtime = 31,
}

impl Priority {
    /// Get the numeric priority level (0-31).
    pub fn level(self) -> usize {
        self as usize
    }

    /// Create a priority from a numeric level.
    pub fn from_level(level: usize) -> Self {
        match level {
            0..=3 => Priority::Idle,
            4..=9 => Priority::Low,
            10..=13 => Priority::BelowNormal,
            14..=17 => Priority::Normal,
            18..=21 => Priority::AboveNormal,
            22..=27 => Priority::High,
            _ => Priority::Realtime,
        }
    }

    /// Get the next higher priority.
    pub fn higher(self) -> Self {
        match self {
            Priority::Idle => Priority::Low,
            Priority::Low => Priority::BelowNormal,
            Priority::BelowNormal => Priority::Normal,
            Priority::Normal => Priority::AboveNormal,
            Priority::AboveNormal => Priority::High,
            Priority::High => Priority::Realtime,
            Priority::Realtime => Priority::Realtime,
        }
    }

    /// Get the next lower priority.
    pub fn lower(self) -> Self {
        match self {
            Priority::Idle => Priority::Idle,
            Priority::Low => Priority::Idle,
            Priority::BelowNormal => Priority::Low,
            Priority::Normal => Priority::BelowNormal,
            Priority::AboveNormal => Priority::Normal,
            Priority::High => Priority::AboveNormal,
            Priority::Realtime => Priority::High,
        }
    }
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

/// Priority boost for I/O bound tasks.
pub const IO_BOOST: usize = 4;

/// Priority penalty for CPU bound tasks.
pub const CPU_PENALTY: usize = 2;

/// Maximum priority boost.
pub const MAX_BOOST: usize = 8;
