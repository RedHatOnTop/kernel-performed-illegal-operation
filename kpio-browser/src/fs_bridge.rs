//! File System Bridge Module
//!
//! This module provides filesystem access from the browser to the kernel VFS.
//! It wraps syscalls for file operations and provides a high-level API.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// File descriptor type
pub type Fd = i32;

/// File type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// Regular file
    File,
    /// Directory
    Directory,
    /// Symbolic link
    Symlink,
    /// Block device
    BlockDevice,
    /// Character device
    CharDevice,
    /// Named pipe (FIFO)
    Fifo,
    /// Socket
    Socket,
    /// Unknown type
    Unknown,
}

/// Directory entry
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Entry name
    pub name: String,
    /// File type
    pub file_type: FileType,
    /// File size in bytes
    pub size: u64,
    /// Last modified timestamp (Unix epoch)
    pub modified: u64,
    /// Creation timestamp (Unix epoch)
    pub created: u64,
    /// Is hidden file
    pub hidden: bool,
    /// Is readonly
    pub readonly: bool,
}

/// File metadata
#[derive(Debug, Clone)]
pub struct FileMeta {
    /// File type
    pub file_type: FileType,
    /// File size in bytes
    pub size: u64,
    /// Last modified timestamp
    pub modified: u64,
    /// Last accessed timestamp
    pub accessed: u64,
    /// Creation timestamp
    pub created: u64,
    /// Permissions (Unix mode)
    pub mode: u32,
    /// Owner user ID
    pub uid: u32,
    /// Owner group ID
    pub gid: u32,
    /// Number of hard links
    pub nlink: u32,
}

/// Open file flags
#[derive(Debug, Clone, Copy)]
pub struct OpenFlags {
    /// Read access
    pub read: bool,
    /// Write access
    pub write: bool,
    /// Create if not exists
    pub create: bool,
    /// Truncate existing file
    pub truncate: bool,
    /// Append mode
    pub append: bool,
    /// Exclusive create (fail if exists)
    pub exclusive: bool,
}

impl OpenFlags {
    /// Read-only mode
    pub const READ: Self = Self {
        read: true,
        write: false,
        create: false,
        truncate: false,
        append: false,
        exclusive: false,
    };

    /// Write-only mode (create/truncate)
    pub const WRITE: Self = Self {
        read: false,
        write: true,
        create: true,
        truncate: true,
        append: false,
        exclusive: false,
    };

    /// Read-write mode
    pub const READ_WRITE: Self = Self {
        read: true,
        write: true,
        create: false,
        truncate: false,
        append: false,
        exclusive: false,
    };

    /// Append mode
    pub const APPEND: Self = Self {
        read: false,
        write: true,
        create: true,
        truncate: false,
        append: true,
        exclusive: false,
    };
}

/// Seek origin
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekFrom {
    /// From beginning of file
    Start(u64),
    /// From end of file
    End(i64),
    /// From current position
    Current(i64),
}

/// File system bridge
pub struct FsBridge {
    /// Current working directory
    cwd: spin::Mutex<String>,
}

impl FsBridge {
    /// Create a new file system bridge
    pub fn new() -> Self {
        Self {
            cwd: spin::Mutex::new(String::from("/")),
        }
    }

    /// Get current working directory
    pub fn cwd(&self) -> String {
        self.cwd.lock().clone()
    }

    /// Change current working directory
    pub fn chdir(&self, path: &str) -> Result<(), FsError> {
        // Validate path exists and is a directory
        let meta = self.metadata(path)?;
        if meta.file_type != FileType::Directory {
            return Err(FsError::NotDirectory);
        }

        *self.cwd.lock() = self.normalize_path(path);
        Ok(())
    }

    /// Normalize a path (resolve . and ..)
    fn normalize_path(&self, path: &str) -> String {
        if path.starts_with('/') {
            String::from(path)
        } else {
            let cwd = self.cwd.lock().clone();
            if cwd.ends_with('/') {
                alloc::format!("{}{}", cwd, path)
            } else {
                alloc::format!("{}/{}", cwd, path)
            }
        }
    }

    /// Read entire file contents
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, FsError> {
        let _normalized = self.normalize_path(path);

        // TODO: Implement via syscall
        // let fd = syscall::open(normalized, O_RDONLY)?;
        // let mut buffer = Vec::new();
        // syscall::read(fd, &mut buffer)?;
        // syscall::close(fd)?;

        // Mock implementation
        Ok(vec![b'H', b'e', b'l', b'l', b'o'])
    }

    /// Write entire file contents
    pub fn write_file(&self, path: &str, data: &[u8]) -> Result<(), FsError> {
        let _normalized = self.normalize_path(path);
        let _data = data;

        // TODO: Implement via syscall
        // let fd = syscall::open(normalized, O_WRONLY | O_CREAT | O_TRUNC)?;
        // syscall::write(fd, data)?;
        // syscall::close(fd)?;

        Ok(())
    }

    /// Append to file
    pub fn append_file(&self, path: &str, data: &[u8]) -> Result<(), FsError> {
        let _normalized = self.normalize_path(path);
        let _data = data;

        // TODO: Implement via syscall
        Ok(())
    }

    /// Read directory contents
    pub fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        let _normalized = self.normalize_path(path);

        // TODO: Implement via syscall
        // let entries = syscall::readdir(normalized)?;

