//! Raw system call interface.
//!
//! This module provides the low-level system call mechanism using
//! the x86_64 `syscall` instruction.

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
