//! Storage subsystem for KPIO.
//!
//! This module provides the storage subsystem including:
//! - Virtual Filesystem (VFS) layer
//! - FUSE protocol for WASM filesystem implementations
//! - Block device drivers (VirtIO-Blk, NVMe, AHCI)
//! - Filesystem implementations (ext4, FAT32, NTFS read-only)

#![no_std]
#![feature(allocator_api)]

extern crate alloc;

pub mod cache;
pub mod driver;
pub mod fs;
pub mod fuse;
pub mod partition;
pub mod vfs;

use alloc::string::String;
use alloc::vec::Vec;

/// Storage subsystem error types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    /// Device not found.
    DeviceNotFound,
    /// Device I/O error.
    IoError,
    /// Device not ready.
    NotReady,
    /// Invalid block number.
    InvalidBlock,
    /// Buffer too small.
    BufferTooSmall,
    /// Unsupported operation.
    Unsupported,
    /// File not found.
    FileNotFound,
    /// Directory not found.
    DirectoryNotFound,
    /// Not a directory.
    NotADirectory,
    /// Not a file.
    NotAFile,
    /// Already exists.
    AlreadyExists,
    /// Permission denied.
    PermissionDenied,
    /// Directory not empty.
    DirectoryNotEmpty,
    /// Read-only filesystem.
    ReadOnly,
    /// No space left on device.
    NoSpace,
    /// Filesystem is full.
    FilesystemFull,
    /// Invalid path.
    InvalidPath,
    /// Invalid name.
    InvalidName,
    /// Name too long.
    NameTooLong,
    /// Too many symbolic links.
    TooManySymlinks,
    /// Invalid filesystem.
    InvalidFilesystem,
    /// Filesystem corruption detected.
    Corrupted,
    /// Invalid argument.
    InvalidArgument,
    /// Operation would block.
    WouldBlock,
    /// End of file.
    EndOfFile,
    /// Not implemented.
    NotImplemented,
    /// Invalid file descriptor.
    InvalidFd,
    /// Too many open files.
    TooManyOpenFiles,
    /// Cross-device link.
    CrossDeviceLink,
}

/// Block device information.
#[derive(Debug, Clone)]
pub struct BlockDeviceInfo {
    /// Device name.
    pub name: [u8; 32],
    /// Name length.
    pub name_len: usize,
    /// Block size in bytes.
    pub block_size: u32,
    /// Total number of blocks.
    pub total_blocks: u64,
    /// Device is read-only.
    pub read_only: bool,
    /// Device supports TRIM/DISCARD.
    pub supports_trim: bool,
    /// Optimal I/O size in blocks.
    pub optimal_io_size: u32,
    /// Physical sector size.
    pub physical_block_size: u32,
}

impl BlockDeviceInfo {
    /// Get the total capacity in bytes.
    pub fn capacity(&self) -> u64 {
        self.total_blocks * self.block_size as u64
    }

    /// Get the device name as a string.
    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("unknown")
    }
}

/// File type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// Regular file.
    Regular,
    /// Directory.
    Directory,
    /// Symbolic link.
    Symlink,
    /// Block device.
    BlockDevice,
    /// Character device.
    CharDevice,
    /// FIFO (named pipe).
    Fifo,
    /// Socket.
    Socket,
    /// Unknown type.
    Unknown,
}

/// File permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilePermissions(pub u16);

impl FilePermissions {
    /// Owner read permission.
    pub const OWNER_READ: u16 = 0o400;
    /// Owner write permission.
    pub const OWNER_WRITE: u16 = 0o200;
    /// Owner execute permission.
    pub const OWNER_EXEC: u16 = 0o100;
    /// Group read permission.
    pub const GROUP_READ: u16 = 0o040;
    /// Group write permission.
    pub const GROUP_WRITE: u16 = 0o020;
    /// Group execute permission.
    pub const GROUP_EXEC: u16 = 0o010;
    /// Others read permission.
    pub const OTHER_READ: u16 = 0o004;
    /// Others write permission.
    pub const OTHER_WRITE: u16 = 0o002;
    /// Others execute permission.
    pub const OTHER_EXEC: u16 = 0o001;

    /// Default file permissions (rw-r--r--).
    pub const DEFAULT_FILE: Self = FilePermissions(0o644);
    /// Default directory permissions (rwxr-xr-x).
    pub const DEFAULT_DIR: Self = FilePermissions(0o755);

    /// Check if owner can read.
    pub fn owner_can_read(&self) -> bool {
        self.0 & Self::OWNER_READ != 0
    }

    /// Check if owner can write.
    pub fn owner_can_write(&self) -> bool {
        self.0 & Self::OWNER_WRITE != 0
    }

    /// Check if owner can execute.
    pub fn owner_can_exec(&self) -> bool {
        self.0 & Self::OWNER_EXEC != 0
    }
}

