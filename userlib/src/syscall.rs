//! Raw system call interface.
//!
//! This module provides the low-level system call mechanism using
//! the x86_64 `syscall` instruction.

use alloc::string::String;
use alloc::vec::Vec;
use core::arch::asm;

/// Linux x86_64 syscall numbers used by the kernel's syscall dispatcher.
///
/// These MUST match the numbers handled by ring3_syscall_dispatch /
/// linux_syscall_dispatch in the kernel.
pub mod linux {
    // File I/O
    pub const SYS_READ: u64 = 0;
    pub const SYS_WRITE: u64 = 1;
    pub const SYS_OPEN: u64 = 2;
    pub const SYS_CLOSE: u64 = 3;
    pub const SYS_STAT: u64 = 4;
    pub const SYS_FSTAT: u64 = 5;
    pub const SYS_LSEEK: u64 = 8;
    pub const SYS_MMAP: u64 = 9;
    pub const SYS_MUNMAP: u64 = 11;
    pub const SYS_BRK: u64 = 12;
    pub const SYS_IOCTL: u64 = 16;
    pub const SYS_PIPE: u64 = 22;
    pub const SYS_SCHED_YIELD: u64 = 24;
    pub const SYS_NANOSLEEP: u64 = 35;
    pub const SYS_GETPID: u64 = 39;
    pub const SYS_FORK: u64 = 57;
    pub const SYS_EXECVE: u64 = 59;
    pub const SYS_EXIT: u64 = 60;
    pub const SYS_WAIT4: u64 = 61;
    pub const SYS_FCNTL: u64 = 72;
    pub const SYS_FSYNC: u64 = 74;
    pub const SYS_GETCWD: u64 = 79;
    pub const SYS_CHDIR: u64 = 80;
    pub const SYS_RENAME: u64 = 82;
    pub const SYS_MKDIR: u64 = 83;
    pub const SYS_RMDIR: u64 = 84;
    pub const SYS_UNLINK: u64 = 87;
    pub const SYS_GETTIMEOFDAY: u64 = 96;
    pub const SYS_GETPPID: u64 = 110;
    pub const SYS_ARCH_PRCTL: u64 = 158;
    pub const SYS_GETTID: u64 = 186;
    pub const SYS_FUTEX: u64 = 202;
    pub const SYS_GETDENTS64: u64 = 217;
    pub const SYS_CLOCK_GETTIME: u64 = 228;
    pub const SYS_EXIT_GROUP: u64 = 231;
    pub const SYS_GETRANDOM: u64 = 318;

    // Threading
    pub const SYS_CLONE: u64 = 56;
    pub const SYS_FUTEX_WAIT: u64 = 202; // same as SYS_FUTEX (op-based)
    pub const SYS_FUTEX_WAKE: u64 = 202;
}

/// Legacy KPIO syscall number enum.
///
/// Kept for backward compatibility with IPC / GPU / extension code.
/// New code should use `linux::SYS_*` constants for standard syscalls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum SyscallNumber {
    // Standard file I/O — use Linux numbers
    Read = 0,
    Write = 1,
    Open = 2,
    Close = 3,
    Mmap = 9,
    Munmap = 11,
    Brk = 12,
    Fork = 57,
    Exec = 59,
    Exit = 60,
    Wait = 61,
    GetPid = 39,
    GetPpid = 110,
    Yield = 24,
    Sleep = 35,
    GetTime = 228,

    // KPIO IPC (500+, no collision with Linux numbers)
    ChannelCreate = 500,
    ChannelSend = 501,
    ChannelRecv = 502,
    ChannelClose = 503,
    ShmCreate = 504,
    ShmMap = 505,
    ShmUnmap = 506,

    // KPIO Sockets (via Linux socket ABI in future; stubs for now)
    SocketCreate = 510,
    SocketBind = 511,
    SocketListen = 512,
    SocketAccept = 513,
    SocketConnect = 514,
    SocketSend = 515,
    SocketRecv = 516,

    // KPIO Threading (520+)
    ThreadCreate = 520,
    ThreadExit = 521,
    ThreadJoin = 522,
    FutexWait = 523,
    FutexWake = 524,

    // KPIO GPU (530+)
    GpuAlloc = 530,
    GpuSubmit = 531,
    GpuPresent = 532,
    GpuSetPriority = 533,
    GpuWait = 534,

    // KPIO Epoll (540+)
    EpollCreate = 540,
    EpollCtl = 541,
    EpollWait = 542,

    // KPIO Extensions (600+)
    DebugPrint = 600,
    TabRegister = 601,
    TabSetState = 602,
    TabGetMemory = 603,
    WasmCacheGet = 604,
    WasmCachePut = 605,

    // KPIO App Management (610+)
    AppInstall = 610,
    AppLaunch = 611,
    AppTerminate = 612,
    AppGetInfo = 613,
    AppList = 614,
    AppUninstall = 615,

    // Process Info  
    ProcessInfo = 620,
}

