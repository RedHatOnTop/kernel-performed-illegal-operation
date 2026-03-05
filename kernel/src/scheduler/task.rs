//! Task definition and management.
//!
//! This module defines the Task structure and its associated types.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use super::context::{setup_initial_stack, SwitchContext};
use super::priority::Priority;

/// Base virtual address for kernel stacks with guard pages.
///
/// Located in the kernel high half at P4 index 384, safely above
/// the bootloader's physical memory mapping (~P4[261]) and below
/// the kernel heap (P4[296]).
const STACK_REGION_BASE: u64 = 0xFFFF_C000_0000_0000;

/// Size of one stack slot: guard page (4 KiB) + stack (KERNEL_STACK_SIZE).
const STACK_SLOT_SIZE: u64 = 4096 + KERNEL_STACK_SIZE as u64;

/// Counter for allocating stack slots (each task gets a unique slot).
static NEXT_STACK_SLOT: AtomicU64 = AtomicU64::new(0);

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

/// Information about a kernel stack with a guard page.
///
/// When dropped, the physical frames should be freed (not implemented
/// yet — tasks currently live until kernel shutdown).
#[derive(Debug)]
pub struct KernelStackGuard {
    /// Virtual address of the guard page (unmapped, 4 KiB).
    pub guard_page_virt: u64,
    /// Virtual address of the stack bottom (first mapped byte).
    pub stack_bottom_virt: u64,
    /// Virtual address of the stack top (stack_bottom + stack_size).
    pub stack_top_virt: u64,
    /// Number of physical frames allocated for the stack.
    pub frame_count: usize,
    /// Physical addresses of allocated frames (for cleanup).
    pub frames: Vec<u64>,
}

/// Allocate a kernel stack with a guard page.
///
/// Layout (addresses grow upward):
///   guard_page  (4 KiB, unmapped — faults on access)
///   stack pages (KERNEL_STACK_SIZE bytes, mapped + WRITABLE)
///
/// Returns (stack_top: u64, guard: KernelStackGuard) or panics on OOM.
fn alloc_kernel_stack_with_guard(task_name: &str) -> (u64, KernelStackGuard) {
    let slot = NEXT_STACK_SLOT.fetch_add(1, Ordering::Relaxed);
    let slot_base = STACK_REGION_BASE + slot * STACK_SLOT_SIZE;
    let guard_virt = slot_base;
    let stack_bottom_virt = slot_base + 4096; // Right after guard page
    let stack_pages = KERNEL_STACK_SIZE / 4096; // 4 pages for 16 KiB

    let phys_offset = crate::memory::user_page_table::get_phys_offset();

    // Guard page: intentionally NOT mapped — accessing it causes a page fault.
    // We don't need to do anything; the page table entry just doesn't exist.

    // Allocate and map stack pages.
    let mut frames = Vec::with_capacity(stack_pages);
    for page_idx in 0..stack_pages {
        let frame_phys = crate::memory::allocate_frame()
            .expect("alloc_kernel_stack_with_guard: out of physical frames")
            as u64;
        frames.push(frame_phys);

        let page_virt = stack_bottom_virt + (page_idx as u64 * 4096);

        // Map this virtual page to the physical frame.
        // We walk/create the page table entries for this virtual address.
        // SAFETY: we are mapping kernel-only pages in a region we control.
        unsafe {
            map_kernel_page(page_virt, frame_phys, phys_offset);
        }
    }

    let stack_top = (stack_bottom_virt + KERNEL_STACK_SIZE as u64) & !0xF;

    crate::serial_println!(
        "[STACK] guard page at {:#x}, stack {:#x}-{:#x} for '{}'",
        guard_virt,
        stack_bottom_virt,
        stack_top,
        task_name
    );

    let guard = KernelStackGuard {
        guard_page_virt: guard_virt,
        stack_bottom_virt,
        stack_top_virt: stack_top,
        frame_count: stack_pages,
        frames,
    };

    (stack_top, guard)
}

