//! Process Management
//!
//! This module provides process creation, scheduling, and lifecycle management
//! for userspace programs.

pub mod context;
pub mod manager;
pub mod table;

pub use context::{ProcessContext, ContextFlags};
pub use manager::ProcessManager;
pub use table::{ProcessId, ProcessTable, ProcessState};