/// System call error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum SyscallError {
    /// Operation not permitted.
    PermissionDenied = -1,
    /// No such file or directory.
    NotFound = -2,
    /// Invalid argument.
    InvalidArgument = -22,
    /// Out of memory.
    OutOfMemory = -12,
    /// Operation would block.
    WouldBlock = -11,
    /// Not connected.
    NotConnected = -107,
    /// I/O error.
    IoError = -5,
    /// File already exists.
    AlreadyExists = -17,
    /// Unknown error.
    Unknown = -255,
}

impl SyscallError {
    /// Convert raw return value to error.
    pub fn from_raw(val: i64) -> Self {
        match val {
            -1 => Self::PermissionDenied,
            -2 => Self::NotFound,
            -22 => Self::InvalidArgument,
            -12 => Self::OutOfMemory,
            -11 => Self::WouldBlock,
            -107 => Self::NotConnected,
            -5 => Self::IoError,
            _ => Self::Unknown,
        }
    }
}

/// Result type for system calls.
pub type SyscallResult = Result<u64, SyscallError>;

/// Convert raw syscall return value to Result.
#[inline]
fn convert_result(ret: i64) -> SyscallResult {
    if ret >= 0 {
        Ok(ret as u64)
    } else {
        Err(SyscallError::from_raw(ret))
    }
}

/// System call with no arguments.
#[inline]
pub unsafe fn syscall0(nr: SyscallNumber) -> SyscallResult {
    let ret: i64;
    asm!(
        "syscall",
        inout("rax") nr as u64 => ret,
        out("rcx") _,  // clobbered by syscall
        out("r11") _,  // clobbered by syscall
        options(nostack, preserves_flags)
    );
    convert_result(ret)
}

