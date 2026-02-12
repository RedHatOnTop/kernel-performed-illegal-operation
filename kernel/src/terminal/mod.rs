//! Terminal Subsystem
//!
//! Linux-compatible shell with in-memory filesystem,
//! environment variables, command history, ANSI colour support,
//! and 80+ commands.

pub mod ansi;
pub mod commands;
pub mod fs;
pub mod shell;
