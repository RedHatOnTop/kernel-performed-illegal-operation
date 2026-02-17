// WASI Preview 2 — Filesystem (wasi:filesystem/types + preopens)
//
// This module wraps the existing VFS and P1 file descriptor logic
// into the WASI P2 resource-based `Descriptor` model.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use crate::wasi::{Vfs, FdType, WasiError};
use super::streams::{InputStreamData, MemoryInputStream, OutputStreamData, MemoryOutputStream};
use super::StreamError;

// ---------------------------------------------------------------------------
// Descriptor — P2 filesystem resource
// ---------------------------------------------------------------------------

/// Filesystem descriptor — represents an open file or directory.
#[derive(Debug, Clone)]
pub struct Descriptor {
    /// The absolute path within the VFS that this descriptor refers to.
    pub path: String,
    /// Whether this is a file or directory.
    pub kind: DescriptorKind,
    /// Read permission.
    pub readable: bool,
    /// Write permission.
    pub writable: bool,
    /// Current read/write offset (files only).
    pub offset: u64,
}

/// Kind of descriptor — file or directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorKind {
    File,
    Directory,
}

/// File or directory metadata.
#[derive(Debug, Clone)]
pub struct DescriptorStat {
    /// File type.
    pub descriptor_type: DescriptorType,
    /// Size in bytes (0 for directories).
    pub size: u64,
    /// Last data access timestamp (nanoseconds, 0 if unknown).
    pub data_access_timestamp: u64,
    /// Last data modification timestamp (nanoseconds, 0 if unknown).
    pub data_modification_timestamp: u64,
    /// Status change timestamp (nanoseconds, 0 if unknown).
    pub status_change_timestamp: u64,
    /// Number of hard links.
    pub link_count: u64,
}

/// Descriptor type enum (matches wasi:filesystem/types).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorType {
    Unknown,
    BlockDevice,
    CharacterDevice,
    Directory,
    Fifo,
    SymbolicLink,
    RegularFile,
    Socket,
}

/// Directory entry from readdir.
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    /// Entry type.
    pub entry_type: DescriptorType,
    /// Entry name (filename only, not full path).
    pub name: String,
}

/// Metadata hash value for change detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataHashValue {
    pub upper: u64,
    pub lower: u64,
}

// ---------------------------------------------------------------------------
// Descriptor operations (work with VFS)
// ---------------------------------------------------------------------------

impl Descriptor {
    /// Create a new file descriptor.
    pub fn new_file(path: String, readable: bool, writable: bool) -> Self {
        Self {
            path,
            kind: DescriptorKind::File,
            readable,
            writable,
            offset: 0,
        }
    }

    /// Create a new directory descriptor.
    pub fn new_directory(path: String) -> Self {
        Self {
            path,
            kind: DescriptorKind::Directory,
            readable: true,
            writable: false,
            offset: 0,
        }
    }

    /// Open a file relative to this descriptor (must be a directory).
    ///
    /// Returns a new Descriptor for the opened file/directory.
    pub fn open_at(
        &self,
        vfs: &Vfs,
        path: &str,
        create: bool,
        exclusive: bool,
        truncate: bool,
        writable: bool,
    ) -> Result<Descriptor, WasiError> {
        if self.kind != DescriptorKind::Directory {
            return Err(WasiError::NotDir);
        }

        // Resolve the full path
        let full_path = if path.starts_with('/') {
            String::from(path)
        } else {
            let mut base = self.path.clone();
            if !base.ends_with('/') {
                base.push('/');
            }
            base.push_str(path);
            base
        };

        // Check if target is a directory
        if vfs.is_dir(&full_path) {
            return Ok(Descriptor::new_directory(full_path));
        }

        // Handle file open/create
        if vfs.exists(&full_path) {
            if exclusive {
                return Err(WasiError::Exist);
            }
            if truncate {
                // Can't truncate in immutable borrow — caller handles this
            }
            Ok(Descriptor::new_file(full_path, true, writable))
        } else if create {
            // File doesn't exist but create flag is set
            // Caller must create the file in the VFS
            Ok(Descriptor::new_file(full_path, true, writable))
        } else {
            Err(WasiError::NoEnt)
        }
    }

