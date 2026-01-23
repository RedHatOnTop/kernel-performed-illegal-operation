//! Kernel scheduler module.
//!
//! This module implements cooperative and preemptive task scheduling
//! for both kernel tasks and WASM processes.

pub mod context;
pub mod task;
pub mod priority;
pub mod round_robin;

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

/// Timer tick handler (called from timer interrupt).
pub fn timer_tick() {
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
    
    /// Current time slice remaining.
    time_slice_remaining: u64,
    
    /// Next task ID.
    next_task_id: u64,
    
    /// Whether preemption is enabled.
    preemption_enabled: bool,
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
            time_slice_remaining: DEFAULT_TIME_SLICE,
            next_task_id: 1,
            preemption_enabled: true,
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
    /// Note: This is called from the timer interrupt handler.
    /// We only decrement the time slice here. Actual scheduling
    /// should happen when the interrupt returns, not inside the handler.
    pub fn timer_tick(&mut self) {
        if !self.preemption_enabled {
            return;
        }
        
        if self.time_slice_remaining > 0 {
            self.time_slice_remaining -= 1;
        }
        
        // Note: We don't call schedule() here because we're inside
        // an interrupt handler. In a full implementation, we would
        // set a flag and reschedule when returning to user space or
        // at a safe point in the kernel.
        // For now, cooperative scheduling via yield_now() is used.
    }
    
    /// Perform context switch.
    fn context_switch(&self) {
        // Context switch implementation
        // This involves saving and restoring CPU registers
        // For now, this is a placeholder
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
