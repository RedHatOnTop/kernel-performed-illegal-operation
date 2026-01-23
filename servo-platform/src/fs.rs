//! File system abstraction layer for KPIO
//!
//! This module provides file and directory operations by communicating
//! with the KPIO kernel's file system service.

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{IoError, PlatformError, Result};
use crate::ipc::ServiceChannel;

/// File system service channel
static mut FS_SERVICE: Option<ServiceChannel> = None;

/// Initialize file system subsystem
pub fn init() {
    log::debug!("[KPIO FS] Initializing file system subsystem");
}

/// File handle
#[derive(Debug)]
pub struct File {
    handle: u64,
    path: String,
    position: u64,
    readable: bool,
    writable: bool,
}

/// Open options
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
    
    pub fn open(&self, path: &str) -> Result<File> {
        let request = FsRequest::Open {
            path: String::from(path),
            read: self.read,
            write: self.write,
            append: self.append,
            truncate: self.truncate,
            create: self.create,
            create_new: self.create_new,
        };
        
        let response = send_fs_request(&request)?;
        
        match response {
            FsResponse::Opened { handle } => {
                Ok(File {
                    handle,
                    path: String::from(path),
                    position: 0,
                    readable: self.read,
                    writable: self.write || self.append,
                })
            }
            FsResponse::Error(e) => Err(PlatformError::Io(e)),
            _ => Err(PlatformError::Io(IoError::Other)),
        }
    }
}

impl File {
    /// Open file for reading
    pub fn open(path: &str) -> Result<File> {
        OpenOptions::new().read(true).open(path)
    }
    
    /// Create file for writing
    pub fn create(path: &str) -> Result<File> {
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
    }
    
    /// Read from file
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if !self.readable {
            return Err(PlatformError::Io(IoError::PermissionDenied));
        }
        
        let request = FsRequest::Read {
            handle: self.handle,
            offset: self.position,
            len: buf.len(),
        };
        
        let response = send_fs_request(&request)?;
        
        match response {
            FsResponse::Data(data) => {
                let len = data.len().min(buf.len());
                buf[..len].copy_from_slice(&data[..len]);
                self.position += len as u64;
                Ok(len)
            }
            FsResponse::Error(e) => Err(PlatformError::Io(e)),
            _ => Err(PlatformError::Io(IoError::Other)),
        }
    }
    
    /// Write to file
    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if !self.writable {
            return Err(PlatformError::Io(IoError::PermissionDenied));
        }
        
        let request = FsRequest::Write {
            handle: self.handle,
            offset: self.position,
            data: buf.to_vec(),
        };
        
        let response = send_fs_request(&request)?;
        
        match response {
            FsResponse::Written(len) => {
                self.position += len as u64;
                Ok(len)
            }
            FsResponse::Error(e) => Err(PlatformError::Io(e)),
            _ => Err(PlatformError::Io(IoError::Other)),
        }
    }
    
    /// Seek to position
    pub fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::End(n) => {
                let size = self.metadata()?.len;
                if n >= 0 {
                    size + n as u64
                } else {
                    size.saturating_sub((-n) as u64)
                }
            }
            SeekFrom::Current(n) => {
                if n >= 0 {
                    self.position + n as u64
                } else {
                    self.position.saturating_sub((-n) as u64)
                }
            }
        };
        
        self.position = new_pos;
        Ok(new_pos)
    }
    
    /// Get file metadata
    pub fn metadata(&self) -> Result<Metadata> {
        let request = FsRequest::Metadata {
            path: self.path.clone(),
        };
        
        let response = send_fs_request(&request)?;
        
        match response {
            FsResponse::Metadata(meta) => Ok(meta),
            FsResponse::Error(e) => Err(PlatformError::Io(e)),
            _ => Err(PlatformError::Io(IoError::Other)),
        }
    }
    
    /// Sync all data to disk
    pub fn sync_all(&self) -> Result<()> {
        let request = FsRequest::Sync { handle: self.handle };
        let _ = send_fs_request(&request)?;
        Ok(())
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let _ = send_fs_request(&FsRequest::Close { handle: self.handle });
    }
}

