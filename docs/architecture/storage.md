# Storage Subsystem Design Document

**Document Version:** 1.1.0  
**Last Updated:** 2026-02-24  
**Status:** Implemented (VirtIO block + FAT32 read-only VFS working; E2E integration test verified)

---

## Table of Contents

1. [Overview](#1-overview)
2. [Design Principles](#2-design-principles)
3. [Architecture](#3-architecture)
4. [Virtual Filesystem (VFS)](#4-virtual-filesystem-vfs)
5. [Immutable Root Filesystem](#5-immutable-root-filesystem)
6. [FUSE-like WASM Modules](#6-fuse-like-wasm-modules)
7. [Filesystem Drivers](#7-filesystem-drivers)
8. [Block Device Layer](#8-block-device-layer)
9. [Cache Management](#9-cache-management)
10. [Security and Capabilities](#10-security-and-capabilities)

---

## 1. Overview

### 1.1 Purpose

This document specifies the design of the KPIO storage subsystem, which provides a secure and flexible storage layer through user-space filesystem implementations running as WASM modules.

### 1.2 Scope

This document covers:
- Virtual Filesystem (VFS) abstraction layer
- Immutable root filesystem design
- FUSE-like WASM filesystem interface
- Filesystem drivers (EXT4, NTFS, FAT32, exFAT)
- Block device interface
- Caching and performance

This document does NOT cover:
- RAID implementation (future extension)
- Network filesystems (future extension)
- Encryption at rest (addressed separately)

### 1.3 Source Location

```
storage/
    Cargo.toml
    src/
        lib.rs
        vfs/
            mod.rs
            path.rs         # Path resolution
            mount.rs        # Mount point management
            inode.rs        # Inode abstraction
            dentry.rs       # Directory entry cache
            file.rs         # File handle operations
        fs/
            mod.rs
            fuse.rs         # FUSE protocol implementation
            ext4/
                mod.rs
                superblock.rs
                inode.rs
                extent.rs
                journal.rs
            ntfs/
                mod.rs
                mft.rs
                attribute.rs
                index.rs
            fat/
                mod.rs
                fat32.rs
                exfat.rs
        block/
            mod.rs
            virtio.rs       # VirtIO-Blk driver
            nvme.rs         # NVMe driver (future)
            partition.rs    # Partition table parsing
        cache/
            mod.rs
            page.rs         # Page cache
            buffer.rs       # Buffer cache
            writeback.rs    # Write-back management
```

---

## 2. Design Principles

### 2.1 User-Space Filesystems

All filesystem implementations run in user space as WASM modules:

| Benefit | Description |
|---------|-------------|
| Isolation | Filesystem bugs cannot crash the kernel |
| Sandboxing | Limited access to system resources |
| Hot-Swappable | Can upgrade filesystems without reboot |
| Debuggable | Standard debugging and profiling tools apply |

### 2.2 Immutable System Root

The root filesystem is immutable by design:

- System files are read-only
- Updates are atomic (A/B partition switching)
- User data is on separate mutable partitions
- Reduces attack surface significantly

### 2.3 Capability-Based Access

All filesystem access is governed by capabilities:

- Applications declare required paths upfront
- Runtime grants minimal necessary access
- No ambient authority to browse filesystem

---

## 3. Architecture

### 3.1 Storage Stack Overview

```
+=========================================================================+
|                        APPLICATION LAYER                                 |
+=========================================================================+
|                                                                          |
|  +------------------+  +------------------+  +------------------------+  |
|  |  WASM App        |  |  Shell           |  |  File Manager          |  |
|  |  (File I/O)      |  |  (WASM Service)  |  |  (WASM Service)        |  |
|  +--------+---------+  +--------+---------+  +-----------+------------+  |
|           |                     |                        |               |
|           +---------------------+------------------------+               |
|                                 |                                        |
+=========================================================================+
|                        WASI FILESYSTEM                                   |
+=========================================================================+
|                                                                          |
|  +--------------------------------------------------------------------+  |
|  |                    WASI Filesystem Interface                        |  |
|  |  - fd_read, fd_write, fd_seek                                       |  |
|  |  - path_open, path_create_directory, path_remove_directory          |  |
|  |  - path_rename, path_symlink, path_readlink                         |  |
|  +-------------------------------+------------------------------------+  |
|                                  |                                       |
+=========================================================================+
|                        VIRTUAL FILESYSTEM (VFS)                          |
+=========================================================================+
|                                                                          |
|  +--------------------------------------------------------------------+  |
|  |                    VFS Layer (Kernel)                               |  |
|  |  - Mount table                                                      |  |
|  |  - Path resolution                                                  |  |
|  |  - Dentry cache                                                     |  |
|  |  - Inode cache                                                      |  |
|  +--------------------------------------------------------------------+  |
|                                                                          |
|  +------------------+  +------------------+  +------------------------+  |
|  |  Root FS         |  |  User FS         |  |  Temp FS               |  |
|  |  (Immutable)     |  |  (Mutable)       |  |  (RAM-backed)          |  |
|  +--------+---------+  +--------+---------+  +-----------+------------+  |
|           |                     |                        |               |
+=========================================================================+
|                        FUSE PROTOCOL                                     |
+=========================================================================+
|                                                                          |
|  +--------------------------------------------------------------------+  |
|  |                    FUSE Message Dispatcher                          |  |
|  |  (IPC between VFS and WASM FS drivers)                              |  |
|  +-------------------------------+------------------------------------+  |
|                                  |                                       |
+=========================================================================+
|                        FILESYSTEM DRIVERS (WASM)                         |
+=========================================================================+
|                                                                          |
|  +------------------+  +------------------+  +------------------------+  |
|  |  EXT4 Driver     |  |  NTFS Driver     |  |  FAT32 Driver          |  |
|  |  (WASM Module)   |  |  (WASM Module)   |  |  (WASM Module)         |  |
|  +--------+---------+  +--------+---------+  +-----------+------------+  |
|           |                     |                        |               |
+=========================================================================+
|                        BLOCK DEVICE LAYER                                |
+=========================================================================+
|                                                                          |
|  +------------------+  +------------------+  +------------------------+  |
|  |  VirtIO-Blk      |  |  NVMe (Future)   |  |  RAM Disk              |  |
|  +------------------+  +------------------+  +------------------------+  |
|                                                                          |
+=========================================================================+
```

### 3.2 Data Flow

```
Application                VFS              FUSE              FS Driver
    |                       |                |                    |
    | path_open("/data/x")  |                |                    |
    |---------------------->|                |                    |
    |                       | resolve path   |                    |
    |                       |--------------->|                    |
    |                       |                | FUSE_LOOKUP        |
    |                       |                |------------------->|
    |                       |                |                    |
    |                       |                |<--- inode info ----|
    |                       |                |                    |
    |                       |<---------------|                    |
    |<--- fd --------------|                |                    |
    |                       |                |                    |
    | fd_read(fd, buf, len) |                |                    |
    |---------------------->|                |                    |
    |                       | check cache    |                    |
    |                       |                |                    |
    |                       | (cache miss)   |                    |
    |                       |--------------->|                    |
    |                       |                | FUSE_READ          |
    |                       |                |------------------->|
    |                       |                |                    |
    |                       |                |<--- data ----------|
    |                       |                |                    |
    |                       |<-- update -----|                    |
    |                       |   cache        |                    |
    |<--- data -------------|                |                    |
```

---

## 4. Virtual Filesystem (VFS)

### 4.1 VFS Core Structure

```rust
// storage/src/vfs/mod.rs

use alloc::sync::Arc;
use spin::RwLock;

pub struct Vfs {
    /// Mount table
    mounts: RwLock<MountTable>,
    
    /// Dentry cache
    dentry_cache: DentryCache,
    
    /// Inode cache
    inode_cache: InodeCache,
    
    /// Open file table (system-wide)
    file_table: RwLock<FileTable>,
    
    /// FUSE dispatcher
    fuse: FuseDispatcher,
}

impl Vfs {
    pub fn new() -> Self {
        Self {
            mounts: RwLock::new(MountTable::new()),
            dentry_cache: DentryCache::new(10000),
            inode_cache: InodeCache::new(10000),
            file_table: RwLock::new(FileTable::new()),
            fuse: FuseDispatcher::new(),
        }
    }
    
    /// Open a file by path
    pub async fn open(
        &self,
        path: &Path,
        flags: OpenFlags,
        caps: &Capabilities,
    ) -> Result<FileHandle, VfsError> {
        // Validate capability
        if !caps.can_access(path, flags) {
            return Err(VfsError::PermissionDenied);
        }
        
        // Resolve path to inode
        let (mount, inode) = self.resolve_path(path).await?;
        
        // Check open flags against inode mode
        self.check_access(&inode, flags)?;
        
        // Create file handle
        let file = File::new(mount, inode, flags);
        let handle = self.file_table.write().insert(file);
        
        Ok(handle)
    }
    
    /// Read from an open file
    pub async fn read(
        &self,
        handle: FileHandle,
        buf: &mut [u8],
    ) -> Result<usize, VfsError> {
        let file = self.file_table.read()
            .get(handle)
            .ok_or(VfsError::BadFileHandle)?
            .clone();
        
        // Check if file is open for reading
        if !file.flags.contains(OpenFlags::READ) {
            return Err(VfsError::PermissionDenied);
        }
        
        // Get current position
        let pos = file.position();
        
        // Try page cache first
        if let Some(data) = self.page_cache.read(&file.inode, pos, buf.len()) {
            buf[..data.len()].copy_from_slice(&data);
            file.advance_position(data.len());
            return Ok(data.len());
        }
        
        // Cache miss - request from filesystem
        let request = FuseRequest::Read {
            inode: file.inode.number,
            offset: pos,
            size: buf.len() as u32,
        };
        
        let response = self.fuse.send(&file.mount, request).await?;
        
        match response {
            FuseResponse::Data(data) => {
                let len = data.len().min(buf.len());
                buf[..len].copy_from_slice(&data[..len]);
                
                // Update cache
                self.page_cache.insert(&file.inode, pos, &data);
                
                // Update position
                file.advance_position(len);
                
                Ok(len)
            }
            FuseResponse::Error(e) => Err(e.into()),
            _ => Err(VfsError::InvalidResponse),
        }
    }
    
    /// Write to an open file
    pub async fn write(
        &self,
        handle: FileHandle,
        data: &[u8],
    ) -> Result<usize, VfsError> {
        let file = self.file_table.read()
            .get(handle)
            .ok_or(VfsError::BadFileHandle)?
            .clone();
        
        // Check if file is open for writing
        if !file.flags.contains(OpenFlags::WRITE) {
            return Err(VfsError::PermissionDenied);
        }
        
        // Check if filesystem is read-only
        if file.mount.read_only {
            return Err(VfsError::ReadOnlyFilesystem);
        }
        
        let pos = file.position();
        
        // Send write request
        let request = FuseRequest::Write {
            inode: file.inode.number,
            offset: pos,
            data: data.to_vec(),
        };
        
        let response = self.fuse.send(&file.mount, request).await?;
        
        match response {
            FuseResponse::Written { size } => {
                // Invalidate cache
                self.page_cache.invalidate(&file.inode, pos, size);
                
                // Update position
                file.advance_position(size);
                
                Ok(size)
            }
            FuseResponse::Error(e) => Err(e.into()),
            _ => Err(VfsError::InvalidResponse),
        }
    }
    
    /// Resolve a path to mount point and inode
    async fn resolve_path(&self, path: &Path) -> Result<(Arc<Mount>, Inode), VfsError> {
        // Start from root
        let mounts = self.mounts.read();
        let root_mount = mounts.root().ok_or(VfsError::NoRootFilesystem)?;
        
        let mut current_mount = root_mount.clone();
        let mut current_inode = self.get_root_inode(&current_mount).await?;
        
        for component in path.components() {
            // Check for mount point crossing
            if let Some(child_mount) = mounts.get_mount(&current_inode, component) {
                current_mount = child_mount;
                current_inode = self.get_root_inode(&current_mount).await?;
                continue;
            }
            
            // Check dentry cache
            let cache_key = (current_inode.number, component.to_string());
            if let Some(cached) = self.dentry_cache.get(&cache_key) {
                current_inode = self.inode_cache.get(cached)
                    .ok_or(VfsError::StaleInode)?;
                continue;
            }
            
            // Lookup via FUSE
            let request = FuseRequest::Lookup {
                parent: current_inode.number,
                name: component.to_string(),
            };
            
            let response = self.fuse.send(&current_mount, request).await?;
            
            match response {
                FuseResponse::Entry { inode, attr } => {
                    // Cache the entry
                    self.dentry_cache.insert(cache_key, inode);
                    self.inode_cache.insert(inode, Inode::from_attr(inode, attr));
                    
                    current_inode = self.inode_cache.get(inode)
                        .ok_or(VfsError::StaleInode)?;
                }
                FuseResponse::Error(FuseError::NotFound) => {
                    return Err(VfsError::NotFound);
                }
                FuseResponse::Error(e) => return Err(e.into()),
                _ => return Err(VfsError::InvalidResponse),
            }
        }
        
        Ok((current_mount, current_inode))
    }
}
```

### 4.2 Path Resolution

```rust
// storage/src/vfs/path.rs

use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub struct Path {
    /// Normalized path components
    components: Vec<String>,
    
    /// Whether path is absolute
    absolute: bool,
}

impl Path {
    pub fn parse(path: &str) -> Result<Self, PathError> {
        let absolute = path.starts_with('/');
        let components: Vec<String> = path
            .split('/')
            .filter(|s| !s.is_empty() && *s != ".")
            .map(|s| {
                // Validate component
                if s.len() > 255 {
                    return Err(PathError::ComponentTooLong);
                }
                if s.contains('\0') {
                    return Err(PathError::InvalidCharacter);
                }
                Ok(s.to_string())
            })
            .collect::<Result<_, _>>()?;
        
        // Resolve ".."
        let mut resolved = Vec::new();
        for component in components {
            if component == ".." {
                if resolved.is_empty() && absolute {
                    // Can't go above root
                    continue;
                }
                resolved.pop();
            } else {
                resolved.push(component);
            }
        }
        
        Ok(Self {
            components: resolved,
            absolute,
        })
    }
    
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.components.iter().map(|s| s.as_str())
    }
    
    pub fn join(&self, other: &Path) -> Path {
        if other.absolute {
            return other.clone();
        }
        
        let mut components = self.components.clone();
        components.extend(other.components.iter().cloned());
        
        Path {
            components,
            absolute: self.absolute,
        }
    }
    
    pub fn parent(&self) -> Option<Path> {
        if self.components.is_empty() {
            return None;
        }
        
        let mut components = self.components.clone();
        components.pop();
        
        Some(Path {
            components,
            absolute: self.absolute,
        })
    }
    
    pub fn filename(&self) -> Option<&str> {
        self.components.last().map(|s| s.as_str())
    }
}
```

### 4.3 Mount Management

```rust
// storage/src/vfs/mount.rs

use alloc::sync::Arc;
use alloc::collections::BTreeMap;

pub struct MountTable {
    /// Root mount
    root: Option<Arc<Mount>>,
    
    /// Mount points indexed by (parent inode, name)
    mounts: BTreeMap<(u64, String), Arc<Mount>>,
    
    /// All mounts for iteration
    all_mounts: Vec<Arc<Mount>>,
}

pub struct Mount {
    /// Unique mount ID
    pub id: u64,
    
    /// Filesystem type
    pub fs_type: String,
    
    /// Source device (if any)
    pub source: Option<String>,
    
    /// Mount point path
    pub mount_point: Path,
    
    /// Read-only flag
    pub read_only: bool,
    
    /// Filesystem service channel
    pub fs_channel: IpcChannel,
    
    /// Mount options
    pub options: MountOptions,
}

#[derive(Debug, Clone)]
pub struct MountOptions {
    /// Disallow execution
    pub noexec: bool,
    
    /// Disallow setuid
    pub nosuid: bool,
    
    /// Disallow device nodes
    pub nodev: bool,
    
    /// Access time updates
    pub atime: AtimeMode,
}

#[derive(Debug, Clone, Copy)]
pub enum AtimeMode {
    /// Always update atime
    Always,
    /// Update only if atime < mtime
    Relative,
    /// Never update atime
    Never,
}

impl MountTable {
    pub fn mount(
        &mut self,
        source: Option<&str>,
        target: &Path,
        fs_type: &str,
        options: MountOptions,
        fs_channel: IpcChannel,
    ) -> Result<Arc<Mount>, MountError> {
        // Check if already mounted
        if target.components.is_empty() {
            if self.root.is_some() {
                return Err(MountError::AlreadyMounted);
            }
        }
        
        let mount = Arc::new(Mount {
            id: self.next_mount_id(),
            fs_type: fs_type.to_string(),
            source: source.map(|s| s.to_string()),
            mount_point: target.clone(),
            read_only: options.noexec, // TODO: proper read-only flag
            fs_channel,
            options,
        });
        
        if target.components.is_empty() {
            self.root = Some(mount.clone());
        } else {
            // Resolve parent inode
            // TODO: proper parent resolution
            let key = (0, target.filename().unwrap_or("").to_string());
            self.mounts.insert(key, mount.clone());
        }
        
        self.all_mounts.push(mount.clone());
        
        Ok(mount)
    }
    
    pub fn unmount(&mut self, target: &Path) -> Result<(), MountError> {
        // Find mount
        let mount = self.all_mounts.iter()
            .find(|m| m.mount_point.components == target.components)
            .ok_or(MountError::NotMounted)?;
        
        // Check for submounts
        let has_submounts = self.all_mounts.iter()
            .any(|m| m.mount_point.to_string().starts_with(&target.to_string()));
        
        if has_submounts {
            return Err(MountError::Busy);
        }
        
        // Remove mount
        let id = mount.id;
        self.all_mounts.retain(|m| m.id != id);
        
        Ok(())
    }
}
```

### 4.4 Inode Abstraction

```rust
// storage/src/vfs/inode.rs

use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone)]
pub struct Inode {
    /// Inode number (unique within filesystem)
    pub number: u64,
    
    /// File type and mode
    pub mode: InodeMode,
    
    /// Owner user ID
    pub uid: u32,
    
    /// Owner group ID
    pub gid: u32,
    
    /// File size in bytes
    pub size: u64,
    
    /// Number of hard links
    pub nlink: u32,
    
    /// Access time
    pub atime: Timestamp,
    
    /// Modification time
    pub mtime: Timestamp,
    
    /// Change time
    pub ctime: Timestamp,
    
    /// Block count (512-byte blocks)
    pub blocks: u64,
    
    /// Generation number (for NFS)
    pub generation: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct InodeMode {
    /// File type
    pub file_type: FileType,
    
    /// Permission bits
    pub permissions: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Regular,
    Directory,
    Symlink,
    CharDevice,
    BlockDevice,
    Fifo,
    Socket,
}

#[derive(Debug, Clone, Copy)]
pub struct Timestamp {
    pub seconds: i64,
    pub nanoseconds: u32,
}

impl Inode {
    pub fn from_attr(number: u64, attr: FuseAttr) -> Self {
        Self {
            number,
            mode: InodeMode {
                file_type: FileType::from_mode(attr.mode),
                permissions: attr.mode & 0o7777,
            },
            uid: attr.uid,
            gid: attr.gid,
            size: attr.size,
            nlink: attr.nlink,
            atime: Timestamp {
                seconds: attr.atime,
                nanoseconds: attr.atimensec,
            },
            mtime: Timestamp {
                seconds: attr.mtime,
                nanoseconds: attr.mtimensec,
            },
            ctime: Timestamp {
                seconds: attr.ctime,
                nanoseconds: attr.ctimensec,
            },
            blocks: attr.blocks,
            generation: 0,
        }
    }
    
    pub fn is_directory(&self) -> bool {
        self.mode.file_type == FileType::Directory
    }
    
    pub fn is_regular(&self) -> bool {
        self.mode.file_type == FileType::Regular
    }
    
    pub fn is_symlink(&self) -> bool {
        self.mode.file_type == FileType::Symlink
    }
}

pub struct InodeCache {
    cache: RwLock<LruCache<u64, Inode>>,
}

impl InodeCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: RwLock::new(LruCache::new(capacity)),
        }
    }
    
    pub fn get(&self, ino: u64) -> Option<Inode> {
        self.cache.write().get(&ino).cloned()
    }
    
    pub fn insert(&self, ino: u64, inode: Inode) {
        self.cache.write().insert(ino, inode);
    }
    
    pub fn invalidate(&self, ino: u64) {
        self.cache.write().remove(&ino);
    }
}
```

---

## 5. Immutable Root Filesystem

### 5.1 Design Overview

```
+------------------------------------------------------------------+
|                    DISK LAYOUT                                     |
+------------------------------------------------------------------+
|                                                                    |
|  +--------------+  +--------------+  +-------------------------+  |
|  |  EFI System  |  |  Boot        |  |  Root A (Immutable)     |  |
|  |  Partition   |  |  Partition   |  |  - /bin, /lib, /etc     |  |
|  |  (FAT32)     |  |  (EXT4)      |  |  - Verified on boot     |  |
|  +--------------+  +--------------+  +-------------------------+  |
|                                                                    |
|  +-------------------------+  +----------------------------------+ |
|  |  Root B (Immutable)     |  |  Data Partition (Mutable)        | |
|  |  - Backup root          |  |  - /home                          | |
|  |  - For A/B updates      |  |  - /var                           | |
|  +-------------------------+  +----------------------------------+ |
|                                                                    |
+------------------------------------------------------------------+
```

### 5.2 Root Filesystem Service

```rust
// storage/src/fs/root.rs

pub struct ImmutableRootFs {
    /// Underlying block device
    device: BlockDevice,
    
    /// Partition info
    partition: PartitionInfo,
    
    /// SquashFS image reader
    squashfs: SquashFs,
    
    /// Verification state
    verified: bool,
    
    /// Hash tree for dm-verity style verification
    hash_tree: Option<HashTree>,
}

impl ImmutableRootFs {
    pub async fn mount(
        device: BlockDevice,
        partition: PartitionInfo,
    ) -> Result<Self, FsError> {
        let squashfs = SquashFs::open(&device, partition.start_lba)?;
        
        // Verify integrity if hash tree present
        let hash_tree = Self::load_hash_tree(&device, &partition)?;
        let verified = if let Some(ref tree) = hash_tree {
            tree.verify_root().await?
        } else {
            false
        };
        
        Ok(Self {
            device,
            partition,
            squashfs,
            verified,
            hash_tree,
        })
    }
    
    fn handle_fuse_request(&self, request: FuseRequest) -> FuseResponse {
        match request {
            FuseRequest::Lookup { parent, name } => {
                self.lookup(parent, &name)
            }
            FuseRequest::Read { inode, offset, size } => {
                // Verify block integrity before returning
                if let Some(ref tree) = self.hash_tree {
                    let block = offset / BLOCK_SIZE as u64;
                    if !tree.verify_block(block) {
                        return FuseResponse::Error(FuseError::CorruptedData);
                    }
                }
                
                self.read(inode, offset, size)
            }
            FuseRequest::Write { .. } => {
                // Read-only filesystem
                FuseResponse::Error(FuseError::ReadOnlyFilesystem)
            }
            FuseRequest::Readdir { inode, offset } => {
                self.readdir(inode, offset)
            }
            FuseRequest::Getattr { inode } => {
                self.getattr(inode)
            }
            _ => FuseResponse::Error(FuseError::NotSupported),
        }
    }
}

struct HashTree {
    /// Root hash (known good value)
    root_hash: [u8; 32],
    
    /// Tree levels
    levels: Vec<Vec<[u8; 32]>>,
    
    /// Block size for hashing
    block_size: usize,
}

impl HashTree {
    async fn verify_root(&self) -> Result<bool, FsError> {
        // Calculate root hash from level 0
        let calculated = self.calculate_root_hash()?;
        Ok(calculated == self.root_hash)
    }
    
    fn verify_block(&self, block_num: u64) -> bool {
        // Walk up the tree verifying each level
        let mut current_hash = self.calculate_block_hash(block_num);
        let mut index = block_num as usize;
        
        for level in &self.levels {
            let expected = &level[index];
            if current_hash != *expected {
                return false;
            }
            
            // Move up
            let sibling_index = index ^ 1;
            let sibling = level.get(sibling_index)
                .unwrap_or(&[0u8; 32]);
            
            current_hash = if index % 2 == 0 {
                hash_pair(&current_hash, sibling)
            } else {
                hash_pair(sibling, &current_hash)
            };
            
            index /= 2;
        }
        
        current_hash == self.root_hash
    }
}
```

### 5.3 Overlay Filesystem

For development and temporary modifications:

```rust
// storage/src/fs/overlay.rs

pub struct OverlayFs {
    /// Lower layer (read-only)
    lower: Arc<dyn Filesystem>,
    
    /// Upper layer (read-write)
    upper: Arc<dyn Filesystem>,
    
    /// Work directory for atomic operations
    work: Arc<dyn Filesystem>,
    
    /// Whiteout tracking
    whiteouts: HashSet<Path>,
}

impl OverlayFs {
    fn handle_fuse_request(&mut self, request: FuseRequest) -> FuseResponse {
        match request {
            FuseRequest::Lookup { parent, name } => {
                // Check whiteout first
                if self.is_whiteout(parent, &name) {
                    return FuseResponse::Error(FuseError::NotFound);
                }
                
                // Check upper layer
                if let Ok(response) = self.upper.lookup(parent, &name) {
                    return response;
                }
                
                // Fall through to lower layer
                self.lower.lookup(parent, &name)
            }
            
            FuseRequest::Read { inode, offset, size } => {
                // Determine which layer has the file
                if self.is_in_upper(inode) {
                    self.upper.read(inode, offset, size)
                } else {
                    self.lower.read(inode, offset, size)
                }
            }
            
            FuseRequest::Write { inode, offset, data } => {
                // Copy-on-write: copy from lower to upper if needed
                if !self.is_in_upper(inode) {
                    self.copy_up(inode)?;
                }
                
                self.upper.write(inode, offset, data)
            }
            
            FuseRequest::Unlink { parent, name } => {
                // Create whiteout
                if self.is_in_lower(parent, &name) {
                    self.create_whiteout(parent, &name)?;
                }
                
                // Remove from upper if present
                if self.is_in_upper(parent, &name) {
                    self.upper.unlink(parent, &name)?;
                }
                
                FuseResponse::Ok
            }
            
            _ => {
                // Other operations go to upper layer
                self.upper.handle_request(request)
            }
        }
    }
    
    fn copy_up(&mut self, inode: u64) -> Result<u64, FsError> {
        // Read from lower
        let attr = self.lower.getattr(inode)?;
        let data = self.lower.read_all(inode)?;
        
        // Create in upper
        let upper_inode = self.upper.create(attr.mode, attr.uid, attr.gid)?;
        self.upper.write(upper_inode, 0, data)?;
        
        // Update mapping
        self.inode_mapping.insert(inode, upper_inode);
        
        Ok(upper_inode)
    }
}
```

---

## 6. FUSE-like WASM Modules

### 6.1 FUSE Protocol

```rust
// storage/src/fs/fuse.rs

#[derive(Debug, Serialize, Deserialize)]
pub enum FuseRequest {
    // Inode operations
    Lookup {
        parent: u64,
        name: String,
    },
    Forget {
        inode: u64,
        nlookup: u64,
    },
    Getattr {
        inode: u64,
    },
    Setattr {
        inode: u64,
        attr: SetAttr,
    },
    
    // File operations
    Open {
        inode: u64,
        flags: u32,
    },
    Read {
        inode: u64,
        offset: u64,
        size: u32,
    },
    Write {
        inode: u64,
        offset: u64,
        data: Vec<u8>,
    },
    Flush {
        inode: u64,
    },
    Release {
        inode: u64,
    },
    Fsync {
        inode: u64,
        datasync: bool,
    },
    
    // Directory operations
    Opendir {
        inode: u64,
    },
    Readdir {
        inode: u64,
        offset: u64,
    },
    Releasedir {
        inode: u64,
    },
    
    // Name operations
    Mknod {
        parent: u64,
        name: String,
        mode: u32,
        rdev: u32,
    },
    Mkdir {
        parent: u64,
        name: String,
        mode: u32,
    },
    Unlink {
        parent: u64,
        name: String,
    },
    Rmdir {
        parent: u64,
        name: String,
    },
    Rename {
        parent: u64,
        name: String,
        new_parent: u64,
        new_name: String,
    },
    Link {
        inode: u64,
        new_parent: u64,
        new_name: String,
    },
    Symlink {
        parent: u64,
        name: String,
        target: String,
    },
    Readlink {
        inode: u64,
    },
    
    // Extended attributes
    Getxattr {
        inode: u64,
        name: String,
    },
    Setxattr {
        inode: u64,
        name: String,
        value: Vec<u8>,
        flags: u32,
    },
    Listxattr {
        inode: u64,
    },
    Removexattr {
        inode: u64,
        name: String,
    },
    
    // Filesystem operations
    Statfs,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FuseResponse {
    // Success responses
    Entry {
        inode: u64,
        attr: FuseAttr,
    },
    Attr(FuseAttr),
    Data(Vec<u8>),
    Written {
        size: usize,
    },
    Opened {
        fh: u64,
    },
    Dirents(Vec<DirEntry>),
    Created {
        inode: u64,
        attr: FuseAttr,
    },
    Link {
        target: String,
    },
    Xattr(Vec<u8>),
    XattrList(Vec<String>),
    Statfs(StatfsInfo),
    Ok,
    
    // Error responses
    Error(FuseError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuseAttr {
    pub ino: u64,
    pub size: u64,
    pub blocks: u64,
    pub atime: i64,
    pub mtime: i64,
    pub ctime: i64,
    pub atimensec: u32,
    pub mtimensec: u32,
    pub ctimensec: u32,
    pub mode: u32,
    pub nlink: u32,
    pub uid: u32,
    pub gid: u32,
    pub rdev: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    pub inode: u64,
    pub offset: u64,
    pub file_type: u8,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatfsInfo {
    pub blocks: u64,
    pub bfree: u64,
    pub bavail: u64,
    pub files: u64,
    pub ffree: u64,
    pub bsize: u32,
    pub namelen: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum FuseError {
    NotFound,
    PermissionDenied,
    Exists,
    NotDirectory,
    IsDirectory,
    InvalidArgument,
    NoSpace,
    ReadOnlyFilesystem,
    NotEmpty,
    NameTooLong,
    NoEntry,
    IO,
    CorruptedData,
    NotSupported,
}
```

### 6.2 FUSE Dispatcher

```rust
// storage/src/fs/fuse.rs

pub struct FuseDispatcher {
    /// Registered filesystem handlers
    handlers: RwLock<HashMap<u64, FuseHandler>>,
    
    /// Request ID counter
    next_request_id: AtomicU64,
    
    /// Pending requests
    pending: RwLock<HashMap<u64, PendingRequest>>,
}

struct FuseHandler {
    /// IPC channel to the WASM filesystem service
    channel: IpcChannel,
    
    /// Filesystem statistics
    stats: FsStats,
}

struct PendingRequest {
    waker: Option<Waker>,
    response: Option<FuseResponse>,
}

impl FuseDispatcher {
    pub async fn send(
        &self,
        mount: &Mount,
        request: FuseRequest,
    ) -> Result<FuseResponse, FuseError> {
        let handlers = self.handlers.read();
        let handler = handlers.get(&mount.id)
            .ok_or(FuseError::NotFound)?;
        
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        
        // Register pending request
        {
            let mut pending = self.pending.write();
            pending.insert(request_id, PendingRequest {
                waker: None,
                response: None,
            });
        }
        
        // Send request
        let message = FuseMessage {
            id: request_id,
            request,
        };
        
        handler.channel.send(&message).await
            .map_err(|_| FuseError::IO)?;
        
        // Wait for response
        let response = ResponseFuture {
            dispatcher: self,
            request_id,
        }.await;
        
        // Update statistics
        handler.stats.record_operation(&request);
        
        Ok(response)
    }
    
    pub fn register(
        &self,
        mount_id: u64,
        channel: IpcChannel,
    ) -> Result<(), DispatcherError> {
        let mut handlers = self.handlers.write();
        
        if handlers.contains_key(&mount_id) {
            return Err(DispatcherError::AlreadyRegistered);
        }
        
        handlers.insert(mount_id, FuseHandler {
            channel,
            stats: FsStats::new(),
        });
        
        Ok(())
    }
    
    pub fn handle_response(&self, mount_id: u64, response: FuseMessage) {
        let mut pending = self.pending.write();
        
        if let Some(req) = pending.get_mut(&response.id) {
            req.response = Some(response.response);
            if let Some(waker) = req.waker.take() {
                waker.wake();
            }
        }
    }
}
```

### 6.3 WASM Filesystem Module Interface

```rust
// Exported interface for WASM filesystem modules

/// Initialize the filesystem
#[export_name = "fs_init"]
pub extern "C" fn init(config_ptr: *const u8, config_len: usize) -> i32;

/// Handle a FUSE request
#[export_name = "fs_handle_request"]
pub extern "C" fn handle_request(
    request_ptr: *const u8,
    request_len: usize,
    response_ptr: *mut u8,
    response_max_len: usize,
) -> i32;

/// Shutdown the filesystem
#[export_name = "fs_shutdown"]
pub extern "C" fn shutdown() -> i32;

// Example implementation skeleton
pub struct WasmFilesystem {
    block_device: BlockDeviceHandle,
    superblock: Superblock,
    // ...
}

impl WasmFilesystem {
    pub fn handle_request(&mut self, request: FuseRequest) -> FuseResponse {
        match request {
            FuseRequest::Lookup { parent, name } => {
                self.lookup(parent, &name)
            }
            FuseRequest::Read { inode, offset, size } => {
                self.read(inode, offset, size)
            }
            // ... other operations
            _ => FuseResponse::Error(FuseError::NotSupported),
        }
    }
}
```

---

## 7. Filesystem Drivers

### 7.1 EXT4 Driver

```rust
// storage/src/fs/ext4/mod.rs

pub struct Ext4Filesystem {
    /// Block device access
    device: BlockDeviceHandle,
    
    /// Superblock
    superblock: Superblock,
    
    /// Block group descriptors
    group_descs: Vec<GroupDescriptor>,
    
    /// Inode cache
    inode_cache: LruCache<u64, Ext4Inode>,
    
    /// Block bitmap cache
    block_bitmap_cache: LruCache<u32, Bitmap>,
    
    /// Journal (if enabled)
    journal: Option<Journal>,
}

// storage/src/fs/ext4/superblock.rs

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Superblock {
    pub inodes_count: u32,
    pub blocks_count_lo: u32,
    pub r_blocks_count_lo: u32,
    pub free_blocks_count_lo: u32,
    pub free_inodes_count: u32,
    pub first_data_block: u32,
    pub log_block_size: u32,
    pub log_cluster_size: u32,
    pub blocks_per_group: u32,
    pub clusters_per_group: u32,
    pub inodes_per_group: u32,
    pub mtime: u32,
    pub wtime: u32,
    pub mnt_count: u16,
    pub max_mnt_count: u16,
    pub magic: u16,
    pub state: u16,
    pub errors: u16,
    pub minor_rev_level: u16,
    pub lastcheck: u32,
    pub checkinterval: u32,
    pub creator_os: u32,
    pub rev_level: u32,
    pub def_resuid: u16,
    pub def_resgid: u16,
    // ... EXT4 specific fields
    pub first_ino: u32,
    pub inode_size: u16,
    pub block_group_nr: u16,
    pub feature_compat: u32,
    pub feature_incompat: u32,
    pub feature_ro_compat: u32,
    pub uuid: [u8; 16],
    pub volume_name: [u8; 16],
    // ... more fields
}

impl Superblock {
    pub fn load(device: &BlockDeviceHandle) -> Result<Self, FsError> {
        let mut buf = [0u8; 1024];
        device.read(1024, &mut buf)?; // Superblock at offset 1024
        
        let sb: Superblock = unsafe { core::ptr::read(buf.as_ptr() as *const _) };
        
        if sb.magic != 0xEF53 {
            return Err(FsError::InvalidFilesystem);
        }
        
        Ok(sb)
    }
    
    pub fn block_size(&self) -> u32 {
        1024 << self.log_block_size
    }
    
    pub fn blocks_count(&self) -> u64 {
        // Combine lo and hi for 64-bit block count
        self.blocks_count_lo as u64
    }
    
    pub fn group_count(&self) -> u32 {
        (self.blocks_count() as u32 + self.blocks_per_group - 1) / self.blocks_per_group
    }
}

// storage/src/fs/ext4/inode.rs

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Ext4Inode {
    pub mode: u16,
    pub uid: u16,
    pub size_lo: u32,
    pub atime: u32,
    pub ctime: u32,
    pub mtime: u32,
    pub dtime: u32,
    pub gid: u16,
    pub links_count: u16,
    pub blocks_lo: u32,
    pub flags: u32,
    pub osd1: u32,
    pub block: [u32; 15],
    pub generation: u32,
    pub file_acl_lo: u32,
    pub size_high: u32,
    pub obso_faddr: u32,
    // ... EXT4 specific fields
}

impl Ext4Filesystem {
    pub fn lookup(&mut self, parent: u64, name: &str) -> FuseResponse {
        // Read parent inode
        let parent_inode = match self.read_inode(parent) {
            Ok(inode) => inode,
            Err(e) => return FuseResponse::Error(e.into()),
        };
        
        // Check if parent is a directory
        if !parent_inode.is_directory() {
            return FuseResponse::Error(FuseError::NotDirectory);
        }
        
        // Read directory entries
        let entries = match self.read_directory(&parent_inode) {
            Ok(entries) => entries,
            Err(e) => return FuseResponse::Error(e.into()),
        };
        
        // Find matching entry
        for entry in entries {
            if entry.name == name {
                let inode = match self.read_inode(entry.inode as u64) {
                    Ok(inode) => inode,
                    Err(e) => return FuseResponse::Error(e.into()),
                };
                
                return FuseResponse::Entry {
                    inode: entry.inode as u64,
                    attr: inode.to_fuse_attr(entry.inode as u64),
                };
            }
        }
        
        FuseResponse::Error(FuseError::NotFound)
    }
    
    fn read_inode(&mut self, ino: u64) -> Result<Ext4Inode, FsError> {
        // Check cache
        if let Some(inode) = self.inode_cache.get(&ino) {
            return Ok(inode.clone());
        }
        
        // Calculate location
        let group = (ino - 1) / self.superblock.inodes_per_group as u64;
        let index = (ino - 1) % self.superblock.inodes_per_group as u64;
        
        let gd = &self.group_descs[group as usize];
        let inode_table_block = gd.inode_table_lo as u64;
        
        let inode_offset = inode_table_block * self.superblock.block_size() as u64
            + index * self.superblock.inode_size as u64;
        
        // Read inode
        let mut buf = vec![0u8; self.superblock.inode_size as usize];
        self.device.read(inode_offset, &mut buf)?;
        
        let inode: Ext4Inode = unsafe { core::ptr::read(buf.as_ptr() as *const _) };
        
        // Cache it
        self.inode_cache.insert(ino, inode.clone());
        
        Ok(inode)
    }
}
```

### 7.2 NTFS Driver

```rust
// storage/src/fs/ntfs/mod.rs

pub struct NtfsFilesystem {
    /// Block device
    device: BlockDeviceHandle,
    
    /// Boot sector
    boot_sector: NtfsBootSector,
    
    /// MFT location
    mft_start: u64,
    
    /// MFT record cache
    mft_cache: LruCache<u64, MftRecord>,
    
    /// Cluster size
    cluster_size: u32,
}

// storage/src/fs/ntfs/mft.rs

#[repr(C, packed)]
pub struct MftRecord {
    pub signature: [u8; 4],         // "FILE"
    pub update_sequence_offset: u16,
    pub update_sequence_size: u16,
    pub lsn: u64,
    pub sequence_number: u16,
    pub hard_link_count: u16,
    pub first_attribute_offset: u16,
    pub flags: u16,
    pub used_size: u32,
    pub allocated_size: u32,
    pub base_record_reference: u64,
    pub next_attribute_id: u16,
    // Attributes follow
}

impl MftRecord {
    pub fn is_valid(&self) -> bool {
        &self.signature == b"FILE"
    }
    
    pub fn is_in_use(&self) -> bool {
        self.flags & 0x0001 != 0
    }
    
    pub fn is_directory(&self) -> bool {
        self.flags & 0x0002 != 0
    }
}

// storage/src/fs/ntfs/attribute.rs

#[derive(Debug, Clone)]
pub enum NtfsAttribute {
    StandardInformation(StandardInformation),
    FileName(FileName),
    Data(DataAttribute),
    IndexRoot(IndexRoot),
    IndexAllocation(IndexAllocation),
    Bitmap(BitmapAttribute),
    // ... other types
}

#[repr(C, packed)]
pub struct AttributeHeader {
    pub type_code: u32,
    pub record_length: u32,
    pub non_resident: u8,
    pub name_length: u8,
    pub name_offset: u16,
    pub flags: u16,
    pub instance: u16,
}

#[repr(C, packed)]
pub struct FileName {
    pub parent_reference: u64,
    pub creation_time: u64,
    pub modification_time: u64,
    pub mft_modification_time: u64,
    pub access_time: u64,
    pub allocated_size: u64,
    pub data_size: u64,
    pub flags: u32,
    pub reparse_value: u32,
    pub name_length: u8,
    pub name_type: u8,
    // Name follows (UTF-16LE)
}

impl NtfsFilesystem {
    pub fn lookup(&mut self, parent: u64, name: &str) -> FuseResponse {
        // Read parent MFT record
        let parent_record = match self.read_mft_record(parent) {
            Ok(r) => r,
            Err(e) => return FuseResponse::Error(e.into()),
        };
        
        if !parent_record.is_directory() {
            return FuseResponse::Error(FuseError::NotDirectory);
        }
        
        // Find INDEX_ROOT attribute
        let index_root = match self.find_attribute(&parent_record, 0x90) {
            Some(NtfsAttribute::IndexRoot(ir)) => ir,
            _ => return FuseResponse::Error(FuseError::IO),
        };
        
        // Search index
        let name_utf16: Vec<u16> = name.encode_utf16().collect();
        
        if let Some(entry) = self.search_index(&index_root, &name_utf16) {
            let child_record = match self.read_mft_record(entry.mft_reference & 0xFFFFFFFFFFFF) {
                Ok(r) => r,
                Err(e) => return FuseResponse::Error(e.into()),
            };
            
            return FuseResponse::Entry {
                inode: entry.mft_reference & 0xFFFFFFFFFFFF,
                attr: self.record_to_attr(&child_record),
            };
        }
        
        // Check INDEX_ALLOCATION for B+ tree nodes
        if let Some(NtfsAttribute::IndexAllocation(ia)) = self.find_attribute(&parent_record, 0xA0) {
            if let Some(entry) = self.search_index_allocation(&ia, &name_utf16) {
                let child_record = match self.read_mft_record(entry.mft_reference & 0xFFFFFFFFFFFF) {
                    Ok(r) => r,
                    Err(e) => return FuseResponse::Error(e.into()),
                };
                
                return FuseResponse::Entry {
                    inode: entry.mft_reference & 0xFFFFFFFFFFFF,
                    attr: self.record_to_attr(&child_record),
                };
            }
        }
        
        FuseResponse::Error(FuseError::NotFound)
    }
    
    pub fn read(&mut self, inode: u64, offset: u64, size: u32) -> FuseResponse {
        let record = match self.read_mft_record(inode) {
            Ok(r) => r,
            Err(e) => return FuseResponse::Error(e.into()),
        };
        
        // Find DATA attribute
        let data_attr = match self.find_attribute(&record, 0x80) {
            Some(NtfsAttribute::Data(d)) => d,
            _ => return FuseResponse::Error(FuseError::IO),
        };
        
        if data_attr.is_resident {
            // Data is in the MFT record itself
            let data = &data_attr.resident_data;
            let start = offset as usize;
            let end = (offset as usize + size as usize).min(data.len());
            
            if start >= data.len() {
                return FuseResponse::Data(vec![]);
            }
            
            FuseResponse::Data(data[start..end].to_vec())
        } else {
            // Data is in clusters
            let mut result = Vec::with_capacity(size as usize);
            let mut remaining = size as u64;
            let mut current_offset = offset;
            
            for run in &data_attr.data_runs {
                if current_offset >= run.length * self.cluster_size as u64 {
                    current_offset -= run.length * self.cluster_size as u64;
                    continue;
                }
                
                let cluster_offset = current_offset / self.cluster_size as u64;
                let byte_offset = current_offset % self.cluster_size as u64;
                
                let cluster = run.start + cluster_offset;
                let read_len = remaining.min(
                    (run.length - cluster_offset) * self.cluster_size as u64 - byte_offset
                );
                
                let disk_offset = cluster * self.cluster_size as u64 + byte_offset;
                let mut buf = vec![0u8; read_len as usize];
                
                if let Err(e) = self.device.read(disk_offset, &mut buf) {
                    return FuseResponse::Error(FuseError::IO);
                }
                
                result.extend_from_slice(&buf);
                remaining -= read_len;
                current_offset = 0;
                
                if remaining == 0 {
                    break;
                }
            }
            
            FuseResponse::Data(result)
        }
    }
}
```

### 7.3 FAT32/exFAT Driver

```rust
// storage/src/fs/fat/fat32.rs

pub struct Fat32Filesystem {
    /// Block device
    device: BlockDeviceHandle,
    
    /// Boot sector
    boot_sector: Fat32BootSector,
    
    /// FAT cache
    fat_cache: LruCache<u32, FatSector>,
    
    /// Root directory cluster
    root_cluster: u32,
    
    /// Bytes per cluster
    cluster_size: u32,
}

#[repr(C, packed)]
pub struct Fat32BootSector {
    pub jump: [u8; 3],
    pub oem_name: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub fat_count: u8,
    pub root_entry_count: u16,     // 0 for FAT32
    pub total_sectors_16: u16,     // 0 for FAT32
    pub media_type: u8,
    pub sectors_per_fat_16: u16,   // 0 for FAT32
    pub sectors_per_track: u16,
    pub head_count: u16,
    pub hidden_sectors: u32,
    pub total_sectors_32: u32,
    // FAT32 specific
    pub sectors_per_fat_32: u32,
    pub ext_flags: u16,
    pub fs_version: u16,
    pub root_cluster: u32,
    pub fs_info_sector: u16,
    pub backup_boot_sector: u16,
    pub reserved: [u8; 12],
    pub drive_number: u8,
    pub reserved1: u8,
    pub boot_signature: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub fs_type: [u8; 8],
}

#[repr(C, packed)]
pub struct DirectoryEntry {
    pub name: [u8; 8],
    pub ext: [u8; 3],
    pub attributes: u8,
    pub nt_reserved: u8,
    pub creation_time_tenths: u8,
    pub creation_time: u16,
    pub creation_date: u16,
    pub last_access_date: u16,
    pub first_cluster_high: u16,
    pub modification_time: u16,
    pub modification_date: u16,
    pub first_cluster_low: u16,
    pub file_size: u32,
}

impl DirectoryEntry {
    pub fn first_cluster(&self) -> u32 {
        (self.first_cluster_high as u32) << 16 | self.first_cluster_low as u32
    }
    
    pub fn is_directory(&self) -> bool {
        self.attributes & 0x10 != 0
    }
    
    pub fn is_long_name(&self) -> bool {
        self.attributes == 0x0F
    }
    
    pub fn short_name(&self) -> String {
        let name = core::str::from_utf8(&self.name)
            .unwrap_or("")
            .trim_end();
        let ext = core::str::from_utf8(&self.ext)
            .unwrap_or("")
            .trim_end();
        
        if ext.is_empty() {
            name.to_string()
        } else {
            format!("{}.{}", name, ext)
        }
    }
}

impl Fat32Filesystem {
    fn read_cluster(&mut self, cluster: u32) -> Result<Vec<u8>, FsError> {
        let first_data_sector = self.boot_sector.reserved_sectors as u32
            + self.boot_sector.fat_count as u32 * self.boot_sector.sectors_per_fat_32;
        
        let sector = first_data_sector + (cluster - 2) * self.boot_sector.sectors_per_cluster as u32;
        let offset = sector as u64 * self.boot_sector.bytes_per_sector as u64;
        
        let mut buf = vec![0u8; self.cluster_size as usize];
        self.device.read(offset, &mut buf)?;
        
        Ok(buf)
    }
    
    fn next_cluster(&mut self, cluster: u32) -> Result<Option<u32>, FsError> {
        let fat_offset = cluster * 4;
        let fat_sector = self.boot_sector.reserved_sectors as u32
            + fat_offset / self.boot_sector.bytes_per_sector as u32;
        let entry_offset = fat_offset % self.boot_sector.bytes_per_sector as u32;
        
        // Check cache
        if let Some(sector) = self.fat_cache.get(&fat_sector) {
            let entry = u32::from_le_bytes([
                sector.data[entry_offset as usize],
                sector.data[entry_offset as usize + 1],
                sector.data[entry_offset as usize + 2],
                sector.data[entry_offset as usize + 3],
            ]) & 0x0FFFFFFF;
            
            return Ok(if entry >= 0x0FFFFFF8 { None } else { Some(entry) });
        }
        
        // Read FAT sector
        let offset = fat_sector as u64 * self.boot_sector.bytes_per_sector as u64;
        let mut buf = vec![0u8; self.boot_sector.bytes_per_sector as usize];
        self.device.read(offset, &mut buf)?;
        
        let entry = u32::from_le_bytes([
            buf[entry_offset as usize],
            buf[entry_offset as usize + 1],
            buf[entry_offset as usize + 2],
            buf[entry_offset as usize + 3],
        ]) & 0x0FFFFFFF;
        
        // Cache it
        self.fat_cache.insert(fat_sector, FatSector { data: buf });
        
        Ok(if entry >= 0x0FFFFFF8 { None } else { Some(entry) })
    }
    
    fn read_directory(&mut self, cluster: u32) -> Result<Vec<(String, DirectoryEntry)>, FsError> {
        let mut entries = Vec::new();
        let mut current_cluster = cluster;
        let mut long_name_parts: Vec<String> = Vec::new();
        
        loop {
            let data = self.read_cluster(current_cluster)?;
            
            for chunk in data.chunks(32) {
                if chunk[0] == 0x00 {
                    // End of directory
                    return Ok(entries);
                }
                
                if chunk[0] == 0xE5 {
                    // Deleted entry
                    continue;
                }
                
                let entry: DirectoryEntry = unsafe {
                    core::ptr::read(chunk.as_ptr() as *const _)
                };
                
                if entry.is_long_name() {
                    // Long filename entry
                    let lfn = self.parse_long_name_entry(chunk);
                    long_name_parts.insert(0, lfn);
                } else {
                    // Short name entry
                    let name = if !long_name_parts.is_empty() {
                        let full_name = long_name_parts.join("");
                        long_name_parts.clear();
                        full_name
                    } else {
                        entry.short_name()
                    };
                    
                    entries.push((name, entry));
                }
            }
            
            // Get next cluster
            match self.next_cluster(current_cluster)? {
                Some(next) => current_cluster = next,
                None => break,
            }
        }
        
        Ok(entries)
    }
}
```

---

## 8. Block Device Layer

### 8.1 Block Device Interface

```rust
// storage/src/block/mod.rs

pub trait BlockDevice: Send + Sync {
    /// Read sectors from the device
    fn read_sectors(
        &self,
        start_lba: u64,
        count: u32,
        buf: &mut [u8],
    ) -> Result<(), BlockError>;
    
    /// Write sectors to the device
    fn write_sectors(
        &self,
        start_lba: u64,
        count: u32,
        buf: &[u8],
    ) -> Result<(), BlockError>;
    
    /// Flush write cache
    fn flush(&self) -> Result<(), BlockError>;
    
    /// Get device information
    fn info(&self) -> BlockDeviceInfo;
}

#[derive(Debug, Clone)]
pub struct BlockDeviceInfo {
    /// Sector size in bytes
    pub sector_size: u32,
    
    /// Total number of sectors
    pub sector_count: u64,
    
    /// Device model/name
    pub model: String,
    
    /// Is read-only
    pub read_only: bool,
    
    /// Supports TRIM/discard
    pub supports_discard: bool,
}

pub struct BlockDeviceHandle {
    device: Arc<dyn BlockDevice>,
    sector_size: u32,
}

impl BlockDeviceHandle {
    pub fn read(&self, offset: u64, buf: &mut [u8]) -> Result<(), BlockError> {
        let sector_size = self.sector_size as u64;
        let start_lba = offset / sector_size;
        let end_lba = (offset + buf.len() as u64 + sector_size - 1) / sector_size;
        let sector_count = (end_lba - start_lba) as u32;
        
        // Read aligned sectors
        let mut sector_buf = vec![0u8; sector_count as usize * self.sector_size as usize];
        self.device.read_sectors(start_lba, sector_count, &mut sector_buf)?;
        
        // Copy to output buffer
        let start_offset = (offset % sector_size) as usize;
        buf.copy_from_slice(&sector_buf[start_offset..start_offset + buf.len()]);
        
        Ok(())
    }
    
    pub fn write(&self, offset: u64, buf: &[u8]) -> Result<(), BlockError> {
        let sector_size = self.sector_size as u64;
        let start_lba = offset / sector_size;
        let end_lba = (offset + buf.len() as u64 + sector_size - 1) / sector_size;
        
        // Handle partial first/last sector
        let start_offset = (offset % sector_size) as usize;
        
        if start_offset != 0 || buf.len() % self.sector_size as usize != 0 {
            // Read-modify-write
            let sector_count = (end_lba - start_lba) as u32;
            let mut sector_buf = vec![0u8; sector_count as usize * self.sector_size as usize];
            self.device.read_sectors(start_lba, sector_count, &mut sector_buf)?;
            
            sector_buf[start_offset..start_offset + buf.len()].copy_from_slice(buf);
            
            self.device.write_sectors(start_lba, sector_count, &sector_buf)?;
        } else {
            // Direct write
            self.device.write_sectors(start_lba, buf.len() as u32 / self.sector_size, buf)?;
        }
        
        Ok(())
    }
}
```

### 8.2 VirtIO Block Driver

```rust
// storage/src/block/virtio.rs

use virtio_drivers::{VirtIOBlk, VirtIOHeader};

pub struct VirtioBlkDriver {
    inner: VirtIOBlk<'static, HalImpl>,
    info: BlockDeviceInfo,
}

impl VirtioBlkDriver {
    pub fn new(header: &'static mut VirtIOHeader) -> Result<Self, BlockError> {
        let inner = VirtIOBlk::new(header)
            .map_err(|e| BlockError::InitFailed(format!("{:?}", e)))?;
        
        let capacity = inner.capacity();
        
        Ok(Self {
            info: BlockDeviceInfo {
                sector_size: 512,
                sector_count: capacity,
                model: "VirtIO Block Device".to_string(),
                read_only: inner.readonly(),
                supports_discard: false,
            },
            inner,
        })
    }
}

impl BlockDevice for VirtioBlkDriver {
    fn read_sectors(
        &self,
        start_lba: u64,
        count: u32,
        buf: &mut [u8],
    ) -> Result<(), BlockError> {
        for i in 0..count {
            let offset = i as usize * 512;
            self.inner.read_block(
                start_lba + i as u64,
                &mut buf[offset..offset + 512],
            ).map_err(|e| BlockError::ReadFailed(format!("{:?}", e)))?;
        }
        Ok(())
    }
    
    fn write_sectors(
        &self,
        start_lba: u64,
        count: u32,
        buf: &[u8],
    ) -> Result<(), BlockError> {
        if self.info.read_only {
            return Err(BlockError::ReadOnly);
        }
        
        for i in 0..count {
            let offset = i as usize * 512;
            self.inner.write_block(
                start_lba + i as u64,
                &buf[offset..offset + 512],
            ).map_err(|e| BlockError::WriteFailed(format!("{:?}", e)))?;
        }
        Ok(())
    }
    
    fn flush(&self) -> Result<(), BlockError> {
        // VirtIO handles this via the request queue
        Ok(())
    }
    
    fn info(&self) -> BlockDeviceInfo {
        self.info.clone()
    }
}
```

### 8.3 Partition Table Parsing

```rust
// storage/src/block/partition.rs

#[derive(Debug, Clone)]
pub struct PartitionInfo {
    /// Partition number (1-based)
    pub number: u32,
    
    /// Start LBA
    pub start_lba: u64,
    
    /// Size in sectors
    pub sectors: u64,
    
    /// Partition type
    pub partition_type: PartitionType,
    
    /// Bootable flag
    pub bootable: bool,
    
    /// Partition name (GPT only)
    pub name: Option<String>,
    
    /// Partition UUID (GPT only)
    pub uuid: Option<[u8; 16]>,
}

#[derive(Debug, Clone, Copy)]
pub enum PartitionType {
    /// EFI System Partition
    EfiSystem,
    /// Linux filesystem
    LinuxFilesystem,
    /// Linux swap
    LinuxSwap,
    /// Windows NTFS
    WindowsNtfs,
    /// FAT32
    Fat32,
    /// Unknown
    Unknown(u8),
}

pub fn parse_partition_table(device: &dyn BlockDevice) -> Result<Vec<PartitionInfo>, PartitionError> {
    let mut sector = [0u8; 512];
    device.read_sectors(0, 1, &mut sector)?;
    
    // Check for GPT
    if sector[510] == 0x55 && sector[511] == 0xAA {
        // Check for GPT signature in LBA 1
        let mut gpt_header = [0u8; 512];
        device.read_sectors(1, 1, &mut gpt_header)?;
        
        if &gpt_header[0..8] == b"EFI PART" {
            return parse_gpt(device);
        }
    }
    
    // Fall back to MBR
    parse_mbr(&sector)
}

fn parse_gpt(device: &dyn BlockDevice) -> Result<Vec<PartitionInfo>, PartitionError> {
    let mut header = [0u8; 512];
    device.read_sectors(1, 1, &mut header)?;
    
    let partition_entry_lba = u64::from_le_bytes(header[72..80].try_into().unwrap());
    let partition_entry_count = u32::from_le_bytes(header[80..84].try_into().unwrap());
    let partition_entry_size = u32::from_le_bytes(header[84..88].try_into().unwrap());
    
    let mut partitions = Vec::new();
    let entries_per_sector = 512 / partition_entry_size;
    let sector_count = (partition_entry_count + entries_per_sector - 1) / entries_per_sector;
    
    let mut entry_data = vec![0u8; sector_count as usize * 512];
    device.read_sectors(partition_entry_lba, sector_count, &mut entry_data)?;
    
    for i in 0..partition_entry_count {
        let offset = i as usize * partition_entry_size as usize;
        let entry = &entry_data[offset..offset + partition_entry_size as usize];
        
        // Check if entry is used (type GUID not zero)
        let type_guid = &entry[0..16];
        if type_guid.iter().all(|&b| b == 0) {
            continue;
        }
        
        let start_lba = u64::from_le_bytes(entry[32..40].try_into().unwrap());
        let end_lba = u64::from_le_bytes(entry[40..48].try_into().unwrap());
        
        let mut name_bytes = [0u16; 36];
        for j in 0..36 {
            name_bytes[j] = u16::from_le_bytes([
                entry[56 + j * 2],
                entry[57 + j * 2],
            ]);
        }
        let name = String::from_utf16_lossy(&name_bytes)
            .trim_end_matches('\0')
            .to_string();
        
        partitions.push(PartitionInfo {
            number: i + 1,
            start_lba,
            sectors: end_lba - start_lba + 1,
            partition_type: PartitionType::from_gpt_guid(type_guid),
            bootable: false, // GPT doesn't have bootable flag
            name: Some(name),
            uuid: Some(entry[16..32].try_into().unwrap()),
        });
    }
    
    Ok(partitions)
}
```

---

## 9. Cache Management

### 9.1 Page Cache

```rust
// storage/src/cache/page.rs

pub struct PageCache {
    /// Cached pages indexed by (inode, page_offset)
    pages: RwLock<LruCache<(u64, u64), CachedPage>>,
    
    /// Maximum cache size in pages
    max_pages: usize,
    
    /// Current dirty page count
    dirty_count: AtomicUsize,
    
    /// Write-back threshold (percentage)
    writeback_threshold: usize,
}

struct CachedPage {
    data: Box<[u8; PAGE_SIZE]>,
    dirty: bool,
    accessed: Instant,
    modified: Option<Instant>,
}

const PAGE_SIZE: usize = 4096;

impl PageCache {
    pub fn new(max_pages: usize) -> Self {
        Self {
            pages: RwLock::new(LruCache::new(max_pages)),
            max_pages,
            dirty_count: AtomicUsize::new(0),
            writeback_threshold: 80,
        }
    }
    
    pub fn read(&self, inode: u64, offset: u64, len: usize) -> Option<Vec<u8>> {
        let page_offset = offset / PAGE_SIZE as u64;
        let page_start = (offset % PAGE_SIZE as u64) as usize;
        
        let mut cache = self.pages.write();
        let page = cache.get(&(inode, page_offset))?;
        
        let end = (page_start + len).min(PAGE_SIZE);
        Some(page.data[page_start..end].to_vec())
    }
    
    pub fn insert(&self, inode: u64, offset: u64, data: &[u8]) {
        let page_offset = offset / PAGE_SIZE as u64;
        
        let mut page_data = Box::new([0u8; PAGE_SIZE]);
        let start = (offset % PAGE_SIZE as u64) as usize;
        let end = (start + data.len()).min(PAGE_SIZE);
        page_data[start..end].copy_from_slice(&data[..end - start]);
        
        let mut cache = self.pages.write();
        cache.insert((inode, page_offset), CachedPage {
            data: page_data,
            dirty: false,
            accessed: Instant::now(),
            modified: None,
        });
        
        // Check if eviction needed
        self.maybe_evict(&mut cache);
    }
    
    pub fn mark_dirty(&self, inode: u64, offset: u64) {
        let page_offset = offset / PAGE_SIZE as u64;
        
        let mut cache = self.pages.write();
        if let Some(page) = cache.get_mut(&(inode, page_offset)) {
            if !page.dirty {
                page.dirty = true;
                page.modified = Some(Instant::now());
                self.dirty_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    
    pub fn invalidate(&self, inode: u64, offset: u64, len: usize) {
        let start_page = offset / PAGE_SIZE as u64;
        let end_page = (offset + len as u64 + PAGE_SIZE as u64 - 1) / PAGE_SIZE as u64;
        
        let mut cache = self.pages.write();
        for page in start_page..end_page {
            if let Some(removed) = cache.remove(&(inode, page)) {
                if removed.dirty {
                    self.dirty_count.fetch_sub(1, Ordering::Relaxed);
                }
            }
        }
    }
    
    fn maybe_evict(&self, cache: &mut LruCache<(u64, u64), CachedPage>) {
        let dirty_ratio = self.dirty_count.load(Ordering::Relaxed) * 100 / self.max_pages;
        
        if dirty_ratio > self.writeback_threshold {
            // Trigger write-back
            // (handled by separate write-back task)
        }
        
        // Evict clean pages if over capacity
        while cache.len() > self.max_pages {
            // Find LRU clean page
            let to_remove: Vec<_> = cache.iter()
                .filter(|(_, page)| !page.dirty)
                .take(self.max_pages / 10)
                .map(|(key, _)| *key)
                .collect();
            
            for key in to_remove {
                cache.remove(&key);
            }
            
            if cache.len() <= self.max_pages {
                break;
            }
            
            // If still over, evict dirty pages (will be written back first)
            // This is a fallback and should rarely happen
        }
    }
}
```

### 9.2 Write-Back Management

```rust
// storage/src/cache/writeback.rs

pub struct WriteBackManager {
    /// Page cache reference
    cache: Arc<PageCache>,
    
    /// VFS reference for flushing
    vfs: Arc<Vfs>,
    
    /// Write-back interval
    interval: Duration,
    
    /// Maximum dirty age before forced write-back
    max_dirty_age: Duration,
}

impl WriteBackManager {
    pub async fn run(&self) -> ! {
        let mut interval = tokio::time::interval(self.interval);
        
        loop {
            interval.tick().await;
            
            self.write_back_dirty_pages().await;
        }
    }
    
    async fn write_back_dirty_pages(&self) {
        let now = Instant::now();
        let mut to_flush: Vec<(u64, u64, Box<[u8; PAGE_SIZE]>)> = Vec::new();
        
        // Collect dirty pages that need flushing
        {
            let mut cache = self.cache.pages.write();
            
            for ((inode, offset), page) in cache.iter_mut() {
                if !page.dirty {
                    continue;
                }
                
                // Check if old enough
                if let Some(modified) = page.modified {
                    if now.duration_since(modified) >= self.max_dirty_age {
                        to_flush.push((*inode, *offset, page.data.clone()));
                        page.dirty = false;
                        page.modified = None;
                    }
                }
            }
        }
        
        // Flush pages outside the lock
        for (inode, offset, data) in to_flush {
            let result = self.vfs.write_through(inode, offset * PAGE_SIZE as u64, &*data).await;
            
            if result.is_err() {
                // Re-mark as dirty on failure
                self.cache.mark_dirty(inode, offset * PAGE_SIZE as u64);
            } else {
                self.cache.dirty_count.fetch_sub(1, Ordering::Relaxed);
            }
        }
    }
    
    pub async fn sync(&self) {
        // Force flush all dirty pages
        let mut all_dirty: Vec<(u64, u64, Box<[u8; PAGE_SIZE]>)> = Vec::new();
        
        {
            let mut cache = self.cache.pages.write();
            
            for ((inode, offset), page) in cache.iter_mut() {
                if page.dirty {
                    all_dirty.push((*inode, *offset, page.data.clone()));
                    page.dirty = false;
                    page.modified = None;
                }
            }
        }
        
        for (inode, offset, data) in all_dirty {
            let _ = self.vfs.write_through(inode, offset * PAGE_SIZE as u64, &*data).await;
            self.cache.dirty_count.fetch_sub(1, Ordering::Relaxed);
        }
    }
}
```

---

## 10. Security and Capabilities

### 10.1 Filesystem Capabilities

```rust
// runtime/src/extensions/fs_cap.rs

#[derive(Debug, Clone)]
pub enum FilesystemCapability {
    /// Read access to a specific path
    Read {
        path: PathPattern,
    },
    
    /// Write access to a specific path
    Write {
        path: PathPattern,
    },
    
    /// Create files in a directory
    Create {
        directory: PathPattern,
    },
    
    /// Delete files
    Delete {
        path: PathPattern,
    },
    
    /// Execute files
    Execute {
        path: PathPattern,
    },
    
    /// Full access to a path
    Full {
        path: PathPattern,
    },
}

#[derive(Debug, Clone)]
pub enum PathPattern {
    /// Exact path match
    Exact(Path),
    
    /// Path prefix (directory and contents)
    Prefix(Path),
    
    /// Glob pattern
    Glob(String),
}

impl FilesystemCapability {
    pub fn allows(&self, path: &Path, access: AccessType) -> bool {
        match (self, access) {
            (Self::Read { path: pattern }, AccessType::Read) => pattern.matches(path),
            (Self::Write { path: pattern }, AccessType::Write) => pattern.matches(path),
            (Self::Create { directory }, AccessType::Create) => {
                path.parent().map(|p| directory.matches(&p)).unwrap_or(false)
            }
            (Self::Delete { path: pattern }, AccessType::Delete) => pattern.matches(path),
            (Self::Execute { path: pattern }, AccessType::Execute) => pattern.matches(path),
            (Self::Full { path: pattern }, _) => pattern.matches(path),
            _ => false,
        }
    }
}

impl PathPattern {
    pub fn matches(&self, path: &Path) -> bool {
        match self {
            Self::Exact(p) => p == path,
            Self::Prefix(prefix) => {
                path.to_string().starts_with(&prefix.to_string())
            }
            Self::Glob(pattern) => {
                glob_match(pattern, &path.to_string())
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AccessType {
    Read,
    Write,
    Create,
    Delete,
    Execute,
}
```

### 10.2 Sandboxed Filesystem View

```rust
// runtime/src/extensions/sandbox_fs.rs

pub struct SandboxedFilesystem {
    /// Underlying VFS
    vfs: Arc<Vfs>,
    
    /// Capability set
    capabilities: Vec<FilesystemCapability>,
    
    /// Path mappings (sandbox path -> real path)
    mappings: HashMap<Path, Path>,
    
    /// Temporary directory for this sandbox
    temp_dir: Option<Path>,
}

impl SandboxedFilesystem {
    pub fn new(vfs: Arc<Vfs>, capabilities: Vec<FilesystemCapability>) -> Self {
        Self {
            vfs,
            capabilities,
            mappings: HashMap::new(),
            temp_dir: None,
        }
    }
    
    pub fn add_mapping(&mut self, sandbox_path: Path, real_path: Path) {
        self.mappings.insert(sandbox_path, real_path);
    }
    
    fn translate_path(&self, sandbox_path: &Path) -> Result<Path, VfsError> {
        // Check mappings
        for (sandbox, real) in &self.mappings {
            if sandbox_path.to_string().starts_with(&sandbox.to_string()) {
                let suffix = &sandbox_path.to_string()[sandbox.to_string().len()..];
                return Ok(real.join(&Path::parse(suffix)?));
            }
        }
        
        // No mapping - path is used as-is (subject to capability checks)
        Ok(sandbox_path.clone())
    }
    
    fn check_capability(&self, path: &Path, access: AccessType) -> Result<(), VfsError> {
        for cap in &self.capabilities {
            if cap.allows(path, access) {
                return Ok(());
            }
        }
        
        Err(VfsError::PermissionDenied)
    }
    
    pub async fn open(
        &self,
        sandbox_path: &Path,
        flags: OpenFlags,
    ) -> Result<FileHandle, VfsError> {
        let real_path = self.translate_path(sandbox_path)?;
        
        // Check capabilities
        if flags.contains(OpenFlags::READ) {
            self.check_capability(&real_path, AccessType::Read)?;
        }
        if flags.contains(OpenFlags::WRITE) {
            self.check_capability(&real_path, AccessType::Write)?;
        }
        if flags.contains(OpenFlags::CREATE) {
            self.check_capability(&real_path, AccessType::Create)?;
        }
        
        // Delegate to real VFS
        self.vfs.open(&real_path, flags, &self.capabilities).await
    }
    
    pub async fn read_dir(&self, sandbox_path: &Path) -> Result<Vec<DirEntry>, VfsError> {
        let real_path = self.translate_path(sandbox_path)?;
        
        self.check_capability(&real_path, AccessType::Read)?;
        
        let entries = self.vfs.read_dir(&real_path).await?;
        
        // Filter entries based on capabilities
        let filtered: Vec<_> = entries.into_iter()
            .filter(|entry| {
                let entry_path = real_path.join(&Path::parse(&entry.name).unwrap_or_default());
                self.check_capability(&entry_path, AccessType::Read).is_ok()
            })
            .collect();
        
        Ok(filtered)
    }
}
```

---

## Appendix A: Supported Filesystems

| Filesystem | Status | Read | Write | Notes |
|------------|--------|------|-------|-------|
| SquashFS | Implemented | Yes | No | Root filesystem |
| EXT4 | Implemented | Yes | Yes | Data partitions |
| NTFS | Implemented | Yes | Limited | Windows compatibility |
| FAT32 | Implemented | Yes | Yes | EFI, USB drives |
| exFAT | Planned | - | - | Large file support |
| tmpfs | Implemented | Yes | Yes | RAM-backed |
| overlayfs | Implemented | Yes | Yes | Development mode |

---

## Appendix B: Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Sequential read | > 500 MB/s | With VirtIO |
| Sequential write | > 400 MB/s | With VirtIO |
| Random read IOPS | > 50,000 | 4KB blocks |
| Random write IOPS | > 30,000 | 4KB blocks |
| Metadata ops/sec | > 100,000 | Create/delete/stat |
| Cache hit ratio | > 90% | Active working set |
| Page cache size | 25% RAM | Default |
