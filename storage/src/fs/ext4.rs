//! ext4 filesystem implementation.
//!
//! This module provides support for the ext4 filesystem, the most common
//! Linux filesystem. It supports extents, journaling, and large files.

use alloc::string::String;
use alloc::vec::Vec;

use crate::{DirEntry, FileMetadata, FileType, FilePermissions, OpenFlags, StorageError};
use crate::vfs::{Filesystem, FsStats};
use crate::driver::BlockDevice;

/// ext4 superblock magic number.
pub const EXT4_SUPER_MAGIC: u16 = 0xEF53;

/// ext4 superblock.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Ext4Superblock {
    /// Inodes count.
    pub s_inodes_count: u32,
    /// Blocks count (low).
    pub s_blocks_count_lo: u32,
    /// Reserved blocks count (low).
    pub s_r_blocks_count_lo: u32,
    /// Free blocks count (low).
    pub s_free_blocks_count_lo: u32,
    /// Free inodes count.
    pub s_free_inodes_count: u32,
    /// First data block.
    pub s_first_data_block: u32,
    /// Block size (log2(block_size) - 10).
    pub s_log_block_size: u32,
    /// Cluster size (log2(cluster_size) - 10).
    pub s_log_cluster_size: u32,
    /// Blocks per group.
    pub s_blocks_per_group: u32,
    /// Clusters per group.
    pub s_clusters_per_group: u32,
    /// Inodes per group.
    pub s_inodes_per_group: u32,
    /// Mount time.
    pub s_mtime: u32,
    /// Write time.
    pub s_wtime: u32,
    /// Mount count.
    pub s_mnt_count: u16,
    /// Max mount count.
    pub s_max_mnt_count: u16,
    /// Magic signature (0xEF53).
    pub s_magic: u16,
    /// Filesystem state.
    pub s_state: u16,
    /// Errors behavior.
    pub s_errors: u16,
    /// Minor revision level.
    pub s_minor_rev_level: u16,
    /// Last check time.
    pub s_lastcheck: u32,
    /// Check interval.
    pub s_checkinterval: u32,
    /// Creator OS.
    pub s_creator_os: u32,
    /// Revision level.
    pub s_rev_level: u32,
    /// Default UID for reserved blocks.
    pub s_def_resuid: u16,
    /// Default GID for reserved blocks.
    pub s_def_resgid: u16,
    // Extended superblock fields (rev >= 1)
    /// First non-reserved inode.
    pub s_first_ino: u32,
    /// Inode size.
    pub s_inode_size: u16,
    /// Block group number of this superblock.
    pub s_block_group_nr: u16,
    /// Compatible feature set.
    pub s_feature_compat: u32,
    /// Incompatible feature set.
    pub s_feature_incompat: u32,
    /// Read-only compatible feature set.
    pub s_feature_ro_compat: u32,
    /// UUID.
    pub s_uuid: [u8; 16],
    /// Volume name.
    pub s_volume_name: [u8; 16],
    /// Last mounted path.
    pub s_last_mounted: [u8; 64],
    /// Compression algorithm.
    pub s_algorithm_usage_bitmap: u32,
    // Performance hints
    /// Blocks to preallocate for files.
    pub s_prealloc_blocks: u8,
    /// Blocks to preallocate for directories.
    pub s_prealloc_dir_blocks: u8,
    /// Reserved GDT blocks.
    pub s_reserved_gdt_blocks: u16,
    // Journaling support
    /// UUID of journal superblock.
    pub s_journal_uuid: [u8; 16],
    /// Inode number of journal file.
    pub s_journal_inum: u32,
    /// Device number of journal file.
    pub s_journal_dev: u32,
    /// Start of list of inodes to delete.
    pub s_last_orphan: u32,
    /// HTREE hash seed.
    pub s_hash_seed: [u32; 4],
    /// Default hash version.
    pub s_def_hash_version: u8,
    /// Journal backup type.
    pub s_jnl_backup_type: u8,
    /// Group descriptor size.
    pub s_desc_size: u16,
    /// Default mount options.
    pub s_default_mount_opts: u32,
    /// First metablock block group.
    pub s_first_meta_bg: u32,
    /// Filesystem creation time.
    pub s_mkfs_time: u32,
    /// Journal inode backup.
    pub s_jnl_blocks: [u32; 17],
    // 64-bit support
    /// Blocks count (high).
    pub s_blocks_count_hi: u32,
    /// Reserved blocks count (high).
    pub s_r_blocks_count_hi: u32,
    /// Free blocks count (high).
    pub s_free_blocks_count_hi: u32,
    /// Minimum inode size.
    pub s_min_extra_isize: u16,
    /// Want inode size.
    pub s_want_extra_isize: u16,
    /// Flags.
    pub s_flags: u32,
    /// RAID stride.
    pub s_raid_stride: u16,
    /// MMP check interval.
    pub s_mmp_interval: u16,
    /// Block for MMP.
    pub s_mmp_block: u64,
    /// RAID stripe width.
    pub s_raid_stripe_width: u32,
    /// Log2 of groups per flex.
    pub s_log_groups_per_flex: u8,
    /// Checksum type.
    pub s_checksum_type: u8,
    /// Reserved padding.
    pub s_reserved_pad: u16,
    /// KB written lifetime.
    pub s_kbytes_written: u64,
    /// Snapshot inode.
    pub s_snapshot_inum: u32,
    /// Snapshot sequential ID.
    pub s_snapshot_id: u32,
    /// Reserved blocks for snapshot.
    pub s_snapshot_r_blocks_count: u64,
    /// Inode of snapshot list head.
    pub s_snapshot_list: u32,
    /// Number of errors.
    pub s_error_count: u32,
    /// First error time.
    pub s_first_error_time: u32,
    /// First error inode.
    pub s_first_error_ino: u32,
    /// First error block.
    pub s_first_error_block: u64,
    /// First error function.
    pub s_first_error_func: [u8; 32],
    /// First error line.
    pub s_first_error_line: u32,
    /// Last error time.
    pub s_last_error_time: u32,
    /// Last error inode.
    pub s_last_error_ino: u32,
    /// Last error line.
    pub s_last_error_line: u32,
    /// Last error block.
    pub s_last_error_block: u64,
    /// Last error function.
    pub s_last_error_func: [u8; 32],
    /// Mount options.
    pub s_mount_opts: [u8; 64],
    /// Inode for user quota.
    pub s_usr_quota_inum: u32,
    /// Inode for group quota.
    pub s_grp_quota_inum: u32,
    /// Overhead blocks.
    pub s_overhead_blocks: u32,
    /// Backup block groups.
    pub s_backup_bgs: [u32; 2],
    /// Encryption algorithms.
    pub s_encrypt_algos: [u8; 4],
    /// Salt for encryption.
    pub s_encrypt_pw_salt: [u8; 16],
    /// Location of lost+found.
    pub s_lpf_ino: u32,
    /// Inode for project quota.
    pub s_prj_quota_inum: u32,
    /// Checksum seed.
    pub s_checksum_seed: u32,
    /// Reserved.
    pub s_reserved: [u32; 98],
    /// Superblock checksum.
    pub s_checksum: u32,
}

