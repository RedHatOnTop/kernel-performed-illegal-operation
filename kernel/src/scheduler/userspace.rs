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

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

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

// ─── Execve context (shared between dispatch and assembly) ───────────

/// When set to 1, the assembly epilogue in `ring3_syscall_entry` will
/// redirect `sysretq` to the new entry point instead of returning to
/// the original caller.
pub static EXECVE_PENDING: AtomicU64 = AtomicU64::new(0);
pub static EXECVE_NEW_RIP: AtomicU64 = AtomicU64::new(0);
pub static EXECVE_NEW_RSP: AtomicU64 = AtomicU64::new(0);
pub static EXECVE_NEW_RFLAGS: AtomicU64 = AtomicU64::new(0);

// ─── Saved syscall frame for fork() ──────────────────────────────────

/// Saved user-mode RIP (from RCX on SYSCALL entry) — the instruction
/// after the `syscall` in user space.  Read by `sys_fork()` so the
/// child can resume at the same instruction as the parent.
pub static SYSCALL_SAVED_USER_RIP: AtomicU64 = AtomicU64::new(0);
/// Saved user-mode RSP (from per-CPU scratch gs:[8]).
pub static SYSCALL_SAVED_USER_RSP: AtomicU64 = AtomicU64::new(0);
/// Saved user-mode RFLAGS (from R11 on SYSCALL entry).
pub static SYSCALL_SAVED_USER_RFLAGS: AtomicU64 = AtomicU64::new(0);

fn set_execve_context(rip: u64, rsp: u64, rflags: u64) {
    EXECVE_NEW_RIP.store(rip, Ordering::SeqCst);
    EXECVE_NEW_RSP.store(rsp, Ordering::SeqCst);
    EXECVE_NEW_RFLAGS.store(rflags, Ordering::SeqCst);
    // Pending must be last (release fence).
    EXECVE_PENDING.store(1, Ordering::SeqCst);
}

