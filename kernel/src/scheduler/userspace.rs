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
        //    rcx = a3 (from rdx), r8 = a4 (from r10), r9 = a5 (from r8)
        //    NOTE: we use the original register values, not the pushed copies.
        //    Order matters: each mov must read its source before it's overwritten.
        "mov r9, r8",           // a5 = original r8 (must precede r8 overwrite)
        "mov r8, r10",          // a4 = original r10
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
extern "C" fn ring3_syscall_dispatch(nr: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> i64 {
    match nr {
        // SYS_READ (0): fd=a1, buf=a2, count=a3
        0 => dispatch_sys_read(a1 as i32, a2, a3 as usize),
        // SYS_WRITE (1): fd=a1, buf=a2, count=a3
        1 => dispatch_sys_write(a1 as i32, a2, a3 as usize),
        // SYS_OPEN (2): path=a1, flags=a2, mode=a3
        2 => dispatch_sys_open(a1, a2 as u32),
        // SYS_CLOSE (3): fd=a1
        3 => dispatch_sys_close(a1 as i32),
        // SYS_LSEEK (8): fd=a1, offset=a2, whence=a3
        8 => dispatch_sys_lseek(a1 as i32, a2 as i64, a3 as u32),
        // SYS_BRK (12): stub
        12 => 0x0060_0000,
        // SYS_SCHED_YIELD (24)
        24 => { super::yield_now(); 0 }
        // SYS_GETPID (39) → returns tgid (POSIX semantics)
        39 => dispatch_sys_getpid(),
        // SYS_SOCKET (41) — Phase 13-1
        41 => dispatch_sys_socket(a1, a2, a3),
        // SYS_CONNECT (42)
        42 => dispatch_sys_connect(a1 as i32, a2, a3 as u32),
        // SYS_ACCEPT (43)
        43 => dispatch_sys_accept(a1 as i32, a2, a3),
        // SYS_SENDTO (44): fd, buf, len, flags, dest_addr, addrlen
        44 => dispatch_sys_sendto(a1 as i32, a2, a3 as usize, a4 as u32, a5),
        // SYS_RECVFROM (45): fd, buf, len, flags, src_addr, addrlen
        45 => dispatch_sys_recvfrom(a1 as i32, a2, a3 as usize, a4 as u32, a5),
        // SYS_SHUTDOWN (48)
        48 => dispatch_sys_shutdown(a1 as i32, a2 as u32),
        // SYS_BIND (49)
        49 => dispatch_sys_bind(a1 as i32, a2, a3 as u32),
        // SYS_LISTEN (50)
        50 => dispatch_sys_listen(a1 as i32, a2 as u32),
        // SYS_GETSOCKNAME (51)
        51 => dispatch_sys_getsockname(a1 as i32, a2, a3),
        // SYS_GETPEERNAME (52)
        52 => dispatch_sys_getpeername(a1 as i32, a2, a3),
        // SYS_SETSOCKOPT (54)
        54 => dispatch_sys_setsockopt(a1 as i32, a2 as u32, a3 as u32, a4, a5 as u32),
        // SYS_GETSOCKOPT (55)
        55 => dispatch_sys_getsockopt(a1 as i32, a2 as u32, a3 as u32, a4, a5),
        // SYS_CLONE (56): flags=a1, child_stack=a2, ptid=a3, ctid=a4, tls=a5
        56 => dispatch_sys_clone(a1, a2, a3, a4, a5),
        // SYS_FORK (57)
        57 => handle_fork(),
        // SYS_EXECVE (59): pathname=a1, argv=a2, envp=a3
        59 => handle_execve(a1),
        // SYS_EXIT (60): thread-aware exit
        60 => dispatch_sys_exit(a1 as i32),
        // SYS_EXIT_GROUP (231): kills entire thread group
        231 => {
            crate::serial_println!("[RING3] SYS_EXIT_GROUP called with status={}", a1 as i64);
            super::exit_current(a1 as i32);
            0
        }
        // SYS_ARCH_PRCTL (158): stub
        158 => 0,
        // SYS_GETTID (186)
        186 => dispatch_sys_gettid(),
        // SYS_FUTEX (202): addr=a1, op=a2, val=a3
        202 => dispatch_sys_futex(a1, a2 as i32, a3 as u32),
        // SYS_SET_TID_ADDRESS (218)
        218 => dispatch_sys_set_tid_address(a1),
        // SYS_ACCEPT4 (288)
        288 => dispatch_sys_accept(a1 as i32, a2, a3),
        // SYS_EPOLL_WAIT (232)
        232 => dispatch_sys_epoll_wait(a1 as i32, a2, a3 as i32, a4 as i32),
        // SYS_EPOLL_CTL (233)
        233 => dispatch_sys_epoll_ctl(a1 as i32, a2 as i32, a3 as i32, a4),
        // SYS_EPOLL_CREATE1 (291)
        291 => dispatch_sys_epoll_create1(a1 as u32),
        // SYS_PIPE2 (293)
        293 => dispatch_sys_pipe2(a1, a2 as u32),
        // SYS_PIPE (22) — alias for pipe2 with flags=0
        22 => dispatch_sys_pipe2(a1, 0),
        // Unknown
        _ => {
            crate::serial_println!("[RING3] Unknown syscall nr={}", nr);
            -38 // -ENOSYS
        }
    }
}

// ─── Threading syscall dispatch wrappers ─────────────────────────────
// These avoid `crate::syscall::` paths which don't exist in the binary crate.

fn dispatch_sys_getpid() -> i64 {
    let pid = super::current_process_pid();
    if pid == 0 { 1 } else { pid as i64 }
}

fn dispatch_sys_gettid() -> i64 {
    super::current_thread_tid() as i64
}

fn dispatch_sys_set_tid_address(tidptr: u64) -> i64 {
    super::set_current_clear_child_tid(tidptr);
    dispatch_sys_gettid()
}

fn dispatch_sys_futex(addr: u64, op: i32, val: u32) -> i64 {
    crate::sync::futex::sys_futex(addr, op, val)
}

fn dispatch_sys_exit(status: i32) -> i64 {
    use crate::process::table::PROCESS_TABLE;

    let current_tid = super::current_thread_tid();
    let process_pid = super::current_process_pid();
    if process_pid != 0 {
        let pid = crate::process::ProcessId(process_pid);
        let is_thread_leader = PROCESS_TABLE.with_process_mut(pid, |proc| {
            proc.tgid.0 == current_tid
        }).unwrap_or(true);

        if !is_thread_leader {
            crate::serial_println!("[IPC] Thread exit tid={} status={}", current_tid, status);

            // Handle clear_child_tid: write 0 and futex_wake
            let ctid_addr = super::get_current_clear_child_tid();
            if ctid_addr != 0 {
                // SAFETY: ctid_addr was set by clone(CLONE_CHILD_CLEARTID),
                // pointing to a valid user-space address in the shared address space.
                unsafe { core::ptr::write_volatile(ctid_addr as *mut u32, 0); }
                crate::sync::futex::sys_futex(ctid_addr, 1, 1); // FUTEX_WAKE
            }

            // Remove thread from process table
            PROCESS_TABLE.with_process_mut(pid, |proc| {
                proc.threads.retain(|t| t.tid.0 != current_tid);
            });

            super::exit_current(status);
            return 0; // unreachable
        }
    }

    crate::serial_println!("[RING3] SYS_EXIT called with status={}", status);
    super::exit_current(status);
    0
}

fn dispatch_sys_clone(flags: u64, child_stack: u64, ptid_ptr: u64, ctid_ptr: u64, tls: u64) -> i64 {
    use crate::process::table::{PROCESS_TABLE, ThreadId, Thread, ProcessState};
    use crate::process::context::ProcessContext;
    use core::sync::atomic::Ordering;

    const CLONE_THREAD: u64 = 0x10000;
    const CLONE_SETTLS: u64 = 0x80000;
    const CLONE_PARENT_SETTID: u64 = 0x100000;
    const CLONE_CHILD_CLEARTID: u64 = 0x200000;

    if flags & CLONE_THREAD == 0 {
        return handle_fork();
    }

    let process_pid = super::current_process_pid();
    if process_pid == 0 {
        return -38; // -ENOSYS
    }
    let pid = crate::process::ProcessId(process_pid);

    let parent_info = PROCESS_TABLE.with_process_mut(pid, |proc| {
        let cr3 = proc.linux_memory.as_ref().map(|m| m.cr3).unwrap_or(proc.page_table_root);
        let tgid = proc.tgid;
        (cr3, tgid)
    });

    let (parent_cr3, tgid) = match parent_info {
        Some(info) => info,
        None => return -3, // -ESRCH
    };

    if parent_cr3 == 0 {
        return -38; // -ENOSYS
    }

    let child_tid = ThreadId::new();

    let user_rip = SYSCALL_SAVED_USER_RIP.load(Ordering::SeqCst);
    let user_rflags = SYSCALL_SAVED_USER_RFLAGS.load(Ordering::SeqCst);
    let child_rsp = if child_stack != 0 {
        child_stack
    } else {
        SYSCALL_SAVED_USER_RSP.load(Ordering::SeqCst)
    };

    if user_rip == 0 || child_rsp == 0 {
        return -22; // -EINVAL
    }

    let tls_base = if flags & CLONE_SETTLS != 0 { tls } else { 0 };
    let clear_child_tid_ptr = if flags & CLONE_CHILD_CLEARTID != 0 { ctid_ptr } else { 0 };

    let thread = Thread {
        tid: child_tid,
        state: ProcessState::Ready,
        context: ProcessContext::default(),
        kernel_stack: 0,
        kernel_stack_size: 0,
        user_stack: child_rsp,
        user_stack_size: 0,
        tls: tls_base,
        clear_child_tid: clear_child_tid_ptr,
    };
    PROCESS_TABLE.with_process_mut(pid, |proc| {
        proc.threads.push(thread);
    });

    if flags & CLONE_PARENT_SETTID != 0 && ptid_ptr != 0 && ptid_ptr < 0x0000_8000_0000_0000 {
        // SAFETY: ptid_ptr validated as user-space address above.
        unsafe { core::ptr::write(ptid_ptr as *mut u32, child_tid.0 as u32); }
    }

    let thread_name = alloc::format!("thread-{}", child_tid.0);
    let child_task = super::task::Task::new_thread(
        &thread_name,
        parent_cr3,
        user_rip,
        child_rsp,
        user_rflags,
        tls_base,
        pid.0,
        child_tid.0,
        clear_child_tid_ptr,
    );

    super::spawn(child_task);

    crate::serial_println!("[IPC] Clone thread tid={} tgid={}", child_tid.0, tgid.0);

    child_tid.0 as i64
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
    /// Epoll instance: id into sync::epoll table.
    Epoll { epoll_id: u64 },
    /// Pipe end: id into the pipe buffer table.
    Pipe { pipe_id: u64, is_write_end: bool },
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
        FdKind::Epoll { .. } => -22, // -EINVAL: cannot read from epoll fd
        FdKind::Pipe { pipe_id, is_write_end } => {
            if *is_write_end {
                return -9; // -EBADF: cannot read from write end
            }
            let pid = *pipe_id;
            drop(table);
            let dst = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, count.min(4096)) };
            ring3_pipe_read(pid, dst) as i64
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
        FdKind::Epoll { .. } => -22, // -EINVAL: cannot write to epoll fd
        FdKind::Pipe { pipe_id, is_write_end } => {
            if !*is_write_end {
                return -9; // -EBADF: cannot write to read end
            }
            let pid = *pipe_id;
            let data = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, count.min(4096)) };
            drop(table);
            ring3_pipe_write(pid, data) as i64
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
            match entry.kind {
                FdKind::Socket { handle_id } => {
                    let handle = network::socket::SocketHandle(handle_id);
                    let _ = network::socket::close(handle);
                }
                FdKind::Epoll { epoll_id } => {
                    crate::sync::epoll::epoll_destroy(epoll_id);
                }
                FdKind::Pipe { pipe_id, is_write_end } => {
                    ring3_pipe_close(pipe_id, is_write_end);
                }
                FdKind::File { .. } => {}
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

// ─── BSD socket syscall helpers ──────────────────────────────────────

/// Read a `struct sockaddr_in` (16 bytes) from a user-space pointer.
///
/// Returns `(ip4_addr, port)` in host byte order, or `-EFAULT` on bad ptr.
///
/// Layout: { u16 sin_family; u16 sin_port[NBO]; u32 sin_addr[NBO]; u8 sin_zero[8]; }
fn read_sockaddr_in(addr_ptr: u64, addrlen: u32) -> Result<network::SocketAddr, i64> {
    if addr_ptr == 0 || addr_ptr >= 0x0000_8000_0000_0000 || addrlen < 16 {
        return Err(-14); // -EFAULT
    }
    // SAFETY: addr_ptr has been validated to be in the user-space range.
    // The kernel maps all user pages before they are accessed.
    let bytes: [u8; 16] = unsafe {
        core::ptr::read(addr_ptr as *const [u8; 16])
    };
    let family = u16::from_ne_bytes([bytes[0], bytes[1]]);
    if family != 2 {
        // AF_INET = 2
        return Err(-97); // -EAFNOSUPPORT
    }
    let port = u16::from_be_bytes([bytes[2], bytes[3]]);
    let ip = network::Ipv4Addr::new(bytes[4], bytes[5], bytes[6], bytes[7]);
    Ok(network::SocketAddr::new(network::IpAddr::V4(ip), port))
}

/// Write a `struct sockaddr_in` to a user-space pointer.
fn write_sockaddr_in(addr_ptr: u64, addrlen_ptr: u64, addr: &network::SocketAddr) {
    if addr_ptr == 0 || addr_ptr >= 0x0000_8000_0000_0000 {
        return;
    }
    let mut buf = [0u8; 16];
    buf[0] = 2; buf[1] = 0; // AF_INET = 2 (little-endian u16)
    let port_be = addr.port.to_be_bytes();
    buf[2] = port_be[0]; buf[3] = port_be[1];
    if let network::IpAddr::V4(ipv4) = &addr.ip {
        let octets = ipv4.octets();
        buf[4] = octets[0]; buf[5] = octets[1];
        buf[6] = octets[2]; buf[7] = octets[3];
    }
    // SAFETY: addr_ptr validated above; writing 16 bytes into mapped user page.
    unsafe {
        core::ptr::write(addr_ptr as *mut [u8; 16], buf);
    }
    if addrlen_ptr != 0 && addrlen_ptr < 0x0000_8000_0000_0000 {
        // SAFETY: length pointer is in user-space range.
        unsafe {
            core::ptr::write(addrlen_ptr as *mut u32, 16u32);
        }
    }
}

/// Look up a socket handle from an FD number.
/// Returns the `SocketHandle` or `-EBADF`/`-ENOTSOCK`.
fn fd_to_socket_handle(fd: i32) -> Result<network::socket::SocketHandle, i64> {
    let table = RING3_FD_TABLE.lock();
    let entry = table.get(fd as usize)
        .and_then(|e| e.as_ref())
        .ok_or(-9i64)?; // -EBADF
    match entry.kind {
        FdKind::Socket { handle_id } => Ok(network::socket::SocketHandle(handle_id)),
        _ => Err(-88), // -ENOTSOCK
    }
}

/// SYS_BIND(49): bind(fd, addr, addrlen)
fn dispatch_sys_bind(fd: i32, addr_ptr: u64, addrlen: u32) -> i64 {
    let handle = match fd_to_socket_handle(fd) {
        Ok(h) => h,
        Err(e) => return e,
    };
    let addr = match read_sockaddr_in(addr_ptr, addrlen) {
        Ok(a) => a,
        Err(e) => return e,
    };
    match network::socket::bind(handle, addr) {
        Ok(()) => {
            crate::serial_println!("[IPC] Socket bind fd={} port={}", fd, addr.port);
            0
        }
        Err(_) => -98, // -EADDRINUSE
    }
}

/// SYS_LISTEN(50): listen(fd, backlog)
fn dispatch_sys_listen(fd: i32, backlog: u32) -> i64 {
    let handle = match fd_to_socket_handle(fd) {
        Ok(h) => h,
        Err(e) => return e,
    };
    match network::socket::listen(handle, backlog) {
        Ok(()) => {
            crate::serial_println!("[IPC] Socket listen fd={}", fd);
            0
        }
        Err(_) => -22, // -EINVAL
    }
}

/// SYS_ACCEPT(43) / SYS_ACCEPT4(288): accept(fd, addr, addrlen)
fn dispatch_sys_accept(fd: i32, addr_ptr: u64, addrlen_ptr: u64) -> i64 {
    let handle = match fd_to_socket_handle(fd) {
        Ok(h) => h,
        Err(e) => return e,
    };
    let peer_handle = match network::socket::accept(handle) {
        Ok(h) => h,
        Err(network::NetworkError::WouldBlock) => return -11, // -EAGAIN
        Err(_) => return -22, // -EINVAL
    };

    // Allocate a new FD for the accepted connection.
    let new_fd = RING3_NEXT_FD.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    if new_fd < 0 || new_fd as usize >= 64 {
        let _ = network::socket::close(peer_handle);
        return -24; // -EMFILE
    }
    {
        let mut table = RING3_FD_TABLE.lock();
        table[new_fd as usize] = Some(FdEntry {
            kind: FdKind::Socket { handle_id: peer_handle.0 },
        });
    }

    // Write remote address if requested.
    if addr_ptr != 0 {
        if let Ok(peer_addr) = network::socket::getpeername(peer_handle) {
            write_sockaddr_in(addr_ptr, addrlen_ptr, &peer_addr);
        }
    }

    crate::serial_println!("[IPC] Socket accept fd={} -> new_fd={}", fd, new_fd);
    new_fd as i64
}

/// SYS_CONNECT(42): connect(fd, addr, addrlen)
fn dispatch_sys_connect(fd: i32, addr_ptr: u64, addrlen: u32) -> i64 {
    let handle = match fd_to_socket_handle(fd) {
        Ok(h) => h,
        Err(e) => return e,
    };
    let addr = match read_sockaddr_in(addr_ptr, addrlen) {
        Ok(a) => a,
        Err(e) => return e,
    };
    match network::socket::connect(handle, addr) {
        Ok(()) => {
            crate::serial_println!("[IPC] Socket connect fd={} port={}", fd, addr.port);
            0
        }
        Err(_) => -111, // -ECONNREFUSED
    }
}

/// SYS_SENDTO(44): sendto(fd, buf, len, flags, dest_addr, addrlen)
///
/// For connected TCP, `dest_addr` is ignored (uses peer from connect).
/// For UDP with `dest_addr != 0`, routes the datagram to a bound socket.
fn dispatch_sys_sendto(fd: i32, buf_ptr: u64, len: usize, _flags: u32, dest_addr_ptr: u64) -> i64 {
    if buf_ptr == 0 || buf_ptr >= 0x0000_8000_0000_0000 || len == 0 {
        return 0;
    }
    let handle = match fd_to_socket_handle(fd) {
        Ok(h) => h,
        Err(e) => return e,
    };
    // SAFETY: buf_ptr is in valid user-space range and len is bounded.
    let data = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, len.min(4096)) };

    // Check if this is a UDP sendto with a destination address.
    if dest_addr_ptr != 0 {
        if let Ok(sock_type) = network::socket::get_type(handle) {
            if sock_type == network::socket::SocketType::Datagram {
                if let Ok(dest) = read_sockaddr_in(dest_addr_ptr, 16) {
                    return match network::socket::sendto_dgram(handle, data, dest) {
                        Ok(n) => n as i64,
                        Err(_) => -11, // -EAGAIN
                    };
                }
            }
        }
    }

    match network::socket::send(handle, data) {
        Ok(n) => n as i64,
        Err(network::NetworkError::WouldBlock) => -11, // -EAGAIN
        Err(_) => -104, // -ECONNRESET
    }
}

