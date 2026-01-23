//! System call handling module.
//!
//! This module implements the system call interface for WASM processes.
//! All system calls follow the capability-based security model.

pub mod handlers;

use core::arch::asm;

/// System call numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum SyscallNumber {
    // ==========================================
    // Process Management (0-19)
    // ==========================================
    /// Exit the current process.
    Exit = 0,
    /// Write to a file descriptor.
    Write = 1,
    /// Read from a file descriptor.
    Read = 2,
    /// Open a file.
    Open = 3,
    /// Close a file descriptor.
    Close = 4,
    /// Memory map.
    Mmap = 5,
    /// Memory unmap.
    Munmap = 6,
    /// Fork process.
    Fork = 7,
    /// Execute new program.
    Exec = 8,
    /// Wait for child process.
    Wait = 9,
    
    // ==========================================
    // IPC (10-19)
    // ==========================================
    /// Create an IPC channel.
    ChannelCreate = 10,
    /// Send an IPC message.
    ChannelSend = 11,
    /// Receive an IPC message.
    ChannelRecv = 12,
    /// Close an IPC channel.
    ChannelClose = 13,
    /// Create shared memory.
    ShmCreate = 14,
    /// Map shared memory.
    ShmMap = 15,
    /// Unmap shared memory.
    ShmUnmap = 16,
    
    // ==========================================
    // Process Info & Control (20-29)
    // ==========================================
    /// Get process info.
    ProcessInfo = 20,
    /// Yield CPU.
    Yield = 21,
    /// Sleep for a duration.
    Sleep = 22,
    /// Get current time.
    GetTime = 23,
    /// Get process ID.
    GetPid = 24,
    /// Get parent process ID.
    GetPpid = 25,
    /// Set process break (heap).
    Brk = 26,
    
    // ==========================================
    // Sockets (30-39)
    // ==========================================
    /// Create a socket.
    SocketCreate = 30,
    /// Bind a socket.
    SocketBind = 31,
    /// Listen on a socket.
    SocketListen = 32,
    /// Accept a connection.
    SocketAccept = 33,
    /// Connect to a remote address.
    SocketConnect = 34,
    /// Send data on a socket.
    SocketSend = 35,
    /// Receive data on a socket.
    SocketRecv = 36,
    
    // ==========================================
    // GPU (40-49)
    // ==========================================
    /// Allocate GPU memory.
    GpuAlloc = 40,
    /// Submit GPU commands.
    GpuSubmit = 41,
    /// Present a frame.
    GpuPresent = 42,
    /// Set GPU priority.
    GpuSetPriority = 43,
    /// Wait for GPU fence.
    GpuWait = 44,
    
    // ==========================================
    // Threading (50-59)
    // ==========================================
    /// Create a thread.
    ThreadCreate = 50,
    /// Exit current thread.
    ThreadExit = 51,
    /// Join a thread.
    ThreadJoin = 52,
    /// Futex wait.
    FutexWait = 53,
    /// Futex wake.
    FutexWake = 54,
    
    // ==========================================
    // Epoll (60-69)
    // ==========================================
    /// Create epoll instance.
    EpollCreate = 60,
    /// Control epoll.
    EpollCtl = 61,
    /// Wait for epoll events.
    EpollWait = 62,
    
    // ==========================================
    // KPIO Extensions - Browser (100-109)
    // ==========================================
    /// Debug print.
    DebugPrint = 100,
    /// Register browser tab.
    TabRegister = 101,
    /// Set tab state.
    TabSetState = 102,
    /// Get tab memory usage.
    TabGetMemory = 103,
    /// WASM cache lookup.
    WasmCacheGet = 104,
    /// WASM cache store.
    WasmCachePut = 105,
}

impl TryFrom<u64> for SyscallNumber {
    type Error = ();
    
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(SyscallNumber::Exit),
            1 => Ok(SyscallNumber::Write),
            2 => Ok(SyscallNumber::Read),
            3 => Ok(SyscallNumber::Open),
            4 => Ok(SyscallNumber::Close),
            5 => Ok(SyscallNumber::Mmap),
            6 => Ok(SyscallNumber::Munmap),
            10 => Ok(SyscallNumber::ChannelCreate),
            11 => Ok(SyscallNumber::ChannelSend),
            12 => Ok(SyscallNumber::ChannelRecv),
            13 => Ok(SyscallNumber::ChannelClose),
            20 => Ok(SyscallNumber::ProcessInfo),
            21 => Ok(SyscallNumber::Yield),
            22 => Ok(SyscallNumber::Sleep),
            23 => Ok(SyscallNumber::GetTime),
            30 => Ok(SyscallNumber::SocketCreate),
            31 => Ok(SyscallNumber::SocketBind),
            32 => Ok(SyscallNumber::SocketListen),
            33 => Ok(SyscallNumber::SocketAccept),
            34 => Ok(SyscallNumber::SocketConnect),
            35 => Ok(SyscallNumber::SocketSend),
            36 => Ok(SyscallNumber::SocketRecv),
            40 => Ok(SyscallNumber::GpuAlloc),
            41 => Ok(SyscallNumber::GpuSubmit),
            42 => Ok(SyscallNumber::GpuPresent),
            100 => Ok(SyscallNumber::DebugPrint),
            _ => Err(()),
        }
    }
}

