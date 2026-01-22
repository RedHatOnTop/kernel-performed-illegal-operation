//! FUSE (Filesystem in Userspace) protocol implementation.
//!
//! This module provides FUSE protocol support for implementing filesystems
//! in WASM modules, allowing user-space filesystem drivers.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::{DirEntry, FileMetadata, MountFlags, OpenFlags, StorageError};
use crate::vfs::{Filesystem, FsStats};

/// FUSE operation codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum FuseOpcode {
    /// Lookup a directory entry.
    Lookup = 1,
    /// Forget about an inode.
    Forget = 2,
    /// Get file attributes.
    Getattr = 3,
    /// Set file attributes.
    Setattr = 4,
    /// Read symbolic link.
    Readlink = 5,
    /// Create a symbolic link.
    Symlink = 6,
    /// Create a file node.
    Mknod = 8,
    /// Create a directory.
    Mkdir = 9,
    /// Remove a file.
    Unlink = 10,
    /// Remove a directory.
    Rmdir = 11,
    /// Rename a file.
    Rename = 12,
    /// Create a hard link.
    Link = 13,
    /// Open a file.
    Open = 14,
    /// Read data.
    Read = 15,
    /// Write data.
    Write = 16,
    /// Get filesystem statistics.
    Statfs = 17,
    /// Release an open file.
    Release = 18,
    /// Synchronize file contents.
    Fsync = 20,
    /// Set an extended attribute.
    Setxattr = 21,
    /// Get an extended attribute.
    Getxattr = 22,
    /// List extended attributes.
    Listxattr = 23,
    /// Remove an extended attribute.
    Removexattr = 24,
    /// Flush data.
    Flush = 25,
    /// Initialize filesystem.
    Init = 26,
    /// Open a directory.
    Opendir = 27,
    /// Read directory.
    Readdir = 28,
    /// Release an open directory.
    Releasedir = 29,
    /// Synchronize directory contents.
    Fsyncdir = 30,
    /// Test for a POSIX file lock.
    Getlk = 31,
    /// Acquire, modify or release a POSIX file lock.
    Setlk = 32,
    /// Acquire, modify or release a POSIX file lock (blocking).
    Setlkw = 33,
    /// Check file access permissions.
    Access = 34,
    /// Create and open a file.
    Create = 35,
    /// Interrupt an operation.
    Interrupt = 36,
    /// Map block index within file to block index within device.
    Bmap = 37,
    /// Clean up filesystem.
    Destroy = 38,
    /// Ioctl.
    Ioctl = 39,
    /// Poll for IO readiness.
    Poll = 40,
    /// Notify reply.
    NotifyReply = 41,
    /// Batch forget.
    BatchForget = 42,
    /// Allocate requested space.
    Fallocate = 43,
    /// Read directory with attributes.
    Readdirplus = 44,
    /// Rename with flags.
    Rename2 = 45,
    /// Find next data or hole after offset.
    Lseek = 46,
    /// Copy file data.
    CopyFileRange = 47,
}

/// FUSE request header.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct FuseInHeader {
    /// Request length.
    pub len: u32,
    /// Operation code.
    pub opcode: u32,
    /// Unique request ID.
    pub unique: u64,
    /// Node ID.
    pub nodeid: u64,
    /// User ID.
    pub uid: u32,
    /// Group ID.
    pub gid: u32,
    /// Process ID.
    pub pid: u32,
    /// Padding.
    pub padding: u32,
}

impl FuseInHeader {
    /// Header size in bytes.
    pub const SIZE: usize = 40;

    /// Parse from bytes.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }

        Some(FuseInHeader {
            len: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            opcode: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            unique: u64::from_le_bytes([data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15]]),
            nodeid: u64::from_le_bytes([data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23]]),
            uid: u32::from_le_bytes([data[24], data[25], data[26], data[27]]),
            gid: u32::from_le_bytes([data[28], data[29], data[30], data[31]]),
            pid: u32::from_le_bytes([data[32], data[33], data[34], data[35]]),
            padding: u32::from_le_bytes([data[36], data[37], data[38], data[39]]),
        })
    }
}

/// FUSE response header.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct FuseOutHeader {
    /// Response length.
    pub len: u32,
    /// Error code (negative errno or 0).
    pub error: i32,
    /// Unique request ID (from request).
    pub unique: u64,
}