        // Mock implementation - return some default entries
        Ok(vec![
            DirEntry {
                name: String::from("Documents"),
                file_type: FileType::Directory,
                size: 0,
                modified: 0,
                created: 0,
                hidden: false,
                readonly: false,
            },
            DirEntry {
                name: String::from("Downloads"),
                file_type: FileType::Directory,
                size: 0,
                modified: 0,
                created: 0,
                hidden: false,
                readonly: false,
            },
            DirEntry {
                name: String::from("test.txt"),
                file_type: FileType::File,
                size: 1024,
                modified: 0,
                created: 0,
                hidden: false,
                readonly: false,
            },
        ])
    }

    /// Get file metadata
    pub fn metadata(&self, path: &str) -> Result<FileMeta, FsError> {
        let _normalized = self.normalize_path(path);

        // TODO: Implement via syscall
        // syscall::stat(normalized)

        // Mock implementation
        Ok(FileMeta {
            file_type: if path.ends_with('/') || !path.contains('.') {
                FileType::Directory
            } else {
                FileType::File
            },
            size: 0,
            modified: 0,
            accessed: 0,
            created: 0,
            mode: 0o755,
            uid: 1000,
            gid: 1000,
            nlink: 1,
        })
    }

    /// Check if path exists
    pub fn exists(&self, path: &str) -> bool {
        self.metadata(path).is_ok()
    }

    /// Check if path is a file
    pub fn is_file(&self, path: &str) -> bool {
        self.metadata(path)
            .map(|m| m.file_type == FileType::File)
            .unwrap_or(false)
    }

    /// Check if path is a directory
    pub fn is_dir(&self, path: &str) -> bool {
        self.metadata(path)
            .map(|m| m.file_type == FileType::Directory)
            .unwrap_or(false)
    }

    /// Create a directory
    pub fn create_dir(&self, path: &str) -> Result<(), FsError> {
        let _normalized = self.normalize_path(path);
        // TODO: syscall::mkdir(normalized, 0o755)
        Ok(())
    }

    /// Create directory and all parent directories
    pub fn create_dir_all(&self, path: &str) -> Result<(), FsError> {
        let normalized = self.normalize_path(path);

        let mut current = String::new();
        for component in normalized.split('/') {
            if component.is_empty() {
                continue;
            }
            current.push('/');
            current.push_str(component);

            if !self.exists(&current) {
                self.create_dir(&current)?;
            }
        }

        Ok(())
    }

    /// Remove a file
    pub fn remove_file(&self, path: &str) -> Result<(), FsError> {
        let _normalized = self.normalize_path(path);
        // TODO: syscall::unlink(normalized)
        Ok(())
    }

    /// Remove a directory (must be empty)
    pub fn remove_dir(&self, path: &str) -> Result<(), FsError> {
        let _normalized = self.normalize_path(path);
        // TODO: syscall::rmdir(normalized)
        Ok(())
    }

    /// Remove directory and all contents
    pub fn remove_dir_all(&self, path: &str) -> Result<(), FsError> {
        let entries = self.read_dir(path)?;

        for entry in entries {
            let entry_path = alloc::format!("{}/{}", path, entry.name);
            if entry.file_type == FileType::Directory {
                self.remove_dir_all(&entry_path)?;
            } else {
                self.remove_file(&entry_path)?;
            }
        }

        self.remove_dir(path)
    }

    /// Rename/move a file or directory
    pub fn rename(&self, from: &str, to: &str) -> Result<(), FsError> {
        let _from_normalized = self.normalize_path(from);
        let _to_normalized = self.normalize_path(to);
        // TODO: syscall::rename(from_normalized, to_normalized)
        Ok(())
    }

    /// Copy a file
    pub fn copy(&self, from: &str, to: &str) -> Result<u64, FsError> {
        let data = self.read_file(from)?;
        self.write_file(to, &data)?;
        Ok(data.len() as u64)
    }

    /// Create a symbolic link
    pub fn symlink(&self, original: &str, link: &str) -> Result<(), FsError> {
        let _original = self.normalize_path(original);
        let _link = self.normalize_path(link);
        // TODO: syscall::symlink(original, link)
        Ok(())
    }

    /// Read a symbolic link target
    pub fn read_link(&self, path: &str) -> Result<String, FsError> {
        let _normalized = self.normalize_path(path);
        // TODO: syscall::readlink(normalized)
        Ok(String::from("/tmp/target"))
    }

    /// Set file permissions
    pub fn set_permissions(&self, path: &str, mode: u32) -> Result<(), FsError> {
        let _normalized = self.normalize_path(path);
        let _mode = mode;
        // TODO: syscall::chmod(normalized, mode)
        Ok(())
    }
}

impl Default for FsBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// File system errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    /// File or directory not found
    NotFound,
    /// Permission denied
    PermissionDenied,
    /// Already exists
    AlreadyExists,
    /// Not a directory
    NotDirectory,
    /// Is a directory
    IsDirectory,
    /// Directory not empty
    NotEmpty,
    /// Too many open files
    TooManyOpenFiles,
    /// No space left on device
    NoSpace,
    /// Read-only file system
    ReadOnly,
    /// Invalid path
    InvalidPath,
    /// I/O error
    IoError,
    /// File too large
    FileTooLarge,
    /// Interrupted
    Interrupted,
}

/// Global file system bridge instance
static FS_BRIDGE: spin::Lazy<FsBridge> = spin::Lazy::new(FsBridge::new);

/// Get the global file system bridge
pub fn fs_bridge() -> &'static FsBridge {
    &FS_BRIDGE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_read() {
        let fs = FsBridge::new();
        let content = fs.read_file("/home/user/test.txt").unwrap();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_directory_listing() {
        let fs = FsBridge::new();
        let entries = fs.read_dir("/home/user").unwrap();
        assert!(entries.iter().any(|e| e.name == "Documents"));
    }

    #[test]
    fn test_file_write() {
        let fs = FsBridge::new();
        fs.write_file("/tmp/test.txt", b"Hello, World!").unwrap();
        // In mock mode, we can't verify content
    }
}