#[allow(dead_code)]
fn clear_execve_context() {
    EXECVE_PENDING.store(0, Ordering::SeqCst);
    EXECVE_NEW_RIP.store(0, Ordering::SeqCst);
    EXECVE_NEW_RSP.store(0, Ordering::SeqCst);
    EXECVE_NEW_RFLAGS.store(0, Ordering::SeqCst);
}

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
        // 5. Save user-mode state so that fork() can build the child's
        //    return frame.  The pushed values are:
        //      [RSP+32] = RCX = user RIP
        //      [RSP+24] = R11 = user RFLAGS
        //    and gs:[8] holds the saved user RSP.
        "lea r8, [rip + {saved_user_rip}]",
        "mov r9, [rsp+32]",
        "mov [r8], r9",
        "lea r8, [rip + {saved_user_rflags}]",
        "mov r9, [rsp+24]",
        "mov [r8], r9",
        "lea r8, [rip + {saved_user_rsp}]",
        "mov r9, gs:[8]",
        "mov [r8], r9",
        // 6. Set up System V calling convention for ring3_syscall_dispatch:
        //    rdi = nr (from rax), rsi = a1 (from rdi), rdx = a2 (from rsi),
        //    rcx = a3 (from rdx)
        //    NOTE: we use the original register values, not the pushed copies.
        "mov rcx, rdx",        // a3 = original rdx
        "mov rdx, rsi",        // a2 = original rsi
        "mov rsi, rdi",        // a1 = original rdi
        "mov rdi, rax",        // nr = syscall number
        // 7. Call Rust dispatcher — returns result in RAX
        "call ring3_syscall_dispatch",

        // ── Execve pending check ─────────────────────────────────
        // If ring3_syscall_dispatch set EXECVE_PENDING, we must NOT
        // return to the old caller.  Instead we redirect sysretq to
        // the new entry point / stack.
        "lea r8, [rip + {execve_pending}]",
        "mov r8, [r8]",
        "test r8, r8",
        "jnz 2f",

        // ── Normal return path ───────────────────────────────────
        // 8. Restore registers
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop r11",              // user RFLAGS
        "pop rcx",              // user RIP
        // 9. Clear in-syscall flag
        "mov qword ptr gs:[32], 0",
        // 10. Restore user RSP from per-CPU scratch
        "mov rsp, gs:[8]",
        // 11. Swap back to user GS and return
        "swapgs",
        "sysretq",

        // ── Execve return path (label 2) ─────────────────────────
        // Clear pending flag
        "2:",
        "lea r8, [rip + {execve_pending}]",
        "mov qword ptr [r8], 0",
        // Load new entry point → RCX (sysretq jumps to RCX)
        "lea r8, [rip + {execve_new_rip}]",
        "mov rcx, [r8]",
        // Load new user RSP → r14 (temp)
        "lea r8, [rip + {execve_new_rsp}]",
        "mov r14, [r8]",
        // Load new RFLAGS → R11 (sysretq loads RFLAGS from R11)
        "lea r8, [rip + {execve_new_rflags}]",
        "mov r11, [r8]",
        // Discard saved frame (5 pushes × 8 bytes = 40)
        "add rsp, 40",
        // Clear in-syscall flag
        "mov qword ptr gs:[32], 0",
        // Set user RSP to the new stack
        "mov rsp, r14",
        // Swap back to user GS
        "swapgs",
        // Zero all GPRs except RCX (new RIP) and R11 (new RFLAGS)
        "xor rax, rax",
        "xor rbx, rbx",
        "xor rdx, rdx",
        "xor rsi, rsi",
        "xor rdi, rdi",
        "xor rbp, rbp",
        "xor r8, r8",
        "xor r9, r9",
        "xor r10, r10",
        "xor r12, r12",
        "xor r13, r13",
        "xor r14, r14",
        "xor r15, r15",
        "sysretq",
        execve_pending = sym EXECVE_PENDING,
        execve_new_rip = sym EXECVE_NEW_RIP,
        execve_new_rsp = sym EXECVE_NEW_RSP,
        execve_new_rflags = sym EXECVE_NEW_RFLAGS,
        saved_user_rip = sym SYSCALL_SAVED_USER_RIP,
        saved_user_rsp = sym SYSCALL_SAVED_USER_RSP,
        saved_user_rflags = sym SYSCALL_SAVED_USER_RFLAGS,
    );
}

/// Rust dispatcher called from the minimal syscall entry point.
///
/// Routes Linux x86_64 syscalls to their handler implementations.
/// Returns value in RAX (negative = -errno).
#[no_mangle]
extern "C" fn ring3_syscall_dispatch(nr: u64, a1: u64, a2: u64, _a3: u64) -> i64 {
    match nr {
        // SYS_READ (0): fd=a1, buf=a2, count=_a3
        0 => dispatch_sys_read(a1 as i32, a2, _a3 as usize),
        // SYS_WRITE (1): fd=a1, buf=a2, count=_a3
        1 => dispatch_sys_write(a1 as i32, a2, _a3 as usize),
        // SYS_OPEN (2): path=a1, flags=a2, mode=_a3
        2 => dispatch_sys_open(a1, a2 as u32),
        // SYS_CLOSE (3): fd=a1
        3 => dispatch_sys_close(a1 as i32),
        // SYS_LSEEK (8): fd=a1, offset=a2, whence=_a3
        8 => dispatch_sys_lseek(a1 as i32, a2 as i64, _a3 as u32),
        // SYS_BRK (12): stub
        12 => 0x0060_0000,
        // SYS_SCHED_YIELD (24)
        24 => { super::yield_now(); 0 }
        // SYS_GETPID (39)
        39 => 1,
        // SYS_FORK (57)
        57 => handle_fork(),
        // SYS_EXECVE (59): pathname=a1, argv=a2, envp=_a3
        59 => handle_execve(a1),
        // SYS_EXIT (60) / SYS_EXIT_GROUP (231)
        60 | 231 => {
            crate::serial_println!("[RING3] SYS_EXIT called with status={}", a1 as i64);
            super::exit_current(a1 as i32);
            0
        }
        // SYS_ARCH_PRCTL (158): stub
        158 => 0,
        // SYS_SOCKET (41) — Phase 13-1
        41 => dispatch_sys_socket(a1, a2, _a3),
        // Unknown
        _ => {
            crate::serial_println!("[RING3] Unknown syscall nr={}", nr);
            -38 // -ENOSYS
        }
    }
}

