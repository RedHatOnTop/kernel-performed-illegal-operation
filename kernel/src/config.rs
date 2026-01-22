//! Kernel configuration constants.
//!
//! This module contains compile-time configuration for the kernel.
//! Values here affect memory layout, limits, and feature availability.

/// Maximum number of CPUs supported.
pub const MAX_CPUS: usize = 64;

/// Kernel heap size in bytes (16 MB).
pub const KERNEL_HEAP_SIZE: usize = 16 * 1024 * 1024;

/// Maximum number of processes.
pub const MAX_PROCESSES: usize = 4096;

/// Stack size per kernel task (64 KB).
pub const KERNEL_STACK_SIZE: usize = 64 * 1024;

/// User-space stack size per process (1 MB).
pub const USER_STACK_SIZE: usize = 1024 * 1024;

/// Page size (4 KB).
pub const PAGE_SIZE: usize = 4096;

/// Large page size (2 MB).
pub const LARGE_PAGE_SIZE: usize = 2 * 1024 * 1024;

/// Kernel virtual address base.
/// The kernel is mapped at the higher half of virtual memory.
pub const KERNEL_BASE: u64 = 0xFFFF_8000_0000_0000;

/// Physical memory map base.
/// Direct mapping of all physical memory for kernel access.
pub const PHYS_MAP_BASE: u64 = 0xFFFF_8880_0000_0000;

/// Kernel heap virtual address.
pub const KERNEL_HEAP_BASE: u64 = 0xFFFF_8000_1000_0000;

/// Kernel stack area base.
pub const KERNEL_STACK_BASE: u64 = 0xFFFF_8000_8000_0000;

/// Maximum IPC message size in bytes.
pub const MAX_IPC_MESSAGE_SIZE: usize = 64 * 1024;

/// Maximum number of IPC channels per process.
pub const MAX_CHANNELS_PER_PROCESS: usize = 256;

/// Timer interrupt frequency in Hz.
pub const TIMER_FREQUENCY: u32 = 1000;

/// Time slice for preemptive scheduling in milliseconds.
pub const TIME_SLICE_MS: u64 = 10;

/// Serial port for debug output (COM1).
pub const DEBUG_SERIAL_PORT: u16 = 0x3F8;

/// Enable kernel debugging features based on build profile.
pub const DEBUG_ENABLED: bool = cfg!(debug_assertions);
