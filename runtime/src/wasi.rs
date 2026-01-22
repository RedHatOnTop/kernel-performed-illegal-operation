//! WASI Preview 2 implementation.
//!
//! This module provides WASI (WebAssembly System Interface) Preview 2
//! support for WASM modules running in the KPIO kernel.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

use crate::RuntimeError;

/// WASI context for a running instance.
pub struct WasiCtx {
    /// Environment variables.
    env: BTreeMap<String, String>,
    
    /// Command line arguments.
    args: Vec<String>,
    
    /// File descriptors.
    fds: BTreeMap<u32, FileDescriptor>,
    
    /// Next file descriptor number.
    next_fd: u32,
    
    /// Exit code (if exited).
    exit_code: Option<u32>,
    
    /// Working directory.
    cwd: String,
}

impl WasiCtx {
    /// Create a new WASI context.
    pub fn new() -> Self {
        let mut ctx = WasiCtx {
            env: BTreeMap::new(),
            args: Vec::new(),
            fds: BTreeMap::new(),
            next_fd: 3, // 0, 1, 2 are stdin, stdout, stderr
            exit_code: None,
            cwd: String::from("/"),
        };
        
        // Set up standard file descriptors
        ctx.fds.insert(0, FileDescriptor::stdin());
        ctx.fds.insert(1, FileDescriptor::stdout());
        ctx.fds.insert(2, FileDescriptor::stderr());
        
        ctx
    }
    
    /// Set command line arguments.
    pub fn args(&mut self, args: Vec<String>) -> &mut Self {
        self.args = args;
        self
    }
    
    /// Set environment variables.
    pub fn env(&mut self, key: &str, value: &str) -> &mut Self {
        self.env.insert(key.into(), value.into());
        self
    }
    
    /// Set the working directory.
    pub fn cwd(&mut self, path: &str) -> &mut Self {
        self.cwd = path.into();
        self
    }
    
    /// Get the exit code (if process has exited).
    pub fn exit_code(&self) -> Option<u32> {
        self.exit_code
    }
    
    /// Allocate a new file descriptor.
    pub fn alloc_fd(&mut self, fd: FileDescriptor) -> u32 {
        let num = self.next_fd;
        self.next_fd += 1;
        self.fds.insert(num, fd);
        num
    }
    
    /// Get a file descriptor.
    pub fn get_fd(&self, fd: u32) -> Option<&FileDescriptor> {
        self.fds.get(&fd)
    }
    
    /// Close a file descriptor.
    pub fn close_fd(&mut self, fd: u32) -> Result<(), WasiError> {
        self.fds.remove(&fd).ok_or(WasiError::BadF)?;
        Ok(())
    }
}

impl Default for WasiCtx {
    fn default() -> Self {
        Self::new()
    }
}

/// A file descriptor.
#[derive(Debug, Clone)]
pub struct FileDescriptor {
    /// File descriptor type.
    fd_type: FdType,
    
    /// File descriptor rights.
    rights: FdRights,
    
    /// Current offset.
    offset: u64,
    
    /// File path (if applicable).
    path: Option<String>,
}

impl FileDescriptor {
    /// Create stdin file descriptor.
    pub fn stdin() -> Self {
        FileDescriptor {
            fd_type: FdType::CharDevice,
            rights: FdRights::READ,
            offset: 0,
            path: None,
        }
    }
    
    /// Create stdout file descriptor.
    pub fn stdout() -> Self {
        FileDescriptor {
            fd_type: FdType::CharDevice,
            rights: FdRights::WRITE,
            offset: 0,
            path: None,
        }
    }
    
    /// Create stderr file descriptor.
    pub fn stderr() -> Self {
        FileDescriptor {
            fd_type: FdType::CharDevice,
            rights: FdRights::WRITE,
            offset: 0,
            path: None,
        }
    }
    
    /// Create a regular file descriptor.
    pub fn file(path: String, rights: FdRights) -> Self {
        FileDescriptor {
            fd_type: FdType::RegularFile,
            rights,
            offset: 0,
            path: Some(path),
        }
    }
    
    /// Create a directory descriptor.
    pub fn directory(path: String) -> Self {
        FileDescriptor {
            fd_type: FdType::Directory,
            rights: FdRights::READ | FdRights::PATH_OPEN,
            offset: 0,
            path: Some(path),
        }
    }
}

/// File descriptor types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdType {
    /// Regular file.
    RegularFile,
    /// Directory.
    Directory,
    /// Block device.
    BlockDevice,
    /// Character device.
    CharDevice,
    /// Socket.
    Socket,
    /// Unknown.
    Unknown,
}

