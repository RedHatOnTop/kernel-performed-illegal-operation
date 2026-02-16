//! Process Context
//!
//! CPU context for process switching and userspace execution.

use core::arch::asm;

/// CPU register context for context switching
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct ProcessContext {
    // General purpose registers (callee-saved first)
    /// R15 register
    pub r15: u64,
    /// R14 register
    pub r14: u64,
    /// R13 register
    pub r13: u64,
    /// R12 register
    pub r12: u64,
    /// RBX register
    pub rbx: u64,
    /// RBP register (frame pointer)
    pub rbp: u64,

    // Caller-saved registers
    /// R11 register
    pub r11: u64,
    /// R10 register
    pub r10: u64,
    /// R9 register
    pub r9: u64,
    /// R8 register
    pub r8: u64,
    /// RDI register
    pub rdi: u64,
    /// RSI register
    pub rsi: u64,
    /// RDX register
    pub rdx: u64,
    /// RCX register
    pub rcx: u64,
    /// RAX register
    pub rax: u64,

    // Segment registers (for user/kernel mode switch)
    /// Data segment
    pub ds: u64,
    /// Extra segment
    pub es: u64,

    // Interrupt frame (pushed by CPU on interrupt/syscall)
    /// Instruction pointer
    pub rip: u64,
    /// Code segment
    pub cs: u64,
    /// RFLAGS register
    pub rflags: u64,
    /// Stack pointer
    pub rsp: u64,
    /// Stack segment
    pub ss: u64,
}

/// Flags for process context
#[derive(Debug, Clone, Copy, Default)]
pub struct ContextFlags {
    /// Process is in userspace
    pub userspace: bool,
    /// FPU state is valid
    pub fpu_valid: bool,
    /// Debug registers are active
    pub debug_active: bool,
}

/// FPU/SSE/AVX state (512 bytes for FXSAVE, 1024+ for XSAVE)
#[derive(Clone)]
#[repr(C, align(64))]
pub struct FpuState {
    /// FXSAVE area (512 bytes)
    pub fxsave_area: [u8; 512],
    /// Extended state (if XSAVE is supported)
    pub extended: [u8; 512],
}

impl Default for FpuState {
    fn default() -> Self {
        Self {
            fxsave_area: [0; 512],
            extended: [0; 512],
        }
    }
}

impl ProcessContext {
    /// Create a new empty context
    pub const fn new() -> Self {
        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            rbx: 0,
            rbp: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rdi: 0,
            rsi: 0,
            rdx: 0,
            rcx: 0,
            rax: 0,
            ds: 0,
            es: 0,
            rip: 0,
            cs: 0,
            rflags: 0,
            rsp: 0,
            ss: 0,
        }
    }

    /// Create a context for userspace entry
    ///
    /// # Arguments
    ///
    /// * `entry_point` - User program entry point
    /// * `stack_pointer` - User stack pointer
    /// * `user_cs` - User code segment selector
    /// * `user_ds` - User data segment selector
    pub fn new_user(entry_point: u64, stack_pointer: u64, user_cs: u16, user_ds: u16) -> Self {
        // RFLAGS: IF (Interrupt Flag) enabled, reserved bit 1 set
        const USER_RFLAGS: u64 = 0x200 | 0x2;

        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            rbx: 0,
            rbp: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rdi: 0,
            rsi: 0,
            rdx: 0,
            rcx: 0,
            rax: 0,
            ds: user_ds as u64,
            es: user_ds as u64,
            rip: entry_point,
            cs: user_cs as u64,
            rflags: USER_RFLAGS,
            rsp: stack_pointer,
            ss: user_ds as u64,
        }
    }

    /// Create a context for kernel thread
    ///
    /// # Arguments
    ///
    /// * `entry_point` - Kernel function entry point
    /// * `stack_pointer` - Kernel stack pointer
    /// * `kernel_cs` - Kernel code segment selector
    /// * `kernel_ds` - Kernel data segment selector
    pub fn new_kernel(
        entry_point: u64,
        stack_pointer: u64,
        kernel_cs: u16,
        kernel_ds: u16,
    ) -> Self {
        // RFLAGS: IF enabled, reserved bit 1 set
        const KERNEL_RFLAGS: u64 = 0x200 | 0x2;

        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            rbx: 0,
            rbp: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rdi: 0,
            rsi: 0,
            rdx: 0,
            rcx: 0,
            rax: 0,
            ds: kernel_ds as u64,
            es: kernel_ds as u64,
            rip: entry_point,
            cs: kernel_cs as u64,
            rflags: KERNEL_RFLAGS,
            rsp: stack_pointer,
            ss: kernel_ds as u64,
        }
    }

    /// Set function argument (first argument goes in RDI)
    pub fn set_arg(&mut self, index: usize, value: u64) {
        match index {
            0 => self.rdi = value,
            1 => self.rsi = value,
            2 => self.rdx = value,
            3 => self.rcx = value,
            4 => self.r8 = value,
            5 => self.r9 = value,
            _ => panic!("Argument index {} out of range (max 5)", index),
        }
    }

    /// Get syscall number (from RAX)
    pub fn syscall_num(&self) -> u64 {
        self.rax
    }

    /// Get syscall arguments
    pub fn syscall_args(&self) -> (u64, u64, u64, u64, u64, u64) {
        (self.rdi, self.rsi, self.rdx, self.r10, self.r8, self.r9)
    }

    /// Set syscall return value
    pub fn set_syscall_return(&mut self, value: u64) {
        self.rax = value;
    }

    /// Set syscall error (negative value)
    pub fn set_syscall_error(&mut self, errno: i64) {
        self.rax = errno as u64;
    }
}

