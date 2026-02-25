//! Virtual Filesystem (VFS) layer.
//!
//! This module provides a unified interface for all filesystem operations,
//! abstracting the differences between various filesystem implementations.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use spin::RwLock;

use crate::{DirEntry, FileMetadata, FileType, MountFlags, OpenFlags, SeekFrom, StorageError};

/// Maximum number of mount points.
const MAX_MOUNTS: usize = 32;

/// Maximum path length.
pub const MAX_PATH_LEN: usize = 4096;

/// Maximum filename length.
pub const MAX_NAME_LEN: usize = 255;

/// Global mount table.
static MOUNT_TABLE: RwLock<MountTable> = RwLock::new(MountTable::new());
static FILESYSTEM_TABLE: RwLock<[Option<&'static dyn Filesystem>; MAX_MOUNTS]> =
    RwLock::new([None; MAX_MOUNTS]);

/// Mount information.
#[derive(Clone, Copy)]
pub struct MountInfo {
    /// Mount point path.
    pub mount_point: [u8; 256],
    /// Mount point length.
    pub mount_point_len: usize,
    /// Device path.
    pub device: [u8; 64],
    /// Device path length.
    pub device_len: usize,
    /// Filesystem type.
    pub fs_type: [u8; 16],
    /// Filesystem type length.
    pub fs_type_len: usize,
    /// Mount flags.
    pub flags: MountFlags,
    /// Whether this mount is active.
    pub active: bool,
}

impl MountInfo {
    /// Create a new empty mount info.
    const fn empty() -> Self {
        MountInfo {
            mount_point: [0; 256],
            mount_point_len: 0,
            device: [0; 64],
            device_len: 0,
            fs_type: [0; 16],
            fs_type_len: 0,
            flags: MountFlags::empty(),
            active: false,
        }
    }

    /// Get mount point as string.
    pub fn mount_point_str(&self) -> &str {
        core::str::from_utf8(&self.mount_point[..self.mount_point_len]).unwrap_or("")
    }

    /// Get device as string.
    pub fn device_str(&self) -> &str {
        core::str::from_utf8(&self.device[..self.device_len]).unwrap_or("")
    }

    /// Get filesystem type as string.
    pub fn fs_type_str(&self) -> &str {
        core::str::from_utf8(&self.fs_type[..self.fs_type_len]).unwrap_or("")
    }
}

/// Mount table.
struct MountTable {
    mounts: [MountInfo; MAX_MOUNTS],
}

impl MountTable {
    /// Create a new mount table.
    const fn new() -> Self {
        MountTable {
            mounts: [MountInfo::empty(); MAX_MOUNTS],
        }
    }

    /// Find a mount point for a path.
    fn find_mount(&self, path: &str) -> Option<usize> {
        let mut best_match: Option<usize> = None;
        let mut best_len = 0;

        for (i, mount) in self.mounts.iter().enumerate() {
            if !mount.active {
                continue;
            }

            let mp = mount.mount_point_str();
            if path.starts_with(mp) && mp.len() > best_len {
                // Make sure it's a proper prefix (not just substring match)
                if path.len() == mp.len() || path.as_bytes()[mp.len()] == b'/' || mp == "/" {
                    best_match = Some(i);
                    best_len = mp.len();
                }
            }
        }

        best_match
    }

    /// Find a free slot.
    fn find_free_slot(&self) -> Option<usize> {
        self.mounts.iter().position(|m| !m.active)
    }
}

/// Filesystem statistics.
#[derive(Debug, Clone)]
pub struct FsStats {
    /// Filesystem type.
    pub fs_type: u32,
    /// Optimal transfer block size.
    pub block_size: u32,
    /// Total data blocks.
    pub total_blocks: u64,
    /// Free blocks.
    pub free_blocks: u64,
    /// Free blocks available to unprivileged user.
    pub available_blocks: u64,
    /// Total file nodes.
    pub total_inodes: u64,
    /// Free file nodes.
    pub free_inodes: u64,
    /// Filesystem ID.
    pub fs_id: u64,
    /// Maximum filename length.
    pub max_name_len: u32,
    /// Fragment size.
    pub fragment_size: u32,
    /// Mount flags.
    pub flags: MountFlags,
}

impl Default for FsStats {
    fn default() -> Self {
        FsStats {
            fs_type: 0,
            block_size: 4096,
            total_blocks: 0,
            free_blocks: 0,
            available_blocks: 0,
            total_inodes: 0,
            free_inodes: 0,
            fs_id: 0,
            max_name_len: MAX_NAME_LEN as u32,
            fragment_size: 4096,
            flags: MountFlags::empty(),
        }
    }
}

/// Filesystem trait.
///
/// All filesystem implementations must implement this trait to be usable
/// through the VFS layer.
pub trait Filesystem: Send + Sync {
    /// Get filesystem type name.
    fn fs_type(&self) -> &str;

    /// Get filesystem statistics.
    fn statfs(&self) -> Result<FsStats, StorageError>;

    /// Lookup a file by path.
    fn lookup(&self, path: &str) -> Result<FileMetadata, StorageError>;

    /// Read a directory.
    fn readdir(&self, path: &str, offset: u64) -> Result<Vec<DirEntry>, StorageError>;

    /// Create a file.
    fn create(&self, path: &str, mode: u16) -> Result<u64, StorageError>;

    /// Create a directory.
    fn mkdir(&self, path: &str, mode: u16) -> Result<(), StorageError>;

    /// Remove a file.
    fn unlink(&self, path: &str) -> Result<(), StorageError>;

    /// Remove a directory.
    fn rmdir(&self, path: &str) -> Result<(), StorageError>;

    /// Rename a file or directory.
    fn rename(&self, old_path: &str, new_path: &str) -> Result<(), StorageError>;

    /// Create a symbolic link.
    fn symlink(&self, target: &str, link_path: &str) -> Result<(), StorageError>;

    /// Read a symbolic link.
    fn readlink(&self, path: &str) -> Result<String, StorageError>;

    /// Create a hard link.
    fn link(&self, old_path: &str, new_path: &str) -> Result<(), StorageError>;

    /// Set file attributes.
    fn setattr(&self, path: &str, attr: &FileMetadata) -> Result<(), StorageError>;

    /// Open a file.
    fn open(&self, path: &str, flags: OpenFlags) -> Result<u64, StorageError>;

    /// Close a file.
    fn close(&self, handle: u64) -> Result<(), StorageError>;

    /// Read from a file.
    fn read(&self, handle: u64, offset: u64, buffer: &mut [u8]) -> Result<usize, StorageError>;

    /// Write to a file.
    fn write(&self, handle: u64, offset: u64, data: &[u8]) -> Result<usize, StorageError>;

    /// Flush file buffers.
    fn flush(&self, handle: u64) -> Result<(), StorageError>;

    /// Sync file data to disk.
    fn fsync(&self, handle: u64, data_only: bool) -> Result<(), StorageError>;

    /// Truncate a file.
    fn truncate(&self, path: &str, size: u64) -> Result<(), StorageError>;

    /// Allocate space for a file.
    fn fallocate(&self, handle: u64, offset: u64, len: u64) -> Result<(), StorageError>;

    /// Sync the entire filesystem.
    fn sync(&self) -> Result<(), StorageError>;
}

/// VFS handle for open files.
#[derive(Debug)]
pub struct VfsHandle {
    /// Filesystem index in mount table.
    pub mount_idx: usize,
    /// Filesystem-specific handle.
    pub fs_handle: u64,
    /// Current file offset.
    pub offset: u64,
    /// Open flags.
    pub flags: OpenFlags,
    /// Relative path within the filesystem (for fstat/seek).
    pub rel_path: String,
}

fn relative_path<'a>(mount: &MountInfo, path: &'a str) -> &'a str {
    let mount_point = mount.mount_point_str();
    if mount_point == "/" {
        return path;
    }

    if let Some(stripped) = path.strip_prefix(mount_point) {
        if stripped.is_empty() {
            "/"
        } else {
            stripped
        }
    } else {
        path
    }
}