impl FuseOutHeader {
    /// Header size in bytes.
    pub const SIZE: usize = 16;

    /// Serialize to bytes.
    pub fn to_bytes(&self, buffer: &mut [u8]) -> usize {
        if buffer.len() < Self::SIZE {
            return 0;
        }

        buffer[0..4].copy_from_slice(&self.len.to_le_bytes());
        buffer[4..8].copy_from_slice(&self.error.to_le_bytes());
        buffer[8..16].copy_from_slice(&self.unique.to_le_bytes());

        Self::SIZE
    }
}

/// FUSE init request.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct FuseInitIn {
    /// Major version.
    pub major: u32,
    /// Minor version.
    pub minor: u32,
    /// Maximum readahead.
    pub max_readahead: u32,
    /// Flags.
    pub flags: u32,
}

/// FUSE init response.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct FuseInitOut {
    /// Major version.
    pub major: u32,
    /// Minor version.
    pub minor: u32,
    /// Maximum readahead.
    pub max_readahead: u32,
    /// Flags.
    pub flags: u32,
    /// Maximum background requests.
    pub max_background: u16,
    /// Congestion threshold.
    pub congestion_threshold: u16,
    /// Maximum write size.
    pub max_write: u32,
    /// Time granularity (ns).
    pub time_gran: u32,
    /// Maximum pages in a request.
    pub max_pages: u16,
    /// Padding.
    pub padding: u16,
    /// Reserved.
    pub reserved: [u32; 8],
}

/// FUSE attribute structure.
#[derive(Debug, Clone, Default)]
#[repr(C)]
pub struct FuseAttr {
    /// Inode.
    pub ino: u64,
    /// Size.
    pub size: u64,
    /// Blocks.
    pub blocks: u64,
    /// Access time (seconds).
    pub atime: u64,
    /// Modification time (seconds).
    pub mtime: u64,
    /// Change time (seconds).
    pub ctime: u64,
    /// Access time (nanoseconds).
    pub atimensec: u32,
    /// Modification time (nanoseconds).
    pub mtimensec: u32,
    /// Change time (nanoseconds).
    pub ctimensec: u32,
    /// Mode.
    pub mode: u32,
    /// Number of links.
    pub nlink: u32,
    /// User ID.
    pub uid: u32,
    /// Group ID.
    pub gid: u32,
    /// Device.
    pub rdev: u32,
    /// Block size.
    pub blksize: u32,
    /// Padding.
    pub padding: u32,
}

impl FuseAttr {
    /// Size in bytes.
    pub const SIZE: usize = 88;

    /// Convert to FileMetadata.
    pub fn to_metadata(&self) -> FileMetadata {
        use crate::FileType;

        let file_type = match self.mode & 0o170000 {
            0o040000 => FileType::Directory,
            0o120000 => FileType::Symlink,
            0o060000 => FileType::BlockDevice,
            0o020000 => FileType::CharDevice,
            0o010000 => FileType::Fifo,
            0o140000 => FileType::Socket,
            _ => FileType::Regular,
        };

        FileMetadata {
            file_type,
            permissions: crate::FilePermissions((self.mode & 0o777) as u16),
            size: self.size,
            nlink: self.nlink,
            uid: self.uid,
            gid: self.gid,
            block_size: self.blksize,
            blocks: self.blocks,
            atime: self.atime * 1_000_000_000 + self.atimensec as u64,
            mtime: self.mtime * 1_000_000_000 + self.mtimensec as u64,
            ctime: self.ctime * 1_000_000_000 + self.ctimensec as u64,
            crtime: 0,
            inode: self.ino,
            dev: 0,
            rdev: self.rdev as u64,
        }
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self, buffer: &mut [u8]) -> usize {
        if buffer.len() < Self::SIZE {
            return 0;
        }

        buffer[0..8].copy_from_slice(&self.ino.to_le_bytes());
        buffer[8..16].copy_from_slice(&self.size.to_le_bytes());
        buffer[16..24].copy_from_slice(&self.blocks.to_le_bytes());
        buffer[24..32].copy_from_slice(&self.atime.to_le_bytes());
        buffer[32..40].copy_from_slice(&self.mtime.to_le_bytes());
        buffer[40..48].copy_from_slice(&self.ctime.to_le_bytes());
        buffer[48..52].copy_from_slice(&self.atimensec.to_le_bytes());
        buffer[52..56].copy_from_slice(&self.mtimensec.to_le_bytes());
        buffer[56..60].copy_from_slice(&self.ctimensec.to_le_bytes());
        buffer[60..64].copy_from_slice(&self.mode.to_le_bytes());
        buffer[64..68].copy_from_slice(&self.nlink.to_le_bytes());
        buffer[68..72].copy_from_slice(&self.uid.to_le_bytes());
        buffer[72..76].copy_from_slice(&self.gid.to_le_bytes());
        buffer[76..80].copy_from_slice(&self.rdev.to_le_bytes());
        buffer[80..84].copy_from_slice(&self.blksize.to_le_bytes());
        buffer[84..88].copy_from_slice(&self.padding.to_le_bytes());

        Self::SIZE
    }
}

