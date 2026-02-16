//! I/O Operations
//!
//! Defines operation codes and operation-specific structures for async I/O.

use alloc::vec::Vec;

/// Operation codes for async I/O.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    /// No operation.
    Nop = 0,
    /// Read from file.
    Read = 1,
    /// Write to file.
    Write = 2,
    /// Vectored read.
    Readv = 3,
    /// Vectored write.
    Writev = 4,
    /// Fsync file.
    Fsync = 5,
    /// Read fixed buffer.
    ReadFixed = 6,
    /// Write fixed buffer.
    WriteFixed = 7,
    /// Poll for events.
    PollAdd = 8,
    /// Remove poll.
    PollRemove = 9,
    /// Sync file range.
    SyncFileRange = 10,
    /// Send message.
    SendMsg = 11,
    /// Receive message.
    RecvMsg = 12,
    /// Timeout.
    Timeout = 13,
    /// Remove timeout.
    TimeoutRemove = 14,
    /// Accept connection.
    Accept = 15,
    /// Cancel operation.
    AsyncCancel = 16,
    /// Link timeout.
    LinkTimeout = 17,
    /// Connect socket.
    Connect = 18,
    /// Fallocate.
    Fallocate = 19,
    /// Open file.
    Openat = 20,
    /// Close file.
    Close = 21,
    /// Update files.
    FilesUpdate = 22,
    /// Statx.
    Statx = 23,
    /// Read with buffer.
    ReadBuf = 24,
    /// Provide buffers.
    ProvideBuffers = 25,
    /// Remove buffers.
    RemoveBuffers = 26,
    /// Tee.
    Tee = 27,
    /// Shutdown socket.
    Shutdown = 28,
    /// Rename file.
    Renameat = 29,
    /// Unlink file.
    Unlinkat = 30,
    /// Mkdir.
    Mkdirat = 31,
    /// Symlink.
    Symlinkat = 32,
    /// Link.
    Linkat = 33,
    /// Message ring.
    MsgRing = 34,
    /// Fsetxattr.
    Fsetxattr = 35,
    /// Setxattr.
    Setxattr = 36,
    /// Fgetxattr.
    Fgetxattr = 37,
    /// Getxattr.
    Getxattr = 38,
    /// Socket.
    Socket = 39,
    /// Uring command.
    UringCmd = 40,
    /// Send with zero copy.
    SendZc = 41,
    /// Send message with zero copy.
    SendMsgZc = 42,
}

/// I/O operation result.
#[derive(Debug, Clone, Copy)]
pub enum IoResult {
    /// Bytes read/written.
    Bytes(usize),
    /// File descriptor.
    Fd(i32),
    /// Success with no value.
    Success,
    /// Error.
    Error(IoOpError),
    /// Operation pending.
    Pending,
    /// Operation cancelled.
    Cancelled,
}

impl IoResult {
    /// Check if result is success.
    pub fn is_ok(&self) -> bool {
        matches!(
            self,
            IoResult::Bytes(_) | IoResult::Fd(_) | IoResult::Success
        )
    }

    /// Check if result is error.
    pub fn is_err(&self) -> bool {
        matches!(self, IoResult::Error(_))
    }

    /// Get bytes if applicable.
    pub fn bytes(&self) -> Option<usize> {
        match self {
            IoResult::Bytes(n) => Some(*n),
            _ => None,
        }
    }

    /// Convert to result code for completion.
    pub fn to_result_code(&self) -> i64 {
        match self {
            IoResult::Bytes(n) => *n as i64,
            IoResult::Fd(fd) => *fd as i64,
            IoResult::Success => 0,
            IoResult::Error(e) => -(e.to_errno() as i64),
            IoResult::Pending => 0,
            IoResult::Cancelled => -125, // ECANCELED
        }
    }
}

