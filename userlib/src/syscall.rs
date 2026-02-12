//! Raw system call interface.
//!
//! This module provides the low-level system call mechanism using
//! the x86_64 `syscall` instruction.

use alloc::string::String;
use alloc::vec::Vec;
use core::arch::asm;

/// System call numbers - must match kernel's SyscallNumber enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum SyscallNumber {
    // Process Management (0-19)
    Exit = 0,
    Write = 1,
    Read = 2,
    Open = 3,
    Close = 4,
    Mmap = 5,
    Munmap = 6,
    Fork = 7,
    Exec = 8,
    Wait = 9,
    
    // IPC (10-19)
    ChannelCreate = 10,
    ChannelSend = 11,
    ChannelRecv = 12,
    ChannelClose = 13,
    ShmCreate = 14,
    ShmMap = 15,
    ShmUnmap = 16,
    
    // Process Info & Control (20-29)
    ProcessInfo = 20,
    Yield = 21,
    Sleep = 22,
    GetTime = 23,
    GetPid = 24,
    GetPpid = 25,
    Brk = 26,
    
    // Sockets (30-39)
    SocketCreate = 30,
    SocketBind = 31,
    SocketListen = 32,
    SocketAccept = 33,
    SocketConnect = 34,
    SocketSend = 35,
    SocketRecv = 36,
    
    // GPU (40-49)
    GpuAlloc = 40,
    GpuSubmit = 41,
    GpuPresent = 42,
    GpuSetPriority = 43,
    GpuWait = 44,
    
    // Threading (50-59)
    ThreadCreate = 50,
    ThreadExit = 51,
    ThreadJoin = 52,
    FutexWait = 53,
    FutexWake = 54,
    
    // Epoll (60-69)
    EpollCreate = 60,
    EpollCtl = 61,
    EpollWait = 62,
    
    // KPIO Extensions (100+)
    DebugPrint = 100,
    TabRegister = 101,
    TabSetState = 102,
    TabGetMemory = 103,
    WasmCacheGet = 104,
    WasmCachePut = 105,
    
    // App Management (106-111)
    AppInstall = 106,
    AppLaunch = 107,
    AppTerminate = 108,
    AppGetInfo = 109,
    AppList = 110,
    AppUninstall = 111,
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
    // TODO: Parse result to extract fd, ip, port
    Ok((result, [0, 0, 0, 0], 0))
}

pub fn net_send(fd: u64, buf: &[u8]) -> SyscallResult {
    unsafe { syscall3(SyscallNumber::SocketSend, fd, buf.as_ptr() as u64, buf.len() as u64) }
}

