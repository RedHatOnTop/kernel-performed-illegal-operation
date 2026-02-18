//! Linux x86_64 Syscall Compatibility Layer
//!
//! Implements proper syscall entry/exit using `swapgs` + stack swap + register
//! save/restore, and a dispatch table mapping Linux syscall numbers to handlers.
//!
//! # Syscall ABI (Linux x86_64)
//!
//! | Register | Purpose                |
//! |----------|------------------------|
//! | RAX      | Syscall number         |
//! | RDI      | Argument 1             |
//! | RSI      | Argument 2             |
//! | RDX      | Argument 3             |
//! | R10      | Argument 4 (not RCX!)  |
//! | R8       | Argument 5             |
//! | R9       | Argument 6             |
//! | RAX      | Return value           |
//! | RCX      | Saved RIP (by SYSCALL) |
//! | R11      | Saved RFLAGS (by SYSCALL) |

use super::linux_handlers;
use super::percpu;

// ─── Linux errno constants ────────────────────────────────────────────
/// We return negative errno values from syscall handlers (e.g. -ENOENT).

pub const EPERM: i64 = 1;
pub const ENOENT: i64 = 2;
pub const ESRCH: i64 = 3;
pub const EINTR: i64 = 4;
pub const EIO: i64 = 5;
pub const ENXIO: i64 = 6;
pub const EBADF: i64 = 9;
pub const EAGAIN: i64 = 11;
pub const ENOMEM: i64 = 12;
pub const EACCES: i64 = 13;
pub const EFAULT: i64 = 14;
pub const EEXIST: i64 = 17;
pub const ENOTDIR: i64 = 20;
pub const EISDIR: i64 = 21;
pub const EINVAL: i64 = 22;
pub const EMFILE: i64 = 24;
pub const ENOSPC: i64 = 28;
pub const ESPIPE: i64 = 29;
pub const EROFS: i64 = 30;
pub const ENOSYS: i64 = 38;
pub const ENOTEMPTY: i64 = 39;

// ─── Linux syscall numbers (x86_64) ──────────────────────────────────

pub const SYS_READ: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_OPEN: u64 = 2;
pub const SYS_CLOSE: u64 = 3;
pub const SYS_STAT: u64 = 4;
pub const SYS_FSTAT: u64 = 5;
pub const SYS_LSEEK: u64 = 8;
pub const SYS_MMAP: u64 = 9;
pub const SYS_MPROTECT: u64 = 10;
pub const SYS_MUNMAP: u64 = 11;
pub const SYS_BRK: u64 = 12;
pub const SYS_IOCTL: u64 = 16;
pub const SYS_ACCESS: u64 = 21;
pub const SYS_DUP: u64 = 32;
pub const SYS_DUP2: u64 = 33;
pub const SYS_GETPID: u64 = 39;
pub const SYS_EXIT: u64 = 60;
pub const SYS_UNAME: u64 = 63;
pub const SYS_GETUID: u64 = 102;
pub const SYS_GETGID: u64 = 104;
pub const SYS_GETEUID: u64 = 107;
pub const SYS_GETEGID: u64 = 108;
pub const SYS_ARCH_PRCTL: u64 = 158;
pub const SYS_OPENAT: u64 = 257;
pub const SYS_EXIT_GROUP: u64 = 231;
pub const SYS_SET_TID_ADDRESS: u64 = 218;
pub const SYS_CLOCK_GETTIME: u64 = 228;
pub const SYS_GETRANDOM: u64 = 318;

/// AT_FDCWD sentinel value used by `openat`.
pub const AT_FDCWD: i32 = -100;

// ─── Naked syscall entry point ───────────────────────────────────────