/// SYS_RECVFROM(45): recvfrom(fd, buf, len, flags, src_addr, addrlen)
fn dispatch_sys_recvfrom(fd: i32, buf_ptr: u64, len: usize, _flags: u32, _src_addr_ptr: u64) -> i64 {
    if buf_ptr == 0 || buf_ptr >= 0x0000_8000_0000_0000 || len == 0 {
        return 0;
    }
    let handle = match fd_to_socket_handle(fd) {
        Ok(h) => h,
        Err(e) => return e,
    };
    // SAFETY: buf_ptr is in valid user-space range.
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, len.min(4096)) };
    match network::socket::recv(handle, buf) {
        Ok(n) => n as i64,
        Err(network::NetworkError::WouldBlock) => -11, // -EAGAIN
        Err(_) => -104, // -ECONNRESET
    }
}

/// SYS_SHUTDOWN(48): shutdown(fd, how)
fn dispatch_sys_shutdown(fd: i32, how: u32) -> i64 {
    let handle = match fd_to_socket_handle(fd) {
        Ok(h) => h,
        Err(e) => return e,
    };
    match network::socket::shutdown(handle, how) {
        Ok(()) => 0,
        Err(_) => -107, // -ENOTCONN
    }
}

/// SYS_GETSOCKNAME(51): getsockname(fd, addr, addrlen)
fn dispatch_sys_getsockname(fd: i32, addr_ptr: u64, addrlen_ptr: u64) -> i64 {
    let handle = match fd_to_socket_handle(fd) {
        Ok(h) => h,
        Err(e) => return e,
    };
    match network::socket::getsockname(handle) {
        Ok(addr) => {
            write_sockaddr_in(addr_ptr, addrlen_ptr, &addr);
            0
        }
        Err(_) => -22, // -EINVAL
    }
}