bitflags::bitflags! {
    /// File descriptor rights.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FdRights: u64 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const SEEK = 1 << 2;
        const SYNC = 1 << 3;
        const TELL = 1 << 4;
        const ADVISE = 1 << 5;
        const ALLOCATE = 1 << 6;
        const PATH_CREATE_DIR = 1 << 7;
        const PATH_CREATE_FILE = 1 << 8;
        const PATH_OPEN = 1 << 9;
        const PATH_READLINK = 1 << 10;
        const PATH_REMOVE = 1 << 11;
        const PATH_RENAME = 1 << 12;
        const PATH_FILESTAT = 1 << 13;
        const PATH_LINK = 1 << 14;
        const PATH_SYMLINK = 1 << 15;
        const POLL_FD = 1 << 16;
        const SOCK_RECV = 1 << 17;
        const SOCK_SEND = 1 << 18;
    }
}

/// WASI error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum WasiError {
    /// Success.
    Success = 0,
    /// Argument list too long.
    TooBig = 1,
    /// Permission denied.
    Access = 2,
    /// Address in use.
    AddrInUse = 3,
    /// Address not available.
    AddrNotAvail = 4,
    /// Address family not supported.
    AfNoSupport = 5,
    /// Resource unavailable.
    Again = 6,
    /// Connection already in progress.
    Already = 7,
    /// Bad file descriptor.
    BadF = 8,
    /// Bad message.
    BadMsg = 9,
    /// Device or resource busy.
    Busy = 10,
    /// Operation canceled.
    Canceled = 11,
    /// No child processes.
    Child = 12,
    /// Connection aborted.
    ConnAborted = 13,
    /// Connection refused.
    ConnRefused = 14,
    /// Connection reset.
    ConnReset = 15,
    /// Resource deadlock would occur.
    DeadLk = 16,
    /// Destination address required.
    DestAddrReq = 17,
    /// Mathematics argument out of domain.
    Dom = 18,
    /// File exists.
    Exist = 20,
    /// Bad address.
    Fault = 21,
    /// File too large.
    FBig = 22,
    /// Host unreachable.
    HostUnreach = 23,
    /// Identifier removed.
    IdRm = 24,
    /// Illegal byte sequence.
    IlSeq = 25,
    /// Operation in progress.
    InProgress = 26,
    /// Interrupted function.
    Intr = 27,
    /// Invalid argument.
    Inval = 28,
    /// I/O error.
    Io = 29,
    /// Socket is connected.
    IsConn = 30,
    /// Is a directory.
    IsDir = 31,
    /// Too many levels of symbolic links.
    Loop = 32,
    /// File descriptor value too large.
    MFile = 33,
    /// Too many links.
    MLink = 34,
    /// Message too large.
    MsgSize = 35,
    /// Filename too long.
    NameTooLong = 37,
    /// Network is down.
    NetDown = 38,
    /// Connection aborted by network.
    NetReset = 39,
    /// Network unreachable.
    NetUnreach = 40,
    /// Too many files open in system.
    NFile = 41,
    /// No buffer space available.
    NoBufs = 42,
    /// No such device.
    NoDev = 43,
    /// No such file or directory.
    NoEnt = 44,
    /// Executable file format error.
    NoExec = 45,
    /// No locks available.
    NoLck = 46,
    /// Not enough space.
    NoMem = 48,
    /// No message of the desired type.
    NoMsg = 49,
    /// Protocol not available.
    NoProtoOpt = 50,
    /// No space left on device.
    NoSpc = 51,
    /// Function not supported.
    NoSys = 52,
    /// Socket is not connected.
    NotConn = 53,
    /// Not a directory.
    NotDir = 54,
    /// Directory not empty.
    NotEmpty = 55,
    /// State not recoverable.
    NotRecoverable = 56,
    /// Not a socket.
    NotSock = 57,
    /// Not supported.
    NotSup = 58,
    /// Inappropriate I/O control operation.
    NoTty = 59,
    /// No such device or address.
    NxIo = 60,
    /// Value too large to be stored in data type.
    Overflow = 61,
    /// Previous owner died.
    OwnerDead = 62,
    /// Operation not permitted.
    Perm = 63,
    /// Broken pipe.
    Pipe = 64,
    /// Protocol error.
    Proto = 65,
    /// Protocol not supported.
    ProtoNoSupport = 66,
    /// Protocol wrong type for socket.
    ProtoType = 67,
    /// Result too large.
    Range = 68,
    /// Read-only file system.
    RoFs = 69,
    /// Invalid seek.
    SPipe = 70,
    /// No such process.
    SRch = 71,
    /// Connection timed out.
    TimedOut = 73,
    /// Text file busy.
    TxtBsy = 74,
    /// Cross-device link.
    XDev = 75,
    /// Capabilities insufficient.
    NotCapable = 76,
}

impl From<WasiError> for RuntimeError {
    fn from(err: WasiError) -> Self {
        RuntimeError::WasiError(alloc::format!("WASI error: {:?}", err))
    }
}
