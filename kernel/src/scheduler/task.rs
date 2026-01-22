//! Task definition and management.
//!
//! This module defines the Task structure and its associated types.

use alloc::boxed::Box;
use alloc::string::String;
use core::sync::atomic::{AtomicU64, Ordering};

use super::priority::Priority;

/// Unique task identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(pub u64);

impl TaskId {
    /// The kernel task ID (always 0).
    pub const KERNEL: TaskId = TaskId(0);
    
    /// The idle task ID (always 1).
    pub const IDLE: TaskId = TaskId(1);
}

/// Task state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Task is ready to run.
    Ready,
    /// Task is currently running.
    Running,
    /// Task is blocked waiting for an event.
    Blocked,
    /// Task is sleeping.
    Sleeping,
    /// Task has terminated.
    Terminated,
}

/// Task type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    /// Kernel task (runs in ring 0).
    Kernel,
    /// WASM process (runs in sandboxed environment).
    WasmProcess,
    /// Idle task.
    Idle,
}

/// Task context (CPU registers saved during context switch).
#[derive(Debug, Default, Clone)]
#[repr(C)]
pub struct TaskContext {
    /// General purpose registers.
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    
    /// Instruction pointer.
    pub rip: u64,
    /// Stack pointer.
    pub rsp: u64,
    /// Flags.
    pub rflags: u64,
    /// Code segment.
    pub cs: u64,
    /// Stack segment.
    pub ss: u64,
    
    /// CR3 (page table root).
    pub cr3: u64,
    
    /// FPU/SSE state (if enabled).
    pub fpu_state: Option<Box<FpuState>>,
}

/// FPU/SSE state (512 bytes for FXSAVE).
#[derive(Debug, Clone)]
#[repr(C, align(16))]
pub struct FpuState {
    pub data: [u8; 512],
}

impl Default for FpuState {
    fn default() -> Self {
        FpuState { data: [0; 512] }
    }
}

/// Task statistics.
#[derive(Debug, Default, Clone)]
pub struct TaskStats {
    /// Total CPU time in nanoseconds.
    pub cpu_time_ns: u64,
    /// Number of context switches.
    pub context_switches: u64,
    /// Number of page faults.
    pub page_faults: u64,
    /// Number of syscalls.
    pub syscalls: u64,
    /// Time of creation (ticks since boot).
    pub created_at: u64,
    /// Time of last schedule (ticks since boot).
    pub last_scheduled: u64,
}

/// A task in the system.
pub struct Task {
    /// Unique task ID.
    id: TaskId,
    /// Task name.
    name: String,
    /// Task type.
    task_type: TaskType,
    /// Task state.
    state: TaskState,
    /// Task priority.
    priority: Priority,
    /// CPU context.
    context: TaskContext,
    /// Task statistics.
    stats: TaskStats,
    /// Exit code (set when terminated).
    exit_code: Option<i32>,
    /// Stack top address.
    stack_top: u64,
    /// Stack size.
    stack_size: usize,
    /// Parent task ID (if any).
    parent: Option<TaskId>,
}

impl Task {
    /// Create a new kernel task.
    pub fn new_kernel(name: &str, entry: u64, stack_top: u64) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(2);
        
        let mut context = TaskContext::default();
        context.rip = entry;
        context.rsp = stack_top;
        context.rflags = 0x202; // IF flag set
        context.cs = 0x08; // Kernel code segment
        context.ss = 0x10; // Kernel data segment
        
        Task {
            id: TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
            name: String::from(name),
            task_type: TaskType::Kernel,
            state: TaskState::Ready,
            priority: Priority::Normal,
            context,
            stats: TaskStats::default(),
            exit_code: None,
            stack_top,
            stack_size: 64 * 1024, // 64 KB default
            parent: None,
        }
    }
    
    /// Create a new WASM process task.
    pub fn new_wasm(name: &str, stack_top: u64) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(2);
        
        let mut context = TaskContext::default();
        context.rsp = stack_top;
        context.rflags = 0x202;
        context.cs = 0x08;
        context.ss = 0x10;
        
        Task {
            id: TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
            name: String::from(name),
            task_type: TaskType::WasmProcess,
            state: TaskState::Ready,
            priority: Priority::Normal,
            context,
            stats: TaskStats::default(),
            exit_code: None,
            stack_top,
            stack_size: 64 * 1024,
            parent: None,
        }
    }
    
    /// Create the idle task.
    pub fn new_idle() -> Self {
        Task {
            id: TaskId::IDLE,
            name: String::from("idle"),
            task_type: TaskType::Idle,
            state: TaskState::Ready,
            priority: Priority::Idle,
            context: TaskContext::default(),
            stats: TaskStats::default(),
            exit_code: None,
            stack_top: 0,
            stack_size: 4096,
            parent: None,
        }
    }
    
    /// Get the task ID.
    pub fn id(&self) -> TaskId {
        self.id
    }
    
    /// Get the task name.
    pub fn name(&self) -> &str {
        &self.name
    }
    
    /// Get the task type.
    pub fn task_type(&self) -> TaskType {
        self.task_type
    }
    
    /// Get the task state.
    pub fn state(&self) -> TaskState {
        self.state
    }
    
    /// Set the task state.
    pub fn set_state(&mut self, state: TaskState) {
        self.state = state;
    }
    
    /// Get the task priority.
    pub fn priority(&self) -> Priority {
        self.priority
    }
    
    /// Set the task priority.
    pub fn set_priority(&mut self, priority: Priority) {
        self.priority = priority;
    }
    
    /// Get a reference to the task context.
    pub fn context(&self) -> &TaskContext {
        &self.context
    }
    
    /// Get a mutable reference to the task context.
    pub fn context_mut(&mut self) -> &mut TaskContext {
        &mut self.context
    }
    
    /// Get a reference to the task statistics.
    pub fn stats(&self) -> &TaskStats {
        &self.stats
    }
    
    /// Get a mutable reference to the task statistics.
    pub fn stats_mut(&mut self) -> &mut TaskStats {
        &mut self.stats
    }
    
    /// Get the exit code.
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }
    
    /// Set the exit code.
    pub fn set_exit_code(&mut self, code: i32) {
        self.exit_code = Some(code);
    }
    
    /// Get the stack top address.
    pub fn stack_top(&self) -> u64 {
        self.stack_top
    }
    
    /// Get the stack size.
    pub fn stack_size(&self) -> usize {
        self.stack_size
    }
    
    /// Get the parent task ID.
    pub fn parent(&self) -> Option<TaskId> {
        self.parent
    }
    
    /// Set the parent task ID.
    pub fn set_parent(&mut self, parent: TaskId) {
        self.parent = Some(parent);
    }
    
    /// Check if the task is runnable.
    pub fn is_runnable(&self) -> bool {
        matches!(self.state, TaskState::Ready | TaskState::Running)
    }
}
