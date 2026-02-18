//! Virtual File System
//!
//! Provides a uniform file-operation interface over the in-memory
//! filesystem (terminal::fs).  A global file descriptor table maps
//! integer fds to open inodes + cursor offsets so that syscall
//! read / write / open / close operate on real data.

#![allow(dead_code)]

pub mod fd;
pub mod sandbox;

use alloc::string::String;
use alloc::vec::Vec;

/// File open flags (subset of POSIX O_*).
#[derive(Debug, Clone, Copy)]
pub struct OpenFlags(pub u32);

impl OpenFlags {
    pub const RDONLY: OpenFlags = OpenFlags(0);
    pub const WRONLY: OpenFlags = OpenFlags(1);
    pub const RDWR: OpenFlags = OpenFlags(2);
    pub const CREAT: OpenFlags = OpenFlags(0o100);
    pub const TRUNC: OpenFlags = OpenFlags(0o1000);
    pub const APPEND: OpenFlags = OpenFlags(0o2000);

    pub fn readable(self) -> bool {
        self.0 & 3 != 1
    }
    pub fn writable(self) -> bool {
        self.0 & 3 != 0
    }
    pub fn create(self) -> bool {
        self.0 & 0o100 != 0
    }
    pub fn truncate(self) -> bool {
        self.0 & 0o1000 != 0
    }
    pub fn append(self) -> bool {
        self.0 & 0o2000 != 0
    }
}

/// Stat information for a file.
#[derive(Debug, Clone)]
pub struct FileStat {
    pub ino: u64,
    pub size: u64,
    pub mode: u16,
    pub nlink: u32,
    pub uid: u32,
    pub gid: u32,
    pub is_dir: bool,
    pub is_file: bool,
    pub is_symlink: bool,
}

/// VFS error.
#[derive(Debug, Clone)]
pub enum VfsError {
    NotFound,
    PermissionDenied,
    IsDirectory,
    NotDirectory,
    AlreadyExists,
    InvalidFd,
    IoError,
    NoSpace,
}

/// High-level VFS operations wrapping terminal::fs.
///
/// These are convenience functions for kernel-internal use.
/// Syscall handlers should go through fd::FdTable instead.

/// Stat a path.
pub fn stat(path: &str) -> Result<FileStat, VfsError> {
    use crate::terminal::fs;

    let ino = fs::with_fs(|f| f.resolve(path)).ok_or(VfsError::NotFound)?;

    fs::with_fs(|f| {
        let inode = f.get(ino).ok_or(VfsError::NotFound)?;
        Ok(FileStat {
            ino,
            size: inode.size,
            mode: inode.mode.0,
            nlink: inode.nlink,
            uid: inode.uid,
            gid: inode.gid,
            is_dir: inode.mode.is_dir(),
            is_file: inode.mode.is_file(),
            is_symlink: inode.mode.is_symlink(),
        })
    })
}

/// Read all bytes from a regular file or /proc entry.
pub fn read_all(path: &str) -> Result<Vec<u8>, VfsError> {
    use crate::terminal::fs;

    let ino = fs::with_fs(|f| f.resolve(path)).ok_or(VfsError::NotFound)?;

    fs::with_fs(|f| f.read_file(ino)).map_err(|_| VfsError::IoError)
}

/// Write bytes to a file (create if missing, truncate existing).
pub fn write_all(path: &str, data: &[u8]) -> Result<(), VfsError> {
    use crate::terminal::fs;

    // Try to resolve existing
    let existing = fs::with_fs(|f| f.resolve(path));
    if let Some(ino) = existing {
        fs::with_fs(|f| f.write_file(ino, data)).map_err(|_| VfsError::IoError)
    } else {
        // Create — need parent dir + file name
        let (parent_path, name) = split_path(path);
        let parent_ino = fs::with_fs(|f| f.resolve(parent_path)).ok_or(VfsError::NotFound)?;
        fs::with_fs(|f| f.create_file(parent_ino, name, data))
            .map(|_| ())
            .map_err(|_| VfsError::IoError)
    }
}

/// List directory entries.
pub fn readdir(path: &str) -> Result<Vec<(String, u64)>, VfsError> {
    use crate::terminal::fs;

    let ino = fs::with_fs(|f| f.resolve(path)).ok_or(VfsError::NotFound)?;

    fs::with_fs(|f| f.readdir_all(ino)).ok_or(VfsError::NotDirectory)
}

/// Split "/a/b/c" into ("/a/b", "c").
pub fn split_path(path: &str) -> (&str, &str) {
    if let Some(pos) = path.rfind('/') {
        let parent = if pos == 0 { "/" } else { &path[..pos] };
        let name = &path[pos + 1..];
        (parent, name)
    } else {
        ("/", path)
    }
}
