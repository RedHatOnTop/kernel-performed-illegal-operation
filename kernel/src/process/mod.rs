//! Process Management
//!
//! This module provides process creation, scheduling, and lifecycle management
//! for userspace programs.

pub mod context;
pub mod linux;
pub mod manager;
pub mod signal;
pub mod table;
pub mod test_programs;

pub use context::{ContextFlags, ProcessContext};
pub use linux::{launch_linux_process, LinuxProcessError, ProcessHandle};
pub use manager::ProcessManager;
pub use signal::SignalState;
pub use table::{ProcessId, ProcessState, ProcessTable};
