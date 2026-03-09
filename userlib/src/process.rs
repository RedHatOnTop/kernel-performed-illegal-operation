//! Process management for userspace.
//!
//! This module provides functions to control the current process
//! and create new processes.

use crate::syscall::{linux, raw_syscall0, raw_syscall1, raw_syscall2, raw_syscall3, SyscallError, SyscallResult};

/// Exit the current process.
pub fn exit(code: i32) -> ! {
    unsafe {
        let _ = raw_syscall1(linux::SYS_EXIT, code as u64);
    }
    // Should never reach here
    loop {}
}

/// Get current process ID.
pub fn getpid() -> u64 {
    unsafe { raw_syscall0(linux::SYS_GETPID).unwrap_or(0) }
}

/// Get parent process ID.
pub fn getppid() -> u64 {
    unsafe { raw_syscall0(linux::SYS_GETPPID).unwrap_or(0) }
}

/// Fork the current process.
///
/// Returns:
/// - `Ok(0)` in the child process
/// - `Ok(child_pid)` in the parent process
/// - `Err(_)` if fork failed
pub fn fork() -> SyscallResult {
    unsafe { raw_syscall0(linux::SYS_FORK) }
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
        raw_syscall3(
            linux::SYS_WAIT4,
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
        let _ = raw_syscall0(linux::SYS_SCHED_YIELD);
    }
}

/// Sleep for milliseconds.
pub fn sleep_ms(ms: u64) {
    let nanos = ms * 1_000_000;
    let ts = [nanos / 1_000_000_000, nanos % 1_000_000_000];
    unsafe {
        let _ = raw_syscall1(linux::SYS_NANOSLEEP, ts.as_ptr() as u64);
    }
}

/// Get current time in nanoseconds since boot.
pub fn get_time() -> u64 {
    // CLOCK_MONOTONIC = 1
    let mut tp = [0u64; 2];
    unsafe {
        let _ = raw_syscall2(linux::SYS_CLOCK_GETTIME, 1, tp.as_mut_ptr() as u64);
    }
    tp[0] * 1_000_000_000 + tp[1]
}
