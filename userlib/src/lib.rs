//! KPIO OS Userspace Library
//!
//! This library provides system call wrappers and runtime support
//! for userspace applications running on KPIO OS.
//!
//! # Architecture
//!
//! Userspace programs use the `syscall` instruction to communicate
//! with the kernel. This library provides safe Rust wrappers around
//! the raw system calls.
//!
//! # Example
//!
//! ```rust,no_run
//! use userlib::io;
//!
//! fn main() {
//!     io::print("Hello from userspace!\n");
//! }
//! ```

#![no_std]
#![allow(unsafe_op_in_unsafe_fn)]

extern crate alloc;

pub mod allocator;
pub mod app;
pub mod io;
pub mod ipc;
pub mod mem;
pub mod process;
pub mod syscall;
pub mod thread;

/// std compatibility layer for running std-based applications
pub mod std;

/// Re-export commonly used types.
pub mod prelude {
    pub use crate::io::{print, println};
    pub use crate::process::{exit, getpid};
    pub use crate::syscall::SyscallError;
}

// Global allocator for userspace applications
#[global_allocator]
static ALLOCATOR: allocator::UserAllocator = allocator::UserAllocator::new();