// ─── Inline syscall implementations using crate-local modules ────────

/// Per-process open file state, keyed by (process, fd).
/// Simple global table: maps fd -> (vfs_ino, offset).
///
/// For simplicity we use a global fd table (single-process model for now).
static RING3_FD_TABLE: spin::Mutex<[Option<FdEntry>; 64]> = spin::Mutex::new([None; 64]);
static RING3_NEXT_FD: core::sync::atomic::AtomicI32 = core::sync::atomic::AtomicI32::new(3);

#[derive(Clone, Copy)]
enum FdKind {
    /// VFS file: inode number + read offset.
    File { ino: u64, offset: usize },
    /// Network socket: handle id from network::socket.
    Socket { handle_id: u32 },
}

#[derive(Clone, Copy)]
struct FdEntry {
    kind: FdKind,
}

fn dispatch_sys_open(path_ptr: u64, _flags: u32) -> i64 {
    // Read null-terminated path from user memory
    if path_ptr == 0 || path_ptr >= 0x0000_8000_0000_0000 {
        return -14; // -EFAULT
    }
    let path = unsafe {
        let ptr = path_ptr as *const u8;
        let mut len = 0usize;
        while len < 256 && *ptr.add(len) != 0 {
            len += 1;
        }
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len))
    };
    if path.is_empty() {
        return -2; // -ENOENT
    }

    // Resolve in the in-memory VFS
    let ino_opt = crate::terminal::fs::with_fs(|fs| fs.resolve(path));
    let ino = match ino_opt {
        Some(i) => i,
        None => return -2, // -ENOENT
    };

    // Allocate fd
    let fd = RING3_NEXT_FD.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    if fd < 0 || fd as usize >= 64 {
        return -24; // -EMFILE
    }
    let mut table = RING3_FD_TABLE.lock();
    table[fd as usize] = Some(FdEntry { kind: FdKind::File { ino, offset: 0 } });
    fd as i64
}

fn dispatch_sys_read(fd: i32, buf_ptr: u64, count: usize) -> i64 {
    if buf_ptr == 0 || buf_ptr >= 0x0000_8000_0000_0000 || count == 0 {
        return 0;
    }
    // fd 0 = stdin (return 0 for now)
    if fd == 0 {
        return 0;
    }

    let mut table = RING3_FD_TABLE.lock();
    let entry = match table.get_mut(fd as usize).and_then(|e| e.as_mut()) {
        Some(e) => e,
        None => return -9, // -EBADF
    };

    match &mut entry.kind {
        FdKind::File { ino, offset } => {
            let ino_val = *ino;
            let off_val = *offset;
            // Read from VFS
            let result = crate::terminal::fs::with_fs(|fs| {
                fs.read_file(ino_val)
            });
            let data = match result {
                Ok(d) => d,
                Err(_) => return -5i64, // -EIO
            };

            let available = data.len().saturating_sub(off_val);
            let to_copy = count.min(available);
            if to_copy == 0 {
                return 0;
            }

            // Copy to user buffer
            let src = &data[off_val..off_val + to_copy];
            unsafe {
                core::ptr::copy_nonoverlapping(src.as_ptr(), buf_ptr as *mut u8, to_copy);
            }
            *offset += to_copy;
            to_copy as i64
        }
        FdKind::Socket { handle_id } => {
            let handle = network::socket::SocketHandle(*handle_id);
            let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, count.min(4096)) };
            match network::socket::recv(handle, buf) {
                Ok(n) => n as i64,
                Err(_) => -11, // -EAGAIN
            }
        }
    }
}

