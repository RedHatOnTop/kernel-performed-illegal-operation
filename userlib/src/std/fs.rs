//! std::fs compatibility layer for KPIO
//!
//! Provides file system operations via KPIO syscalls.

use alloc::string::String;
use alloc::vec::Vec;

use super::net::IoError;
use crate::syscall;

/// File handle
pub struct File {
    fd: u64,
}

impl File {
    /// Open file for reading
    pub fn open(path: &str) -> Result<File, IoError> {
        OpenOptions::new().read(true).open(path)
    }

    /// Create file for writing
    pub fn create(path: &str) -> Result<File, IoError> {
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
    }

    /// Read from file
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        syscall::fs_read(self.fd, buf)
            .map(|n| n as usize)
            .map_err(|_| IoError::Other)
    }

    /// Read exact bytes
    pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), IoError> {
        let mut total = 0;
        while total < buf.len() {
            match self.read(&mut buf[total..]) {
                Ok(0) => return Err(IoError::UnexpectedEof),
                Ok(n) => total += n,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Read all to Vec
    pub fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize, IoError> {
        let mut tmp = [0u8; 4096];
        let mut total = 0;
        loop {
            match self.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    total += n;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(total)
    }

    /// Read to string
    pub fn read_to_string(&mut self, buf: &mut String) -> Result<usize, IoError> {
        let mut bytes = Vec::new();
        let n = self.read_to_end(&mut bytes)?;
        match core::str::from_utf8(&bytes) {
            Ok(s) => {
                buf.push_str(s);
                Ok(n)
            }
            Err(_) => Err(IoError::InvalidData),
        }
    }

    /// Write to file
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> {
        syscall::fs_write(self.fd, buf)
            .map(|n| n as usize)
            .map_err(|_| IoError::Other)
    }

    /// Write all bytes
    pub fn write_all(&mut self, buf: &[u8]) -> Result<(), IoError> {
        let mut written = 0;
        while written < buf.len() {
            match self.write(&buf[written..]) {
                Ok(0) => return Err(IoError::WriteZero),
                Ok(n) => written += n,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Flush
    pub fn flush(&mut self) -> Result<(), IoError> {
        syscall::fs_sync(self.fd)
            .map(|_| ())
            .map_err(|_| IoError::Other)
    }

    /// Seek
    pub fn seek(&mut self, pos: SeekFrom) -> Result<u64, IoError> {
        let (whence, offset) = match pos {
            SeekFrom::Start(n) => (0, n as i64),
            SeekFrom::End(n) => (2, n),
            SeekFrom::Current(n) => (1, n),
        };

        syscall::fs_seek(self.fd, offset, whence).map_err(|_| IoError::Other)
    }

    /// Get metadata
    pub fn metadata(&self) -> Result<Metadata, IoError> {
        let (size, is_dir, is_file) = syscall::fs_stat_fd(self.fd).map_err(|_| IoError::Other)?;

        Ok(Metadata {
            size,
            is_dir,
            is_file,
        })
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let _ = syscall::fs_close(self.fd);
    }
}

/// Seek position
#[derive(Debug, Clone, Copy)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

/// Open options builder
#[derive(Debug, Clone, Default)]
pub struct OpenOptions {
    read: bool,
    write: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
}

impl OpenOptions {
    pub fn new() -> Self {
        OpenOptions::default()
    }

    pub fn read(&mut self, read: bool) -> &mut Self {
        self.read = read;
        self
    }

    pub fn write(&mut self, write: bool) -> &mut Self {
        self.write = write;
        self
    }

    pub fn append(&mut self, append: bool) -> &mut Self {
        self.append = append;
        self
    }

    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.truncate = truncate;
        self
    }

    pub fn create(&mut self, create: bool) -> &mut Self {
        self.create = create;
        self
    }

    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.create_new = create_new;
        self
    }

    pub fn open(&self, path: &str) -> Result<File, IoError> {
        let flags = encode_flags(self);

        let fd = syscall::fs_open(path, flags).map_err(|e| match e {
            syscall::SyscallError::NotFound => IoError::NotFound,
            syscall::SyscallError::PermissionDenied => IoError::PermissionDenied,
            syscall::SyscallError::AlreadyExists => IoError::AlreadyExists,
            _ => IoError::Other,
        })?;

        Ok(File { fd })
    }
}

fn encode_flags(opts: &OpenOptions) -> u32 {
    let mut flags = 0u32;
    if opts.read {
        flags |= 0x01;
    }
    if opts.write {
        flags |= 0x02;
    }
    if opts.append {
        flags |= 0x04;
    }
    if opts.truncate {
        flags |= 0x08;
    }
    if opts.create {
        flags |= 0x10;
    }
    if opts.create_new {
        flags |= 0x20;
    }
    flags
}

/// File metadata
#[derive(Debug, Clone)]
pub struct Metadata {
    pub size: u64,
    pub is_dir: bool,
    pub is_file: bool,
}

impl Metadata {
    pub fn len(&self) -> u64 {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub fn is_dir(&self) -> bool {
        self.is_dir
    }

    pub fn is_file(&self) -> bool {
        self.is_file
    }
}

/// Directory entry
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

// ============================================
// Free functions
// ============================================

/// Read entire file to string
pub fn read_to_string(path: &str) -> Result<String, IoError> {
    let mut file = File::open(path)?;
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    Ok(s)
}

/// Read entire file to bytes
pub fn read(path: &str) -> Result<Vec<u8>, IoError> {
    let mut file = File::open(path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

/// Write bytes to file
pub fn write(path: &str, contents: &[u8]) -> Result<(), IoError> {
    let mut file = File::create(path)?;
    file.write_all(contents)
}

/// Check if path exists
pub fn exists(path: &str) -> bool {
    metadata(path).is_ok()
}

/// Get metadata for path
pub fn metadata(path: &str) -> Result<Metadata, IoError> {
    let (size, is_dir, is_file) = syscall::fs_stat(path).map_err(|_| IoError::NotFound)?;

    Ok(Metadata {
        size,
        is_dir,
        is_file,
    })
}

/// Read directory
pub fn read_dir(path: &str) -> Result<Vec<DirEntry>, IoError> {
    syscall::fs_readdir(path)
        .map(|entries| {
            entries
                .into_iter()
                .map(|(name, is_dir)| DirEntry { name, is_dir })
                .collect()
        })
        .map_err(|_| IoError::NotFound)
}

/// Create directory
pub fn create_dir(path: &str) -> Result<(), IoError> {
    syscall::fs_mkdir(path)
        .map(|_| ())
        .map_err(|_| IoError::Other)
}

/// Create directory and parents
pub fn create_dir_all(path: &str) -> Result<(), IoError> {
    syscall::fs_mkdir_all(path)
        .map(|_| ())
        .map_err(|_| IoError::Other)
}

/// Remove file
pub fn remove_file(path: &str) -> Result<(), IoError> {
    syscall::fs_unlink(path)
        .map(|_| ())
        .map_err(|_| IoError::NotFound)
}

/// Remove directory
pub fn remove_dir(path: &str) -> Result<(), IoError> {
    syscall::fs_rmdir(path)
        .map(|_| ())
        .map_err(|_| IoError::NotFound)
}

/// Rename file or directory
pub fn rename(from: &str, to: &str) -> Result<(), IoError> {
    syscall::fs_rename(from, to)
        .map(|_| ())
        .map_err(|_| IoError::Other)
}

/// Copy file
pub fn copy(from: &str, to: &str) -> Result<u64, IoError> {
    let contents = read(from)?;
    write(to, &contents)?;
    Ok(contents.len() as u64)
}