/// File metadata/attributes.
#[derive(Debug, Clone)]
pub struct FileMetadata {
    /// File type.
    pub file_type: FileType,
    /// File permissions.
    pub permissions: FilePermissions,
    /// File size in bytes.
    pub size: u64,
    /// Number of hard links.
    pub nlink: u32,
    /// Owner user ID.
    pub uid: u32,
    /// Owner group ID.
    pub gid: u32,
    /// Block size for filesystem I/O.
    pub block_size: u32,
    /// Number of blocks allocated.
    pub blocks: u64,
    /// Access time (nanoseconds since epoch).
    pub atime: u64,
    /// Modification time (nanoseconds since epoch).
    pub mtime: u64,
    /// Status change time (nanoseconds since epoch).
    pub ctime: u64,
    /// Creation time (nanoseconds since epoch).
    pub crtime: u64,
    /// Inode number.
    pub inode: u64,
    /// Device ID.
    pub dev: u64,
    /// Device ID (for special files).
    pub rdev: u64,
}

impl Default for FileMetadata {
    fn default() -> Self {
        FileMetadata {
            file_type: FileType::Regular,
            permissions: FilePermissions::DEFAULT_FILE,
            size: 0,
            nlink: 1,
            uid: 0,
            gid: 0,
            block_size: 4096,
            blocks: 0,
            atime: 0,
            mtime: 0,
            ctime: 0,
            crtime: 0,
            inode: 0,
            dev: 0,
            rdev: 0,
        }
    }
}

/// Directory entry.
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Entry name.
    pub name: [u8; 256],
    /// Name length.
    pub name_len: usize,
    /// Inode number.
    pub inode: u64,
    /// File type.
    pub file_type: FileType,
}

impl DirEntry {
    /// Get the name as a string.
    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }

    /// Check if this is a dot entry (. or ..).
    pub fn is_dot_entry(&self) -> bool {
        let name = self.name_str();
        name == "." || name == ".."
    }
}

/// Open file flags.
bitflags::bitflags! {
    /// Flags for opening files.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OpenFlags: u32 {
        /// Open for reading.
        const READ = 0x0001;
        /// Open for writing.
        const WRITE = 0x0002;
        /// Create if not exists.
        const CREATE = 0x0004;
        /// Truncate to zero length.
        const TRUNCATE = 0x0008;
        /// Append mode.
        const APPEND = 0x0010;
        /// Exclusive create (fail if exists).
        const EXCLUSIVE = 0x0020;
        /// Don't follow symbolic links.
        const NOFOLLOW = 0x0040;
        /// Directory open (fail if not directory).
        const DIRECTORY = 0x0080;
        /// Non-blocking mode.
        const NONBLOCK = 0x0100;
        /// Synchronous I/O.
        const SYNC = 0x0200;
        /// Data synchronous I/O.
        const DSYNC = 0x0400;
    }
}

/// Seek origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekFrom {
    /// Seek from the beginning of the file.
    Start(u64),
    /// Seek from the end of the file.
    End(i64),
    /// Seek from the current position.
    Current(i64),
}

/// Mount flags.
bitflags::bitflags! {
    /// Flags for mounting filesystems.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MountFlags: u32 {
        /// Mount read-only.
        const READ_ONLY = 0x0001;
        /// Don't update access times.
        const NOATIME = 0x0002;
        /// Don't update directory access times.
        const NODIRATIME = 0x0004;
        /// Don't allow set-user-ID execution.
        const NOSUID = 0x0008;
        /// Don't interpret special files.
        const NODEV = 0x0010;
        /// Don't allow execution.
        const NOEXEC = 0x0020;
        /// Synchronous I/O.
        const SYNC = 0x0040;
        /// Mandatory locking.
        const MANDLOCK = 0x0080;
        /// Silent mount.
        const SILENT = 0x0100;
        /// Update access time relative to modify time.
        const RELATIME = 0x0200;
    }
}

/// Storage subsystem initialization.
pub fn init() -> Result<(), StorageError> {
    // Initialize block device drivers
    driver::init()?;

    // Initialize VFS
    vfs::init()?;

    // Initialize cache
    cache::init()?;

    Ok(())
}

/// Mount a filesystem.
pub fn mount(
    device: &str,
    mount_point: &str,
    fs_type: &str,
    flags: MountFlags,
) -> Result<(), StorageError> {
    vfs::mount(device, mount_point, fs_type, flags)
}

/// Unmount a filesystem.
pub fn unmount(mount_point: &str) -> Result<(), StorageError> {
    vfs::unmount(mount_point)
}

/// List all mounted filesystems.
pub fn list_mounts() -> Vec<vfs::MountInfo> {
    vfs::list_mounts()
}

/// Get filesystem statistics.
pub fn statfs(path: &str) -> Result<vfs::FsStats, StorageError> {
    vfs::statfs(path)
}
