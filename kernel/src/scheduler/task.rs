//! Task definition and management.
//!
//! This module defines the Task structure and its associated types.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use super::context::{SwitchContext, setup_initial_stack};
use super::priority::Priority;

/// Unique task identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(pub u64);

impl TaskId {
    /// The kernel task ID (always 0).
    pub const KERNEL: TaskId = TaskId(0);

    /// The idle task ID (always 1).
    pub const IDLE: TaskId = TaskId(1);

    /// Create a TaskId from a raw value.
    pub fn new(id: u64) -> Self {
        TaskId(id)
    }
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
    /// User-space process (runs in Ring 3 with its own page table).
    UserProcess,
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

/// Default kernel stack size: 16 KiB per task.
const KERNEL_STACK_SIZE: usize = 16 * 1024;

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
    /// CPU context (full register set — used for user/kernel boundary).
    context: TaskContext,
    /// Scheduler switch context (callee-saved regs — used by switch_context asm).
    pub switch_ctx: SwitchContext,
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
    /// Owned kernel stack allocation (kept alive for the task's lifetime).
    _kernel_stack: Option<Vec<u8>>,
    /// CR3 page table root for user-space processes.
    /// 0 means "use current kernel CR3" (kernel tasks).
    cr3: u64,
    /// Kernel stack top address for TSS RSP0 and PerCpu.
    /// Used when switching to this task so that Ring 3→0
    /// transitions land on the correct kernel stack.
    kernel_stack_top_addr: u64,
    /// Associated process ID (for user-space tasks).
    process_pid: u64,
}

impl Task {
    /// Create a new kernel task with its own kernel stack.
    ///
    /// Allocates a dedicated kernel stack and initialises the
    /// `SwitchContext` so that `switch_context()` will start
    /// execution at `entry` on the new stack.
    pub fn new_kernel(name: &str, entry: u64, _stack_top_legacy: u64) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(2);

        // Allocate a dedicated kernel stack for this task.
        let mut stack = Vec::with_capacity(KERNEL_STACK_SIZE);
        stack.resize(KERNEL_STACK_SIZE, 0u8);
        let stack_bottom = stack.as_ptr() as u64;
        // Stack grows downward — top is bottom + size, 16-byte aligned.
        let stack_top = (stack_bottom + KERNEL_STACK_SIZE as u64) & !0xF;

        let mut context = TaskContext::default();
        context.rip = entry;
        context.rsp = stack_top;
        context.rflags = 0x202; // IF flag set
        context.cs = 0x08;     // Kernel code segment
        context.ss = 0x10;     // Kernel data segment

        // Prepare the stack: push the entry point as a fake
        // return address.  When `switch_context()` restores RSP
        // and executes `ret`, it pops this address and jumps there.
        let initial_rsp = setup_initial_stack(stack_top, entry);

        let switch_ctx = SwitchContext {
            rip: entry,          // informational only
            rsp: initial_rsp,
            rbp: 0,
            rbx: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        };