/// Linux-compatible syscall entry point.
///
/// This is a `naked` function whose address is written into the LSTAR MSR.
/// When user code executes `syscall`:
///
/// - CPU saves RIP → RCX, RFLAGS → R11
/// - CPU loads CS/SS from STAR MSR (kernel segments)
/// - CPU clears RFLAGS bits per SFMASK
/// - CPU jumps to LSTAR (this function)
///
/// We must:
/// 1. `swapgs` — swap user GS ↔ kernel GS base
/// 2. Save user RSP in per-cpu scratch, load kernel stack
/// 3. Push all caller registers
/// 4. Call Rust dispatch function
/// 5. Pop registers
/// 6. Restore user RSP, `swapgs`, `sysretq`
#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "C" fn linux_syscall_entry() {
    core::arch::naked_asm!(
        // ----- Step 1: switch to kernel GS -----
        "swapgs",

        // ----- Step 2: save user RSP, load kernel stack -----
        "mov gs:[8], rsp",          // [gs:8] = user_rsp_scratch
        "mov rsp, gs:[0]",          // rsp = kernel_rsp from PerCpuData

        // ----- Step 3: push all registers (build SyscallFrame on stack) -----
        // We push in reverse order so the struct fields match pop order.
        // Frame layout (top → bottom, growing down):
        //   user_rsp, r11 (saved rflags), rcx (saved rip),
        //   rax (syscall nr), rdi, rsi, rdx, r10, r8, r9,
        //   rbx, rbp, r12, r13, r14, r15
        "push gs:[8]",              // user RSP (from scratch)
        "push r11",                 // saved RFLAGS
        "push rcx",                 // saved RIP (return address)

        "push rax",                 // syscall number
        "push rdi",                 // arg1
        "push rsi",                 // arg2
        "push rdx",                 // arg3
        "push r10",                 // arg4 (Linux uses r10 not rcx)
        "push r8",                  // arg5
        "push r9",                  // arg6

        // Callee-saved (preserve across syscall)
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // Mark in-syscall
        "mov qword ptr gs:[32], 1", // in_syscall = 1

        // ----- Step 4: call Rust dispatcher -----
        // Args for linux_syscall_dispatch_inner(nr, a1, a2, a3, a4, a5, a6):
        // Already in correct registers from user: rdi=a1, rsi=a2, rdx=a3
        // But we need: rdi=nr, rsi=a1, rdx=a2, rcx=a3, r8=a4, r9=a5, stack=a6
        //
        // Saved values on stack (from top): r15,r14,r13,r12,rbp,rbx, r9,r8,r10,rdx,rsi,rdi, rax, rcx,r11,user_rsp
        //
        // Reload from stack for proper calling convention:
        "mov rdi, [rsp + 10*8]",    // nr  = saved rax (syscall number)
        "mov rsi, [rsp + 9*8]",     // a1  = saved rdi
        "mov rdx, [rsp + 8*8]",     // a2  = saved rsi
        "mov rcx, [rsp + 7*8]",     // a3  = saved rdx
        "mov r8,  [rsp + 6*8]",     // a4  = saved r10
        "mov r9,  [rsp + 5*8]",     // a5  = saved r8
        // a6 = saved r9 — push as 7th arg on stack
        "push qword ptr [rsp + 4*8 + 8]", // +8 because we just pushed

        // Align stack to 16 bytes (we've pushed 17 qwords = 136 bytes; + 8 = 144 = aligned)
        // Actually 16+1 = 17 pushes. The kernel stack was already 16-aligned,
        // 17*8 = 136 → not 16-aligned. Add 8 for the 7th arg push → 144 ✓

        "call linux_syscall_dispatch_inner",

        // Clean up the 7th arg we pushed
        "add rsp, 8",

        // RAX now holds the return value

        // ----- Step 5: clear in-syscall flag -----
        "mov qword ptr gs:[32], 0",

        // ----- Step 6: restore registers -----
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",

        // Skip saved r9, r8, r10, rdx, rsi, rdi, rax (7 slots)
        "add rsp, 7*8",

        // Pop saved RIP → RCX, saved RFLAGS → R11
        "pop rcx",
        "pop r11",
        // Pop user RSP
        "pop rsp",

        // ----- Step 7: return to userspace -----
        "swapgs",
        "sysretq",
    );
}

// ─── Rust dispatch function ──────────────────────────────────────────

/// Inner dispatch function called from the naked entry point.
///
/// # Arguments
///
/// Linux x86_64 syscall register mapping:
/// - `nr`  — syscall number (was in RAX)
/// - `a1`–`a6` — arguments (were in RDI, RSI, RDX, R10, R8, R9)
///
/// # Returns
///
/// Return value in RAX. Negative values are `-errno`.
#[no_mangle]
extern "C" fn linux_syscall_dispatch_inner(
    nr: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    a6: u64,
) -> i64 {
    linux_syscall_dispatch(nr, a1, a2, a3, a4, a5, a6)
}