fn get_filesystem(mount_idx: usize) -> Result<&'static dyn Filesystem, StorageError> {
    let table = FILESYSTEM_TABLE.read();
    table[mount_idx].ok_or(StorageError::InvalidFilesystem)
}

/// File handle table.
static FILE_HANDLES: RwLock<FileHandleTable> = RwLock::new(FileHandleTable::new());

/// Maximum number of open file handles.
const MAX_FILE_HANDLES: usize = 1024;

/// File handle table.
struct FileHandleTable {
    handles: [Option<VfsHandle>; MAX_FILE_HANDLES],
    next_fd: u32,
}

impl FileHandleTable {
    const fn new() -> Self {
        const NONE: Option<VfsHandle> = None;
        FileHandleTable {
            handles: [NONE; MAX_FILE_HANDLES],
            next_fd: 3, // 0, 1, 2 reserved for stdin, stdout, stderr
        }
    }

    fn allocate(&mut self, handle: VfsHandle) -> Result<u32, StorageError> {
        for (i, slot) in self.handles.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(handle);
                return Ok(i as u32);
            }
        }
        Err(StorageError::TooManyOpenFiles)
    }

    fn get(&self, fd: u32) -> Option<&VfsHandle> {
        self.handles.get(fd as usize)?.as_ref()
    }

    fn get_mut(&mut self, fd: u32) -> Option<&mut VfsHandle> {
        self.handles.get_mut(fd as usize)?.as_mut()
    }

    fn free(&mut self, fd: u32) -> Option<VfsHandle> {
        self.handles.get_mut(fd as usize)?.take()
    }
}