    /// Get the stat of this descriptor.
    pub fn stat(&self, vfs: &Vfs) -> Result<DescriptorStat, WasiError> {
        match self.kind {
            DescriptorKind::Directory => {
                if !vfs.is_dir(&self.path) && self.path != "/" {
                    return Err(WasiError::NoEnt);
                }
                Ok(DescriptorStat {
                    descriptor_type: DescriptorType::Directory,
                    size: 0,
                    data_access_timestamp: 0,
                    data_modification_timestamp: 0,
                    status_change_timestamp: 0,
                    link_count: 1,
                })
            }
            DescriptorKind::File => {
                let size = vfs.file_size(&self.path).map_err(|_| WasiError::NoEnt)?;
                Ok(DescriptorStat {
                    descriptor_type: DescriptorType::RegularFile,
                    size: size as u64,
                    data_access_timestamp: 0,
                    data_modification_timestamp: 0,
                    status_change_timestamp: 0,
                    link_count: 1,
                })
            }
        }
    }

    /// Read the directory contents.
    pub fn readdir(&self, vfs: &Vfs) -> Result<Vec<DirectoryEntry>, WasiError> {
        if self.kind != DescriptorKind::Directory {
            return Err(WasiError::NotDir);
        }

        let entries = vfs.readdir(&self.path).map_err(|_| WasiError::NoEnt)?;
        let mut result = Vec::new();
        for entry in entries {
            let entry_type = if vfs.is_dir(&entry.name) {
                DescriptorType::Directory
            } else {
                DescriptorType::RegularFile
            };
            result.push(DirectoryEntry {
                entry_type,
                name: entry.name,
            });
        }
        Ok(result)
    }

    /// Create an input stream for reading this file.
    pub fn read_via_stream(
        &self,
        vfs: &Vfs,
        offset: u64,
    ) -> Result<InputStreamData, WasiError> {
        if self.kind != DescriptorKind::File {
            return Err(WasiError::IsDir);
        }
        if !self.readable {
            return Err(WasiError::NotCapable);
        }
        let data = vfs.read_file(&self.path).map_err(|_| WasiError::NoEnt)?;
        let offset = offset as usize;
        let sliced = if offset < data.len() {
            data[offset..].to_vec()
        } else {
            Vec::new()
        };
        Ok(InputStreamData::Memory(MemoryInputStream::new(sliced)))
    }

    /// Create an output stream for writing to this file.
    pub fn write_via_stream(&self) -> Result<OutputStreamData, WasiError> {
        if self.kind != DescriptorKind::File {
            return Err(WasiError::IsDir);
        }
        if !self.writable {
            return Err(WasiError::NotCapable);
        }
        Ok(OutputStreamData::Memory(MemoryOutputStream::new()))
    }

    /// Compute a hash of the file metadata for change detection.
    pub fn metadata_hash(&self, vfs: &Vfs) -> Result<MetadataHashValue, WasiError> {
        let stat = self.stat(vfs)?;
        // Simple hash: combine size and type
        let upper = stat.size;
        let lower = stat.descriptor_type as u64;
        Ok(MetadataHashValue { upper, lower })
    }
}

// ---------------------------------------------------------------------------
// Preopens — pre-opened directories
// ---------------------------------------------------------------------------

/// A pre-opened directory for WASI P2.
#[derive(Debug, Clone)]
pub struct Preopen {
    /// The guest-visible path (e.g., "/").
    pub guest_path: String,
    /// The descriptor for this preopen.
    pub descriptor: Descriptor,
}