/// SYS_GETPEERNAME(52): getpeername(fd, addr, addrlen)
fn dispatch_sys_getpeername(fd: i32, addr_ptr: u64, addrlen_ptr: u64) -> i64 {
    let handle = match fd_to_socket_handle(fd) {
        Ok(h) => h,
        Err(e) => return e,
    };
    match network::socket::getpeername(handle) {
        Ok(addr) => {
            write_sockaddr_in(addr_ptr, addrlen_ptr, &addr);
            0
        }
        Err(_) => -107, // -ENOTCONN
    }
}

/// SYS_SETSOCKOPT(54): setsockopt(fd, level, optname, optval, optlen)
fn dispatch_sys_setsockopt(fd: i32, level: u32, optname: u32, optval_ptr: u64, optlen: u32) -> i64 {
    let handle = match fd_to_socket_handle(fd) {
        Ok(h) => h,
        Err(e) => return e,
    };
    // Read the option value (u32) from user-space.
    let value: u32 = if optval_ptr != 0 && optval_ptr < 0x0000_8000_0000_0000 && optlen >= 4 {
        // SAFETY: optval_ptr is in valid user-space range.
        unsafe { core::ptr::read(optval_ptr as *const u32) }
    } else {
        0
    };
    match network::socket::setsockopt(handle, level, optname, value) {
        Ok(()) => 0,
        Err(_) => -92, // -ENOPROTOOPT
    }
}

