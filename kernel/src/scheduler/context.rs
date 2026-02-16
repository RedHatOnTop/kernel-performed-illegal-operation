//! Context switch implementation.
//!
//! This module provides the low-level context switch functionality
//! for task scheduling.

use core::arch::naked_asm;

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
    /// Return address (pushed by call instruction).
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
/// This function saves the current task's context and restores the next
/// task's context, effectively switching execution to the next task.
///
/// # Safety
///
/// - Both pointers must be valid and properly aligned.
/// - The contexts must be properly initialized.
/// - This function never returns to the caller if switching to a new task.
#[unsafe(naked)]
pub unsafe extern "C" fn switch_context(_current: *mut SwitchContext, _next: *const SwitchContext) {
    // rdi = current context pointer
    // rsi = next context pointer
    naked_asm!(
        // Save current context
        "mov [rdi + 0x00], r15",
        "mov [rdi + 0x08], r14",
        "mov [rdi + 0x10], r13",
        "mov [rdi + 0x18], r12",
        "mov [rdi + 0x20], rbx",
        "mov [rdi + 0x28], rbp",
        "mov [rdi + 0x30], rsp",
        // Save return address
        "mov rax, [rsp]",
        "mov [rdi + 0x38], rax",
        // Restore next context
        "mov r15, [rsi + 0x00]",
        "mov r14, [rsi + 0x08]",
        "mov r13, [rsi + 0x10]",
        "mov r12, [rsi + 0x18]",
        "mov rbx, [rsi + 0x20]",
        "mov rbp, [rsi + 0x28]",
        "mov rsp, [rsi + 0x30]",
        // Push return address and return
        "mov rax, [rsi + 0x38]",
        "push rax",
        "ret",
    );
}

/// Initialize a new task's stack for first-time execution.
///
/// Sets up the stack so that when switch_context switches to this task,
/// it will start executing at the entry point.
///
/// # Arguments
///
/// - `stack_top`: Pointer to the top of the stack (highest address).
/// - `entry`: Entry point function that never returns.
///
/// # Returns
///
/// The initial stack pointer value to use in SwitchContext.
pub fn setup_task_stack(stack_top: *mut u8, entry: fn() -> !) -> u64 {
    let stack_top = stack_top as u64;

    // Align stack to 16 bytes (required by System V ABI).
    let stack_top = stack_top & !0xF;

    // The stack layout for a new task:
    // [stack_top - 8]: return address (entry point)
    // [stack_top - 16]: fake rbp (0)
    // ... (room for saved registers)

    // We don't actually push anything here; the SwitchContext
    // will be used directly to set up the registers.

    // Return stack pointer that will be used after restoring context.
    // Account for the return address that will be pushed.
    stack_top - 8
}

/// Task entry point wrapper.
///
/// This function is called when a new task starts executing.
/// It sets up any necessary state and calls the actual task function.
#[inline(never)]
pub extern "C" fn task_entry_trampoline() -> ! {
    // This is a placeholder. In a real implementation, this would:
    // 1. Pop the actual entry point from a task-local storage
    // 2. Call the entry point
    // 3. Handle task termination

    crate::serial_println!("[TASK] Task started");

    loop {
        x86_64::instructions::hlt();
    }
}