/// Initialize the VFS layer.
pub fn init() -> Result<(), StorageError> {
    // Create root mount point
    let mut table = MOUNT_TABLE.write();
    let slot = table.find_free_slot().ok_or(StorageError::NoSpace)?;

    table.mounts[slot] = MountInfo {
        mount_point: {
            let mut mp = [0u8; 256];
            mp[0] = b'/';
            mp
        },
        mount_point_len: 1,
        device: [0; 64],
        device_len: 0,
        fs_type: {
            let mut ft = [0u8; 16];
            ft[..6].copy_from_slice(b"rootfs");
            ft
        },
        fs_type_len: 6,
        flags: MountFlags::empty(),
        active: true,
    };

    Ok(())
}

/// Returns `true` if a filesystem is currently mounted at `mount_point`.
pub fn is_mounted(mount_point: &str) -> bool {
    let table = MOUNT_TABLE.read();
    for mount in table.mounts.iter() {
        if mount.active && mount.mount_point_str() == mount_point {
            return true;
        }
    }
    false
}

/// Mount a filesystem.
pub fn mount(
    device: &str,
    mount_point: &str,
    fs_type: &str,
    flags: MountFlags,
) -> Result<(), StorageError> {
    let mut table = MOUNT_TABLE.write();

    // Check if mount point already exists
    for mount in table.mounts.iter() {
        if mount.active && mount.mount_point_str() == mount_point {
            return Err(StorageError::AlreadyExists);
        }
    }

    let slot = table.find_free_slot().ok_or(StorageError::NoSpace)?;

    let mut info = MountInfo::empty();

    // Copy mount point
    let mp_bytes = mount_point.as_bytes();
    let mp_len = mp_bytes.len().min(255);
    info.mount_point[..mp_len].copy_from_slice(&mp_bytes[..mp_len]);
    info.mount_point_len = mp_len;

    // Copy device
    let dev_bytes = device.as_bytes();
    let dev_len = dev_bytes.len().min(63);
    info.device[..dev_len].copy_from_slice(&dev_bytes[..dev_len]);
    info.device_len = dev_len;

    // Copy filesystem type
    let fs_bytes = fs_type.as_bytes();
    let fs_len = fs_bytes.len().min(15);
    info.fs_type[..fs_len].copy_from_slice(&fs_bytes[..fs_len]);
    info.fs_type_len = fs_len;

    info.flags = flags;
    info.active = true;

    let device_idx = crate::driver::find_device(device).ok_or(StorageError::DeviceNotFound)?;
    let block_device = crate::driver::get_device(device_idx).ok_or(StorageError::DeviceNotFound)?;

    let fs: &'static dyn Filesystem = match crate::fs::FilesystemType::from_str(fs_type) {
        crate::fs::FilesystemType::Fat32 => {
            let fat = crate::fs::fat32::Fat32Filesystem::mount(block_device)?;
            Box::leak(Box::new(fat))
        }
        _ => return Err(StorageError::Unsupported),
    };

    table.mounts[slot] = info;
    FILESYSTEM_TABLE.write()[slot] = Some(fs);

    Ok(())
}

/// Unmount a filesystem.
pub fn unmount(mount_point: &str) -> Result<(), StorageError> {
    let mut table = MOUNT_TABLE.write();

    for (idx, mount) in table.mounts.iter_mut().enumerate() {
        if mount.active && mount.mount_point_str() == mount_point {
            // Don't allow unmounting root
            if mount_point == "/" {
                return Err(StorageError::PermissionDenied);
            }

            mount.active = false;
            FILESYSTEM_TABLE.write()[idx] = None;
            return Ok(());
        }
    }

    Err(StorageError::FileNotFound)
}