/// FUSE entry response.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct FuseEntryOut {
    /// Inode ID.
    pub nodeid: u64,
    /// Generation number.
    pub generation: u64,
    /// Cache timeout for entry (seconds).
    pub entry_valid: u64,
    /// Cache timeout for attributes (seconds).
    pub attr_valid: u64,
    /// Cache timeout for entry (nanoseconds).
    pub entry_valid_nsec: u32,
    /// Cache timeout for attributes (nanoseconds).
    pub attr_valid_nsec: u32,
    /// Attributes.
    pub attr: FuseAttr,
}

/// FUSE open response.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct FuseOpenOut {
    /// File handle.
    pub fh: u64,
    /// Open flags.
    pub open_flags: u32,
    /// Padding.
    pub padding: u32,
}

/// FUSE read request.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct FuseReadIn {
    /// File handle.
    pub fh: u64,
    /// Offset.
    pub offset: u64,
    /// Size.
    pub size: u32,
    /// Read flags.
    pub read_flags: u32,
    /// Lock owner.
    pub lock_owner: u64,
    /// Flags.
    pub flags: u32,
    /// Padding.
    pub padding: u32,
}

/// FUSE write request.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct FuseWriteIn {
    /// File handle.
    pub fh: u64,
    /// Offset.
    pub offset: u64,
    /// Size.
    pub size: u32,
    /// Write flags.
    pub write_flags: u32,
    /// Lock owner.
    pub lock_owner: u64,
    /// Flags.
    pub flags: u32,
    /// Padding.
    pub padding: u32,
}

/// FUSE write response.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct FuseWriteOut {
    /// Bytes written.
    pub size: u32,
    /// Padding.
    pub padding: u32,
}

/// FUSE filesystem implementation.
pub struct FuseFilesystem {
    /// Filesystem name.
    name: [u8; 32],
    /// Name length.
    name_len: usize,
    /// FUSE major version.
    major: u32,
    /// FUSE minor version.
    minor: u32,
    /// Maximum read size.
    max_read: u32,
    /// Maximum write size.
    max_write: u32,
    /// Request buffer.
    request_buffer: [u8; 65536],
    /// Response buffer.
    response_buffer: [u8; 65536],
}

impl FuseFilesystem {
    /// FUSE protocol major version.
    pub const FUSE_KERNEL_VERSION: u32 = 7;
    /// FUSE protocol minor version.
    pub const FUSE_KERNEL_MINOR_VERSION: u32 = 31;

    /// Create a new FUSE filesystem.
    pub fn new(name: &str) -> Self {
        let mut name_buf = [0u8; 32];
        let name_bytes = name.as_bytes();
        let len = name_bytes.len().min(31);
        name_buf[..len].copy_from_slice(&name_bytes[..len]);

        FuseFilesystem {
            name: name_buf,
            name_len: len,
            major: Self::FUSE_KERNEL_VERSION,
            minor: Self::FUSE_KERNEL_MINOR_VERSION,
            max_read: 65536,
            max_write: 65536,
            request_buffer: [0; 65536],
            response_buffer: [0; 65536],
        }
    }