/// Switch from current context to new context
///
/// # Safety
///
/// - `old` must be a valid pointer to save current context
/// - `new` must be a valid pointer to context to restore
/// - Both contexts must have valid stack pointers
#[unsafe(naked)]
pub unsafe extern "C" fn context_switch(old: *mut ProcessContext, new: *const ProcessContext) {
    // This is a naked function - no prologue/epilogue
    core::arch::naked_asm!(
        // Save callee-saved registers to old context
        "mov [rdi + 0*8], r15",
        "mov [rdi + 1*8], r14",
        "mov [rdi + 2*8], r13",
        "mov [rdi + 3*8], r12",
        "mov [rdi + 4*8], rbx",
        "mov [rdi + 5*8], rbp",
        // Save stack pointer
        "mov [rdi + 19*8], rsp", // rsp offset in struct
        // Load callee-saved registers from new context
        "mov r15, [rsi + 0*8]",
        "mov r14, [rsi + 1*8]",
        "mov r13, [rsi + 2*8]",
        "mov r12, [rsi + 3*8]",
        "mov rbx, [rsi + 4*8]",
        "mov rbp, [rsi + 5*8]",
        // Load stack pointer
        "mov rsp, [rsi + 19*8]",
        // Return to new context
        "ret",
    );
}

/// Enter userspace for the first time
///
/// # Safety
///
/// - `context` must point to a valid user context
/// - The context must have valid user CS/SS selectors
/// - User page tables must be set up
#[unsafe(naked)]
pub unsafe extern "C" fn enter_userspace(context: *const ProcessContext) -> ! {
    core::arch::naked_asm!(
        // Load all general purpose registers from context
        "mov r15, [rdi + 0*8]",
        "mov r14, [rdi + 1*8]",
        "mov r13, [rdi + 2*8]",
        "mov r12, [rdi + 3*8]",
        "mov rbx, [rdi + 4*8]",
        "mov rbp, [rdi + 5*8]",
        "mov r11, [rdi + 6*8]",
        "mov r10, [rdi + 7*8]",
        "mov r9,  [rdi + 8*8]",
        "mov r8,  [rdi + 9*8]",
        // rdi loaded last
        "mov rsi, [rdi + 11*8]",
        "mov rdx, [rdi + 12*8]",
        "mov rcx, [rdi + 13*8]",
        "mov rax, [rdi + 14*8]",
        // Load segment registers
        "mov ds, [rdi + 15*8]",
        "mov es, [rdi + 16*8]",
        // Prepare iretq frame on stack
        // Push SS
        "push qword ptr [rdi + 21*8]",
        // Push RSP
        "push qword ptr [rdi + 20*8]",
        // Push RFLAGS
        "push qword ptr [rdi + 19*8]",
        // Push CS
        "push qword ptr [rdi + 18*8]",
        // Push RIP
        "push qword ptr [rdi + 17*8]",
        // Finally load RDI
        "mov rdi, [rdi + 10*8]",
        // Return to userspace
        "iretq",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_size() {
        // Ensure context struct is the expected size
        assert_eq!(core::mem::size_of::<ProcessContext>(), 22 * 8);
    }

    #[test]
    fn test_user_context() {
        let ctx = ProcessContext::new_user(0x400000, 0x7FFFFF000, 0x1B, 0x23);

        assert_eq!(ctx.rip, 0x400000);
        assert_eq!(ctx.rsp, 0x7FFFFF000);
        assert_eq!(ctx.cs, 0x1B);
        assert_eq!(ctx.ss, 0x23);
        assert_ne!(ctx.rflags & 0x200, 0); // IF set
    }

    #[test]
    fn test_syscall_args() {
        let mut ctx = ProcessContext::new();
        ctx.rax = 1; // syscall number
        ctx.rdi = 10;
        ctx.rsi = 20;
        ctx.rdx = 30;
        ctx.r10 = 40;
        ctx.r8 = 50;
        ctx.r9 = 60;

        assert_eq!(ctx.syscall_num(), 1);
        assert_eq!(ctx.syscall_args(), (10, 20, 30, 40, 50, 60));
    }
}
