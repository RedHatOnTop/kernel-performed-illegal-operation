//! WASI Preview 2 implementation.
//!
//! This module provides WASI (WebAssembly System Interface) Preview 2
//! support for WASM modules running in the KPIO kernel.

use alloc::string::String;
use alloc::vec;
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
    
    /// Get a mutable file descriptor.
    pub fn get_fd_mut(&mut self, fd: u32) -> Option<&mut FileDescriptor> {
        self.fds.get_mut(&fd)
    }
    
    /// Close a file descriptor.
    pub fn close_fd(&mut self, fd: u32) -> Result<(), WasiError> {
        self.fds.remove(&fd).ok_or(WasiError::BadF)?;
        Ok(())
    }
    
    // ==================== WASI Preview 2 Functions ====================
    
    /// fd_read - Read from file descriptor.
    pub fn fd_read(&mut self, fd: u32, buf: &mut [u8]) -> Result<usize, WasiError> {
        let file = self.fds.get_mut(&fd).ok_or(WasiError::BadF)?;
        
        if !file.rights.contains(FdRights::READ) {
            return Err(WasiError::Access);
        }
        
        match file.fd_type {
            FdType::CharDevice if fd == 0 => {
                // stdin - return 0 for now (no input)
                Ok(0)
            }
            FdType::RegularFile => {
                // In real implementation, read from VFS
                // For now, return mock data or 0
                let bytes_read = buf.len().min(64);
                file.offset += bytes_read as u64;
                Ok(bytes_read)
            }
            _ => Err(WasiError::BadF),
        }
    }
    
    /// fd_write - Write to file descriptor.
    pub fn fd_write(&mut self, fd: u32, buf: &[u8]) -> Result<usize, WasiError> {
        let file = self.fds.get_mut(&fd).ok_or(WasiError::BadF)?;
        
        if !file.rights.contains(FdRights::WRITE) {
            return Err(WasiError::Access);
        }
        
        match file.fd_type {
            FdType::CharDevice if fd == 1 || fd == 2 => {
                // stdout/stderr - write to serial/console
                // In real implementation, send to console
                Ok(buf.len())
            }
            FdType::RegularFile => {
                // In real implementation, write to VFS
                file.offset += buf.len() as u64;
                Ok(buf.len())
            }
            _ => Err(WasiError::BadF),
        }
    }
    
    /// fd_seek - Seek in file descriptor.
    pub fn fd_seek(&mut self, fd: u32, offset: i64, whence: Whence) -> Result<u64, WasiError> {
        let file = self.fds.get_mut(&fd).ok_or(WasiError::BadF)?;
        
        if !file.rights.contains(FdRights::SEEK) {
            return Err(WasiError::Access);
        }
        
        match file.fd_type {
            FdType::RegularFile => {
                let new_offset = match whence {
                    Whence::Set => offset as u64,
                    Whence::Cur => {
                        if offset < 0 {
                            file.offset.saturating_sub((-offset) as u64)
                        } else {
                            file.offset.saturating_add(offset as u64)
                        }
                    }
                    Whence::End => {
                        // In real impl, get file size and add offset
                        0 // placeholder
                    }
                };
                file.offset = new_offset;
                Ok(new_offset)
            }
            _ => Err(WasiError::SPipe),
        }
    }
    
    /// fd_tell - Get current offset.
    pub fn fd_tell(&self, fd: u32) -> Result<u64, WasiError> {
        let file = self.fds.get(&fd).ok_or(WasiError::BadF)?;
        
        if !file.rights.contains(FdRights::TELL) {
            return Err(WasiError::Access);
        }
        
        Ok(file.offset)
    }
    
    /// fd_close - Close file descriptor.
    pub fn fd_close(&mut self, fd: u32) -> Result<(), WasiError> {
        // Prevent closing stdin/stdout/stderr
        if fd < 3 {
            return Err(WasiError::BadF);
        }
        self.close_fd(fd)
    }
    
    /// fd_fdstat_get - Get file descriptor status.
    pub fn fd_fdstat_get(&self, fd: u32) -> Result<FdStat, WasiError> {
        let file = self.fds.get(&fd).ok_or(WasiError::BadF)?;
        
        Ok(FdStat {
            fs_filetype: file.fd_type,
            fs_flags: FdFlags::empty(),
            fs_rights_base: file.rights,
            fs_rights_inheriting: file.rights,
        })
    }
    
    /// fd_prestat_get - Get preopened directory info.
    pub fn fd_prestat_get(&self, fd: u32) -> Result<Prestat, WasiError> {
        let file = self.fds.get(&fd).ok_or(WasiError::BadF)?;
        
        match file.fd_type {
            FdType::Directory => {
                let path_len = file.path.as_ref().map(|p| p.len()).unwrap_or(0);
                Ok(Prestat {
                    tag: PrestatTag::Dir,
                    inner: PrestatInner { dir_name_len: path_len },
                })
            }
            _ => Err(WasiError::BadF),
        }
    }
    
    /// fd_prestat_dir_name - Get preopened directory name.
    pub fn fd_prestat_dir_name(&self, fd: u32, buf: &mut [u8]) -> Result<(), WasiError> {
        let file = self.fds.get(&fd).ok_or(WasiError::BadF)?;
        
        match file.fd_type {
            FdType::Directory => {
                if let Some(path) = &file.path {
                    let bytes = path.as_bytes();
                    let len = bytes.len().min(buf.len());
                    buf[..len].copy_from_slice(&bytes[..len]);
                }
                Ok(())
            }
            _ => Err(WasiError::BadF),
        }
    }
    
    /// path_open - Open a file.
    pub fn path_open(
        &mut self,
        dir_fd: u32,
        _dirflags: LookupFlags,
        path: &str,
        oflags: OFlags,
        rights: FdRights,
        _inheriting_rights: FdRights,
        _fdflags: FdFlags,
    ) -> Result<u32, WasiError> {
        let dir = self.fds.get(&dir_fd).ok_or(WasiError::BadF)?;
        
        if dir.fd_type != FdType::Directory {
            return Err(WasiError::NotDir);
        }
        
        if !dir.rights.contains(FdRights::PATH_OPEN) {
            return Err(WasiError::Access);
        }
        
        // Resolve path relative to directory
        let full_path = if path.starts_with('/') {
            path.into()
        } else {
            let dir_path = dir.path.as_deref().unwrap_or("/");
            if dir_path.ends_with('/') {
                alloc::format!("{}{}", dir_path, path)
            } else {
                alloc::format!("{}/{}", dir_path, path)
            }
        };
        
        // Determine if creating or opening
        let fd_type = if oflags.contains(OFlags::DIRECTORY) {
            FdType::Directory
        } else {
            FdType::RegularFile
        };
        
        // Create file descriptor
        let new_fd = FileDescriptor {
            fd_type,
            rights,
            offset: 0,
            path: Some(full_path),
        };
        
        Ok(self.alloc_fd(new_fd))
    }
    
    /// path_create_directory - Create directory.
    pub fn path_create_directory(&mut self, dir_fd: u32, path: &str) -> Result<(), WasiError> {
        let dir = self.fds.get(&dir_fd).ok_or(WasiError::BadF)?;
        
        if dir.fd_type != FdType::Directory {
            return Err(WasiError::NotDir);
        }
        
        if !dir.rights.contains(FdRights::PATH_CREATE_DIR) {
            return Err(WasiError::Access);
        }
        
        // In real impl, create directory via VFS
        let _ = path;
        Ok(())
    }
    
    /// path_remove_directory - Remove directory.
    pub fn path_remove_directory(&mut self, dir_fd: u32, path: &str) -> Result<(), WasiError> {
        let dir = self.fds.get(&dir_fd).ok_or(WasiError::BadF)?;
        
        if dir.fd_type != FdType::Directory {
            return Err(WasiError::NotDir);
        }
        
        if !dir.rights.contains(FdRights::PATH_REMOVE) {
            return Err(WasiError::Access);
        }
        
        // In real impl, remove directory via VFS
        let _ = path;
        Ok(())
    }
    
    /// path_unlink_file - Remove file.
    pub fn path_unlink_file(&mut self, dir_fd: u32, path: &str) -> Result<(), WasiError> {
        let dir = self.fds.get(&dir_fd).ok_or(WasiError::BadF)?;
        
        if dir.fd_type != FdType::Directory {
            return Err(WasiError::NotDir);
        }
        
        if !dir.rights.contains(FdRights::PATH_REMOVE) {
            return Err(WasiError::Access);
        }
        
        // In real impl, remove file via VFS
        let _ = path;
        Ok(())
    }
    
    /// path_rename - Rename file or directory.
    pub fn path_rename(
        &mut self, 
        old_dir_fd: u32, 
        old_path: &str,
        new_dir_fd: u32,
        new_path: &str,
    ) -> Result<(), WasiError> {
        let old_dir = self.fds.get(&old_dir_fd).ok_or(WasiError::BadF)?;
        let new_dir = self.fds.get(&new_dir_fd).ok_or(WasiError::BadF)?;
        
        if old_dir.fd_type != FdType::Directory || new_dir.fd_type != FdType::Directory {
            return Err(WasiError::NotDir);
        }
        
        if !old_dir.rights.contains(FdRights::PATH_RENAME) {
            return Err(WasiError::Access);
        }
        
        // In real impl, rename via VFS
        let _ = (old_path, new_path);
        Ok(())
    }
    
    /// fd_readdir - Read directory entries.
    pub fn fd_readdir(&mut self, fd: u32, buf: &mut [u8], cookie: u64) -> Result<usize, WasiError> {
        let file = self.fds.get(&fd).ok_or(WasiError::BadF)?;
        
        if file.fd_type != FdType::Directory {
            return Err(WasiError::NotDir);
        }
        
        if !file.rights.contains(FdRights::READ) {
            return Err(WasiError::Access);
        }
        
        // In real impl, read directory via VFS
        let _ = (buf, cookie);
        Ok(0)
    }
    
    /// args_get - Get command line arguments.
    pub fn args_get(&self) -> &[String] {
        &self.args
    }
    
    /// args_sizes_get - Get argument buffer sizes.
    pub fn args_sizes_get(&self) -> (usize, usize) {
        let count = self.args.len();
        let total_size: usize = self.args.iter().map(|s| s.len() + 1).sum();
        (count, total_size)
    }
    
    /// environ_get - Get environment variables.
    pub fn environ_get(&self) -> Vec<(String, String)> {
        self.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
    
    /// environ_sizes_get - Get environment buffer sizes.
    pub fn environ_sizes_get(&self) -> (usize, usize) {
        let count = self.env.len();
        let total_size: usize = self.env.iter()
            .map(|(k, v)| k.len() + 1 + v.len() + 1)
            .sum();
        (count, total_size)
    }
    
    /// clock_time_get - Get current time.
    pub fn clock_time_get(&self, clock_id: ClockId, _precision: u64) -> Result<u64, WasiError> {
        // In real impl, get time from kernel
        match clock_id {
            ClockId::Realtime => Ok(0), // placeholder
            ClockId::Monotonic => Ok(0), // placeholder
            ClockId::ProcessCputime => Ok(0),
            ClockId::ThreadCputime => Ok(0),
        }
    }
    
    /// proc_exit - Exit the process.
    pub fn proc_exit(&mut self, code: u32) -> ! {
        self.exit_code = Some(code);
        // In real impl, terminate the WASM instance
        panic!("WASI proc_exit called with code {}", code);
    }
    
    /// random_get - Get random bytes.
    pub fn random_get(&self, buf: &mut [u8]) -> Result<(), WasiError> {
        // Simple PRNG for now
        let mut state: u64 = 0x12345678_9ABCDEF0;
        for byte in buf.iter_mut() {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            *byte = state as u8;
        }
        Ok(())
    }
    
    /// Preopen a directory.
    pub fn preopen_dir(&mut self, path: &str) -> u32 {
        let fd = FileDescriptor::directory(path.into());
        self.alloc_fd(fd)
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

/// Seek whence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whence {
    /// Seek from beginning.
    Set = 0,
    /// Seek from current position.
    Cur = 1,
    /// Seek from end.
    End = 2,
}

bitflags::bitflags! {
    /// File descriptor flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FdFlags: u16 {
        const APPEND = 1 << 0;
        const DSYNC = 1 << 1;
        const NONBLOCK = 1 << 2;
        const RSYNC = 1 << 3;
        const SYNC = 1 << 4;
    }
}