/// List all mounts.
pub fn list_mounts() -> Vec<MountInfo> {
    let table = MOUNT_TABLE.read();
    table.mounts.iter().filter(|m| m.active).cloned().collect()
}

/// Get filesystem statistics for a path.
pub fn statfs(path: &str) -> Result<FsStats, StorageError> {
    let table = MOUNT_TABLE.read();
    let idx = table.find_mount(path).ok_or(StorageError::FileNotFound)?;
    let mount = table.mounts[idx];
    drop(table);

    let fs = get_filesystem(idx)?;
    let mut stats = fs.statfs()?;
    stats.flags = mount.flags;
    Ok(stats)
}

/// Open a file.
pub fn open(path: &str, flags: OpenFlags) -> Result<u32, StorageError> {
    // Validate path
    if path.is_empty() || !path.starts_with('/') {
        return Err(StorageError::InvalidPath);
    }

    if path.len() > MAX_PATH_LEN {
        return Err(StorageError::NameTooLong);
    }

    let table = MOUNT_TABLE.read();
    let mount_idx = table.find_mount(path).ok_or(StorageError::FileNotFound)?;
    let mount = table.mounts[mount_idx];
    drop(table);

    let fs = get_filesystem(mount_idx)?;
    let rel = relative_path(&mount, path);
    let fs_handle = fs.open(rel, flags)?;

    // Create VFS handle
    let handle = VfsHandle {
        mount_idx,
        fs_handle,
        offset: 0,
        flags,
        rel_path: String::from(rel),
    };

    let mut handles = FILE_HANDLES.write();
    handles.allocate(handle)
}

/// Close a file.
pub fn close(fd: u32) -> Result<(), StorageError> {
    let mut handles = FILE_HANDLES.write();
    let handle = handles.free(fd).ok_or(StorageError::InvalidFd)?;
    drop(handles);

    let fs = get_filesystem(handle.mount_idx)?;
    fs.close(handle.fs_handle)?;
    Ok(())
}

/// Read from a file.
pub fn read(fd: u32, buffer: &mut [u8]) -> Result<usize, StorageError> {
    let mut handles = FILE_HANDLES.write();
    let handle = handles.get_mut(fd).ok_or(StorageError::InvalidFd)?;

    if !handle.flags.contains(OpenFlags::READ) {
        return Err(StorageError::PermissionDenied);
    }

    let fs = get_filesystem(handle.mount_idx)?;
    let bytes_read = fs.read(handle.fs_handle, handle.offset, buffer)?;
    handle.offset += bytes_read as u64;

    Ok(bytes_read)
}

/// Write to a file.
pub fn write(fd: u32, data: &[u8]) -> Result<usize, StorageError> {
    let mut handles = FILE_HANDLES.write();
    let handle = handles.get_mut(fd).ok_or(StorageError::InvalidFd)?;

    if !handle.flags.contains(OpenFlags::WRITE) {
        return Err(StorageError::PermissionDenied);
    }

    let fs = get_filesystem(handle.mount_idx)?;
    let bytes_written = fs.write(handle.fs_handle, handle.offset, data)?;
    handle.offset += bytes_written as u64;

    Ok(bytes_written)
}

/// Seek in a file.
pub fn seek(fd: u32, pos: SeekFrom) -> Result<u64, StorageError> {
    let mut handles = FILE_HANDLES.write();
    let handle = handles.get_mut(fd).ok_or(StorageError::InvalidFd)?;

    let new_offset = match pos {
        SeekFrom::Start(offset) => offset,
        SeekFrom::End(offset) => {
            // Get actual file size via filesystem lookup
            let fs = get_filesystem(handle.mount_idx)?;
            let metadata = fs.lookup(&handle.rel_path)?;
            let size: u64 = metadata.size;
            if offset < 0 {
                size.checked_sub((-offset) as u64)
                    .ok_or(StorageError::InvalidArgument)?
            } else {
                size + offset as u64
            }
        }
        SeekFrom::Current(offset) => {
            if offset < 0 {
                handle
                    .offset
                    .checked_sub((-offset) as u64)
                    .ok_or(StorageError::InvalidArgument)?
            } else {
                handle.offset + offset as u64
            }
        }
    };

    handle.offset = new_offset;
    Ok(new_offset)
}