/// Dispatch a Linux syscall number to the appropriate handler.
///
/// Returns the syscall result (>=0 for success, negative errno on error).
pub fn linux_syscall_dispatch(
    nr: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    a6: u64,
) -> i64 {
    match nr {
        // File I/O
        SYS_READ => linux_handlers::sys_read(a1 as i32, a2, a3),
        SYS_WRITE => linux_handlers::sys_write(a1 as i32, a2, a3),
        SYS_OPEN => linux_handlers::sys_open(a1, a2 as u32, a3 as u32),
        SYS_CLOSE => linux_handlers::sys_close(a1 as i32),
        SYS_STAT => linux_handlers::sys_stat(a1, a2),
        SYS_FSTAT => linux_handlers::sys_fstat(a1 as i32, a2),
        SYS_LSEEK => linux_handlers::sys_lseek(a1 as i32, a2 as i64, a3 as u32),
        SYS_ACCESS => linux_handlers::sys_access(a1, a2 as u32),
        SYS_OPENAT => linux_handlers::sys_openat(a1 as i32, a2, a3 as u32, a4 as u32),
        SYS_DUP => linux_handlers::sys_dup(a1 as i32),
        SYS_DUP2 => linux_handlers::sys_dup2(a1 as i32, a2 as i32),
        SYS_IOCTL => linux_handlers::sys_ioctl(a1 as i32, a2, a3),

        // Process
        SYS_EXIT => linux_handlers::sys_exit(a1 as i32),
        SYS_EXIT_GROUP => linux_handlers::sys_exit(a1 as i32),
        SYS_GETPID => linux_handlers::sys_getpid(),
        SYS_GETUID | SYS_GETEUID => 0, // root
        SYS_GETGID | SYS_GETEGID => 0, // root

        // Memory management
        SYS_BRK => linux_handlers::sys_brk(a1),
        SYS_MMAP => linux_handlers::sys_mmap(a1, a2, a3 as u32, a4 as u32, a5 as i32, a6),
        SYS_MPROTECT => linux_handlers::sys_mprotect(a1, a2, a3 as u32),
        SYS_MUNMAP => linux_handlers::sys_munmap(a1, a2),

        // Misc
        SYS_UNAME => linux_handlers::sys_uname(a1),
        SYS_ARCH_PRCTL => linux_handlers::sys_arch_prctl(a1 as i32, a2),
        SYS_SET_TID_ADDRESS => linux_handlers::sys_set_tid_address(a1),
        SYS_CLOCK_GETTIME => linux_handlers::sys_clock_gettime(a1 as i32, a2),
        SYS_GETRANDOM => linux_handlers::sys_getrandom(a1, a2, a3 as u32),

        // Everything else → ENOSYS
        unknown => {
            crate::serial_println!(
                "[KPIO/Linux] Unimplemented syscall #{} (a1={:#x}, a2={:#x})",
                unknown,
                a1,
                a2
            );
            -ENOSYS
        }
    }
}

// ─── LSTAR MSR update ────────────────────────────────────────────────

/// Set the LSTAR MSR to point to `linux_syscall_entry`.
///
/// Call this after `setup_syscall_msr()` to switch from the KPIO stub
/// to the real Linux-compatible entry point.
pub fn install_linux_syscall_entry() {
    const LSTAR_MSR: u32 = 0xC000_0082;

    unsafe {
        use x86_64::registers::model_specific::Msr;
        Msr::new(LSTAR_MSR).write(linux_syscall_entry as u64);
    }

    crate::serial_println!(
        "[KPIO/Linux] LSTAR set to linux_syscall_entry @ {:#x}",
        linux_syscall_entry as u64
    );
}

// ─── User pointer validation ─────────────────────────────────────────

/// Maximum canonical user-space address.
/// x86_64: user addresses must be < 0x0000_8000_0000_0000.
const USER_ADDR_MAX: u64 = 0x0000_8000_0000_0000;

/// Validate that a user-space buffer is within the user address range.
///
/// Returns `Ok(ptr)` if the entire range `[ptr, ptr+len)` is valid
/// user-space memory, or `Err(-EFAULT)` otherwise.
///
/// # Note
///
/// This checks the virtual address range only. It does NOT verify
/// that the pages are actually mapped (that would require walking the
/// page table). The CPU will #PF if they aren't, which is handled
/// by the page fault handler.
pub fn validate_user_ptr(ptr: u64, len: u64) -> Result<u64, i64> {
    if ptr == 0 && len == 0 {
        return Ok(0); // NULL+0 is allowed (e.g. for optional bufs)
    }
    if ptr == 0 {
        return Err(-EFAULT);
    }
    let end = ptr.checked_add(len).ok_or(-EFAULT)?;
    if end > USER_ADDR_MAX {
        return Err(-EFAULT);
    }
    Ok(ptr)
}

