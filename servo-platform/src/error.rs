//! Platform error types

use alloc::string::String;
use core::fmt;

/// Platform error type
#[derive(Debug, Clone)]
pub enum PlatformError {
    /// I/O error
    Io(IoError),
    /// Network error
    Network(NetError),
    /// GPU error
    Gpu(GpuError),
    /// Thread error
    Thread(ThreadError),
    /// IPC error
    Ipc(IpcError),
    /// Generic error with message
    Other(String),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformError::Io(e) => write!(f, "I/O error: {:?}", e),
            PlatformError::Network(e) => write!(f, "Network error: {:?}", e),
            PlatformError::Gpu(e) => write!(f, "GPU error: {:?}", e),
            PlatformError::Thread(e) => write!(f, "Thread error: {:?}", e),
            PlatformError::Ipc(e) => write!(f, "IPC error: {:?}", e),
            PlatformError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

/// I/O error kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoError {
    NotFound,
    PermissionDenied,
    AlreadyExists,
    WouldBlock,
    InvalidInput,
    InvalidData,
    TimedOut,
    Interrupted,
    UnexpectedEof,
    Other,
}

/// Network error kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetError {
    ConnectionRefused,
    ConnectionReset,
    ConnectionAborted,
    NotConnected,
    AddrInUse,
    AddrNotAvailable,
    NetworkDown,
    NetworkUnreachable,
    HostUnreachable,
    TimedOut,
    InvalidInput,
    DnsLookupFailed,
    TlsError,
    Other,
}

/// GPU error kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuError {
    OutOfMemory,
    DeviceLost,
    InvalidHandle,
    UnsupportedFormat,
    SurfaceError,
    ShaderError,
    Other,
}

/// Thread error kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadError {
    SpawnFailed,
    JoinFailed,
    LockPoisoned,
    WouldBlock,
    Other,
}

/// IPC error kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    NotConnected,
    ChannelClosed,
    BufferFull,
    MessageTooLarge,
    InvalidMessage,
    Timeout,
    PermissionDenied,
    Other,
}

/// Result type for platform operations
pub type Result<T> = core::result::Result<T, PlatformError>;
