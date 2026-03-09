//! I/O functions for userspace.
//!
//! This module provides standard I/O operations like print and read.

use crate::syscall::{self, linux, raw_syscall1, raw_syscall2, raw_syscall3, SyscallResult};

/// File descriptor for stdin.
pub const STDIN: u64 = 0;
/// File descriptor for stdout.
pub const STDOUT: u64 = 1;
/// File descriptor for stderr.
pub const STDERR: u64 = 2;

/// Write bytes to a file descriptor.
pub fn write(fd: u64, buf: &[u8]) -> SyscallResult {
    unsafe {
        raw_syscall3(
            linux::SYS_WRITE,
            fd,
            buf.as_ptr() as u64,
            buf.len() as u64,
        )
    }
}

/// Read bytes from a file descriptor.
pub fn read(fd: u64, buf: &mut [u8]) -> SyscallResult {
    unsafe {
        raw_syscall3(
            linux::SYS_READ,
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
        let _ = raw_syscall2(linux::SYS_WRITE, 1, s.as_ptr() as u64);
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

/// Seek whence constants.
pub mod seek {
    /// Seek from beginning of file.
    pub const SEEK_SET: u32 = 0;
    /// Seek from current position.
    pub const SEEK_CUR: u32 = 1;
    /// Seek from end of file.
    pub const SEEK_END: u32 = 2;
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

    /// Open a file at `path` with the given flags.
    pub fn open(path: &str, open_flags: u32) -> Result<Self, syscall::SyscallError> {
        let fd = syscall::fs_open(path, open_flags)?;
        Ok(Self { fd })
    }

    /// Create a new file (or truncate existing) for writing.
    pub fn create(path: &str) -> Result<Self, syscall::SyscallError> {
        let fd = syscall::fs_open(
            path,
            flags::O_WRONLY | flags::O_CREAT | flags::O_TRUNC,
        )?;
        Ok(Self { fd })
    }

    /// Write data to the file.
    pub fn write(&self, buf: &[u8]) -> SyscallResult {
        write(self.fd, buf)
    }

    /// Read data from the file.
    pub fn read(&self, buf: &mut [u8]) -> SyscallResult {
        read(self.fd, buf)
    }

    /// Seek to a position in the file.
    pub fn seek(&self, offset: i64, whence: u32) -> Result<u64, syscall::SyscallError> {
        syscall::fs_seek(self.fd, offset, whence)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        unsafe {
            let _ = raw_syscall1(linux::SYS_CLOSE, self.fd);
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