bitflags::bitflags! {
    /// Lookup flags for path operations.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LookupFlags: u32 {
        const SYMLINK_FOLLOW = 1 << 0;
    }
}

bitflags::bitflags! {
    /// Open flags for path_open.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OFlags: u16 {
        const CREAT = 1 << 0;
        const DIRECTORY = 1 << 1;
        const EXCL = 1 << 2;
        const TRUNC = 1 << 3;
    }
}

/// File descriptor stat.
#[derive(Debug, Clone)]
pub struct FdStat {
    pub fs_filetype: FdType,
    pub fs_flags: FdFlags,
    pub fs_rights_base: FdRights,
    pub fs_rights_inheriting: FdRights,
}

/// Prestat tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrestatTag {
    Dir = 0,
}

/// Prestat inner data.
#[repr(C)]
#[derive(Clone, Copy)]
pub union PrestatInner {
    pub dir_name_len: usize,
}

impl core::fmt::Debug for PrestatInner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Safety: All union fields have the same size
        write!(f, "PrestatInner {{ dir_name_len: {} }}", unsafe { self.dir_name_len })
    }
}

/// Prestat.
#[derive(Debug, Clone, Copy)]
pub struct Prestat {
    pub tag: PrestatTag,
    pub inner: PrestatInner,
}

/// Clock ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockId {
    Realtime = 0,
    Monotonic = 1,
    ProcessCputime = 2,
    ThreadCputime = 3,
}

impl ClockId {
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(ClockId::Realtime),
            1 => Some(ClockId::Monotonic),
            2 => Some(ClockId::ProcessCputime),
            3 => Some(ClockId::ThreadCputime),
            _ => None,
        }
    }
}

/// Directory entry.
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Inode number.
    pub d_ino: u64,
    /// Offset to next entry.
    pub d_next: u64,
    /// Name length.
    pub d_namlen: u32,
    /// Entry type.
    pub d_type: FdType,
    /// Entry name.
    pub name: String,
}

/// File stat.
#[derive(Debug, Clone, Default)]
pub struct FileStat {
    pub dev: u64,
    pub ino: u64,
    pub filetype: u8,
    pub nlink: u64,
    pub size: u64,
    pub atim: u64,
    pub mtim: u64,
    pub ctim: u64,
}
