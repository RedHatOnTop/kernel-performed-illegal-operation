//! ELF Binary Loader
//!
//! This module provides functionality to load and execute ELF64 binaries
//! in userspace.
//!
//! # ELF64 Format Support
//!
//! - ELF64 header parsing
//! - Program header (LOAD segments)
//! - Section header (optional, for debugging)
//! - Position Independent Executables (PIE)
//!
//! # Security
//!
//! - Validates all ELF headers and offsets
//! - Enforces W^X (Write XOR Execute) policy
//! - Maps userspace memory with appropriate permissions

pub mod elf;
pub mod program;
pub mod segment_loader;

pub use elf::{Elf64Loader, ElfError, LoadedProgram};
pub use program::{ProgramState, UserProgram};
pub use segment_loader::{LoadResult, SegmentLoadError};