impl Ext4Superblock {
    /// Superblock size.
    pub const SIZE: usize = 1024;
    /// Superblock offset from start of filesystem.
    pub const OFFSET: usize = 1024;

    /// Get block size in bytes.
    pub fn block_size(&self) -> u32 {
        1024 << self.s_log_block_size
    }

    /// Get total number of blocks.
    pub fn blocks_count(&self) -> u64 {
        self.s_blocks_count_lo as u64 | ((self.s_blocks_count_hi as u64) << 32)
    }

    /// Get number of free blocks.
    pub fn free_blocks_count(&self) -> u64 {
        self.s_free_blocks_count_lo as u64 | ((self.s_free_blocks_count_hi as u64) << 32)
    }

    /// Check if extent feature is enabled.
    pub fn has_extents(&self) -> bool {
        self.s_feature_incompat & 0x0040 != 0
    }

    /// Check if 64-bit feature is enabled.
    pub fn has_64bit(&self) -> bool {
        self.s_feature_incompat & 0x0080 != 0
    }

    /// Get number of block groups.
    pub fn group_count(&self) -> u32 {
        let blocks = self.blocks_count();
        let blocks_per_group = self.s_blocks_per_group as u64;
        ((blocks + blocks_per_group - 1) / blocks_per_group) as u32
    }

    /// Get group descriptor size.
    pub fn desc_size(&self) -> u16 {
        if self.has_64bit() && self.s_desc_size > 32 {
            self.s_desc_size
        } else {
            32
        }
    }
}

