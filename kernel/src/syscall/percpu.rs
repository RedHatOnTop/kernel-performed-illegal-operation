//! Per-CPU Data Area
//!
//! Provides a per-CPU data structure used during syscall entry/exit.
//! The kernel GS base (`KERNEL_GS_BASE` MSR) is set to point to this
//! structure so that `swapgs` at syscall entry gives the kernel access
//! to the kernel stack pointer and a scratch area for the user RSP.
//!
//! # Memory Layout (at GS base)
//!
//! | Offset | Field            | Description                       |
//! |--------|------------------|-----------------------------------|
//! |   0    | kernel_rsp       | Kernel stack top for this CPU     |
//! |   8    | user_rsp_scratch | Saved user RSP during syscall     |
//! |  16    | current_pid      | Current process ID on this CPU    |
//! |  24    | cpu_id           | CPU core number                   |
//! |  32    | in_syscall       | Non-zero while processing syscall |

use core::sync::atomic::{AtomicBool, Ordering};

/// Maximum number of CPUs supported.
const MAX_CPUS: usize = 64;

/// Per-CPU data structure.
///
/// This struct is accessed via the GS segment during syscall entry.
/// Field offsets must match the assembly constants in `linux.rs`.
#[repr(C, align(64))]
pub struct PerCpuData {
    /// Kernel stack pointer for the current process on this CPU.
    /// Loaded by `swapgs; mov rsp, [gs:0]` during syscall entry.
    pub kernel_rsp: u64,

    /// Scratch space for saving user RSP.
    /// Written by `mov [gs:8], rsp` during syscall entry.
    pub user_rsp_scratch: u64,

    /// Process ID of the currently running process on this CPU.
    pub current_pid: u64,

    /// CPU core ID.
    pub cpu_id: u64,

    /// Non-zero while a syscall is being processed.
    pub in_syscall: u64,

    /// Padding to cache-line boundary.
    _pad: [u64; 3],
}

impl PerCpuData {
    const fn new() -> Self {
        Self {
            kernel_rsp: 0,
            user_rsp_scratch: 0,
            current_pid: 0,
            cpu_id: 0,
            in_syscall: 0,
            _pad: [0; 3],
        }
    }
}

// Verify layout at compile time.
const _: () = {
    assert!(core::mem::size_of::<PerCpuData>() == 64);
    assert!(core::mem::align_of::<PerCpuData>() == 64);
};

/// Static array of per-CPU data, one entry per logical CPU.
static mut PER_CPU_ARRAY: [PerCpuData; MAX_CPUS] = {
    const INIT: PerCpuData = PerCpuData::new();
    [INIT; MAX_CPUS]
};

/// Whether per-CPU data has been initialised.
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Assembly-visible field offsets.
/// These MUST match the struct layout above.
pub mod offsets {
    /// Offset of `kernel_rsp` within [`PerCpuData`](super::PerCpuData).
    pub const KERNEL_RSP: usize = 0;
    /// Offset of `user_rsp_scratch` within [`PerCpuData`](super::PerCpuData).
    pub const USER_RSP_SCRATCH: usize = 8;
    /// Offset of `current_pid` within [`PerCpuData`](super::PerCpuData).
    pub const CURRENT_PID: usize = 16;
    /// Offset of `cpu_id` within [`PerCpuData`](super::PerCpuData).
    pub const CPU_ID: usize = 24;
    /// Offset of `in_syscall` within [`PerCpuData`](super::PerCpuData).
    pub const IN_SYSCALL: usize = 32;
}

/// MSR number for `KERNEL_GS_BASE` (swapped with GS base on `swapgs`).
const KERNEL_GS_BASE_MSR: u32 = 0xC000_0102;

/// Initialise per-CPU data for the bootstrap processor (CPU 0).
///
/// Sets the `KERNEL_GS_BASE` MSR so that `swapgs` in syscall entry
/// will load the address of the BSP's `PerCpuData`.
///
/// # Safety
///
/// Must be called exactly once during kernel init, after GDT/IDT setup.
pub fn init() {
    let cpu_id: usize = 0; // BSP = CPU 0

    unsafe {
        PER_CPU_ARRAY[cpu_id].cpu_id = cpu_id as u64;

        // Set KERNEL_GS_BASE to point to this CPU's PerCpuData.
        // When user code runs, GS base will hold the user value.
        // On `swapgs`, GS base ↔ KERNEL_GS_BASE are swapped.
        let per_cpu_addr = &PER_CPU_ARRAY[cpu_id] as *const PerCpuData as u64;

        use x86_64::registers::model_specific::Msr;
        Msr::new(KERNEL_GS_BASE_MSR).write(per_cpu_addr);
    }

    INITIALIZED.store(true, Ordering::Release);
}

/// Update the kernel stack pointer for the current CPU.
///
/// Called during process switch to set the kernel RSP that will be
/// loaded on the next `syscall` instruction from userspace.
pub fn set_kernel_rsp(cpu: usize, rsp: u64) {
    assert!(cpu < MAX_CPUS, "CPU ID out of range");
    unsafe {
        PER_CPU_ARRAY[cpu].kernel_rsp = rsp;
    }
}

/// Update the current PID on a CPU.
pub fn set_current_pid(cpu: usize, pid: u64) {
    assert!(cpu < MAX_CPUS, "CPU ID out of range");
    unsafe {
        PER_CPU_ARRAY[cpu].current_pid = pid;
    }
}

/// Get the current PID on a CPU.
pub fn get_current_pid(cpu: usize) -> u64 {
    assert!(cpu < MAX_CPUS, "CPU ID out of range");
    unsafe { PER_CPU_ARRAY[cpu].current_pid }
}

/// Get the saved user RSP (set by syscall entry assembly).
pub fn get_user_rsp(cpu: usize) -> u64 {
    assert!(cpu < MAX_CPUS, "CPU ID out of range");
    unsafe { PER_CPU_ARRAY[cpu].user_rsp_scratch }
}

/// Get a raw pointer to the per-CPU data for a given CPU.
///
/// Used for setting KERNEL_GS_BASE on AP startup.
pub fn get_per_cpu_ptr(cpu: usize) -> *const PerCpuData {
    assert!(cpu < MAX_CPUS, "CPU ID out of range");
    unsafe { &PER_CPU_ARRAY[cpu] as *const PerCpuData }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percpu_data_size() {
        assert_eq!(core::mem::size_of::<PerCpuData>(), 64);
    }

    #[test]
    fn test_field_offsets() {
        assert_eq!(offsets::KERNEL_RSP, 0);
        assert_eq!(offsets::USER_RSP_SCRATCH, 8);
        assert_eq!(offsets::CURRENT_PID, 16);
        assert_eq!(offsets::CPU_ID, 24);
        assert_eq!(offsets::IN_SYSCALL, 32);
    }

    #[test]
    fn test_set_kernel_rsp() {
        unsafe {
            PER_CPU_ARRAY[0].kernel_rsp = 0;
        }
        set_kernel_rsp(0, 0xDEAD_BEEF_0000);
        assert_eq!(unsafe { PER_CPU_ARRAY[0].kernel_rsp }, 0xDEAD_BEEF_0000);
    }

    #[test]
    fn test_set_current_pid() {
        unsafe {
            PER_CPU_ARRAY[0].current_pid = 0;
        }
        set_current_pid(0, 42);
        assert_eq!(get_current_pid(0), 42);
    }
}
