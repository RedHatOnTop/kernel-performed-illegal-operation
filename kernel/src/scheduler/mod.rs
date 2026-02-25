//! Kernel scheduler module.
//!
//! This module implements cooperative and preemptive task scheduling
//! for both kernel tasks and WASM processes.

pub mod context;
pub mod optimization;
pub mod priority;
pub mod round_robin;
pub mod task;

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;

pub use context::SwitchContext;
pub use priority::Priority;
pub use task::{Task, TaskId, TaskState};

/// Maximum number of priority levels.
const MAX_PRIORITY_LEVELS: usize = 32;

/// Default time slice in timer ticks.
const DEFAULT_TIME_SLICE: u64 = 10;

/// Global scheduler instance.
static SCHEDULER: Mutex<Option<Scheduler>> = Mutex::new(None);

/// Current task ID.
static CURRENT_TASK_ID: AtomicU64 = AtomicU64::new(0);

/// Total context switches counter.
static CONTEXT_SWITCHES: AtomicU64 = AtomicU64::new(0);

/// Boot tick counter (incremented every timer tick).
static BOOT_TICKS: AtomicU64 = AtomicU64::new(0);

/// Preemption nesting counter.
/// When > 0, preemptive scheduling is inhibited.
static PREEMPT_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Flag: a reschedule was requested while preemption was disabled.
static PREEMPT_PENDING: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Initialize the scheduler.
pub fn init() {
    let mut scheduler = SCHEDULER.lock();
    *scheduler = Some(Scheduler::new());

    let sched = scheduler.as_mut().unwrap();

    // Create idle task
    let idle_task = Task::new_idle();
    sched.add_task(idle_task);

    // Register the boot (main kernel) context as the currently
    // running task.  Its SwitchContext will be filled in by the
    // first call to `switch_context()`.
    let boot_task = Task::new_boot_task();
    let boot_arc = Arc::new(Mutex::new(boot_task));
    sched.all_tasks.push(boot_arc.clone());
    sched.current_task = Some(boot_arc);
    CURRENT_TASK_ID.store(0, Ordering::Relaxed);

    crate::serial_println!("[SCHED] Scheduler initialized with preemptive support");
}

/// Spawn a new task.
pub fn spawn(task: Task) -> TaskId {
    let id = task.id();
    if let Some(ref mut scheduler) = *SCHEDULER.lock() {
        scheduler.add_task(task);
    }
    id
}

/// Schedule the next task (may trigger a context switch).
///
/// This function is safe to call from interrupt context or from
/// cooperative yield points.  It extracts context-switch pointers
/// under the scheduler lock, drops the lock, and then performs
/// the assembly-level register swap.
pub fn schedule() {
    // Don't reschedule while preemption is disabled.
    if PREEMPT_COUNT.load(Ordering::Relaxed) > 0 {
        PREEMPT_PENDING.store(true, Ordering::Relaxed);
        return;
    }

    // Determine prev/next SwitchContext pointers under the lock.
    let switch_info: Option<(*mut SwitchContext, *const SwitchContext)> = {
        if let Some(mut guard) = SCHEDULER.try_lock() {
            if let Some(ref mut scheduler) = *guard {
                scheduler.prepare_switch()
            } else {
                None
            }
        } else {
            // Lock is held — defer the switch.
            None
        }
    };
    // Lock is dropped here — safe to switch stacks.

    if let Some((prev_ptr, next_ptr)) = switch_info {
        CONTEXT_SWITCHES.fetch_add(1, Ordering::Relaxed);
        unsafe {
            context::switch_context(prev_ptr, next_ptr);
        }
    }
}

/// Get the current task ID.
pub fn current_task_id() -> TaskId {
    TaskId(CURRENT_TASK_ID.load(Ordering::Relaxed))
}

/// Yield the current task.
pub fn yield_now() {
    schedule();
}

/// Block the current task.
pub fn block_current() {
    if let Some(ref mut scheduler) = *SCHEDULER.lock() {
        scheduler.block_task(current_task_id());
    }
}

/// Unblock a task.
pub fn unblock(task_id: TaskId) {
    if let Some(ref mut scheduler) = *SCHEDULER.lock() {
        scheduler.unblock_task(task_id);
    }
}

/// Exit the current task.
pub fn exit_current(exit_code: i32) {
    if let Some(ref mut scheduler) = *SCHEDULER.lock() {
        scheduler.exit_task(current_task_id(), exit_code);
    }
    schedule();
}

/// Get total context switches.
pub fn context_switch_count() -> u64 {
    CONTEXT_SWITCHES.load(Ordering::Relaxed)
}

/// Get boot tick count (incremented each timer_tick).
pub fn boot_ticks() -> u64 {
    BOOT_TICKS.load(Ordering::Relaxed)
}

/// Get total number of tasks (including blocked/terminated).
pub fn total_task_count() -> usize {
    if let Some(ref sched) = *SCHEDULER.lock() {
        sched.all_tasks.len()
    } else {
        0
    }
}