/// I/O operation error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoOpError {
    /// Permission denied.
    PermissionDenied,
    /// No such file or directory.
    NotFound,
    /// I/O error.
    IoError,
    /// Invalid argument.
    InvalidArgument,
    /// Bad file descriptor.
    BadFd,
    /// Resource busy.
    Busy,
    /// File exists.
    Exists,
    /// Not a directory.
    NotADirectory,
    /// Is a directory.
    IsADirectory,
    /// Invalid seek.
    InvalidSeek,
    /// Too many open files.
    TooManyOpenFiles,
    /// No space left.
    NoSpace,
    /// Read-only filesystem.
    ReadOnly,
    /// Operation would block.
    WouldBlock,
    /// Interrupted.
    Interrupted,
    /// Connection refused.
    ConnectionRefused,
    /// Connection reset.
    ConnectionReset,
    /// Not connected.
    NotConnected,
    /// Already connected.
    AlreadyConnected,
    /// Timeout.
    TimedOut,
    /// Operation not supported.
    NotSupported,
    /// Operation cancelled.
    Cancelled,
    /// Unknown error.
    Unknown(i32),
}

impl IoOpError {
    /// Convert to errno.
    pub fn to_errno(&self) -> i32 {
        match self {
            IoOpError::PermissionDenied => 1,    // EPERM
            IoOpError::NotFound => 2,            // ENOENT
            IoOpError::IoError => 5,             // EIO
            IoOpError::InvalidArgument => 22,    // EINVAL
            IoOpError::BadFd => 9,               // EBADF
            IoOpError::Busy => 16,               // EBUSY
            IoOpError::Exists => 17,             // EEXIST
            IoOpError::NotADirectory => 20,      // ENOTDIR
            IoOpError::IsADirectory => 21,       // EISDIR
            IoOpError::InvalidSeek => 29,        // ESPIPE
            IoOpError::TooManyOpenFiles => 24,   // EMFILE
            IoOpError::NoSpace => 28,            // ENOSPC
            IoOpError::ReadOnly => 30,           // EROFS
            IoOpError::WouldBlock => 11,         // EAGAIN
            IoOpError::Interrupted => 4,         // EINTR
            IoOpError::ConnectionRefused => 111, // ECONNREFUSED
            IoOpError::ConnectionReset => 104,   // ECONNRESET
            IoOpError::NotConnected => 107,      // ENOTCONN
            IoOpError::AlreadyConnected => 106,  // EISCONN
            IoOpError::TimedOut => 110,          // ETIMEDOUT
            IoOpError::NotSupported => 95,       // EOPNOTSUPP
            IoOpError::Cancelled => 125,         // ECANCELED
            IoOpError::Unknown(e) => *e,
        }
    }

    /// Create from errno.
    pub fn from_errno(errno: i32) -> Self {
        match errno {
            1 => IoOpError::PermissionDenied,
            2 => IoOpError::NotFound,
            5 => IoOpError::IoError,
            22 => IoOpError::InvalidArgument,
            9 => IoOpError::BadFd,
            16 => IoOpError::Busy,
            17 => IoOpError::Exists,
            20 => IoOpError::NotADirectory,
            21 => IoOpError::IsADirectory,
            29 => IoOpError::InvalidSeek,
            24 => IoOpError::TooManyOpenFiles,
            28 => IoOpError::NoSpace,
            30 => IoOpError::ReadOnly,
            11 => IoOpError::WouldBlock,
            4 => IoOpError::Interrupted,
            111 => IoOpError::ConnectionRefused,
            104 => IoOpError::ConnectionReset,
            107 => IoOpError::NotConnected,
            106 => IoOpError::AlreadyConnected,
            110 => IoOpError::TimedOut,
            95 => IoOpError::NotSupported,
            125 => IoOpError::Cancelled,
            e => IoOpError::Unknown(e),
        }
    }
}

/// I/O operation with all parameters.
#[derive(Debug)]
pub struct IoOp {
    /// Operation code.
    pub opcode: OpCode,
    /// File descriptor.
    pub fd: i32,
    /// Buffer address.
    pub buffer: Option<IoBuffer>,
    /// Offset.
    pub offset: u64,
    /// Length.
    pub len: u32,
    /// Operation-specific flags.
    pub flags: u32,
    /// User data.
    pub user_data: u64,
}

impl IoOp {
    /// Create a read operation.
    pub fn read(fd: i32, buffer: IoBuffer, offset: u64, user_data: u64) -> Self {
        let len = buffer.len() as u32;
        Self {
            opcode: OpCode::Read,
            fd,
            buffer: Some(buffer),
            offset,
            len,
            flags: 0,
            user_data,
        }
    }