pub fn net_recv(fd: u64, buf: &mut [u8]) -> SyscallResult {
    unsafe { syscall3(SyscallNumber::SocketRecv, fd, buf.as_mut_ptr() as u64, buf.len() as u64) }
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

pub fn fs_open(path: &str, flags: u32) -> SyscallResult {
    unsafe { syscall3(SyscallNumber::Open, path.as_ptr() as u64, path.len() as u64, flags as u64) }
}

pub fn fs_close(fd: u64) -> SyscallResult {
    unsafe { syscall1(SyscallNumber::Close, fd) }
}

pub fn fs_read(fd: u64, buf: &mut [u8]) -> SyscallResult {
    unsafe { syscall3(SyscallNumber::Read, fd, buf.as_mut_ptr() as u64, buf.len() as u64) }
}

pub fn fs_write(fd: u64, buf: &[u8]) -> SyscallResult {
    unsafe { syscall3(SyscallNumber::Write, fd, buf.as_ptr() as u64, buf.len() as u64) }
}

pub fn fs_seek(fd: u64, offset: i64, whence: u32) -> Result<u64, SyscallError> {
    // TODO: Implement lseek syscall
    Ok(offset as u64)
}

pub fn fs_sync(fd: u64) -> SyscallResult {
    // TODO: Implement fsync syscall
    Ok(0)
}

pub fn fs_stat(path: &str) -> Result<(u64, bool, bool), SyscallError> {
    // TODO: Implement stat syscall
    Ok((0, false, true))
}

pub fn fs_stat_fd(fd: u64) -> Result<(u64, bool, bool), SyscallError> {
    // TODO: Implement fstat syscall
    Ok((0, false, true))
}

pub fn fs_readdir(path: &str) -> Result<Vec<(String, bool)>, SyscallError> {
    // TODO: Implement readdir syscall
    Ok(Vec::new())
}

pub fn fs_mkdir(path: &str) -> SyscallResult {
    // TODO: Implement mkdir syscall
    Ok(0)
}

pub fn fs_mkdir_all(path: &str) -> SyscallResult {
    // TODO: Implement mkdir -p equivalent
    Ok(0)
}

pub fn fs_unlink(path: &str) -> SyscallResult {
    // TODO: Implement unlink syscall
    Ok(0)
}

pub fn fs_rmdir(path: &str) -> SyscallResult {
    // TODO: Implement rmdir syscall
    Ok(0)
}

pub fn fs_rename(from: &str, to: &str) -> SyscallResult {
    // TODO: Implement rename syscall
    Ok(0)
}

// --- Time ---

pub fn time_monotonic() -> SyscallResult {
    unsafe { syscall1(SyscallNumber::GetTime, 0) }
}

pub fn time_realtime() -> Result<(u64, u32), SyscallError> {
    let result = unsafe { syscall1(SyscallNumber::GetTime, 1) }?;
    Ok((result, 0))
}

pub fn sleep_ns(nanos: u64) -> SyscallResult {
    unsafe { syscall1(SyscallNumber::Sleep, nanos) }
}

// --- Threading ---

pub fn thread_id() -> SyscallResult {
    // TODO: Implement gettid syscall
    Ok(1)
}

pub fn sched_yield() -> SyscallResult {
    unsafe { syscall0(SyscallNumber::Yield) }
}

pub fn thread_spawn(entry: usize, arg: usize, stack_size: usize) -> SyscallResult {
    unsafe { syscall3(SyscallNumber::ThreadCreate, entry as u64, arg as u64, stack_size as u64) }
}

pub fn thread_exit(code: i32) -> ! {
    unsafe { syscall1(SyscallNumber::ThreadExit, code as u64) };
    loop { core::hint::spin_loop(); }
}

pub fn thread_join(handle: u64) -> SyscallResult {
    unsafe { syscall1(SyscallNumber::ThreadJoin, handle) }
}

pub fn thread_is_finished(handle: u64) -> Result<bool, SyscallError> {
    // TODO: Implement check
    Ok(false)
}

pub fn thread_park() -> SyscallResult {
    // TODO: Implement park syscall
    Ok(0)
}

pub fn thread_park_timeout(nanos: u64) -> SyscallResult {
    // TODO: Implement park_timeout syscall
    Ok(0)
}

pub fn thread_unpark(id: u64) -> SyscallResult {
    // TODO: Implement unpark syscall
    Ok(0)
}

pub fn futex_wait(addr: usize, expected: u32) -> SyscallResult {
    unsafe { syscall2(SyscallNumber::FutexWait, addr as u64, expected as u64) }
}

pub fn futex_wake(addr: usize, count: u32) -> SyscallResult {
    unsafe { syscall2(SyscallNumber::FutexWake, addr as u64, count as u64) }
}

pub fn cpu_count() -> Result<usize, SyscallError> {
    // TODO: Implement sysconf(_SC_NPROCESSORS_ONLN)
    Ok(1)
}

// --- Environment ---

pub fn get_args() -> Result<Vec<String>, SyscallError> {
    // TODO: Implement getargs syscall
    Ok(Vec::new())
}

pub fn env_get(key: &str) -> Result<String, SyscallError> {
    // TODO: Implement getenv syscall
    Err(SyscallError::NotFound)
}

pub fn env_set(key: &str, value: &str) -> SyscallResult {
    // TODO: Implement setenv syscall
    Ok(0)
}

pub fn env_remove(key: &str) -> SyscallResult {
    // TODO: Implement unsetenv syscall
    Ok(0)
}

pub fn env_list() -> Result<Vec<(String, String)>, SyscallError> {
    // TODO: Implement environ iteration
    Ok(Vec::new())
}

pub fn getcwd() -> Result<String, SyscallError> {
    // TODO: Implement getcwd syscall
    Ok(String::from("/"))
}

pub fn chdir(path: &str) -> SyscallResult {
    // TODO: Implement chdir syscall
    Ok(0)
}

pub fn current_exe() -> Result<String, SyscallError> {
    // TODO: Implement /proc/self/exe equivalent
    Ok(String::from("/bin/unknown"))
}

// --- IO ---

pub fn stdin_read(buf: &mut [u8]) -> SyscallResult {
    unsafe { syscall3(SyscallNumber::Read, 0, buf.as_mut_ptr() as u64, buf.len() as u64) }
}

pub fn stdout_write(buf: &[u8]) -> SyscallResult {
    unsafe { syscall3(SyscallNumber::Write, 1, buf.as_ptr() as u64, buf.len() as u64) }
}

pub fn stderr_write(buf: &[u8]) -> SyscallResult {
    unsafe { syscall3(SyscallNumber::Write, 2, buf.as_ptr() as u64, buf.len() as u64) }
}