fn dispatch_sys_write(fd: i32, buf_ptr: u64, count: usize) -> i64 {
    if count == 0 || buf_ptr == 0 || buf_ptr >= 0x0000_8000_0000_0000 {
        return 0;
    }
    // fd 1 (stdout) or fd 2 (stderr) → serial
    if fd == 1 || fd == 2 {
        let buf = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, count.min(4096)) };
        if let Ok(s) = core::str::from_utf8(buf) {
            crate::serial_print!("{}", s);
        }
        return count as i64;
    }

    let mut table = RING3_FD_TABLE.lock();
    let entry = match table.get_mut(fd as usize).and_then(|e| e.as_mut()) {
        Some(e) => e,
        None => return -9, // -EBADF
    };

    match &entry.kind {
        FdKind::File { ino, .. } => {
            let data = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, count) };
            let ino_val = *ino;
            drop(table);

            let result = crate::terminal::fs::with_fs(|fs| {
                fs.write_file(ino_val, data)
            });
            match result {
                Ok(_) => count as i64,
                Err(_) => -5, // -EIO
            }
        }
        FdKind::Socket { handle_id } => {
            let handle = network::socket::SocketHandle(*handle_id);
            let data = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, count.min(4096)) };
            drop(table);
            match network::socket::send(handle, data) {
                Ok(n) => n as i64,
                Err(_) => -11, // -EAGAIN
            }
        }
    }
}

fn dispatch_sys_close(fd: i32) -> i64 {
    if fd < 3 {
        return 0; // Don't close stdin/stdout/stderr
    }
    let mut table = RING3_FD_TABLE.lock();
    if let Some(slot) = table.get_mut(fd as usize) {
        if let Some(entry) = slot.take() {
            if let FdKind::Socket { handle_id } = entry.kind {
                let handle = network::socket::SocketHandle(handle_id);
                let _ = network::socket::close(handle);
            }
        }
        0
    } else {
        -9 // -EBADF
    }
}

/// SYS_SOCKET(41): create a network socket.
/// Args: domain (AF_INET=2), socktype (SOCK_STREAM=1, SOCK_DGRAM=2), protocol.
fn dispatch_sys_socket(domain: u64, socktype: u64, _protocol: u64) -> i64 {
    const AF_INET: u64 = 2;
    const SOCK_STREAM: u64 = 1;
    const SOCK_DGRAM: u64 = 2;

    if domain != AF_INET {
        return -97; // -EAFNOSUPPORT
    }

    let st = match socktype & 0xFF {
        SOCK_STREAM => network::socket::SocketType::Stream,
        SOCK_DGRAM => network::socket::SocketType::Datagram,
        _ => return -95, // -EOPNOTSUPP
    };

    let handle = match network::socket::create(st) {
        Ok(h) => h,
        Err(_) => return -24, // -EMFILE
    };

    // Allocate FD
    let fd = RING3_NEXT_FD.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    if fd < 0 || fd as usize >= 64 {
        let _ = network::socket::close(handle);
        return -24; // -EMFILE
    }
    let mut table = RING3_FD_TABLE.lock();
    table[fd as usize] = Some(FdEntry {
        kind: FdKind::Socket { handle_id: handle.0 },
    });
    crate::serial_println!("[Socket] created fd={}", fd);
    fd as i64
}