/// Get the default preopens (root directory).
pub fn default_preopens() -> Vec<Preopen> {
    let mut preopens = Vec::new();
    preopens.push(Preopen {
        guest_path: String::from("/"),
        descriptor: Descriptor::new_directory(String::from("/")),
    });
    preopens
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasi::Vfs;

    #[test]
    fn descriptor_new_file() {
        let desc = Descriptor::new_file(String::from("/test.txt"), true, false);
        assert_eq!(desc.kind, DescriptorKind::File);
        assert!(desc.readable);
        assert!(!desc.writable);
        assert_eq!(desc.offset, 0);
    }

    #[test]
    fn descriptor_new_directory() {
        let desc = Descriptor::new_directory(String::from("/data"));
        assert_eq!(desc.kind, DescriptorKind::Directory);
        assert!(desc.readable);
    }

    #[test]
    fn descriptor_stat_file() {
        let mut vfs = Vfs::new();
        vfs.create_file("/hello.txt", b"Hello!".to_vec()).unwrap();
        let desc = Descriptor::new_file(String::from("/hello.txt"), true, false);
        let stat = desc.stat(&vfs).unwrap();
        assert_eq!(stat.descriptor_type, DescriptorType::RegularFile);
        assert_eq!(stat.size, 6);
    }

    #[test]
    fn descriptor_stat_directory() {
        let vfs = Vfs::new();
        let desc = Descriptor::new_directory(String::from("/"));
        let stat = desc.stat(&vfs).unwrap();
        assert_eq!(stat.descriptor_type, DescriptorType::Directory);
    }

    #[test]
    fn descriptor_open_at_existing_file() {
        let mut vfs = Vfs::new();
        vfs.create_dir("/data").unwrap();
        vfs.create_file("/data/test.txt", b"content".to_vec()).unwrap();
        let dir = Descriptor::new_directory(String::from("/data"));
        let file = dir.open_at(&vfs, "test.txt", false, false, false, false).unwrap();
        assert_eq!(file.kind, DescriptorKind::File);
        assert_eq!(file.path, "/data/test.txt");
    }

    #[test]
    fn descriptor_open_at_nonexistent() {
        let vfs = Vfs::new();
        let dir = Descriptor::new_directory(String::from("/"));
        let result = dir.open_at(&vfs, "missing.txt", false, false, false, false);
        assert!(result.is_err());
    }

    #[test]
    fn descriptor_read_via_stream() {
        let mut vfs = Vfs::new();
        vfs.create_file("/data.bin", alloc::vec![1, 2, 3, 4, 5]).unwrap();
        let desc = Descriptor::new_file(String::from("/data.bin"), true, false);
        let mut stream = desc.read_via_stream(&vfs, 0).unwrap();
        let data = stream.read(10).unwrap();
        assert_eq!(data, alloc::vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn descriptor_read_via_stream_with_offset() {
        let mut vfs = Vfs::new();
        vfs.create_file("/data.bin", alloc::vec![10, 20, 30, 40, 50]).unwrap();
        let desc = Descriptor::new_file(String::from("/data.bin"), true, false);
        let mut stream = desc.read_via_stream(&vfs, 2).unwrap();
        let data = stream.read(10).unwrap();
        assert_eq!(data, alloc::vec![30, 40, 50]);
    }

    #[test]
    fn descriptor_write_via_stream() {
        let desc = Descriptor::new_file(String::from("/out.txt"), true, true);
        let stream = desc.write_via_stream();
        assert!(stream.is_ok());
    }

    #[test]
    fn descriptor_write_not_writable() {
        let desc = Descriptor::new_file(String::from("/out.txt"), true, false);
        let result = desc.write_via_stream();
        assert!(result.is_err());
    }

    #[test]
    fn descriptor_readdir() {
        let mut vfs = Vfs::new();
        vfs.create_dir("/mydir").unwrap();
        vfs.create_file("/mydir/a.txt", b"a".to_vec()).unwrap();
        vfs.create_file("/mydir/b.txt", b"b".to_vec()).unwrap();
        let dir = Descriptor::new_directory(String::from("/mydir"));
        let entries = dir.readdir(&vfs).unwrap();
        assert!(entries.len() >= 2);
    }

    #[test]
    fn descriptor_metadata_hash() {
        let mut vfs = Vfs::new();
        vfs.create_file("/hash_test.txt", b"test content".to_vec()).unwrap();
        let desc = Descriptor::new_file(String::from("/hash_test.txt"), true, false);
        let hash = desc.metadata_hash(&vfs).unwrap();
        assert_eq!(hash.upper, 12); // size = 12
    }

    #[test]
    fn default_preopens_has_root() {
        let preopens = default_preopens();
        assert_eq!(preopens.len(), 1);
        assert_eq!(preopens[0].guest_path, "/");
    }
}