/// Get file metadata.
pub fn stat(path: &str) -> Result<FileMetadata, StorageError> {
    if path.is_empty() || !path.starts_with('/') {
        return Err(StorageError::InvalidPath);
    }

    let table = MOUNT_TABLE.read();
    let mount_idx = table.find_mount(path).ok_or(StorageError::FileNotFound)?;
    let mount = table.mounts[mount_idx];
    drop(table);

    let fs = get_filesystem(mount_idx)?;
    fs.lookup(relative_path(&mount, path))
}

/// Read a directory.
pub fn readdir(path: &str) -> Result<Vec<DirEntry>, StorageError> {
    if path.is_empty() || !path.starts_with('/') {
        return Err(StorageError::InvalidPath);
    }

    let table = MOUNT_TABLE.read();
    let mount_idx = table.find_mount(path).ok_or(StorageError::FileNotFound)?;
    let mount = table.mounts[mount_idx];
    drop(table);

    let fs = get_filesystem(mount_idx)?;
    fs.readdir(relative_path(&mount, path), 0)
}

/// Create a directory.
pub fn mkdir(path: &str, mode: u16) -> Result<(), StorageError> {
    if path.is_empty() || !path.starts_with('/') {
        return Err(StorageError::InvalidPath);
    }

    let table = MOUNT_TABLE.read();
    let mount_idx = table.find_mount(path).ok_or(StorageError::FileNotFound)?;
    let mount = &table.mounts[mount_idx];

    if mount.flags.contains(MountFlags::READ_ONLY) {
        return Err(StorageError::ReadOnly);
    }

    let fs = get_filesystem(mount_idx)?;
    fs.mkdir(relative_path(mount, path), mode)
}

/// Remove a file.
pub fn unlink(path: &str) -> Result<(), StorageError> {
    if path.is_empty() || !path.starts_with('/') {
        return Err(StorageError::InvalidPath);
    }

    let table = MOUNT_TABLE.read();
    let mount_idx = table.find_mount(path).ok_or(StorageError::FileNotFound)?;
    let mount = &table.mounts[mount_idx];

    if mount.flags.contains(MountFlags::READ_ONLY) {
        return Err(StorageError::ReadOnly);
    }

    let fs = get_filesystem(mount_idx)?;
    fs.unlink(relative_path(mount, path))
}

/// Remove a directory.
pub fn rmdir(path: &str) -> Result<(), StorageError> {
    if path.is_empty() || !path.starts_with('/') {
        return Err(StorageError::InvalidPath);
    }

    let table = MOUNT_TABLE.read();
    let mount_idx = table.find_mount(path).ok_or(StorageError::FileNotFound)?;
    let mount = &table.mounts[mount_idx];

    if mount.flags.contains(MountFlags::READ_ONLY) {
        return Err(StorageError::ReadOnly);
    }

    let fs = get_filesystem(mount_idx)?;
    fs.rmdir(relative_path(mount, path))
}

/// Rename a file or directory.
pub fn rename(old_path: &str, new_path: &str) -> Result<(), StorageError> {
    if old_path.is_empty() || !old_path.starts_with('/') {
        return Err(StorageError::InvalidPath);
    }
    if new_path.is_empty() || !new_path.starts_with('/') {
        return Err(StorageError::InvalidPath);
    }

    let table = MOUNT_TABLE.read();
    let old_mount = table
        .find_mount(old_path)
        .ok_or(StorageError::FileNotFound)?;
    let new_mount = table
        .find_mount(new_path)
        .ok_or(StorageError::FileNotFound)?;

    // Can't rename across filesystems
    if old_mount != new_mount {
        return Err(StorageError::CrossDeviceLink);
    }

    let mount = &table.mounts[old_mount];
    if mount.flags.contains(MountFlags::READ_ONLY) {
        return Err(StorageError::ReadOnly);
    }

    let fs = get_filesystem(old_mount)?;
    fs.rename(relative_path(mount, old_path), relative_path(mount, new_path))
}

/// Sync all filesystems.
pub fn sync_all() -> Result<(), StorageError> {
    let mounts = MOUNT_TABLE.read();
    let fs_table = FILESYSTEM_TABLE.read();
    for (idx, mount) in mounts.mounts.iter().enumerate() {
        if !mount.active {
            continue;
        }
        if let Some(fs) = fs_table[idx] {
            fs.sync()?;
        }
    }
    Ok(())
}