    /// Create a write operation.
    pub fn write(fd: i32, buffer: IoBuffer, offset: u64, user_data: u64) -> Self {
        let len = buffer.len() as u32;
        Self {
            opcode: OpCode::Write,
            fd,
            buffer: Some(buffer),
            offset,
            len,
            flags: 0,
            user_data,
        }
    }

    /// Create a close operation.
    pub fn close(fd: i32, user_data: u64) -> Self {
        Self {
            opcode: OpCode::Close,
            fd,
            buffer: None,
            offset: 0,
            len: 0,
            flags: 0,
            user_data,
        }
    }

    /// Create a nop operation.
    pub fn nop(user_data: u64) -> Self {
        Self {
            opcode: OpCode::Nop,
            fd: -1,
            buffer: None,
            offset: 0,
            len: 0,
            flags: 0,
            user_data,
        }
    }
}

/// I/O buffer.
#[derive(Debug)]
pub enum IoBuffer {
    /// Single buffer.
    Single { addr: u64, len: usize },
    /// Vectored buffer.
    Vectored(Vec<IoVec>),
    /// Fixed buffer (pre-registered).
    Fixed { index: u16, offset: u32, len: u32 },
}

impl IoBuffer {
    /// Get total length.
    pub fn len(&self) -> usize {
        match self {
            IoBuffer::Single { len, .. } => *len,
            IoBuffer::Vectored(vecs) => vecs.iter().map(|v| v.len).sum(),
            IoBuffer::Fixed { len, .. } => *len as usize,
        }
    }

    /// Check if buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// I/O vector.
#[derive(Debug, Clone, Copy)]
pub struct IoVec {
    /// Base address.
    pub base: u64,
    /// Length.
    pub len: usize,
}

impl IoVec {
    /// Create a new I/O vector.
    pub const fn new(base: u64, len: usize) -> Self {
        Self { base, len }
    }
}

/// Poll events.
#[derive(Debug, Clone, Copy)]
pub struct PollEvents(u32);

impl PollEvents {
    /// No events.
    pub const NONE: Self = Self(0);
    /// Readable.
    pub const POLLIN: Self = Self(0x001);
    /// Priority data readable.
    pub const POLLPRI: Self = Self(0x002);
    /// Writable.
    pub const POLLOUT: Self = Self(0x004);
    /// Error.
    pub const POLLERR: Self = Self(0x008);
    /// Hang up.
    pub const POLLHUP: Self = Self(0x010);
    /// Invalid.
    pub const POLLNVAL: Self = Self(0x020);
    /// Read half shutdown.
    pub const POLLRDHUP: Self = Self(0x2000);

    /// Create from raw value.
    pub const fn from_raw(value: u32) -> Self {
        Self(value)
    }

    /// Get raw value.
    pub const fn raw(&self) -> u32 {
        self.0
    }

    /// Check if event is set.
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Union of events.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// Open flags.
#[derive(Debug, Clone, Copy)]
pub struct OpenFlags(u32);

impl OpenFlags {
    /// Read only.
    pub const RDONLY: Self = Self(0);
    /// Write only.
    pub const WRONLY: Self = Self(1);
    /// Read/write.
    pub const RDWR: Self = Self(2);
    /// Create if not exists.
    pub const CREAT: Self = Self(0o100);
    /// Exclusive create.
    pub const EXCL: Self = Self(0o200);
    /// Truncate.
    pub const TRUNC: Self = Self(0o1000);
    /// Append.
    pub const APPEND: Self = Self(0o2000);
    /// Non-blocking.
    pub const NONBLOCK: Self = Self(0o4000);
    /// Sync.
    pub const SYNC: Self = Self(0o4010000);
    /// Directory.
    pub const DIRECTORY: Self = Self(0o200000);

    /// Create from raw value.
    pub const fn from_raw(value: u32) -> Self {
        Self(value)
    }

    /// Get raw value.
    pub const fn raw(&self) -> u32 {
        self.0
    }

    /// Check if flag is set.
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Union of flags.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