    /// Process a FUSE request.
    pub fn process_request(&mut self, request: &[u8]) -> Result<&[u8], StorageError> {
        let header = FuseInHeader::from_bytes(request)
            .ok_or(StorageError::InvalidArgument)?;

        let opcode = match header.opcode {
            1 => FuseOpcode::Lookup,
            2 => FuseOpcode::Forget,
            3 => FuseOpcode::Getattr,
            4 => FuseOpcode::Setattr,
            5 => FuseOpcode::Readlink,
            14 => FuseOpcode::Open,
            15 => FuseOpcode::Read,
            16 => FuseOpcode::Write,
            17 => FuseOpcode::Statfs,
            18 => FuseOpcode::Release,
            26 => FuseOpcode::Init,
            27 => FuseOpcode::Opendir,
            28 => FuseOpcode::Readdir,
            29 => FuseOpcode::Releasedir,
            _ => return Err(StorageError::NotImplemented),
        };

        let response_len = match opcode {
            FuseOpcode::Init => self.handle_init(&header, &request[FuseInHeader::SIZE..])?,
            FuseOpcode::Lookup => self.handle_lookup(&header, &request[FuseInHeader::SIZE..])?,
            FuseOpcode::Getattr => self.handle_getattr(&header)?,
            FuseOpcode::Open | FuseOpcode::Opendir => self.handle_open(&header)?,
            FuseOpcode::Read => self.handle_read(&header, &request[FuseInHeader::SIZE..])?,
            FuseOpcode::Readdir => self.handle_readdir(&header, &request[FuseInHeader::SIZE..])?,
            FuseOpcode::Release | FuseOpcode::Releasedir => self.handle_release(&header)?,
            FuseOpcode::Statfs => self.handle_statfs(&header)?,
            _ => self.send_error(&header, -38)?, // ENOSYS
        };

        Ok(&self.response_buffer[..response_len])
    }

    /// Send an error response.
    fn send_error(&mut self, header: &FuseInHeader, error: i32) -> Result<usize, StorageError> {
        let out_header = FuseOutHeader {
            len: FuseOutHeader::SIZE as u32,
            error,
            unique: header.unique,
        };

        Ok(out_header.to_bytes(&mut self.response_buffer))
    }

    /// Handle FUSE_INIT.
    fn handle_init(&mut self, header: &FuseInHeader, _data: &[u8]) -> Result<usize, StorageError> {
        let out_header = FuseOutHeader {
            len: (FuseOutHeader::SIZE + 64) as u32, // Size of FuseInitOut
            error: 0,
            unique: header.unique,
        };

        let mut offset = out_header.to_bytes(&mut self.response_buffer);

        // FuseInitOut
        self.response_buffer[offset..offset + 4].copy_from_slice(&self.major.to_le_bytes());
        offset += 4;
        self.response_buffer[offset..offset + 4].copy_from_slice(&self.minor.to_le_bytes());
        offset += 4;
        self.response_buffer[offset..offset + 4].copy_from_slice(&self.max_read.to_le_bytes());
        offset += 4;
        self.response_buffer[offset..offset + 4].copy_from_slice(&0u32.to_le_bytes()); // flags
        offset += 4;
        self.response_buffer[offset..offset + 2].copy_from_slice(&16u16.to_le_bytes()); // max_background
        offset += 2;
        self.response_buffer[offset..offset + 2].copy_from_slice(&12u16.to_le_bytes()); // congestion_threshold
        offset += 2;
        self.response_buffer[offset..offset + 4].copy_from_slice(&self.max_write.to_le_bytes());
        offset += 4;
        // ... rest is zeros

        Ok(FuseOutHeader::SIZE + 64)
    }

    /// Handle FUSE_LOOKUP.
    fn handle_lookup(&mut self, header: &FuseInHeader, _data: &[u8]) -> Result<usize, StorageError> {
        // TODO: Implement actual lookup
        self.send_error(header, -2) // ENOENT
    }

    /// Handle FUSE_GETATTR.
    fn handle_getattr(&mut self, header: &FuseInHeader) -> Result<usize, StorageError> {
        let out_header = FuseOutHeader {
            len: (FuseOutHeader::SIZE + 16 + FuseAttr::SIZE) as u32,
            error: 0,
            unique: header.unique,
        };

        let mut offset = out_header.to_bytes(&mut self.response_buffer);

        // attr_valid, attr_valid_nsec
        self.response_buffer[offset..offset + 8].copy_from_slice(&1u64.to_le_bytes());
        offset += 8;
        self.response_buffer[offset..offset + 4].copy_from_slice(&0u32.to_le_bytes());
        offset += 4;
        self.response_buffer[offset..offset + 4].copy_from_slice(&0u32.to_le_bytes()); // padding
        offset += 4;

        // Default attributes for root
        let attr = FuseAttr {
            ino: header.nodeid,
            mode: 0o040755, // Directory
            nlink: 2,
            uid: 0,
            gid: 0,
            ..Default::default()
        };

        offset += attr.to_bytes(&mut self.response_buffer[offset..]);

        Ok(offset)
    }

