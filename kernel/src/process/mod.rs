//! Process Management
//!
//! This module provides process creation, scheduling, and lifecycle management
//! for userspace programs.

pub mod context;
pub mod linux;
pub mod manager;
pub mod table;

pub use context::{ContextFlags, ProcessContext};
pub use linux::{launch_linux_process, LinuxProcessError, ProcessHandle};
pub use manager::ProcessManager;
pub use table::{ProcessId, ProcessState, ProcessTable};