/// SYS_GETSOCKOPT(55): getsockopt(fd, level, optname, optval, optlen)
fn dispatch_sys_getsockopt(fd: i32, level: u32, optname: u32, optval_ptr: u64, optlen_ptr: u64) -> i64 {
    let handle = match fd_to_socket_handle(fd) {
        Ok(h) => h,
        Err(e) => return e,
    };
    match network::socket::getsockopt(handle, level, optname) {
        Ok(val) => {
            if optval_ptr != 0 && optval_ptr < 0x0000_8000_0000_0000 {
                // SAFETY: optval_ptr is in valid user-space range.
                unsafe { core::ptr::write(optval_ptr as *mut u32, val); }
            }
            if optlen_ptr != 0 && optlen_ptr < 0x0000_8000_0000_0000 {
                // SAFETY: optlen_ptr is in valid user-space range.
                unsafe { core::ptr::write(optlen_ptr as *mut u32, 4u32); }
            }
            0
        }
        Err(_) => -92, // -ENOPROTOOPT
    }
}

fn dispatch_sys_lseek(fd: i32, offset: i64, whence: u32) -> i64 {
    let mut table = RING3_FD_TABLE.lock();
    let entry = match table.get_mut(fd as usize).and_then(|e| e.as_mut()) {
        Some(e) => e,
        None => return -9, // -EBADF
    };

    let file_offset = match &mut entry.kind {
        FdKind::File { ino, offset: off } => (ino, off),
        FdKind::Socket { .. } | FdKind::Epoll { .. } | FdKind::Pipe { .. } => {
            return -29 // -ESPIPE (illegal seek)
        }
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

// ─── Pipe buffer infrastructure (Ring 3 path) ────────────────────────

/// Ring 3 pipe buffer: simple circular buffer shared between read/write ends.
const PIPE_BUF_SIZE: usize = 4096;

struct Ring3PipeBuffer {
    data: alloc::vec::Vec<u8>,
    write_pos: usize,
    read_pos: usize,
    count: usize,
    write_closed: bool,
    read_closed: bool,
}

impl Ring3PipeBuffer {
    fn new() -> Self {
        Self {
            data: alloc::vec![0u8; PIPE_BUF_SIZE],
            write_pos: 0,
            read_pos: 0,
            count: 0,
            write_closed: false,
            read_closed: false,
        }
    }

    fn write(&mut self, src: &[u8]) -> usize {
        let space = PIPE_BUF_SIZE - self.count;
        let n = src.len().min(space);
        for i in 0..n {
            self.data[self.write_pos] = src[i];
            self.write_pos = (self.write_pos + 1) % PIPE_BUF_SIZE;
        }
        self.count += n;
        n
    }

    fn read(&mut self, dst: &mut [u8]) -> usize {
        let n = dst.len().min(self.count);
        for i in 0..n {
            dst[i] = self.data[self.read_pos];
            self.read_pos = (self.read_pos + 1) % PIPE_BUF_SIZE;
        }
        self.count -= n;
        n
    }
}

static RING3_PIPE_TABLE: spin::Mutex<Option<alloc::collections::BTreeMap<u64, Ring3PipeBuffer>>> =
    spin::Mutex::new(None);
static RING3_NEXT_PIPE_ID: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(1);

fn with_pipe_table<F, R>(f: F) -> R
where
    F: FnOnce(&mut alloc::collections::BTreeMap<u64, Ring3PipeBuffer>) -> R,
{
    let mut guard = RING3_PIPE_TABLE.lock();
    if guard.is_none() {
        *guard = Some(alloc::collections::BTreeMap::new());
    }
    f(guard.as_mut().expect("pipe table init"))
}

fn ring3_pipe_read(pipe_id: u64, dst: &mut [u8]) -> usize {
    with_pipe_table(|t| {
        match t.get_mut(&pipe_id) {
            Some(pipe) => pipe.read(dst),
            None => 0,
        }
    })
}

fn ring3_pipe_write(pipe_id: u64, src: &[u8]) -> usize {
    with_pipe_table(|t| {
        match t.get_mut(&pipe_id) {
            Some(pipe) => pipe.write(src),
            None => 0,
        }
    })
}

fn ring3_pipe_close(pipe_id: u64, is_write_end: bool) {
    with_pipe_table(|t| {
        if let Some(pipe) = t.get_mut(&pipe_id) {
            if is_write_end {
                pipe.write_closed = true;
            } else {
                pipe.read_closed = true;
            }
            if pipe.write_closed && pipe.read_closed {
                t.remove(&pipe_id);
            }
        }
    });
}

/// Query pipe readiness for epoll: returns EPOLLIN/EPOLLOUT bitmask.
fn ring3_pipe_poll(pipe_id: u64) -> u32 {
    use crate::sync::epoll::{EPOLLIN, EPOLLOUT, EPOLLHUP};
    with_pipe_table(|t| {
        match t.get(&pipe_id) {
            Some(pipe) => {
                let mut flags = 0u32;
                if pipe.count > 0 || pipe.write_closed {
                    flags |= EPOLLIN;
                }
                if pipe.count < PIPE_BUF_SIZE && !pipe.read_closed {
                    flags |= EPOLLOUT;
                }
                if pipe.write_closed && pipe.count == 0 {
                    flags |= EPOLLHUP;
                }
                flags
            }
            None => EPOLLHUP,
        }
    })
}

// ─── Public helpers for kernel-internal epoll tests ──────────────────

/// Create a pipe and return (read_fd, write_fd, pipe_id) for kernel tests.
///
/// Unlike `dispatch_sys_pipe2`, this does not write to user memory — it
/// returns the values directly for use from Rust test code.
pub fn ring3_create_pipe() -> (i32, i32, u64) {
    let pipe_id = RING3_NEXT_PIPE_ID.fetch_add(1, Ordering::Relaxed);
    with_pipe_table(|t| {
        t.insert(pipe_id, Ring3PipeBuffer::new());
    });
    let read_fd = RING3_NEXT_FD.fetch_add(1, Ordering::Relaxed);
    let write_fd = RING3_NEXT_FD.fetch_add(1, Ordering::Relaxed);
    {
        let mut table = RING3_FD_TABLE.lock();
        if (read_fd as usize) < 64 {
            table[read_fd as usize] = Some(FdEntry {
                kind: FdKind::Pipe { pipe_id, is_write_end: false },
            });
        }
        if (write_fd as usize) < 64 {
            table[write_fd as usize] = Some(FdEntry {
                kind: FdKind::Pipe { pipe_id, is_write_end: true },
            });
        }
    }
    (read_fd, write_fd, pipe_id)
}

/// Write data into a pipe buffer (kernel-internal, bypasses syscall path).
pub fn ring3_write_pipe(pipe_id: u64, data: &[u8]) -> usize {
    ring3_pipe_write(pipe_id, data)
}

/// Poll a pipe (or look up fd→pipe_id and poll). Returns epoll event flags.
///
/// This maps an fd to its pipe_id via the FD table, then checks readiness.
/// For fds not found in the table, returns 0.
pub fn ring3_poll_pipe(fd: i32) -> u32 {
    let table = RING3_FD_TABLE.lock();
    match table.get(fd as usize).and_then(|e| e.as_ref()) {
        Some(FdEntry { kind: FdKind::Pipe { pipe_id, .. } }) => {
            let pid = *pipe_id;
            drop(table);
            ring3_pipe_poll(pid)
        }
        _ => 0,
    }
}

// ─── SYS_PIPE2 (293) ────────────────────────────────────────────────

fn dispatch_sys_pipe2(pipefd_ptr: u64, _flags: u32) -> i64 {
    if pipefd_ptr == 0 || pipefd_ptr >= 0x0000_8000_0000_0000 {
        return -14; // -EFAULT
    }

    let pipe_id = RING3_NEXT_PIPE_ID.fetch_add(1, Ordering::Relaxed);
    with_pipe_table(|t| {
        t.insert(pipe_id, Ring3PipeBuffer::new());
    });

    // Allocate two FDs: read end and write end.
    let read_fd = RING3_NEXT_FD.fetch_add(1, Ordering::Relaxed);
    let write_fd = RING3_NEXT_FD.fetch_add(1, Ordering::Relaxed);
    if read_fd < 0 || write_fd < 0 || read_fd as usize >= 64 || write_fd as usize >= 64 {
        ring3_pipe_close(pipe_id, true);
        ring3_pipe_close(pipe_id, false);
        return -24; // -EMFILE
    }

    {
        let mut table = RING3_FD_TABLE.lock();
        table[read_fd as usize] = Some(FdEntry {
            kind: FdKind::Pipe { pipe_id, is_write_end: false },
        });
        table[write_fd as usize] = Some(FdEntry {
            kind: FdKind::Pipe { pipe_id, is_write_end: true },
        });
    }

    // Write [read_fd, write_fd] to user-space pointer.
    // SAFETY: pipefd_ptr validated above to be in user-space range.
    unsafe {
        let arr = pipefd_ptr as *mut [i32; 2];
        (*arr)[0] = read_fd;
        (*arr)[1] = write_fd;
    }

    crate::serial_println!("[Pipe] created pipe_id={} read_fd={} write_fd={}", pipe_id, read_fd, write_fd);
    0
}

// ─── Epoll syscall dispatchers ───────────────────────────────────────

fn dispatch_sys_epoll_create1(_flags: u32) -> i64 {
    let epoll_id = crate::sync::epoll::epoll_create();

    let fd = RING3_NEXT_FD.fetch_add(1, Ordering::Relaxed);
    if fd < 0 || fd as usize >= 64 {
        crate::sync::epoll::epoll_destroy(epoll_id);
        return -24; // -EMFILE
    }

    let mut table = RING3_FD_TABLE.lock();
    table[fd as usize] = Some(FdEntry {
        kind: FdKind::Epoll { epoll_id },
    });

    crate::serial_println!("[Epoll] created epoll_id={} fd={}", epoll_id, fd);
    fd as i64
}

fn dispatch_sys_epoll_ctl(epfd: i32, op: i32, fd: i32, event_ptr: u64) -> i64 {
    // Look up the epoll instance from the FD table.
    let epoll_id = {
        let table = RING3_FD_TABLE.lock();
        match table.get(epfd as usize).and_then(|e| e.as_ref()) {
            Some(FdEntry { kind: FdKind::Epoll { epoll_id } }) => *epoll_id,
            _ => return -9, // -EBADF
        }
    };

    // For DEL, we don't need to read the event struct.
    if op == crate::sync::epoll::EPOLL_CTL_DEL {
        return match crate::sync::epoll::epoll_ctl(epoll_id, op, fd, 0, 0) {
            Ok(()) => 0,
            Err(e) => e,
        };
    }

    // Read struct epoll_event (12 bytes packed: u32 events + u64 data) from user-space.
    if event_ptr == 0 || event_ptr >= 0x0000_8000_0000_0000 {
        return -14; // -EFAULT
    }
    // SAFETY: event_ptr is validated to be in user-space range.
    let (events, data) = unsafe {
        let ev = event_ptr as *const u8;
        let events = u32::from_ne_bytes([*ev, *ev.add(1), *ev.add(2), *ev.add(3)]);
        let data = u64::from_ne_bytes([
            *ev.add(4), *ev.add(5), *ev.add(6), *ev.add(7),
            *ev.add(8), *ev.add(9), *ev.add(10), *ev.add(11),
        ]);
        (events, data)
    };

    match crate::sync::epoll::epoll_ctl(epoll_id, op, fd, events, data) {
        Ok(()) => {
            crate::serial_println!("[Epoll] ctl op={} fd={} events={:#x}", op, fd, events);
            0
        }
        Err(e) => e,
    }
}

fn dispatch_sys_epoll_wait(epfd: i32, events_ptr: u64, maxevents: i32, _timeout: i32) -> i64 {
    if maxevents <= 0 || events_ptr == 0 || events_ptr >= 0x0000_8000_0000_0000 {
        return -22; // -EINVAL
    }

    // Look up the epoll instance.
    let epoll_id = {
        let table = RING3_FD_TABLE.lock();
        match table.get(epfd as usize).and_then(|e| e.as_ref()) {
            Some(FdEntry { kind: FdKind::Epoll { epoll_id } }) => *epoll_id,
            _ => return -9, // -EBADF
        }
    };

    // Build a snapshot of fd→kind so we can poll without holding the FD table lock.
    let fd_snapshot: alloc::vec::Vec<(i32, FdKind)> = {
        let table = RING3_FD_TABLE.lock();
        table.iter().enumerate().filter_map(|(i, slot)| {
            slot.as_ref().map(|e| (i as i32, e.kind))
        }).collect()
    };

    // Poll function: given an fd, return its readiness as epoll event flags.
    let poll_fn = |fd: i32| -> u32 {
        for (fdi, kind) in &fd_snapshot {
            if *fdi == fd {
                return match kind {
                    FdKind::Socket { handle_id } => {
                        let pf = network::socket::poll(network::socket::SocketHandle(*handle_id));
                        // Map PollFlags to EPOLL* constants
                        let mut flags = 0u32;
                        if pf.contains(&network::socket::PollFlags::READABLE) {
                            flags |= crate::sync::epoll::EPOLLIN;
                        }
                        if pf.contains(&network::socket::PollFlags::WRITABLE) {
                            flags |= crate::sync::epoll::EPOLLOUT;
                        }
                        if pf.contains(&network::socket::PollFlags::ERROR) {
                            flags |= crate::sync::epoll::EPOLLERR;
                        }
                        if pf.contains(&network::socket::PollFlags::HANGUP) {
                            flags |= crate::sync::epoll::EPOLLHUP;
                        }
                        flags
                    }
                    FdKind::Pipe { pipe_id, .. } => ring3_pipe_poll(*pipe_id),
                    _ => 0,
                };
            }
        }
        0
    };

    let result = match crate::sync::epoll::epoll_wait(epoll_id, maxevents as usize, poll_fn) {
        Ok(events) => events,
        Err(e) => return e,
    };

    // Write epoll_event structs (12 bytes each) to user-space.
    let count = result.len();
    for (i, ev) in result.iter().enumerate() {
        let base = events_ptr + (i as u64 * 12);
        // SAFETY: events_ptr was validated and we stay within maxevents bounds.
        unsafe {
            let ptr = base as *mut u8;
            let ev_bytes = ev.events.to_ne_bytes();
            let data_bytes = ev.data.to_ne_bytes();
            core::ptr::copy_nonoverlapping(ev_bytes.as_ptr(), ptr, 4);
            core::ptr::copy_nonoverlapping(data_bytes.as_ptr(), ptr.add(4), 8);
        }
        // Log each ready event for QEMU test verification
        let event_name = if ev.events & crate::sync::epoll::EPOLLIN != 0 {
            "EPOLLIN"
        } else if ev.events & crate::sync::epoll::EPOLLOUT != 0 {
            "EPOLLOUT"
        } else {
            "OTHER"
        };
        crate::serial_println!("[Epoll] ready fd={} events={}", ev.data as i32, event_name);
    }

    count as i64
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