/// Seek position
#[derive(Debug, Clone, Copy)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

/// File metadata
#[derive(Debug, Clone)]
pub struct Metadata {
    pub len: u64,
    pub is_dir: bool,
    pub is_file: bool,
    pub readonly: bool,
    pub created: u64,
    pub modified: u64,
    pub accessed: u64,
}

/// Directory entry
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub len: u64,
}

/// Read directory
pub fn read_dir(path: &str) -> Result<Vec<DirEntry>> {
    let request = FsRequest::ReadDir {
        path: String::from(path),
    };
    
    let response = send_fs_request(&request)?;
    
    match response {
        FsResponse::DirEntries(entries) => Ok(entries),
        FsResponse::Error(e) => Err(PlatformError::Io(e)),
        _ => Err(PlatformError::Io(IoError::Other)),
    }
}

/// Create directory
pub fn create_dir(path: &str) -> Result<()> {
    let request = FsRequest::CreateDir {
        path: String::from(path),
    };
    
    let response = send_fs_request(&request)?;
    
    match response {
        FsResponse::Ok => Ok(()),
        FsResponse::Error(e) => Err(PlatformError::Io(e)),
        _ => Err(PlatformError::Io(IoError::Other)),
    }
}

/// Create directory and all parents
pub fn create_dir_all(path: &str) -> Result<()> {
    let request = FsRequest::CreateDirAll {
        path: String::from(path),
    };
    
    let response = send_fs_request(&request)?;
    
    match response {
        FsResponse::Ok => Ok(()),
        FsResponse::Error(e) => Err(PlatformError::Io(e)),
        _ => Err(PlatformError::Io(IoError::Other)),
    }
}

/// Remove file
pub fn remove_file(path: &str) -> Result<()> {
    let request = FsRequest::RemoveFile {
        path: String::from(path),
    };
    
    let response = send_fs_request(&request)?;
    
    match response {
        FsResponse::Ok => Ok(()),
        FsResponse::Error(e) => Err(PlatformError::Io(e)),
        _ => Err(PlatformError::Io(IoError::Other)),
    }
}

/// Remove directory
pub fn remove_dir(path: &str) -> Result<()> {
    let request = FsRequest::RemoveDir {
        path: String::from(path),
    };
    
    let response = send_fs_request(&request)?;
    
    match response {
        FsResponse::Ok => Ok(()),
        FsResponse::Error(e) => Err(PlatformError::Io(e)),
        _ => Err(PlatformError::Io(IoError::Other)),
    }
}

/// Check if path exists
pub fn exists(path: &str) -> bool {
    metadata(path).is_ok()
}

/// Get metadata for path
pub fn metadata(path: &str) -> Result<Metadata> {
    let request = FsRequest::Metadata {
        path: String::from(path),
    };
    
    let response = send_fs_request(&request)?;
    
    match response {
        FsResponse::Metadata(meta) => Ok(meta),
        FsResponse::Error(e) => Err(PlatformError::Io(e)),
        _ => Err(PlatformError::Io(IoError::Other)),
    }
}

// ============================================
// Internal protocol
// ============================================

#[derive(Debug)]
enum FsRequest {
    Open {
        path: String,
        read: bool,
        write: bool,
        append: bool,
        truncate: bool,
        create: bool,
        create_new: bool,
    },
    Close { handle: u64 },
    Read { handle: u64, offset: u64, len: usize },
    Write { handle: u64, offset: u64, data: Vec<u8> },
    Sync { handle: u64 },
    Metadata { path: String },
    ReadDir { path: String },
    CreateDir { path: String },
    CreateDirAll { path: String },
    RemoveFile { path: String },
    RemoveDir { path: String },
}

#[derive(Debug)]
enum FsResponse {
    Opened { handle: u64 },
    Data(Vec<u8>),
    Written(usize),
    Metadata(Metadata),
    DirEntries(Vec<DirEntry>),
    Ok,
    Error(IoError),
}

fn send_fs_request(_request: &FsRequest) -> Result<FsResponse> {
    // TODO: Serialize and send via IPC to kernel FS service
    Err(PlatformError::Io(IoError::Other))
}