    /// Handle FUSE_OPEN / FUSE_OPENDIR.
    fn handle_open(&mut self, header: &FuseInHeader) -> Result<usize, StorageError> {
        let out_header = FuseOutHeader {
            len: (FuseOutHeader::SIZE + 16) as u32,
            error: 0,
            unique: header.unique,
        };

        let mut offset = out_header.to_bytes(&mut self.response_buffer);

        // FuseOpenOut
        self.response_buffer[offset..offset + 8].copy_from_slice(&0u64.to_le_bytes()); // fh
        offset += 8;
        self.response_buffer[offset..offset + 4].copy_from_slice(&0u32.to_le_bytes()); // open_flags
        offset += 4;
        self.response_buffer[offset..offset + 4].copy_from_slice(&0u32.to_le_bytes()); // padding

        Ok(FuseOutHeader::SIZE + 16)
    }

    /// Handle FUSE_READ.
    fn handle_read(&mut self, header: &FuseInHeader, _data: &[u8]) -> Result<usize, StorageError> {
        // TODO: Implement actual read
        let out_header = FuseOutHeader {
            len: FuseOutHeader::SIZE as u32,
            error: 0,
            unique: header.unique,
        };

        Ok(out_header.to_bytes(&mut self.response_buffer))
    }

    /// Handle FUSE_READDIR.
    fn handle_readdir(&mut self, header: &FuseInHeader, _data: &[u8]) -> Result<usize, StorageError> {
        // TODO: Implement actual readdir
        let out_header = FuseOutHeader {
            len: FuseOutHeader::SIZE as u32,
            error: 0,
            unique: header.unique,
        };

        Ok(out_header.to_bytes(&mut self.response_buffer))
    }

    /// Handle FUSE_RELEASE / FUSE_RELEASEDIR.
    fn handle_release(&mut self, header: &FuseInHeader) -> Result<usize, StorageError> {
        let out_header = FuseOutHeader {
            len: FuseOutHeader::SIZE as u32,
            error: 0,
            unique: header.unique,
        };

        Ok(out_header.to_bytes(&mut self.response_buffer))
    }

    /// Handle FUSE_STATFS.
    fn handle_statfs(&mut self, header: &FuseInHeader) -> Result<usize, StorageError> {
        let out_header = FuseOutHeader {
            len: (FuseOutHeader::SIZE + 64) as u32,
            error: 0,
            unique: header.unique,
        };

        let offset = out_header.to_bytes(&mut self.response_buffer);

        // Clear statfs data
        for i in 0..64 {
            self.response_buffer[offset + i] = 0;
        }

        Ok(FuseOutHeader::SIZE + 64)
    }
}

impl Filesystem for FuseFilesystem {
    fn fs_type(&self) -> &str {
        "fuse"
    }

    fn statfs(&self) -> Result<FsStats, StorageError> {
        Ok(FsStats::default())
    }

    fn lookup(&self, _path: &str) -> Result<FileMetadata, StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn readdir(&self, _path: &str, _offset: u64) -> Result<Vec<DirEntry>, StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn create(&self, _path: &str, _mode: u16) -> Result<u64, StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn mkdir(&self, _path: &str, _mode: u16) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn unlink(&self, _path: &str) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn rmdir(&self, _path: &str) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn rename(&self, _old: &str, _new: &str) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn symlink(&self, _target: &str, _link: &str) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn readlink(&self, _path: &str) -> Result<String, StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn link(&self, _old: &str, _new: &str) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn setattr(&self, _path: &str, _attr: &FileMetadata) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn open(&self, _path: &str, _flags: OpenFlags) -> Result<u64, StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn close(&self, _handle: u64) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn read(&self, _handle: u64, _offset: u64, _buffer: &mut [u8]) -> Result<usize, StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn write(&self, _handle: u64, _offset: u64, _data: &[u8]) -> Result<usize, StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn flush(&self, _handle: u64) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn fsync(&self, _handle: u64, _data_only: bool) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn truncate(&self, _path: &str, _size: u64) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn fallocate(&self, _handle: u64, _offset: u64, _len: u64) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn sync(&self) -> Result<(), StorageError> {
        Ok(())
    }
}
