//! Process management for userspace.
//!
//! This module provides functions to control the current process
//! and create new processes.

use crate::syscall::{syscall0, syscall1, syscall3, SyscallError, SyscallNumber, SyscallResult};

/// Exit the current process.
pub fn exit(code: i32) -> ! {
    unsafe {
        let _ = syscall1(SyscallNumber::Exit, code as u64);
    }
    // Should never reach here
    loop {}
}

/// Get current process ID.
pub fn getpid() -> u64 {
    unsafe { syscall0(SyscallNumber::GetPid).unwrap_or(0) }
}

/// Get parent process ID.
pub fn getppid() -> u64 {
    unsafe { syscall0(SyscallNumber::GetPpid).unwrap_or(0) }
}

/// Fork the current process.
///
/// Returns:
/// - `Ok(0)` in the child process
/// - `Ok(child_pid)` in the parent process
/// - `Err(_)` if fork failed
pub fn fork() -> SyscallResult {
    unsafe { syscall0(SyscallNumber::Fork) }
}

/// Wait flags.
pub mod wait {
    /// Don't block if no child has exited.
    pub const WNOHANG: u32 = 1;
    /// Also report stopped children.
    pub const WUNTRACED: u32 = 2;
}

/// Wait status result.
#[derive(Debug, Clone, Copy)]
pub struct WaitStatus {
    raw: i32,
}

impl WaitStatus {
    /// Create from raw status.
    pub const fn from_raw(raw: i32) -> Self {
        Self { raw }
    }

    /// Check if child exited normally.
    pub fn exited(&self) -> bool {
        (self.raw & 0x7f) == 0
    }

    /// Get exit code (if exited normally).
    pub fn exit_code(&self) -> Option<i32> {
        if self.exited() {
            Some((self.raw >> 8) & 0xff)
        } else {
            None
        }
    }

    /// Check if child was killed by signal.
    pub fn signaled(&self) -> bool {
        ((self.raw & 0x7f) + 1) >> 1 > 0
    }

    /// Get signal number (if signaled).
    pub fn signal(&self) -> Option<i32> {
        if self.signaled() {
            Some(self.raw & 0x7f)
        } else {
            None
        }
    }
}

/// Wait for a child process.
///
/// Returns `(pid, status)` on success.
pub fn waitpid(pid: i64, options: u32) -> Result<(u64, WaitStatus), SyscallError> {
    let mut status: i32 = 0;
    let result = unsafe {
        syscall3(
            SyscallNumber::Wait,
            pid as u64,
            &mut status as *mut i32 as u64,
            options as u64,
        )
    }?;

    Ok((result, WaitStatus::from_raw(status)))
}

/// Wait for any child process.
pub fn wait() -> Result<(u64, WaitStatus), SyscallError> {
    waitpid(-1, 0)
}

/// Yield CPU to other processes.
pub fn yield_now() {
    unsafe {
        let _ = syscall0(SyscallNumber::Yield);
    }
}

/// Sleep for milliseconds.
pub fn sleep_ms(ms: u64) {
    unsafe {
        let _ = syscall1(SyscallNumber::Sleep, ms);
    }
}

/// Get current time in nanoseconds since boot.
pub fn get_time() -> u64 {
    unsafe { syscall0(SyscallNumber::GetTime).unwrap_or(0) }
}
