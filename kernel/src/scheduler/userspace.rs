//! User-Space Initialization for Ring 3 Support
//!
//! This module provides the self-contained setup required for Ring 3
//! process execution.  It configures:
//!
//! - **Per-CPU data** (`KERNEL_GS_BASE` MSR) so that `swapgs` in
//!   `linux_syscall_entry` can locate the kernel stack pointer.
//! - **SYSCALL/SYSRET MSRs** (`EFER.SCE`, `STAR`, `LSTAR`, `SFMASK`)
//!   so that the `syscall` instruction transitions cleanly to the
//!   kernel's Linux-compatible entry point.
//!
//! ## Why this lives in `scheduler/`
//!
//! The `syscall` module resides in `lib.rs` only (it depends on
//! `process`, `loader`, `ipc` which are not compiled into the binary
//! crate).  The scheduler IS compiled by the binary crate, and needs
//! per-CPU / MSR setup to drive Ring 3 tasks.  By placing the init
//! code here, we avoid cross-module dependency issues between the
//! binary and library crate trees.
//!
//! The actual `linux_syscall_entry` naked function is `#[no_mangle]`
//! in `syscall/linux.rs` and is referenced via `extern "C"` linkage
//! at link time.

use core::sync::atomic::{AtomicBool, Ordering};

// ─── Per-CPU Data ────────────────────────────────────────────────────

/// Maximum CPUs supported.
const MAX_CPUS: usize = 64;

/// Per-CPU data structure (must match `syscall::percpu::PerCpuData` exactly).
///
/// | Offset | Field            | Description                       |
/// |--------|------------------|-----------------------------------|
/// |   0    | kernel_rsp       | Kernel stack top for this CPU     |
/// |   8    | user_rsp_scratch | Saved user RSP during syscall     |
/// |  16    | current_pid      | Current process ID on this CPU    |
/// |  24    | cpu_id           | CPU core number                   |
/// |  32    | in_syscall       | Non-zero while processing syscall |
#[repr(C, align(64))]
struct PerCpuData {
    kernel_rsp: u64,
    user_rsp_scratch: u64,
    current_pid: u64,
    cpu_id: u64,
    in_syscall: u64,
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

// Compile-time layout assertion.
const _: () = {
    assert!(core::mem::size_of::<PerCpuData>() == 64);
    assert!(core::mem::align_of::<PerCpuData>() == 64);
};

/// Static per-CPU data array (one entry per logical CPU).
static mut PER_CPU_ARRAY: [PerCpuData; MAX_CPUS] = {
    const INIT: PerCpuData = PerCpuData::new();
    [INIT; MAX_CPUS]
};

static INITIALIZED: AtomicBool = AtomicBool::new(false);

// ─── Minimal syscall entry ───────────────────────────────────────────

/// Minimal Linux-compatible syscall entry point for Ring 3 testing.
///
/// Handles SYS_EXIT (60) and SYS_WRITE (1).  The full Linux
/// compatibility layer (`syscall::linux::linux_syscall_entry`) is
/// compiled in the library crate; this is a self-contained version
/// for the binary crate.
///
/// On SYSCALL, CPU saves: RIP → RCX, RFLAGS → R11, loads CS/SS from
/// STAR. We save rcx/r11 plus caller-saved regs, call Rust dispatcher,
/// then restore and `sysretq`.
#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "C" fn ring3_syscall_entry() {
    core::arch::naked_asm!(
        // 1. Swap to kernel GS so gs:[0] = kernel_rsp, gs:[8] = scratch
        "swapgs",
        // 2. Save user RSP in per-CPU scratch, load kernel stack
        "mov gs:[8], rsp",
        "mov rsp, gs:[0]",
        // 3. Mark in-syscall
        "mov qword ptr gs:[32], 1",
        // 4. Save registers we must preserve across the call
        "push rcx",             // user RIP (saved by CPU)
        "push r11",             // user RFLAGS (saved by CPU)
        "push rdi",             // arg1
        "push rsi",             // arg2
        "push rdx",             // arg3
        // 5. Set up System V calling convention for ring3_syscall_dispatch:
        //    rdi = nr (from rax), rsi = a1 (from rdi), rdx = a2 (from rsi),
        //    rcx = a3 (from rdx)
        //    NOTE: we use the original register values, not the pushed copies.
        "mov rcx, rdx",        // a3 = original rdx
        "mov rdx, rsi",        // a2 = original rsi
        "mov rsi, rdi",        // a1 = original rdi
        "mov rdi, rax",        // nr = syscall number
        // 6. Call Rust dispatcher — returns result in RAX
        "call ring3_syscall_dispatch",
        // 7. Restore registers
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop r11",              // user RFLAGS
        "pop rcx",              // user RIP
        // 8. Clear in-syscall flag
        "mov qword ptr gs:[32], 0",
        // 9. Restore user RSP from per-CPU scratch
        "mov rsp, gs:[8]",
        // 10. Swap back to user GS and return
        "swapgs",
        "sysretq",
    );
}