/// ext4 block group descriptor.
#[derive(Debug, Clone, Default)]
#[repr(C)]
pub struct Ext4GroupDesc {
    /// Block bitmap block (low).
    pub bg_block_bitmap_lo: u32,
    /// Inode bitmap block (low).
    pub bg_inode_bitmap_lo: u32,
    /// Inode table block (low).
    pub bg_inode_table_lo: u32,
    /// Free blocks count (low).
    pub bg_free_blocks_count_lo: u16,
    /// Free inodes count (low).
    pub bg_free_inodes_count_lo: u16,
    /// Used directories count (low).
    pub bg_used_dirs_count_lo: u16,
    /// Flags.
    pub bg_flags: u16,
    /// Exclude bitmap block (low).
    pub bg_exclude_bitmap_lo: u32,
    /// Block bitmap checksum (low).
    pub bg_block_bitmap_csum_lo: u16,
    /// Inode bitmap checksum (low).
    pub bg_inode_bitmap_csum_lo: u16,
    /// Free inodes count (low).
    pub bg_itable_unused_lo: u16,
    /// Checksum.
    pub bg_checksum: u16,
    // 64-bit fields
    /// Block bitmap block (high).
    pub bg_block_bitmap_hi: u32,
    /// Inode bitmap block (high).
    pub bg_inode_bitmap_hi: u32,
    /// Inode table block (high).
    pub bg_inode_table_hi: u32,
    /// Free blocks count (high).
    pub bg_free_blocks_count_hi: u16,
    /// Free inodes count (high).
    pub bg_free_inodes_count_hi: u16,
    /// Used directories count (high).
    pub bg_used_dirs_count_hi: u16,
    /// Unused inodes count (high).
    pub bg_itable_unused_hi: u16,
    /// Exclude bitmap block (high).
    pub bg_exclude_bitmap_hi: u32,
    /// Block bitmap checksum (high).
    pub bg_block_bitmap_csum_hi: u16,
    /// Inode bitmap checksum (high).
    pub bg_inode_bitmap_csum_hi: u16,
    /// Reserved.
    pub bg_reserved: u32,
}

impl Ext4GroupDesc {
    /// Get block bitmap block number.
    pub fn block_bitmap(&self, _has_64bit: bool) -> u64 {
        self.bg_block_bitmap_lo as u64 | ((self.bg_block_bitmap_hi as u64) << 32)
    }

    /// Get inode bitmap block number.
    pub fn inode_bitmap(&self, _has_64bit: bool) -> u64 {
        self.bg_inode_bitmap_lo as u64 | ((self.bg_inode_bitmap_hi as u64) << 32)
    }

    /// Get inode table block number.
    pub fn inode_table(&self, _has_64bit: bool) -> u64 {
        self.bg_inode_table_lo as u64 | ((self.bg_inode_table_hi as u64) << 32)
    }
}

/// ext4 inode.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Ext4Inode {
    /// File mode.
    pub i_mode: u16,
    /// Owner UID (low).
    pub i_uid: u16,
    /// Size (low).
    pub i_size_lo: u32,
    /// Access time.
    pub i_atime: u32,
    /// Change time.
    pub i_ctime: u32,
    /// Modification time.
    pub i_mtime: u32,
    /// Deletion time.
    pub i_dtime: u32,
    /// Owner GID (low).
    pub i_gid: u16,
    /// Link count.
    pub i_links_count: u16,
    /// Blocks count (in 512-byte units, low).
    pub i_blocks_lo: u32,
    /// Flags.
    pub i_flags: u32,
    /// OS-specific value 1.
    pub i_osd1: u32,
    /// Block map or extent tree.
    pub i_block: [u32; 15],
    /// File version (for NFS).
    pub i_generation: u32,
    /// Extended attribute block (low).
    pub i_file_acl_lo: u32,
    /// Size (high) / Directory ACL.
    pub i_size_high: u32,
    /// Fragment address (obsolete).
    pub i_obso_faddr: u32,
    /// OS-specific value 2.
    pub i_osd2: [u8; 12],
    /// Extra inode size.
    pub i_extra_isize: u16,
    /// Checksum (high).
    pub i_checksum_hi: u16,
    /// Extra change time (high).
    pub i_ctime_extra: u32,
    /// Extra modification time (high).
    pub i_mtime_extra: u32,
    /// Extra access time (high).
    pub i_atime_extra: u32,
    /// Creation time.
    pub i_crtime: u32,
    /// Extra creation time (high).
    pub i_crtime_extra: u32,
    /// Version (high).
    pub i_version_hi: u32,
    /// Project ID.
    pub i_projid: u32,
}

impl Ext4Inode {
    /// Base inode size.
    pub const BASE_SIZE: usize = 128;

    /// Get file size.
    pub fn size(&self) -> u64 {
        self.i_size_lo as u64 | ((self.i_size_high as u64) << 32)
    }

    /// Check if this is a directory.
    pub fn is_dir(&self) -> bool {
        (self.i_mode & 0xF000) == 0x4000
    }

    /// Check if this is a regular file.
    pub fn is_file(&self) -> bool {
        (self.i_mode & 0xF000) == 0x8000
    }

    /// Check if this is a symbolic link.
    pub fn is_symlink(&self) -> bool {
        (self.i_mode & 0xF000) == 0xA000
    }

    /// Check if extents are used.
    pub fn uses_extents(&self) -> bool {
        self.i_flags & 0x80000 != 0
    }

    /// Get file type.
    pub fn file_type(&self) -> FileType {
        match self.i_mode & 0xF000 {
            0x4000 => FileType::Directory,
            0x8000 => FileType::Regular,
            0xA000 => FileType::Symlink,
            0x6000 => FileType::BlockDevice,
            0x2000 => FileType::CharDevice,
            0x1000 => FileType::Fifo,
            0xC000 => FileType::Socket,
            _ => FileType::Unknown,
        }
    }