fn dispatch_sys_lseek(fd: i32, offset: i64, whence: u32) -> i64 {
    let mut table = RING3_FD_TABLE.lock();
    let entry = match table.get_mut(fd as usize).and_then(|e| e.as_mut()) {
        Some(e) => e,
        None => return -9, // -EBADF
    };

    let file_offset = match &mut entry.kind {
        FdKind::File { ino, offset: off } => (ino, off),
        FdKind::Socket { .. } => return -29, // -ESPIPE (illegal seek on socket)
    };

    let file_size = crate::terminal::fs::with_fs(|fs| {
        fs.read_file(*file_offset.0).map(|d| d.len()).unwrap_or(0)
    });

    let new_offset = match whence {
        0 => offset, // SEEK_SET
        1 => *file_offset.1 as i64 + offset, // SEEK_CUR
        2 => file_size as i64 + offset, // SEEK_END
        _ => return -22, // -EINVAL
    };

    if new_offset < 0 {
        return -22; // -EINVAL
    }
    *file_offset.1 = new_offset as usize;
    new_offset
}

// ─── Fork handler ────────────────────────────────────────────────────

/// Minimal `fork` implementation for the binary-crate syscall path.
///
/// Reads the parent's saved user-mode state (RIP, RSP, RFLAGS) from
/// the statics populated by the assembly prologue, clones the page
/// table with Copy-on-Write, and spawns a child task whose trampoline
/// enters Ring 3 at the instruction after the parent's `syscall` with
/// RAX = 0.
fn handle_fork() -> i64 {
    use crate::memory::user_page_table;

    // 1. Read parent's saved user-mode state.
    let user_rip = SYSCALL_SAVED_USER_RIP.load(Ordering::SeqCst);
    let user_rsp = SYSCALL_SAVED_USER_RSP.load(Ordering::SeqCst);
    let user_rflags = SYSCALL_SAVED_USER_RFLAGS.load(Ordering::SeqCst);

    if user_rip == 0 || user_rsp == 0 {
        crate::serial_println!("[FORK] EFAULT: saved user frame is zero");
        return -14; // -EFAULT
    }

    // 2. Get current CR3.
    let (cr3_frame, _) = x86_64::registers::control::Cr3::read();
    let parent_cr3 = cr3_frame.start_address().as_u64();

    crate::serial_println!(
        "[FORK] parent user_rip={:#x} user_rsp={:#x} rflags={:#x} cr3={:#x}",
        user_rip, user_rsp, user_rflags, parent_cr3,
    );

    // 3. Clone page table with CoW.
    let child_cr3 = match user_page_table::clone_user_page_table(parent_cr3) {
        Ok(cr3) => cr3,
        Err(e) => {
            crate::serial_println!("[FORK] page table clone failed: {}", e);
            return -12; // -ENOMEM
        }
    };

    // 4. Allocate kernel stack for the child.
    let kernel_stack_size: usize = 32 * 1024;
    let mut kernel_stack = alloc::vec::Vec::with_capacity(kernel_stack_size);
    kernel_stack.resize(kernel_stack_size, 0u8);
    let kernel_stack_bottom = kernel_stack.as_ptr() as u64;
    let kernel_stack_top = (kernel_stack_bottom + kernel_stack_size as u64) & !0xF;

    // 5. Assign a PID for the child.
    //    Simple atomic counter — sufficient for testing.
    static NEXT_FORK_PID: AtomicU64 = AtomicU64::new(50);
    let child_pid = NEXT_FORK_PID.fetch_add(1, Ordering::Relaxed);

    // 6. Create the child task with fork-specific trampoline.
    let child_task = super::task::Task::new_forked_process(
        "fork-child",
        child_cr3,
        user_rip,
        user_rsp,
        user_rflags,
        kernel_stack_top,
        kernel_stack,
        child_pid,
    );

    super::spawn(child_task);

    crate::serial_println!(
        "[FORK] parent={} child={} (CoW CR3={:#x})",
        "current", child_pid, child_cr3,
    );

    child_pid as i64
}

// ─── Execve handler ─────────────────────────────────────────────────