        Task {
            id: TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
            name: String::from(name),
            task_type: TaskType::Kernel,
            state: TaskState::Ready,
            priority: Priority::Normal,
            context,
            switch_ctx,
            stats: TaskStats::default(),
            exit_code: None,
            stack_top,
            stack_size: KERNEL_STACK_SIZE,
            parent: None,
            _kernel_stack: Some(stack),
            cr3: 0,
            kernel_stack_top_addr: stack_top,
            process_pid: 0,
        }
    }

    /// Create a new WASM process task.
    pub fn new_wasm(name: &str, _stack_top_legacy: u64) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(2);

        // Allocate dedicated kernel stack.
        let mut stack = Vec::with_capacity(KERNEL_STACK_SIZE);
        stack.resize(KERNEL_STACK_SIZE, 0u8);
        let stack_bottom = stack.as_ptr() as u64;
        let stack_top = (stack_bottom + KERNEL_STACK_SIZE as u64) & !0xF;

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
            switch_ctx: SwitchContext::default(),
            stats: TaskStats::default(),
            exit_code: None,
            stack_top,
            stack_size: KERNEL_STACK_SIZE,
            parent: None,
            _kernel_stack: Some(stack),
            cr3: 0,
            kernel_stack_top_addr: stack_top,
            process_pid: 0,
        }
    }

    /// Create the idle task.
    ///
    /// The idle task gets its own stack but uses a dedicated
    /// idle-loop entry point.
    pub fn new_idle() -> Self {
        let mut stack = Vec::with_capacity(KERNEL_STACK_SIZE);
        stack.resize(KERNEL_STACK_SIZE, 0u8);
        let stack_bottom = stack.as_ptr() as u64;
        let stack_top = (stack_bottom + KERNEL_STACK_SIZE as u64) & !0xF;

        let idle_entry = idle_task_entry as *const () as u64;
        let initial_rsp = setup_initial_stack(stack_top, idle_entry);

        let switch_ctx = SwitchContext {
            rip: idle_entry,     // informational only
            rsp: initial_rsp,
            rbp: 0,
            rbx: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        };

        Task {
            id: TaskId::IDLE,
            name: String::from("idle"),
            task_type: TaskType::Idle,
            state: TaskState::Ready,
            priority: Priority::Idle,
            context: TaskContext::default(),
            switch_ctx,
            stats: TaskStats::default(),
            exit_code: None,
            stack_top,
            stack_size: KERNEL_STACK_SIZE,
            parent: None,
            _kernel_stack: Some(stack),
            cr3: 0,
            kernel_stack_top_addr: stack_top,
            process_pid: 0,
        }
    }

    /// Create the boot (kernel-main) task.
    ///
    /// Represents the initial kernel execution context.  Allocates
    /// its own kernel stack.  The first context switch will save
    /// the actual register state into its `SwitchContext`.
    pub fn new_boot_task() -> Self {
        // Allocate a kernel stack for the boot task so that
        // saved RSP always points to dedicated memory.
        let mut stack = Vec::with_capacity(KERNEL_STACK_SIZE);
        stack.resize(KERNEL_STACK_SIZE, 0u8);
        let stack_bottom = stack.as_ptr() as u64;
        let stack_top = (stack_bottom + KERNEL_STACK_SIZE as u64) & !0xF;

        Task {
            id: TaskId::KERNEL,
            name: String::from("kernel-main"),
            task_type: TaskType::Kernel,
            state: TaskState::Running,
            priority: Priority::Normal,
            context: TaskContext::default(),
            switch_ctx: SwitchContext::default(),
            stats: TaskStats::default(),
            exit_code: None,
            stack_top,
            stack_size: KERNEL_STACK_SIZE,
            parent: None,
            _kernel_stack: Some(stack),
            cr3: 0,
            kernel_stack_top_addr: stack_top,
            process_pid: 0,
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

    /// Get a mutable reference to the switch context.
    pub fn switch_ctx_mut(&mut self) -> &mut SwitchContext {
        &mut self.switch_ctx
    }

    /// Get a reference to the switch context.
    pub fn switch_ctx(&self) -> &SwitchContext {
        &self.switch_ctx
    }

    /// Get the CR3 (page table root) for this task.
    /// Returns 0 for kernel tasks (use current CR3).
    pub fn cr3(&self) -> u64 {
        self.cr3
    }

    /// Get the kernel stack top address (for TSS RSP0).
    pub fn kernel_stack_top_addr(&self) -> u64 {
        self.kernel_stack_top_addr
    }

    /// Get the associated process PID.
    pub fn process_pid(&self) -> u64 {
        self.process_pid
    }

    /// Create a new user-space process task.
    ///
    /// This task starts in kernel mode at a trampoline function.
    /// The trampoline reads the `UserEntryContext` pointer from `r12`
    /// (set in `SwitchContext`), then enters Ring 3 via `iretq`.
    ///
    /// # Arguments
    ///
    /// * `name` - Task name
    /// * `cr3` - Physical address of the process's P4 page table
    /// * `entry_point` - User-space entry point (RIP)
    /// * `user_stack_top` - User-space stack pointer (RSP)
    /// * `user_cs` - User code segment selector (u16, e.g., 0x23)
    /// * `user_ds` - User data segment selector (u16, e.g., 0x1B)
    /// * `kernel_stack_top` - Top of the task's kernel stack (for TSS RSP0)
    /// * `kernel_stack` - Owned kernel stack allocation
    /// * `pid` - Associated process ID
    pub fn new_user_process(
        name: &str,
        cr3: u64,
        entry_point: u64,
        user_stack_top: u64,
        user_cs: u16,
        user_ds: u16,
        kernel_stack_top: u64,
        kernel_stack: Vec<u8>,
        pid: u64,
    ) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(100);

        // Store UserEntryContext on the heap so the trampoline can read it.
        let entry_ctx = UserEntryContext {
            rip: entry_point,
            cs: user_cs as u64,
            rflags: 0x202, // IF + reserved bit-1
            rsp: user_stack_top,
            ss: user_ds as u64,
        };
        let ctx_box = alloc::boxed::Box::new(entry_ctx);
        let ctx_ptr = alloc::boxed::Box::into_raw(ctx_box) as u64;

        // Set up SwitchContext:
        //   rsp → kernel stack with trampoline as return address
        //   r12 → pointer to heap-allocated ProcessContext
        let trampoline_addr = user_process_entry_trampoline as *const () as u64;
        let initial_rsp = setup_initial_stack(kernel_stack_top, trampoline_addr);

        let switch_ctx = SwitchContext {
            rip: trampoline_addr,
            rsp: initial_rsp,
            rbp: 0,
            rbx: 0,
            r12: ctx_ptr,
            r13: 0,
            r14: 0,
            r15: 0,
        };

        Task {
            id: TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
            name: String::from(name),
            task_type: TaskType::UserProcess,
            state: TaskState::Ready,
            priority: Priority::Normal,
            context: TaskContext::default(),
            switch_ctx,
            stats: TaskStats::default(),
            exit_code: None,
            stack_top: kernel_stack_top,
            stack_size: KERNEL_STACK_SIZE,
            parent: None,
            _kernel_stack: Some(kernel_stack),
            cr3,
            kernel_stack_top_addr: kernel_stack_top,
            process_pid: pid,
        }
    }
}