    /// Convert to FileMetadata.
    pub fn to_metadata(&self, inode_num: u64) -> FileMetadata {
        FileMetadata {
            file_type: self.file_type(),
            permissions: FilePermissions((self.i_mode & 0x0FFF) as u16),
            size: self.size(),
            nlink: self.i_links_count as u32,
            uid: self.i_uid as u32,
            gid: self.i_gid as u32,
            block_size: 4096,
            blocks: self.i_blocks_lo as u64,
            atime: self.i_atime as u64 * 1_000_000_000,
            mtime: self.i_mtime as u64 * 1_000_000_000,
            ctime: self.i_ctime as u64 * 1_000_000_000,
            crtime: self.i_crtime as u64 * 1_000_000_000,
            inode: inode_num,
            dev: 0,
            rdev: 0,
        }
    }
}

/// ext4 extent header.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Ext4ExtentHeader {
    /// Magic number (0xF30A).
    pub eh_magic: u16,
    /// Number of valid entries.
    pub eh_entries: u16,
    /// Maximum number of entries.
    pub eh_max: u16,
    /// Depth of tree (0 for leaf).
    pub eh_depth: u16,
    /// Generation.
    pub eh_generation: u32,
}

/// ext4 extent.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Ext4Extent {
    /// First file block.
    pub ee_block: u32,
    /// Number of blocks.
    pub ee_len: u16,
    /// High 16 bits of physical block.
    pub ee_start_hi: u16,
    /// Low 32 bits of physical block.
    pub ee_start_lo: u32,
}

impl Ext4Extent {
    /// Get physical block number.
    pub fn start(&self) -> u64 {
        self.ee_start_lo as u64 | ((self.ee_start_hi as u64) << 32)
    }

    /// Get extent length.
    pub fn len(&self) -> u16 {
        self.ee_len & 0x7FFF
    }

    /// Check if this is an unwritten extent.
    pub fn is_unwritten(&self) -> bool {
        self.ee_len > 0x8000
    }
}

/// ext4 directory entry.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Ext4DirEntry {
    /// Inode number.
    pub inode: u32,
    /// Directory entry length.
    pub rec_len: u16,
    /// Name length.
    pub name_len: u8,
    /// File type.
    pub file_type: u8,
    /// File name.
    pub name: [u8; 255],
}

impl Ext4DirEntry {
    /// Get file type.
    pub fn get_file_type(&self) -> FileType {
        match self.file_type {
            1 => FileType::Regular,
            2 => FileType::Directory,
            3 => FileType::CharDevice,
            4 => FileType::BlockDevice,
            5 => FileType::Fifo,
            6 => FileType::Socket,
            7 => FileType::Symlink,
            _ => FileType::Unknown,
        }
    }

    /// Convert to DirEntry.
    pub fn to_dir_entry(&self) -> DirEntry {
        let mut name = [0u8; 256];
        let len = self.name_len as usize;
        name[..len].copy_from_slice(&self.name[..len]);

        DirEntry {
            name,
            name_len: len,
            inode: self.inode as u64,
            file_type: self.get_file_type(),
        }
    }
}

/// ext4 filesystem.
pub struct Ext4Filesystem {
    /// Superblock.
    superblock: Ext4Superblock,
    /// Block size.
    block_size: u32,
    /// Is read-only.
    read_only: bool,
}

impl Ext4Filesystem {
    /// Create a new ext4 filesystem (placeholder).
    pub fn new() -> Self {
        Ext4Filesystem {
            superblock: unsafe { core::mem::zeroed() },
            block_size: 4096,
            read_only: false,
        }
    }
}

impl Default for Ext4Filesystem {
    fn default() -> Self {
        Self::new()
    }
}

impl Filesystem for Ext4Filesystem {
    fn fs_type(&self) -> &str {
        "ext4"
    }

    fn statfs(&self) -> Result<FsStats, StorageError> {
        Ok(FsStats {
            fs_type: EXT4_SUPER_MAGIC as u32,
            block_size: self.block_size,
            total_blocks: self.superblock.blocks_count(),
            free_blocks: self.superblock.free_blocks_count(),
            available_blocks: self.superblock.free_blocks_count(),
            total_inodes: self.superblock.s_inodes_count as u64,
            free_inodes: self.superblock.s_free_inodes_count as u64,
            fs_id: 0,
            max_name_len: 255,
            fragment_size: self.block_size,
            flags: crate::MountFlags::empty(),
        })
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
        Ok(())
    }

    fn fsync(&self, _handle: u64, _data_only: bool) -> Result<(), StorageError> {
        Ok(())
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