/// Minimal `execve` implementation for the binary-crate syscall path.
///
/// Reads the pathname from user memory, loads the ELF from VFS,
/// maps PT_LOAD segments into the current user address space, sets
/// up a fresh user stack, and sets the EXECVE_PENDING context so that
/// the assembly epilogue in `ring3_syscall_entry` redirects `sysretq`
/// to the new entry point.
fn handle_execve(pathname_ptr: u64) -> i64 {
    use x86_64::structures::paging::PageTableFlags;

    // 1. Read pathname from user memory
    if pathname_ptr == 0 || pathname_ptr >= 0x0000_8000_0000_0000 {
        crate::serial_println!("[EXECVE] EFAULT: bad pathname pointer {:#x}", pathname_ptr);
        return -14; // -EFAULT
    }
    let pathname = unsafe {
        let mut ptr = pathname_ptr as *const u8;
        let mut bytes = alloc::vec::Vec::new();
        for _ in 0..256 {
            let b = *ptr;
            if b == 0 {
                break;
            }
            bytes.push(b);
            ptr = ptr.add(1);
        }
        match alloc::string::String::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => return -22, // -EINVAL
        }
    };
    crate::serial_println!("[EXECVE] execve({:?})", pathname);

    // 2. Read ELF binary from VFS
    let elf_data = match crate::vfs::read_all(&pathname) {
        Ok(data) => data,
        Err(e) => {
            crate::serial_println!("[EXECVE] File not found: {} ({:?})", pathname, e);
            return -2; // -ENOENT
        }
    };
    crate::serial_println!("[EXECVE] Loaded {} bytes from VFS", elf_data.len());

    // 3. Minimal ELF64 header parse
    if elf_data.len() < 64 || elf_data[0..4] != [0x7F, b'E', b'L', b'F'] {
        crate::serial_println!("[EXECVE] ENOEXEC: invalid ELF magic");
        return -8; // -ENOEXEC
    }
    let entry_point = u64::from_le_bytes(elf_data[24..32].try_into().unwrap());
    let phoff = u64::from_le_bytes(elf_data[32..40].try_into().unwrap()) as usize;
    let phentsize = u16::from_le_bytes(elf_data[54..56].try_into().unwrap()) as usize;
    let phnum = u16::from_le_bytes(elf_data[56..58].try_into().unwrap()) as usize;
    crate::serial_println!(
        "[EXECVE] ELF: entry={:#x}, phoff={}, phentsize={}, phnum={}",
        entry_point, phoff, phentsize, phnum,
    );

    // 4. Get current CR3 (we're running on the caller's page table)
    let (cr3_frame, _) = x86_64::registers::control::Cr3::read();
    let cr3 = cr3_frame.start_address().as_u64();
    crate::serial_println!("[EXECVE] CR3={:#x}", cr3);

    // 5. Map PT_LOAD segments into the user page table.
    //    Instead of destroy+recreate (which hangs due to frame-pool
    //    lock contention under SFMASK IF=0), we reuse existing pages
    //    when already mapped and only allocate new ones as needed.
    let code_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
    let writable_flags = code_flags | PageTableFlags::WRITABLE;

    for i in 0..phnum {
        let off = phoff + i * phentsize;
        if off + 56 > elf_data.len() {
            break;
        }
        let p_type = u32::from_le_bytes(elf_data[off..off + 4].try_into().unwrap());
        if p_type != 1 {
            continue; // only PT_LOAD
        }
        let p_flags = u32::from_le_bytes(elf_data[off + 4..off + 8].try_into().unwrap());
        let p_offset = u64::from_le_bytes(elf_data[off + 8..off + 16].try_into().unwrap()) as usize;
        let p_vaddr = u64::from_le_bytes(elf_data[off + 16..off + 24].try_into().unwrap());
        let p_filesz = u64::from_le_bytes(elf_data[off + 32..off + 40].try_into().unwrap()) as usize;
        let p_memsz = u64::from_le_bytes(elf_data[off + 40..off + 48].try_into().unwrap()) as usize;

        let flags = if p_flags & 2 != 0 { writable_flags } else { code_flags };

        // Map each page in the segment's virtual range
        let page_start = p_vaddr & !0xFFF;
        let page_end = (p_vaddr + p_memsz as u64 + 0xFFF) & !0xFFF;
        let mut addr = page_start;
        while addr < page_end {
            // Try to reuse an existing mapping; allocate a new page only
            // when the virtual address is not already mapped.
            let phys = if let Some((existing_phys, _)) =
                crate::memory::user_page_table::read_pte(cr3, addr)
            {
                // Zero the existing page before writing new data
                let offset = crate::memory::user_page_table::get_phys_offset();
                let page_virt = offset + existing_phys;
                unsafe {
                    core::ptr::write_bytes(page_virt as *mut u8, 0, 4096);
                }
                existing_phys
            } else {
                match crate::memory::user_page_table::map_user_page(cr3, addr, flags) {
                    Ok(p) => p,
                    Err(e) => {
                        crate::serial_println!(
                            "[EXECVE] map_user_page({:#x}) failed: {}", addr, e,
                        );
                        return -12; // -ENOMEM
                    }
                }
            };

            // Calculate which slice of file data to copy into this page.
            // The segment starts at file offset p_offset, virtual address
            // p_vaddr.  For the page at `addr`, the corresponding segment
            // byte index is `addr - p_vaddr` (if addr >= p_vaddr).
            let seg_byte = if addr >= p_vaddr {
                (addr - p_vaddr) as usize
            } else {
                0
            };
            let in_page = if p_vaddr > addr {
                (p_vaddr - addr) as usize
            } else {
                0
            };
            let copy_len = p_filesz.saturating_sub(seg_byte).min(0x1000 - in_page);

            if copy_len > 0 {
                let src_start = p_offset + seg_byte;
                if src_start + copy_len <= elf_data.len() {
                    // SAFETY: phys page was zeroed above (or by map_user_page);
                    // we write the ELF file data into it.
                    unsafe {
                        crate::memory::user_page_table::write_to_phys(
                            phys,
                            in_page,
                            &elf_data[src_start..src_start + copy_len],
                        );
                    }
                }
            }

            addr += 0x1000;
        }
        crate::serial_println!(
            "[EXECVE] PT_LOAD: vaddr={:#x} memsz={:#x} filesz={:#x} flags={:#x}",
            p_vaddr, p_memsz, p_filesz, p_flags,
        );
    }

    // 7. Setup a fresh user stack (one page below 0x800000).
    //    Reuse the existing mapping if present.
    let stack_top: u64 = 0x80_0000;
    let stack_base = stack_top - 0x1000;
    if let Some((stack_phys, _)) = crate::memory::user_page_table::read_pte(cr3, stack_base) {
        // Zero the existing stack page
        let offset = crate::memory::user_page_table::get_phys_offset();
        let stack_virt = offset + stack_phys;
        unsafe {
            core::ptr::write_bytes(stack_virt as *mut u8, 0, 4096);
        }
    } else if let Err(e) =
        crate::memory::user_page_table::map_user_page(cr3, stack_base, writable_flags)
    {
        crate::serial_println!("[EXECVE] stack mapping failed: {}", e);
        return -12; // -ENOMEM
    }

    // 8. Tell the assembly epilogue to redirect sysretq
    set_execve_context(entry_point, stack_top, 0x202);
    crate::serial_println!(
        "[EXECVE] Success: new_rip={:#x} new_rsp={:#x} (CR3={:#x})",
        entry_point, stack_top, cr3,
    );
    0
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
        "[RING3] User-space init: KERNEL_GS_BASE + STAR/LSTAR/SFMASK configured"
    );
    crate::serial_println!(
        "[RING3]   LSTAR = {:#x} (ring3_syscall_entry)",
        ring3_syscall_entry as u64
    );
}