/// Minimal context for entering Ring 3 via iretq.
///
/// This is a self-contained version of the fields needed by
/// `iretq` — stored on the heap and recovered by the trampoline.
/// Avoids a dependency on `crate::process::context::ProcessContext`.
#[repr(C)]
pub struct UserEntryContext {
    /// User-mode instruction pointer
    pub rip: u64,
    /// User code-segment selector (Ring 3)
    pub cs: u64,
    /// Initial RFLAGS (0x202 = IF + reserved bit-1)
    pub rflags: u64,
    /// User-mode stack pointer
    pub rsp: u64,
    /// User stack-segment selector (Ring 3)
    pub ss: u64,
}

/// User-space process entry trampoline.
///
/// This function is the initial entry point for user-space tasks.
/// When `switch_context()` first resumes this task, it "returns"
/// here.  The `SwitchContext.r12` register holds a pointer to a
/// heap-allocated `UserEntryContext`.
///
/// The trampoline:
/// 1. Reads the UserEntryContext pointer from `r12`
/// 2. Copies it to the stack and frees the heap allocation
/// 3. Enables interrupts
/// 4. Enters Ring 3 via inline `iretq`
fn user_process_entry_trampoline() -> ! {
    // r12 was restored by switch_context and contains the UserEntryContext pointer
    let ctx_ptr: u64;
    unsafe { core::arch::asm!("mov {}, r12", out(reg) ctx_ptr); }

    // Take ownership of the heap-allocated UserEntryContext
    let ctx = unsafe { *alloc::boxed::Box::from_raw(ctx_ptr as *mut UserEntryContext) };

    crate::serial_println!("[PROC] Entering Ring 3: RIP={:#x} RSP={:#x} CS={:#x} SS={:#x}",
        ctx.rip, ctx.rsp, ctx.cs, ctx.ss);

    // Enable interrupts before entering user space (timer must fire for preemption)
    x86_64::instructions::interrupts::enable();

    // Enter Ring 3 via iretq — prepare the stack frame and execute iretq
    unsafe {
        core::arch::asm!(
            // Push SS
            "push {ss}",
            // Push RSP
            "push {rsp_val}",
            // Push RFLAGS
            "push {rflags}",
            // Push CS
            "push {cs}",
            // Push RIP
            "push {rip}",
            // iretq pops RIP, CS, RFLAGS, RSP, SS and transfers to Ring 3
            "iretq",
            ss = in(reg) ctx.ss,
            rsp_val = in(reg) ctx.rsp,
            rflags = in(reg) ctx.rflags,
            cs = in(reg) ctx.cs,
            rip = in(reg) ctx.rip,
            options(noreturn),
        );
    }
}

/// Idle task entry point — enables interrupts, then loops executing
/// `hlt` until woken by interrupt.
pub fn idle_task_entry() -> ! {
    x86_64::instructions::interrupts::enable();
    loop {
        x86_64::instructions::hlt();
    }
}
