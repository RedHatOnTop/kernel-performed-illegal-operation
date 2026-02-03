//! CPU & Scheduler Optimization Module
//!
//! Provides scheduler tuning, tickless idle, and CPU optimization.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Scheduler tuning parameters
#[derive(Debug, Clone, Copy)]
pub struct SchedulerParams {
    /// Interactive task time slice in microseconds
    pub interactive_timeslice_us: u64,
    /// Interactive priority boost
    pub interactive_priority_boost: i32,
    /// Normal task time slice in microseconds
    pub normal_timeslice_us: u64,
    /// Background task time slice in microseconds
    pub background_timeslice_us: u64,
    /// Background priority adjustment
    pub background_priority: i32,
    /// Minimum granularity before preemption
    pub min_granularity_us: u64,
    /// Wakeup preemption threshold
    pub wakeup_preempt_threshold_us: u64,
}

/// Default optimized scheduler parameters
pub const OPTIMIZED_SCHED_PARAMS: SchedulerParams = SchedulerParams {
    // Interactive tasks (UI, input) - very short timeslice for responsiveness
    interactive_timeslice_us: 1_000,     // 1ms
    interactive_priority_boost: 5,
    
    // Normal tasks
    normal_timeslice_us: 10_000,         // 10ms
    
    // Background tasks (indexing, compression)
    background_timeslice_us: 50_000,     // 50ms
    background_priority: -5,
    
    // Preemption settings
    min_granularity_us: 750,             // Min 750µs before preempt
    wakeup_preempt_threshold_us: 500,    // Preempt if wakeup task waited > 500µs
};

/// Task classification for scheduler
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskClass {
    /// Interactive tasks (UI, input handling)
    Interactive,
    /// Normal user tasks
    Normal,
    /// Background tasks (indexing, updates)
    Background,
    /// Real-time tasks
    RealTime,
    /// Idle task
    Idle,
}

impl TaskClass {
    /// Get timeslice for this task class
    pub fn timeslice_us(&self, params: &SchedulerParams) -> u64 {
        match self {
            TaskClass::Interactive => params.interactive_timeslice_us,
            TaskClass::Normal => params.normal_timeslice_us,
            TaskClass::Background => params.background_timeslice_us,
            TaskClass::RealTime => params.min_granularity_us, // RT tasks run until done
            TaskClass::Idle => u64::MAX, // Idle runs when nothing else
        }
    }

    /// Get priority adjustment for this task class
    pub fn priority_adjustment(&self, params: &SchedulerParams) -> i32 {
        match self {
            TaskClass::Interactive => params.interactive_priority_boost,
            TaskClass::Normal => 0,
            TaskClass::Background => params.background_priority,
            TaskClass::RealTime => 99, // Highest priority
            TaskClass::Idle => -20,    // Lowest priority
        }
    }
}

/// Tickless scheduler for power efficiency
pub struct TicklessScheduler {
    /// Whether tick is currently disabled
    tick_disabled: AtomicBool,
    /// Next scheduled event timestamp
    next_event_ns: AtomicU64,
    /// Maximum idle time in nanoseconds
    max_idle_ns: u64,
    /// Total idle time
    total_idle_ns: AtomicU64,
    /// Number of idle periods
    idle_count: AtomicU64,
}

impl TicklessScheduler {
    /// Maximum idle time (10 seconds)
    const MAX_IDLE_TIME_NS: u64 = 10_000_000_000;

    /// Create a new tickless scheduler
    pub fn new() -> Self {
        Self {
            tick_disabled: AtomicBool::new(false),
            next_event_ns: AtomicU64::new(0),
            max_idle_ns: Self::MAX_IDLE_TIME_NS,
            total_idle_ns: AtomicU64::new(0),
            idle_count: AtomicU64::new(0),
        }
    }

    /// Enter tickless idle mode
    pub fn enter_idle(&self) {
        // Disable periodic tick
        self.tick_disabled.store(true, Ordering::Release);
        self.idle_count.fetch_add(1, Ordering::Relaxed);
        
        // In real implementation:
        // 1. Calculate time until next timer event
        // 2. Set one-shot timer for that time
        // 3. Enter low-power halt state
    }

    /// Exit from idle mode
    pub fn exit_idle(&self, idle_duration_ns: u64) {
        self.tick_disabled.store(false, Ordering::Release);
        self.total_idle_ns.fetch_add(idle_duration_ns, Ordering::Relaxed);
    }

    /// Check if tick is disabled
    pub fn is_tickless(&self) -> bool {
        self.tick_disabled.load(Ordering::Acquire)
    }

    /// Set next scheduled event
    pub fn set_next_event(&self, timestamp_ns: u64) {
        self.next_event_ns.store(timestamp_ns, Ordering::Release);
    }

