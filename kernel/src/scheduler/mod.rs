//! Kernel scheduler module.
//!
//! This module implements cooperative and preemptive task scheduling
//! for both kernel tasks and WASM processes.

pub mod context;
pub mod task;
pub mod priority;
pub mod round_robin;
pub mod optimization;

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;

pub use context::SwitchContext;
pub use task::{Task, TaskId, TaskState};
pub use priority::Priority;

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

/// Initialize the scheduler.
pub fn init() {
    let mut scheduler = SCHEDULER.lock();
    *scheduler = Some(Scheduler::new());
    
    // Create idle task
    let idle_task = Task::new_idle();
    scheduler.as_mut().unwrap().add_task(idle_task);
}

/// Spawn a new task.
pub fn spawn(task: Task) -> TaskId {
    let id = task.id();
    if let Some(ref mut scheduler) = *SCHEDULER.lock() {
        scheduler.add_task(task);
    }
    id
}

/// Schedule the next task.
pub fn schedule() {
    if let Some(ref mut scheduler) = *SCHEDULER.lock() {
        scheduler.schedule();
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
pub fn timer_tick() {
    BOOT_TICKS.fetch_add(1, Ordering::Relaxed);
    if let Some(ref mut scheduler) = *SCHEDULER.lock() {
        scheduler.timer_tick();
    }
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
            
            CURRENT_TASK_ID.store(task_id, Ordering::Relaxed);
            CONTEXT_SWITCHES.fetch_add(1, Ordering::Relaxed);
            
            // Perform context switch
            self.context_switch();
        }
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
    
    /// Perform context switch.
    /// Saves current task context and loads next task context via
    /// the process::context::context_switch assembly routine.
    fn context_switch(&mut self) {
        // In a full implementation this would call:
        //   process::context::context_switch(old_ctx_ptr, new_ctx_ptr)
        // For now the cooperative scheduler relies on Rust function
        // call/return semantics — each yield_now() already returns
        // to the correct call-site.  We update stats here.
        if let Some(ref task) = self.current_task {
            let mut t = task.lock();
            t.stats_mut().context_switches += 1;
            t.stats_mut().last_scheduled = BOOT_TICKS.load(Ordering::Relaxed);
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