/// System call result.
pub type SyscallResult = Result<u64, SyscallError>;

/// System call error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum SyscallError {
    /// Success (not an error).
    Success = 0,
    /// Invalid system call number.
    InvalidSyscall = -1,
    /// Invalid argument.
    InvalidArgument = -2,
    /// Permission denied.
    PermissionDenied = -3,
    /// Resource not found.
    NotFound = -4,
    /// Operation would block.
    WouldBlock = -5,
    /// Resource busy.
    Busy = -6,
    /// Out of memory.
    OutOfMemory = -7,
    /// I/O error.
    IoError = -8,
    /// Connection refused.
    ConnectionRefused = -9,
    /// Connection reset.
    ConnectionReset = -10,
    /// Not connected.
    NotConnected = -11,
    /// Address in use.
    AddressInUse = -12,
    /// Invalid capability.
    InvalidCapability = -13,
    /// Buffer too small.
    BufferTooSmall = -14,
}

/// System call context (registers at syscall time).
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SyscallContext {
    /// System call number.
    pub syscall_num: u64,
    /// First argument.
    pub arg1: u64,
    /// Second argument.
    pub arg2: u64,
    /// Third argument.
    pub arg3: u64,
    /// Fourth argument.
    pub arg4: u64,
    /// Fifth argument.
    pub arg5: u64,
    /// Sixth argument.
    pub arg6: u64,
}

/// Initialize the system call handler.
pub fn init() {
    // Set up SYSCALL/SYSRET MSRs
    setup_syscall_msr();
}

/// Set up the SYSCALL/SYSRET MSRs.
fn setup_syscall_msr() {
    use x86_64::registers::model_specific::{Efer, EferFlags, Msr};
    
    // Enable SYSCALL/SYSRET
    unsafe {
        let mut efer = Efer::read();
        efer |= EferFlags::SYSTEM_CALL_EXTENSIONS;
        Efer::write(efer);
    }
    
    // Set up STAR MSR (segments for SYSCALL/SYSRET)
    // SYSCALL: CS = STAR[47:32], SS = STAR[47:32] + 8
    // SYSRET:  CS = STAR[63:48] + 16, SS = STAR[63:48] + 8
    const STAR_MSR: u32 = 0xC0000081;
    const LSTAR_MSR: u32 = 0xC0000082;
    const SFMASK_MSR: u32 = 0xC0000084;
    
    // Kernel CS = 0x08, Kernel SS = 0x10
    // User CS = 0x1B (0x18 + 3), User SS = 0x23 (0x20 + 3)
    let star_value: u64 = (0x001B_0008u64) << 32;
    
    unsafe {
        // Set STAR
        Msr::new(STAR_MSR).write(star_value);
        
        // Set LSTAR (syscall entry point)
        Msr::new(LSTAR_MSR).write(syscall_entry as u64);
        
        // Set SFMASK (flags to clear on SYSCALL)
        // Clear IF (interrupt flag) and TF (trap flag)
        Msr::new(SFMASK_MSR).write(0x200 | 0x100);
    }
}

/// System call entry point (called from assembly).
#[no_mangle]
extern "C" fn syscall_entry() {
    // This is a stub - actual implementation would:
    // 1. Save user registers
    // 2. Switch to kernel stack
    // 3. Call syscall_dispatch
    // 4. Restore user registers
    // 5. SYSRET back to userspace
}

/// Dispatch a system call.
pub fn dispatch(ctx: &SyscallContext) -> i64 {
    let result = match SyscallNumber::try_from(ctx.syscall_num) {
        Ok(syscall) => handlers::handle(syscall, ctx),
        Err(_) => Err(SyscallError::InvalidSyscall),
    };
    
    match result {
        Ok(value) => value as i64,
        Err(err) => err as i64,
    }
}