/// Rust dispatcher called from the minimal syscall entry point.
///
/// Handles SYS_EXIT (60), SYS_WRITE (1), and a few stubs.
/// Returns value in RAX (negative = -errno).
#[no_mangle]
extern "C" fn ring3_syscall_dispatch(nr: u64, a1: u64, a2: u64, _a3: u64) -> i64 {
    match nr {
        // SYS_WRITE (fd=a1, buf=a2, count=a3)
        1 => {
            // For testing: write to serial regardless of fd
            let count = _a3 as usize;
            if count > 0 && a2 != 0 && a2 < 0x0000_8000_0000_0000 {
                let buf = unsafe { core::slice::from_raw_parts(a2 as *const u8, count.min(256)) };
                if let Ok(s) = core::str::from_utf8(buf) {
                    crate::serial_print!("{}", s);
                }
            }
            count as i64
        }
        // SYS_EXIT (status=a1) / SYS_EXIT_GROUP (231)
        60 | 231 => {
            crate::serial_println!("[RING3] SYS_EXIT called with status={}", a1 as i64);
            super::exit_current(a1 as i32);
            // never reached
            0
        }
        // SYS_BRK — return 0 (stub)
        12 => 0,
        // SYS_ARCH_PRCTL — stub
        158 => 0,
        // Unknown — return -ENOSYS
        _ => {
            crate::serial_println!("[RING3] Unknown syscall nr={}", nr);
            -38 // -ENOSYS
        }
    }
}

// ─── MSR constants ───────────────────────────────────────────────────

const IA32_EFER: u32 = 0xC000_0080;
const IA32_STAR: u32 = 0xC000_0081;
const IA32_LSTAR: u32 = 0xC000_0082;
const IA32_SFMASK: u32 = 0xC000_0084;
const IA32_KERNEL_GS_BASE: u32 = 0xC000_0102;

/// EFER.SCE (System Call Extensions) bit.
const EFER_SCE: u64 = 1 << 0;

// ─── Public API ──────────────────────────────────────────────────────

/// Initialize everything needed for Ring 3 user-space execution.
///
/// Must be called after GDT, IDT, and heap initialization.
///
/// This sets up:
/// 1. Per-CPU data for the BSP (CPU 0) + `KERNEL_GS_BASE` MSR
/// 2. EFER.SCE to enable the SYSCALL instruction
/// 3. STAR MSR for segment selectors
/// 4. LSTAR MSR pointing to `linux_syscall_entry`
/// 5. SFMASK to clear IF and TF on syscall entry
pub fn init() {
    if INITIALIZED.load(Ordering::Acquire) {
        return; // idempotent
    }

    // 1. Per-CPU data
    let cpu_id: usize = 0; // BSP
    unsafe {
        PER_CPU_ARRAY[cpu_id].cpu_id = cpu_id as u64;

        let per_cpu_addr = &PER_CPU_ARRAY[cpu_id] as *const PerCpuData as u64;

        use x86_64::registers::model_specific::Msr;
        Msr::new(IA32_KERNEL_GS_BASE).write(per_cpu_addr);
    }

    // 2. Enable SYSCALL/SYSRET
    unsafe {
        use x86_64::registers::model_specific::Msr;
        let efer = Msr::new(IA32_EFER).read();
        Msr::new(IA32_EFER).write(efer | EFER_SCE);
    }

    // 3. STAR MSR — segment selectors
    //    STAR[47:32] = 0x0008 → SYSCALL: CS = 0x08, SS = 0x10
    //    STAR[63:48] = 0x0010 → SYSRET:  SS = (0x10+8)|3 = 0x1B,
    //                                    CS = (0x10+16)|3 = 0x23
    let star_value: u64 = (0x0010_0008u64) << 32;
    unsafe {
        use x86_64::registers::model_specific::Msr;
        Msr::new(IA32_STAR).write(star_value);
    }

    // 4. LSTAR — syscall entry point
    unsafe {
        use x86_64::registers::model_specific::Msr;
        Msr::new(IA32_LSTAR).write(ring3_syscall_entry as u64);
    }

    // 5. SFMASK — clear IF (0x200) and TF (0x100) on SYSCALL
    unsafe {
        use x86_64::registers::model_specific::Msr;
        Msr::new(IA32_SFMASK).write(0x200 | 0x100);
    }

    INITIALIZED.store(true, Ordering::Release);

    crate::serial_println!(
        "[RING3] Userspace init: KERNEL_GS_BASE + STAR/LSTAR/SFMASK configured"
    );
    crate::serial_println!(
        "[RING3]   LSTAR = {:#x} (ring3_syscall_entry)",
        ring3_syscall_entry as u64
    );
}