/// System call with 1 argument.
#[inline]
pub unsafe fn syscall1(nr: SyscallNumber, arg1: u64) -> SyscallResult {
    let ret: i64;
    asm!(
        "syscall",
        inout("rax") nr as u64 => ret,
        in("rdi") arg1,
        out("rcx") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    convert_result(ret)
}

/// System call with 2 arguments.
#[inline]
pub unsafe fn syscall2(nr: SyscallNumber, arg1: u64, arg2: u64) -> SyscallResult {
    let ret: i64;
    asm!(
        "syscall",
        inout("rax") nr as u64 => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        out("rcx") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    convert_result(ret)
}

/// System call with 3 arguments.
#[inline]
pub unsafe fn syscall3(nr: SyscallNumber, arg1: u64, arg2: u64, arg3: u64) -> SyscallResult {
    let ret: i64;
    asm!(
        "syscall",
        inout("rax") nr as u64 => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        out("rcx") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    convert_result(ret)
}

/// System call with 4 arguments.
#[inline]
pub unsafe fn syscall4(
    nr: SyscallNumber,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
) -> SyscallResult {
    let ret: i64;
    asm!(
        "syscall",
        inout("rax") nr as u64 => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        in("r10") arg4,
        out("rcx") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    convert_result(ret)
}

/// System call with 5 arguments.
#[inline]
pub unsafe fn syscall5(
    nr: SyscallNumber,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
) -> SyscallResult {
    let ret: i64;
    asm!(
        "syscall",
        inout("rax") nr as u64 => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        in("r10") arg4,
        in("r8") arg5,
        out("rcx") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    convert_result(ret)
}

/// System call with 6 arguments.
#[inline]
pub unsafe fn syscall6(
    nr: SyscallNumber,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
) -> SyscallResult {
    let ret: i64;
    asm!(
        "syscall",
        inout("rax") nr as u64 => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        in("r10") arg4,
        in("r8") arg5,
        in("r9") arg6,
        out("rcx") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    convert_result(ret)
}

// ============================================
// Raw syscall wrappers (take u64 number directly)
// ============================================

/// Raw syscall with 0 arguments.
#[inline]
pub unsafe fn raw_syscall0(nr: u64) -> SyscallResult {
    let ret: i64;
    asm!(
        "syscall",
        inout("rax") nr => ret,
        out("rcx") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    convert_result(ret)
}

/// Raw syscall with 1 argument.
#[inline]
pub unsafe fn raw_syscall1(nr: u64, a1: u64) -> SyscallResult {
    let ret: i64;
    asm!(
        "syscall",
        inout("rax") nr => ret,
        in("rdi") a1,
        out("rcx") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    convert_result(ret)
}

/// Raw syscall with 2 arguments.
#[inline]
pub unsafe fn raw_syscall2(nr: u64, a1: u64, a2: u64) -> SyscallResult {
    let ret: i64;
    asm!(
        "syscall",
        inout("rax") nr => ret,
        in("rdi") a1,
        in("rsi") a2,
        out("rcx") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    convert_result(ret)
}

/// Raw syscall with 3 arguments.
#[inline]
pub unsafe fn raw_syscall3(nr: u64, a1: u64, a2: u64, a3: u64) -> SyscallResult {
    let ret: i64;
    asm!(
        "syscall",
        inout("rax") nr => ret,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        out("rcx") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    convert_result(ret)
}

// ============================================
// Helper: null-terminated path buffer
// ============================================

/// Maximum path length for syscall arguments.
const PATH_MAX: usize = 4096;

/// Call a closure with a pointer to a null-terminated copy of `s`
/// stored on the stack. This is required because Linux syscalls
/// expect C-style null-terminated strings.
fn with_cstr<F>(s: &str, f: F) -> SyscallResult
where
    F: FnOnce(*const u8) -> SyscallResult,
{
    let len = s.len();
    if len >= PATH_MAX {
        return Err(SyscallError::InvalidArgument);
    }
    // Stack-allocated buffer with null terminator.
    let mut buf = [0u8; PATH_MAX];
    buf[..len].copy_from_slice(s.as_bytes());
    buf[len] = 0;
    f(buf.as_ptr())
}

// ============================================
// High-level syscall wrappers for std compatibility
// ============================================

// --- Network ---

pub fn net_connect(a: u8, b: u8, c: u8, d: u8, port: u16) -> SyscallResult {
    let ip = ((a as u64) << 24) | ((b as u64) << 16) | ((c as u64) << 8) | (d as u64);
    unsafe { syscall2(SyscallNumber::SocketConnect, ip, port as u64) }
}

pub fn net_bind(a: u8, b: u8, c: u8, d: u8, port: u16) -> SyscallResult {
    let ip = ((a as u64) << 24) | ((b as u64) << 16) | ((c as u64) << 8) | (d as u64);
    unsafe { syscall2(SyscallNumber::SocketBind, ip, port as u64) }
}

pub fn net_accept(fd: u64) -> Result<(u64, [u8; 4], u16), SyscallError> {
    let result = unsafe { syscall1(SyscallNumber::SocketAccept, fd) }?;
    Ok((result, [0, 0, 0, 0], 0))
}

pub fn net_send(fd: u64, buf: &[u8]) -> SyscallResult {
    unsafe {
        syscall3(
            SyscallNumber::SocketSend,
            fd,
            buf.as_ptr() as u64,
            buf.len() as u64,
        )
    }
}

pub fn net_recv(fd: u64, buf: &mut [u8]) -> SyscallResult {
    unsafe {
        syscall3(
            SyscallNumber::SocketRecv,
            fd,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
        )
    }
}

pub fn net_close(fd: u64) -> SyscallResult {
    unsafe { syscall1(SyscallNumber::Close, fd) }
}

pub fn net_shutdown(fd: u64, how: u32) -> SyscallResult {
    unsafe { syscall2(SyscallNumber::Close, fd, how as u64) }
}

pub fn net_dup(fd: u64) -> SyscallResult {
    // TODO: Implement dup syscall
    Ok(fd)
}

pub fn net_local_addr(fd: u64) -> Result<([u8; 4], u16), SyscallError> {
    // TODO: Implement getsockname syscall
    Ok(([0, 0, 0, 0], 0))
}

// --- File System ---

/// Open a file. `path` is passed as a null-terminated C string.
pub fn fs_open(path: &str, flags: u32) -> SyscallResult {
    with_cstr(path, |ptr| unsafe {
        raw_syscall3(linux::SYS_OPEN, ptr as u64, flags as u64, 0)
    })
}

pub fn fs_close(fd: u64) -> SyscallResult {
    unsafe { raw_syscall1(linux::SYS_CLOSE, fd) }
}

pub fn fs_read(fd: u64, buf: &mut [u8]) -> SyscallResult {
    unsafe {
        raw_syscall3(
            linux::SYS_READ,
            fd,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
        )
    }
}

pub fn fs_write(fd: u64, buf: &[u8]) -> SyscallResult {
    unsafe {
        raw_syscall3(
            linux::SYS_WRITE,
            fd,
            buf.as_ptr() as u64,
            buf.len() as u64,
        )
    }
}

/// Seek within an open file descriptor.
pub fn fs_seek(fd: u64, offset: i64, whence: u32) -> Result<u64, SyscallError> {
    unsafe {
        raw_syscall3(linux::SYS_LSEEK, fd, offset as u64, whence as u64)
    }
}

/// Sync an open file descriptor to disk.
pub fn fs_sync(fd: u64) -> SyscallResult {
    unsafe { raw_syscall1(linux::SYS_FSYNC, fd) }
}

/// Linux stat buffer layout (partial — we only read size, mode).
#[repr(C)]
pub struct LinuxStat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_nlink: u64,
    pub st_mode: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub _pad0: u32,
    pub st_rdev: u64,
    pub st_size: i64,
    pub st_blksize: i64,
    pub st_blocks: i64,
    // atime, mtime, ctime follow but we don't need them
    pub _rest: [u64; 6],
}

/// Stat a path. Returns (size, is_dir, is_file).
pub fn fs_stat(path: &str) -> Result<(u64, bool, bool), SyscallError> {
    let mut st = core::mem::MaybeUninit::<LinuxStat>::uninit();
    with_cstr(path, |ptr| unsafe {
        raw_syscall2(linux::SYS_STAT, ptr as u64, st.as_mut_ptr() as u64)
    })?;
    let st = unsafe { st.assume_init() };
    let is_dir = (st.st_mode & 0o170000) == 0o040000;
    let is_file = (st.st_mode & 0o170000) == 0o100000;
    Ok((st.st_size as u64, is_dir, is_file))
}

/// Stat an open file descriptor. Returns (size, is_dir, is_file).
pub fn fs_stat_fd(fd: u64) -> Result<(u64, bool, bool), SyscallError> {
    let mut st = core::mem::MaybeUninit::<LinuxStat>::uninit();
    unsafe {
        raw_syscall2(linux::SYS_FSTAT, fd, st.as_mut_ptr() as u64)?;
    }
    let st = unsafe { st.assume_init() };
    let is_dir = (st.st_mode & 0o170000) == 0o040000;
    let is_file = (st.st_mode & 0o170000) == 0o100000;
    Ok((st.st_size as u64, is_dir, is_file))
}

/// Read directory entries. Opens the directory, reads via
/// SYS_GETDENTS64, parses entries, then closes the fd.
pub fn fs_readdir(path: &str) -> Result<Vec<(String, bool)>, SyscallError> {
    let fd = fs_open(path, 0)?; // O_RDONLY
    let mut buf = [0u8; 4096];
    let mut entries = Vec::new();

    loop {
        let n = unsafe {
            raw_syscall3(linux::SYS_GETDENTS64, fd, buf.as_mut_ptr() as u64, buf.len() as u64)
        };
        match n {
            Ok(0) => break,
            Ok(n) => {
                let mut offset = 0usize;
                while offset < n as usize {
                    if offset + 19 > n as usize {
                        break;
                    }
                    let reclen = u16::from_ne_bytes([buf[offset + 16], buf[offset + 17]]) as usize;
                    let d_type = buf[offset + 18];
                    if reclen == 0 || offset + reclen > n as usize {
                        break;
                    }
                    // Name starts at offset + 19 and is null-terminated
                    let name_start = offset + 19;
                    let name_end = buf[name_start..offset + reclen]
                        .iter()
                        .position(|&b| b == 0)
                        .map(|p| name_start + p)
                        .unwrap_or(offset + reclen);
                    if let Ok(name) = core::str::from_utf8(&buf[name_start..name_end]) {
                        let is_dir = d_type == 4; // DT_DIR
                        entries.push((String::from(name), is_dir));
                    }
                    offset += reclen;
                }
            }
            Err(e) => {
                let _ = fs_close(fd);
                return Err(e);
            }
        }
    }
    let _ = fs_close(fd);
    Ok(entries)
}

/// Create a directory.
pub fn fs_mkdir(path: &str) -> SyscallResult {
    with_cstr(path, |ptr| unsafe {
        raw_syscall2(linux::SYS_MKDIR, ptr as u64, 0o755)
    })
}

/// Recursively create directories (like `mkdir -p`).
pub fn fs_mkdir_all(path: &str) -> SyscallResult {
    // Try creating the full path first
    if fs_mkdir(path).is_ok() {
        return Ok(0);
    }
    // Walk components and create each level
    let mut current = String::new();
    for component in path.split('/') {
        if component.is_empty() {
            current.push('/');
            continue;
        }
        if !current.ends_with('/') {
            current.push('/');
        }
        current.push_str(component);
        let _ = fs_mkdir(&current); // ignore "already exists"
    }
    Ok(0)
}

/// Unlink (delete) a file.
pub fn fs_unlink(path: &str) -> SyscallResult {
    with_cstr(path, |ptr| unsafe {
        raw_syscall1(linux::SYS_UNLINK, ptr as u64)
    })
}

/// Remove an empty directory.
pub fn fs_rmdir(path: &str) -> SyscallResult {
    with_cstr(path, |ptr| unsafe {
        raw_syscall1(linux::SYS_RMDIR, ptr as u64)
    })
}

/// Rename a file or directory.
pub fn fs_rename(from: &str, to: &str) -> SyscallResult {
    let from_len = from.len();
    let to_len = to.len();
    if from_len >= PATH_MAX || to_len >= PATH_MAX {
        return Err(SyscallError::InvalidArgument);
    }
    let mut from_buf = [0u8; PATH_MAX];
    from_buf[..from_len].copy_from_slice(from.as_bytes());
    from_buf[from_len] = 0;
    let mut to_buf = [0u8; PATH_MAX];
    to_buf[..to_len].copy_from_slice(to.as_bytes());
    to_buf[to_len] = 0;
    unsafe {
        raw_syscall2(linux::SYS_RENAME, from_buf.as_ptr() as u64, to_buf.as_ptr() as u64)
    }
}

// --- Time ---

pub fn time_monotonic() -> SyscallResult {
    // CLOCK_MONOTONIC = 1
    let mut tp = [0u64; 2]; // tv_sec, tv_nsec
    unsafe {
        raw_syscall2(linux::SYS_CLOCK_GETTIME, 1, tp.as_mut_ptr() as u64)?;
    }
    Ok(tp[0] * 1_000_000_000 + tp[1])
}

pub fn time_realtime() -> Result<(u64, u32), SyscallError> {
    // CLOCK_REALTIME = 0
    let mut tp = [0u64; 2];
    unsafe {
        raw_syscall2(linux::SYS_CLOCK_GETTIME, 0, tp.as_mut_ptr() as u64)?;
    }
    Ok((tp[0], tp[1] as u32))
}

pub fn sleep_ns(nanos: u64) -> SyscallResult {
    let ts = [nanos / 1_000_000_000, nanos % 1_000_000_000];
    unsafe { raw_syscall2(linux::SYS_NANOSLEEP, ts.as_ptr() as u64, 0) }
}

// --- Threading ---

pub fn thread_id() -> SyscallResult {
    unsafe { raw_syscall0(linux::SYS_GETTID) }
}

pub fn sched_yield() -> SyscallResult {
    unsafe { raw_syscall0(linux::SYS_SCHED_YIELD) }
}

pub fn thread_spawn(entry: usize, arg: usize, stack_size: usize) -> SyscallResult {
    unsafe {
        syscall3(
            SyscallNumber::ThreadCreate,
            entry as u64,
            arg as u64,
            stack_size as u64,
        )
    }
}

pub fn thread_exit(code: i32) -> ! {
    unsafe { let _ = raw_syscall1(linux::SYS_EXIT, code as u64); }
    loop {
        core::hint::spin_loop();
    }
}

pub fn thread_join(handle: u64) -> SyscallResult {
    unsafe { syscall1(SyscallNumber::ThreadJoin, handle) }
}

pub fn thread_is_finished(_handle: u64) -> Result<bool, SyscallError> {
    // TODO: Implement check
    Ok(false)
}

pub fn thread_park() -> SyscallResult {
    // TODO: Implement park syscall
    Ok(0)
}

pub fn thread_park_timeout(_nanos: u64) -> SyscallResult {
    // TODO: Implement park_timeout syscall
    Ok(0)
}

pub fn thread_unpark(_id: u64) -> SyscallResult {
    // TODO: Implement unpark syscall
    Ok(0)
}

pub fn futex_wait(addr: usize, expected: u32) -> SyscallResult {
    unsafe { raw_syscall3(linux::SYS_FUTEX, addr as u64, 0, expected as u64) }
}

pub fn futex_wake(addr: usize, count: u32) -> SyscallResult {
    unsafe { raw_syscall3(linux::SYS_FUTEX, addr as u64, 1, count as u64) }
}

pub fn cpu_count() -> Result<usize, SyscallError> {
    Ok(1)
}

// --- Environment ---

/// Get command-line arguments.
///
/// Arguments are placed on the initial user stack by the kernel's
/// `setup_user_stack`: [argc, argv[0], argv[1], ..., NULL, envp...].
/// Since we cannot reliably locate the initial stack frame from
/// library code, this returns an empty vec until a proper auxv
/// mechanism is implemented.
pub fn get_args() -> Result<Vec<String>, SyscallError> {
    // TODO: Read from initial stack (requires auxv / AT_ARGV)
    Ok(Vec::new())
}

/// Get environment variable by key.
///
/// Environment strings are placed after argv on the initial stack.
/// Not yet wired — returns NotFound.
pub fn env_get(_key: &str) -> Result<String, SyscallError> {
    Err(SyscallError::NotFound)
}

pub fn env_set(_key: &str, _value: &str) -> SyscallResult {
    // TODO: Requires in-process env storage
    Ok(0)
}

pub fn env_remove(_key: &str) -> SyscallResult {
    Ok(0)
}

pub fn env_list() -> Result<Vec<(String, String)>, SyscallError> {
    Ok(Vec::new())
}

/// Get current working directory.
pub fn getcwd() -> Result<String, SyscallError> {
    let mut buf = [0u8; PATH_MAX];
    unsafe {
        raw_syscall2(linux::SYS_GETCWD, buf.as_mut_ptr() as u64, buf.len() as u64)?;
    }
    // Result is null-terminated
    let len = buf.iter().position(|&b| b == 0).unwrap_or(0);
    String::from_utf8(buf[..len].to_vec()).map_err(|_| SyscallError::IoError)
}

/// Change current working directory.
pub fn chdir(path: &str) -> SyscallResult {
    with_cstr(path, |ptr| unsafe {
        raw_syscall1(linux::SYS_CHDIR, ptr as u64)
    })
}

pub fn current_exe() -> Result<String, SyscallError> {
    // TODO: Implement /proc/self/exe equivalent
    Ok(String::from("/bin/unknown"))
}

// --- IO ---

pub fn stdin_read(buf: &mut [u8]) -> SyscallResult {
    unsafe {
        syscall3(
            SyscallNumber::Read,
            0,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
        )
    }
}

pub fn stdout_write(buf: &[u8]) -> SyscallResult {
    unsafe {
        syscall3(
            SyscallNumber::Write,
            1,
            buf.as_ptr() as u64,
            buf.len() as u64,
        )
    }
}

pub fn stderr_write(buf: &[u8]) -> SyscallResult {
    unsafe {
        syscall3(
            SyscallNumber::Write,
            2,
            buf.as_ptr() as u64,
            buf.len() as u64,
        )
    }
}