/// Read a NUL-terminated string from user space.
///
/// Returns `Err(-EFAULT)` if the pointer is invalid.
/// Maximum length is capped at `max_len` to prevent kernel hangs.
pub fn read_user_string(ptr: u64, max_len: usize) -> Result<alloc::string::String, i64> {
    validate_user_ptr(ptr, 1)?; // At least 1 byte must be valid

    let mut buf = alloc::vec::Vec::with_capacity(256);
    let base = ptr as *const u8;

    for i in 0..max_len {
        let addr = ptr + i as u64;
        if addr >= USER_ADDR_MAX {
            return Err(-EFAULT);
        }
        let byte = unsafe { *base.add(i) };
        if byte == 0 {
            break;
        }
        buf.push(byte);
    }

    alloc::string::String::from_utf8(buf).map_err(|_| -EINVAL)
}

/// Copy bytes from kernel buffer to user-space address.
///
/// Returns `Err(-EFAULT)` if the user pointer is invalid.
pub fn copy_to_user(user_dst: u64, src: &[u8]) -> Result<(), i64> {
    validate_user_ptr(user_dst, src.len() as u64)?;
    unsafe {
        core::ptr::copy_nonoverlapping(src.as_ptr(), user_dst as *mut u8, src.len());
    }
    Ok(())
}

/// Copy bytes from user-space address to kernel buffer.
///
/// Returns `Err(-EFAULT)` if the user pointer is invalid.
pub fn copy_from_user(dst: &mut [u8], user_src: u64) -> Result<(), i64> {
    validate_user_ptr(user_src, dst.len() as u64)?;
    unsafe {
        core::ptr::copy_nonoverlapping(user_src as *const u8, dst.as_mut_ptr(), dst.len());
    }
    Ok(())
}

// ─── Init ────────────────────────────────────────────────────────────

/// Initialise the Linux syscall compatibility layer.
///
/// 1. Init per-CPU data + KERNEL_GS_BASE MSR
/// 2. Install `linux_syscall_entry` in LSTAR
///
/// Must be called after GDT/IDT init and heap init.
pub fn init() {
    percpu::init();
    install_linux_syscall_entry();
    crate::serial_println!("[KPIO/Linux] Syscall compatibility layer initialized");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_user_ptr_null() {
        assert!(validate_user_ptr(0, 0).is_ok());
        assert_eq!(validate_user_ptr(0, 1), Err(-EFAULT));
    }

    #[test]
    fn test_validate_user_ptr_valid() {
        assert!(validate_user_ptr(0x400000, 4096).is_ok());
        assert!(validate_user_ptr(0x7FFF_FFFF_F000, 0x1000).is_ok());
    }

    #[test]
    fn test_validate_user_ptr_kernel_range() {
        // Address in kernel range (>= 0x8000_0000_0000_0000)
        assert_eq!(
            validate_user_ptr(0xFFFF_8000_0000_0000, 1),
            Err(-EFAULT)
        );
    }

    #[test]
    fn test_validate_user_ptr_overflow() {
        assert_eq!(
            validate_user_ptr(u64::MAX, 1),
            Err(-EFAULT)
        );
    }

    #[test]
    fn test_dispatch_unknown_syscall() {
        let result = linux_syscall_dispatch(99999, 0, 0, 0, 0, 0, 0);
        assert_eq!(result, -ENOSYS);
    }

    #[test]
    fn test_dispatch_getuid() {
        assert_eq!(linux_syscall_dispatch(SYS_GETUID, 0, 0, 0, 0, 0, 0), 0);
        assert_eq!(linux_syscall_dispatch(SYS_GETGID, 0, 0, 0, 0, 0, 0), 0);
    }

    #[test]
    fn test_errno_values() {
        assert_eq!(ENOSYS, 38);
        assert_eq!(EBADF, 9);
        assert_eq!(EINVAL, 22);
        assert_eq!(EFAULT, 14);
        assert_eq!(ENOENT, 2);
    }
}