    /// Get idle statistics
    pub fn stats(&self) -> TicklessStats {
        TicklessStats {
            total_idle_ns: self.total_idle_ns.load(Ordering::Relaxed),
            idle_count: self.idle_count.load(Ordering::Relaxed),
            currently_idle: self.tick_disabled.load(Ordering::Relaxed),
        }
    }
}

impl Default for TicklessScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Tickless idle statistics
#[derive(Debug, Clone, Copy)]
pub struct TicklessStats {
    pub total_idle_ns: u64,
    pub idle_count: u64,
    pub currently_idle: bool,
}

/// CPU affinity mask for multi-core scheduling
#[derive(Debug, Clone, Copy)]
pub struct CpuAffinityMask {
    mask: u64,
}

impl CpuAffinityMask {
    /// Create mask allowing all CPUs
    pub fn all() -> Self {
        Self { mask: u64::MAX }
    }

    /// Create mask for single CPU
    pub fn single(cpu_id: u32) -> Self {
        Self { mask: 1 << cpu_id }
    }

    /// Create mask for specific CPUs
    pub fn from_cpus(cpus: &[u32]) -> Self {
        let mut mask = 0u64;
        for &cpu in cpus {
            if cpu < 64 {
                mask |= 1 << cpu;
            }
        }
        Self { mask }
    }

    /// Check if CPU is allowed
    pub fn allows(&self, cpu_id: u32) -> bool {
        if cpu_id >= 64 {
            return false;
        }
        (self.mask & (1 << cpu_id)) != 0
    }

    /// Get first allowed CPU
    pub fn first_allowed(&self) -> Option<u32> {
        if self.mask == 0 {
            return None;
        }
        Some(self.mask.trailing_zeros())
    }

    /// Count allowed CPUs
    pub fn count(&self) -> u32 {
        self.mask.count_ones()
    }
}

/// Lock-free counter for performance
pub struct LockFreeCounter {
    value: AtomicU64,
}

impl LockFreeCounter {
    pub const fn new(initial: u64) -> Self {
        Self {
            value: AtomicU64::new(initial),
        }
    }

    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::Relaxed)
    }

    pub fn decrement(&self) -> u64 {
        self.value.fetch_sub(1, Ordering::Relaxed)
    }

    pub fn add(&self, n: u64) -> u64 {
        self.value.fetch_add(n, Ordering::Relaxed)
    }

    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    pub fn set(&self, value: u64) {
        self.value.store(value, Ordering::Relaxed);
    }
}

/// Batch operation accumulator
pub struct BatchAccumulator<T, const N: usize> {
    buffer: [Option<T>; N],
    count: usize,
}

impl<T: Copy, const N: usize> BatchAccumulator<T, N> {
    /// Create new batch accumulator
    pub fn new() -> Self {
        Self {
            buffer: [None; N],
            count: 0,
        }
    }

    /// Add item to batch
    pub fn add(&mut self, item: T) -> bool {
        if self.count < N {
            self.buffer[self.count] = Some(item);
            self.count += 1;
            true
        } else {
            false
        }
    }

    /// Check if batch is full
    pub fn is_full(&self) -> bool {
        self.count >= N
    }

    /// Get and clear batch
    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        let count = self.count;
        self.count = 0;
        self.buffer[..count].iter().filter_map(|x| *x)
    }

    /// Current batch size
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

impl<T: Copy, const N: usize> Default for BatchAccumulator<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_class_timeslice() {
        let params = OPTIMIZED_SCHED_PARAMS;
        
        assert_eq!(TaskClass::Interactive.timeslice_us(&params), 1_000);
        assert_eq!(TaskClass::Normal.timeslice_us(&params), 10_000);
        assert_eq!(TaskClass::Background.timeslice_us(&params), 50_000);
    }

    #[test]
    fn test_cpu_affinity() {
        let mask = CpuAffinityMask::from_cpus(&[0, 2, 4]);
        
        assert!(mask.allows(0));
        assert!(!mask.allows(1));
        assert!(mask.allows(2));
        assert_eq!(mask.count(), 3);
    }

    #[test]
    fn test_lock_free_counter() {
        let counter = LockFreeCounter::new(0);
        
        counter.increment();
        counter.increment();
        assert_eq!(counter.get(), 2);
        
        counter.decrement();
        assert_eq!(counter.get(), 1);
    }

    #[test]
    fn test_batch_accumulator() {
        let mut batch: BatchAccumulator<u32, 4> = BatchAccumulator::new();
        
        assert!(batch.add(1));
        assert!(batch.add(2));
        assert!(batch.add(3));
        assert!(batch.add(4));
        assert!(!batch.add(5)); // Full
        
        assert!(batch.is_full());
        
        let items: Vec<_> = batch.drain().collect();
        assert_eq!(items, vec![1, 2, 3, 4]);
        assert!(batch.is_empty());
    }
}