/// Sleep the current task for a number of ticks.
pub fn sleep_ticks(ticks: u64) {
    if let Some(ref mut scheduler) = *SCHEDULER.lock() {
        let current = current_task_id();
        let wake_at = BOOT_TICKS.load(Ordering::Relaxed) + ticks;
        scheduler.sleep_task(current, wake_at);
    }
    schedule();
}

/// Timer tick handler (called from timer interrupt).
///
/// Increments boot ticks, runs sleep-queue wake-ups and time-slice
/// countdown, then triggers a reschedule if the current task's
/// time slice has expired.
pub fn timer_tick() {
    BOOT_TICKS.fetch_add(1, Ordering::Relaxed);

    // Try to lock the scheduler — if we can't (someone else holds
    // the lock), skip this tick to avoid deadlock.  The timer will
    // fire again on the next tick.
    let should_schedule = {
        if let Some(mut guard) = SCHEDULER.try_lock() {
            if let Some(ref mut scheduler) = *guard {
                scheduler.timer_tick();
                scheduler.needs_reschedule()
            } else {
                false
            }
        } else {
            // Scheduler lock is held elsewhere — skip this tick.
            false
        }
    };

    if should_schedule {
        schedule();
    }
}

// ==================== Preemption Guards ====================

/// Disable preemptive scheduling.
///
/// Increments a nesting counter.  While the counter is > 0,
/// `schedule()` will record a pending request but not perform
/// the actual context switch.
pub fn preempt_disable() {
    PREEMPT_COUNT.fetch_add(1, Ordering::SeqCst);
}

/// Re-enable preemptive scheduling.
///
/// Decrements the nesting counter.  When the counter reaches zero
/// and a reschedule was requested in the meantime, it is performed
/// immediately.
pub fn preempt_enable() {
    let prev = PREEMPT_COUNT.fetch_sub(1, Ordering::SeqCst);
    if prev == 1 {
        // Counter reached zero — check for pending reschedule.
        if PREEMPT_PENDING.swap(false, Ordering::SeqCst) {
            schedule();
        }
    }
}

/// Returns `true` when preemption is currently disabled.
pub fn is_preemption_disabled() -> bool {
    PREEMPT_COUNT.load(Ordering::Relaxed) > 0
}

/// The scheduler implementation.
pub struct Scheduler {
    /// Ready queues (one per priority level).
    ready_queues: [VecDeque<Arc<Mutex<Task>>>; MAX_PRIORITY_LEVELS],

    /// Currently running task.
    current_task: Option<Arc<Mutex<Task>>>,

    /// All tasks in the system.
    all_tasks: Vec<Arc<Mutex<Task>>>,

    /// Blocked tasks waiting on events.
    blocked_tasks: Vec<Arc<Mutex<Task>>>,

    /// Sleep queue: (wake_at_tick, task).
    sleep_queue: Vec<(u64, Arc<Mutex<Task>>)>,

    /// Current time slice remaining.
    time_slice_remaining: u64,

    /// Next task ID.
    next_task_id: u64,

    /// Whether preemption is enabled.
    preemption_enabled: bool,

    /// Flag: need reschedule on next safe point.
    need_reschedule: bool,
}

impl Scheduler {
    /// Create a new scheduler.
    pub fn new() -> Self {
        const EMPTY_QUEUE: VecDeque<Arc<Mutex<Task>>> = VecDeque::new();

        Scheduler {
            ready_queues: [EMPTY_QUEUE; MAX_PRIORITY_LEVELS],
            current_task: None,
            all_tasks: Vec::new(),
            blocked_tasks: Vec::new(),
            sleep_queue: Vec::new(),
            time_slice_remaining: DEFAULT_TIME_SLICE,
            next_task_id: 1,
            preemption_enabled: true,
            need_reschedule: false,
        }
    }

    /// Add a task to the scheduler.
    pub fn add_task(&mut self, task: Task) {
        let priority = task.priority().level();
        let task = Arc::new(Mutex::new(task));
        self.all_tasks.push(task.clone());
        self.ready_queues[priority].push_back(task);
    }

    /// Schedule the next task.
    ///
    /// Moves the current task back onto its ready queue and selects
    /// the highest-priority runnable task.  Does **not** perform the
    /// actual register-level context switch (see `prepare_switch`).
    pub fn schedule(&mut self) {
        // Save current task state
        if let Some(ref current) = self.current_task {
            let mut task = current.lock();
            if task.state() == TaskState::Running {
                task.set_state(TaskState::Ready);
                let priority = task.priority().level();
                drop(task);
                self.ready_queues[priority].push_back(current.clone());
            }
        }

        // Find next task (highest priority first)
        let mut next_task = None;
        for queue in self.ready_queues.iter_mut().rev() {
            if let Some(task) = queue.pop_front() {
                next_task = Some(task);
                break;
            }
        }

        if let Some(task) = next_task {
            task.lock().set_state(TaskState::Running);
            let task_id = task.lock().id().0;
            self.current_task = Some(task);
            self.time_slice_remaining = DEFAULT_TIME_SLICE;
            self.need_reschedule = false;

            CURRENT_TASK_ID.store(task_id, Ordering::Relaxed);
        }
    }