/// Map a single 4 KiB page in the kernel's page table.
///
/// Creates intermediate page table entries as needed.
///
/// # Safety
///
/// The caller must ensure the virtual address is in a safe kernel region.
unsafe fn map_kernel_page(virt: u64, phys: u64, phys_offset: u64) {
    use x86_64::registers::control::Cr3;

    let cr3 = Cr3::read().0.start_address().as_u64();

    let l4_idx = ((virt >> 39) & 0x1FF) as usize;
    let l3_idx = ((virt >> 30) & 0x1FF) as usize;
    let l2_idx = ((virt >> 21) & 0x1FF) as usize;
    let l1_idx = ((virt >> 12) & 0x1FF) as usize;

    let flags_present_write: u64 = 0x03; // PRESENT | WRITABLE

    // SAFETY: All pointer arithmetic and dereferences below operate on
    // page table entries accessed via the kernel's physical memory
    // mapping (phys_offset + physical_address). The caller guarantees
    // the virtual address is in a safe kernel region, and the page
    // table frames are either read from CR3 (the active page table)
    // or freshly allocated via allocate_frame().

    // Walk or create page table entries.
    let l4_table = (phys_offset + cr3) as *mut u64;
    let l4_entry = unsafe { l4_table.add(l4_idx) };
    if unsafe { *l4_entry } & 1 == 0 {
        // Allocate L3 table
        let l3_frame =
            crate::memory::allocate_frame().expect("map_kernel_page: OOM for L3 table") as u64;
        unsafe { core::ptr::write_bytes((phys_offset + l3_frame) as *mut u8, 0, 4096) };
        unsafe { *l4_entry = l3_frame | flags_present_write };
    }

    let l3_phys = unsafe { *l4_entry } & 0x000F_FFFF_FFFF_F000;
    let l3_table = (phys_offset + l3_phys) as *mut u64;
    let l3_entry = unsafe { l3_table.add(l3_idx) };
    if unsafe { *l3_entry } & 1 == 0 {
        // Allocate L2 table
        let l2_frame =
            crate::memory::allocate_frame().expect("map_kernel_page: OOM for L2 table") as u64;
        unsafe { core::ptr::write_bytes((phys_offset + l2_frame) as *mut u8, 0, 4096) };
        unsafe { *l3_entry = l2_frame | flags_present_write };
    }

    let l2_phys = unsafe { *l3_entry } & 0x000F_FFFF_FFFF_F000;
    let l2_table = (phys_offset + l2_phys) as *mut u64;
    let l2_entry = unsafe { l2_table.add(l2_idx) };
    if unsafe { *l2_entry } & 1 == 0 {
        // Allocate L1 table
        let l1_frame =
            crate::memory::allocate_frame().expect("map_kernel_page: OOM for L1 table") as u64;
        unsafe { core::ptr::write_bytes((phys_offset + l1_frame) as *mut u8, 0, 4096) };
        unsafe { *l2_entry = l1_frame | flags_present_write };
    }

    let l1_phys = unsafe { *l2_entry } & 0x000F_FFFF_FFFF_F000;
    let l1_table = (phys_offset + l1_phys) as *mut u64;
    let l1_entry = unsafe { l1_table.add(l1_idx) };
    // Map: PRESENT | WRITABLE | NO_EXECUTE
    unsafe { *l1_entry = phys | flags_present_write | (1u64 << 63) }; // NX bit

    // Invalidate the TLB for this page
    unsafe {
        core::arch::asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags));
    }
}

