//! Context switch implementation.
//!
//! This module provides the low-level context switch functionality
//! for task scheduling.

/// Minimal CPU context for context switching.
///
/// Only callee-saved registers need to be explicitly saved.
/// The calling convention already handles caller-saved registers.
#[derive(Debug, Default, Clone)]
#[repr(C)]
pub struct SwitchContext {
    /// Callee-saved registers (must be preserved across function calls).
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbx: u64,
    pub rbp: u64,
    /// Stack pointer.
    pub rsp: u64,
    /// Instruction pointer (entry point for new tasks, or return
    /// address for resumed tasks).  Unused by the inline-asm
    /// approach, but kept for `setup_initial_stack()`.
    pub rip: u64,
}

impl SwitchContext {
    /// Create a new context for a task.
    ///
    /// # Arguments
    ///
    /// - `entry`: The entry point function.
    /// - `stack_top`: The top of the kernel stack (highest address).
    pub fn new(entry: fn() -> !, stack_top: u64) -> Self {
        SwitchContext {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            rbx: 0,
            rbp: 0,
            rsp: stack_top,
            rip: entry as u64,
        }
    }
}

/// Perform a context switch from the current task to the next task.
///
/// Saves the current task's callee-saved registers and RSP into
/// `current`, then restores the next task's callee-saved registers
/// and RSP from `next`.
///
/// For a **new** task (never run before), the assembly reads
/// `next.rip` and pushes it as a return address so that the final
/// `ret` jumps to the task's entry point.  For a **resumed** task,
/// `next.rsp` already points to a stack where the return address
/// (inside this very function) was saved by the previous `call`/`ret`
/// pair, so the `ret` lands right back at the call site.
///
/// # Safety
///
/// - Both pointers must be valid and properly aligned.
/// - The contexts must be properly initialized.
/// - Must be called with interrupts disabled or in an
///   interrupt-safe context.
#[unsafe(naked)]
pub unsafe extern "C" fn switch_context(
    _current: *mut SwitchContext,
    _next: *const SwitchContext,
) {
    // rdi = current context pointer (save destination)
    // rsi = next    context pointer (restore source)
    //
    // Save callee-saved regs + RSP into *rdi.
    // Restore callee-saved regs + RSP from *rsi.
    // For "first run", rsp points to a stack that has the entry
    // point pushed by `setup_initial_stack`, so `ret` jumps there.
    // For "resumed", rsp is wherever it was when we called this
    // function previously — `ret` goes back to the caller.
    core::arch::naked_asm!(
        // ── Save current context ──
        "mov [rdi + 0x00], r15",
        "mov [rdi + 0x08], r14",
        "mov [rdi + 0x10], r13",
        "mov [rdi + 0x18], r12",
        "mov [rdi + 0x20], rbx",
        "mov [rdi + 0x28], rbp",
        "mov [rdi + 0x30], rsp",
        // rip is implicitly saved as the return address on the
        // stack (pushed by the `call switch_context` instruction).

        // ── Restore next context ──
        "mov r15, [rsi + 0x00]",
        "mov r14, [rsi + 0x08]",
        "mov r13, [rsi + 0x10]",
        "mov r12, [rsi + 0x18]",
        "mov rbx, [rsi + 0x20]",
        "mov rbp, [rsi + 0x28]",
        "mov rsp, [rsi + 0x30]",
        // `ret` pops the return address from the new stack and
        // jumps there.  For a new task, we pushed the entry point
        // in `setup_initial_stack`.  For a resumed task, the `call`
        // instruction that entered this function left the return
        // address on the stack.
        "ret",
    );
}

/// Prepare a new task's kernel stack so that the first
/// `switch_context()` will "return" into `entry`.
///
/// Pushes the entry point onto the stack (simulating the return
/// address that `call switch_context` would have placed) and
/// returns the adjusted RSP to store in `SwitchContext.rsp`.
pub fn setup_initial_stack(stack_top: u64, entry: u64) -> u64 {
    // 16-byte-align the stack top.
    let sp = stack_top & !0xF;
    // Push the entry point as a fake return address.
    let sp = sp - 8;
    unsafe {
        core::ptr::write(sp as *mut u64, entry);
    }
    sp
}

/// Task entry point wrapper.
///
/// This function is called when a new task starts executing.
/// It sets up any necessary state and calls the actual task function.
#[inline(never)]
pub extern "C" fn task_entry_trampoline() -> ! {
    crate::serial_println!("[TASK] Task started");

    loop {
        x86_64::instructions::hlt();
    }
}
