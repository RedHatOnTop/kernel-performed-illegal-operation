//! I/O functions for userspace.
//!
//! This module provides standard I/O operations like print and read.

use crate::syscall::{syscall2, syscall3, SyscallNumber, SyscallResult};

/// File descriptor for stdin.
pub const STDIN: u64 = 0;
/// File descriptor for stdout.
pub const STDOUT: u64 = 1;
/// File descriptor for stderr.
pub const STDERR: u64 = 2;

/// Write bytes to a file descriptor.
pub fn write(fd: u64, buf: &[u8]) -> SyscallResult {
    unsafe {
        syscall3(
            SyscallNumber::Write,
            fd,
            buf.as_ptr() as u64,
            buf.len() as u64,
        )
    }
}

/// Read bytes from a file descriptor.
pub fn read(fd: u64, buf: &mut [u8]) -> SyscallResult {
    unsafe {
        syscall3(
            SyscallNumber::Read,
            fd,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
        )
    }
}

/// Print a string to stdout.
pub fn print(s: &str) {
    let _ = write(STDOUT, s.as_bytes());
}

/// Print a string to stdout with a newline.
pub fn println(s: &str) {
    print(s);
    print("\n");
}

/// Print a string to stderr.
pub fn eprint(s: &str) {
    let _ = write(STDERR, s.as_bytes());
}

/// Print a string to stderr with a newline.
pub fn eprintln(s: &str) {
    eprint(s);
    eprint("\n");
}

/// Debug print (always goes to serial).
pub fn debug_print(s: &str) {
    unsafe {
        let _ = syscall2(
            SyscallNumber::DebugPrint,
            s.as_ptr() as u64,
            s.len() as u64,
        );
    }
}

/// Open file flags.
pub mod flags {
    /// Open for reading only.
    pub const O_RDONLY: u32 = 0;
    /// Open for writing only.
    pub const O_WRONLY: u32 = 1;
    /// Open for reading and writing.
    pub const O_RDWR: u32 = 2;
    /// Create if not exists.
    pub const O_CREAT: u32 = 0o100;
    /// Truncate to zero length.
    pub const O_TRUNC: u32 = 0o1000;
    /// Append mode.
    pub const O_APPEND: u32 = 0o2000;
}

/// File handle wrapper.
pub struct File {
    fd: u64,
}

impl File {
    /// Create a File from a raw file descriptor.
    pub const fn from_raw_fd(fd: u64) -> Self {
        Self { fd }
    }
    
    /// Get the raw file descriptor.
    pub const fn raw_fd(&self) -> u64 {
        self.fd
    }
    
    /// Write data to the file.
    pub fn write(&self, buf: &[u8]) -> SyscallResult {
        write(self.fd, buf)
    }
    
    /// Read data from the file.
    pub fn read(&self, buf: &mut [u8]) -> SyscallResult {
        read(self.fd, buf)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        unsafe {
            let _ = crate::syscall::syscall1(SyscallNumber::Close, self.fd);
        }
    }
}

/// Stdin handle.
pub fn stdin() -> File {
    File::from_raw_fd(STDIN)
}

/// Stdout handle.
pub fn stdout() -> File {
    File::from_raw_fd(STDOUT)
}

/// Stderr handle.
pub fn stderr() -> File {
    File::from_raw_fd(STDERR)
}