    /// Prepare a context switch and return raw pointers.
    ///
    /// Calls `schedule()` to pick the next task, then returns
    /// `(prev_switch_ctx_ptr, next_switch_ctx_ptr)` that the
    /// caller can pass to `switch_context()` **after** dropping
    /// the scheduler lock.
    ///
    /// Returns `None` when there is nothing to switch to, or when
    /// prev == next (same task picked again).
    pub fn prepare_switch(&mut self) -> Option<(*mut SwitchContext, *const SwitchContext)> {
        let prev_arc = self.current_task.clone();

        self.schedule();

        let next_arc = self.current_task.clone();

        // Both must exist.
        let (prev, next) = match (prev_arc, next_arc) {
            (Some(p), Some(n)) => (p, n),
            (None, Some(n)) => {
                // First ever switch — no previous task.
                // We'll set up a bootstrap context instead.
                return None;
            }
            _ => return None,
        };

        // Don't switch to ourselves.
        if Arc::ptr_eq(&prev, &next) {
            return None;
        }

        // Update stats on the new task.
        {
            let mut t = next.lock();
            t.stats_mut().context_switches += 1;
            t.stats_mut().last_scheduled = BOOT_TICKS.load(Ordering::Relaxed);
        }

        // Extract raw pointers into the Arc-owned data.
        // SAFETY: The Arcs keep the Tasks alive.  The pointers
        // are valid until the next time we lock these tasks.
        // We only use them for one `switch_context` call while
        // the scheduler lock is dropped.
        let prev_ptr: *mut SwitchContext = {
            let mut g = prev.lock();
            g.switch_ctx_mut() as *mut SwitchContext
        };
        let next_ptr: *const SwitchContext = {
            let g = next.lock();
            g.switch_ctx() as *const SwitchContext
        };

        Some((prev_ptr, next_ptr))
    }

    /// Block a task.
    pub fn block_task(&mut self, task_id: TaskId) {
        for task in &self.all_tasks {
            if task.lock().id() == task_id {
                task.lock().set_state(TaskState::Blocked);
                self.blocked_tasks.push(task.clone());
                break;
            }
        }
    }

    /// Unblock a task.
    pub fn unblock_task(&mut self, task_id: TaskId) {
        let mut found_index = None;
        for (i, task) in self.blocked_tasks.iter().enumerate() {
            if task.lock().id() == task_id {
                found_index = Some(i);
                break;
            }
        }

        if let Some(index) = found_index {
            let task = self.blocked_tasks.remove(index);
            let priority = task.lock().priority().level();
            task.lock().set_state(TaskState::Ready);
            self.ready_queues[priority].push_back(task);
        }
    }

    /// Exit a task.
    pub fn exit_task(&mut self, task_id: TaskId, exit_code: i32) {
        for task in &self.all_tasks {
            if task.lock().id() == task_id {
                task.lock().set_state(TaskState::Terminated);
                task.lock().set_exit_code(exit_code);
                break;
            }
        }
    }

    /// Handle timer tick.
    /// Decrements time slice, checks sleep queue for wake-ups,
    /// and sets reschedule flag when time slice expires.
    pub fn timer_tick(&mut self) {
        if !self.preemption_enabled {
            return;
        }

        // Wake sleeping tasks whose deadline has passed
        let now = BOOT_TICKS.load(Ordering::Relaxed);
        let mut i = 0;
        while i < self.sleep_queue.len() {
            if self.sleep_queue[i].0 <= now {
                let (_, task) = self.sleep_queue.remove(i);
                let priority = task.lock().priority().level();
                task.lock().set_state(TaskState::Ready);
                self.ready_queues[priority].push_back(task);
            } else {
                i += 1;
            }
        }

        if self.time_slice_remaining > 0 {
            self.time_slice_remaining -= 1;
        }

        if self.time_slice_remaining == 0 {
            self.need_reschedule = true;
        }
    }

    /// Put a task to sleep until a given tick.
    pub fn sleep_task(&mut self, task_id: TaskId, wake_at: u64) {
        for task in &self.all_tasks {
            if task.lock().id() == task_id {
                task.lock().set_state(TaskState::Sleeping);
                self.sleep_queue.push((wake_at, task.clone()));
                break;
            }
        }
    }

    /// Check if reschedule is needed.
    pub fn needs_reschedule(&self) -> bool {
        self.need_reschedule
    }

    /// Clear reschedule flag.
    pub fn clear_reschedule(&mut self) {
        self.need_reschedule = false;
    }

    /// Enable or disable preemption.
    pub fn set_preemption(&mut self, enabled: bool) {
        self.preemption_enabled = enabled;
    }

    /// Allocate a new task ID.
    pub fn alloc_task_id(&mut self) -> TaskId {
        let id = TaskId(self.next_task_id);
        self.next_task_id += 1;
        id
    }

    /// Get number of ready tasks.
    pub fn ready_count(&self) -> usize {
        self.ready_queues.iter().map(|q| q.len()).sum()
    }

    /// Get number of blocked tasks.
    pub fn blocked_count(&self) -> usize {
        self.blocked_tasks.len()
    }
}