/// Check if a faulting address is in the kernel stack guard region.
///
/// Returns `true` if the address falls within a guard page.
/// Called from the page fault handler to provide a clear panic message.
pub fn is_stack_guard_page(addr: u64) -> bool {
    if addr < STACK_REGION_BASE {
        return false;
    }
    let offset = addr - STACK_REGION_BASE;
    let slot_offset = offset % STACK_SLOT_SIZE;
    // Guard page is the first 4 KiB of each slot.
    slot_offset < 4096
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
    /// Either a Vec<u8> (legacy, for user-process kernel stacks) or
    /// a KernelStackGuard (frame-allocated with guard page).
    _kernel_stack: Option<Vec<u8>>,
    /// Guard page info for kernel tasks (None for user processes).
    _stack_guard: Option<KernelStackGuard>,
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

        // Allocate a kernel stack with guard page.
        let (stack_top, stack_guard) = alloc_kernel_stack_with_guard(name);

        let mut context = TaskContext::default();
        context.rip = entry;
        context.rsp = stack_top;
        context.rflags = 0x202; // IF flag set
        context.cs = 0x08; // Kernel code segment
        context.ss = 0x10; // Kernel data segment

        // Prepare the stack: push the entry point as a fake
        // return address.  When `switch_context()` restores RSP
        // and executes `ret`, it pops this address and jumps there.
        let initial_rsp = setup_initial_stack(stack_top, entry);

        let switch_ctx = SwitchContext {
            rip: entry, // informational only
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
            _kernel_stack: None,
            _stack_guard: Some(stack_guard),
            cr3: 0,
            kernel_stack_top_addr: stack_top,
            process_pid: 0,
        }
    }

    /// Create a new WASM process task.
    pub fn new_wasm(name: &str, _stack_top_legacy: u64) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(2);

        // Allocate kernel stack with guard page.
        let (stack_top, stack_guard) = alloc_kernel_stack_with_guard(name);

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
            _kernel_stack: None,
            _stack_guard: Some(stack_guard),
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
        let (stack_top, stack_guard) = alloc_kernel_stack_with_guard("idle");

        let idle_entry = idle_task_entry as *const () as u64;
        let initial_rsp = setup_initial_stack(stack_top, idle_entry);

        let switch_ctx = SwitchContext {
            rip: idle_entry, // informational only
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
            _kernel_stack: None,
            _stack_guard: Some(stack_guard),
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
        // Allocate a kernel stack with guard page for the boot task.
        let (stack_top, stack_guard) = alloc_kernel_stack_with_guard("kernel-main");

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
            _kernel_stack: None,
            _stack_guard: Some(stack_guard),
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
            _stack_guard: None,
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
    unsafe {
        core::arch::asm!("mov {}, r12", out(reg) ctx_ptr);
    }

    // Take ownership of the heap-allocated UserEntryContext
    let ctx = unsafe { *alloc::boxed::Box::from_raw(ctx_ptr as *mut UserEntryContext) };

    crate::serial_println!(
        "[PROC] Entering Ring 3: RIP={:#x} RSP={:#x} CS={:#x} SS={:#x}",
        ctx.rip,
        ctx.rsp,
        ctx.cs,
        ctx.ss
    );

    // Enable interrupts before entering user space (timer must fire for preemption)
    x86_64::instructions::interrupts::enable();

    // Ensure correct SWAPGS state before entering Ring 3.
    //
    // On iretq to Ring 3, we need:
    //   GS_BASE        = 0 (or user value)
    //   KERNEL_GS_BASE = per-CPU address
    //
    // After SYS_EXIT → exit_current() → schedule() → switch_context(),
    // the ring3_syscall_entry epilogue (swapgs + sysretq) never runs.
    // The MSRs may be inverted: GS_BASE = per-CPU, KERNEL_GS_BASE = 0.
    // We detect this by checking KERNEL_GS_BASE; if it's 0, we swapgs.
    unsafe {
        use x86_64::registers::model_specific::Msr;
        const IA32_KERNEL_GS_BASE: u32 = 0xC000_0102;
        let kgs = Msr::new(IA32_KERNEL_GS_BASE).read();
        if kgs == 0 {
            // MSRs are inverted — do swapgs to fix them
            core::arch::asm!("swapgs", options(nomem, nostack));
        }
    }

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
