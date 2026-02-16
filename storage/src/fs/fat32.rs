//! FAT32 filesystem implementation.
//!
//! This module provides support for the FAT32 filesystem, widely used
//! for removable media and EFI system partitions.

use alloc::string::String;
use alloc::vec::Vec;

use crate::vfs::{Filesystem, FsStats};
use crate::{DirEntry, FileMetadata, FilePermissions, FileType, OpenFlags, StorageError};

/// FAT32 Boot Sector / BIOS Parameter Block.
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct Fat32Bpb {
    /// Jump instruction.
    pub jmp_boot: [u8; 3],
    /// OEM name.
    pub oem_name: [u8; 8],
    /// Bytes per sector.
    pub bytes_per_sector: u16,
    /// Sectors per cluster.
    pub sectors_per_cluster: u8,
    /// Reserved sectors.
    pub reserved_sectors: u16,
    /// Number of FATs.
    pub num_fats: u8,
    /// Root entry count (0 for FAT32).
    pub root_entry_count: u16,
    /// Total sectors (16-bit, 0 for FAT32).
    pub total_sectors_16: u16,
    /// Media type.
    pub media: u8,
    /// FAT size (16-bit, 0 for FAT32).
    pub fat_size_16: u16,
    /// Sectors per track.
    pub sectors_per_track: u16,
    /// Number of heads.
    pub num_heads: u16,
    /// Hidden sectors.
    pub hidden_sectors: u32,
    /// Total sectors (32-bit).
    pub total_sectors_32: u32,
    // FAT32 specific fields
    /// FAT size (32-bit).
    pub fat_size_32: u32,
    /// Extended flags.
    pub ext_flags: u16,
    /// Filesystem version.
    pub fs_version: u16,
    /// Root cluster.
    pub root_cluster: u32,
    /// FSInfo sector.
    pub fs_info: u16,
    /// Backup boot sector.
    pub backup_boot_sector: u16,
    /// Reserved.
    pub reserved: [u8; 12],
    /// Drive number.
    pub drive_number: u8,
    /// Reserved.
    pub reserved1: u8,
    /// Extended boot signature.
    pub boot_sig: u8,
    /// Volume serial number.
    pub volume_id: u32,
    /// Volume label.
    pub volume_label: [u8; 11],
    /// Filesystem type.
    pub fs_type: [u8; 8],
}

impl Fat32Bpb {
    /// BPB size in bytes.
    pub const SIZE: usize = 90;

    /// Get cluster size in bytes.
    pub fn cluster_size(&self) -> u32 {
        self.bytes_per_sector as u32 * self.sectors_per_cluster as u32
    }

    /// Get FAT size in sectors.
    pub fn fat_size(&self) -> u32 {
        if self.fat_size_16 != 0 {
            self.fat_size_16 as u32
        } else {
            self.fat_size_32
        }
    }

    /// Get total sectors.
    pub fn total_sectors(&self) -> u32 {
        if self.total_sectors_16 != 0 {
            self.total_sectors_16 as u32
        } else {
            self.total_sectors_32
        }
    }

    /// Get the first data sector.
    pub fn first_data_sector(&self) -> u32 {
        let root_dir_sectors = 0; // FAT32 has no fixed root directory
        self.reserved_sectors as u32 + (self.num_fats as u32 * self.fat_size()) + root_dir_sectors
    }

    /// Get the first sector of a cluster.
    pub fn cluster_to_sector(&self, cluster: u32) -> u32 {
        self.first_data_sector() + (cluster - 2) * self.sectors_per_cluster as u32
    }

    /// Get total data sectors.
    pub fn data_sectors(&self) -> u32 {
        self.total_sectors() - self.first_data_sector()
    }

    /// Get total clusters.
    pub fn total_clusters(&self) -> u32 {
        self.data_sectors() / self.sectors_per_cluster as u32
    }
}

/// FAT32 FSInfo sector.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Fat32FsInfo {
    /// Lead signature (0x41615252).
    pub lead_sig: u32,
    /// Reserved.
    pub reserved1: [u8; 480],
    /// Structure signature (0x61417272).
    pub struc_sig: u32,
    /// Free cluster count.
    pub free_count: u32,
    /// Next free cluster.
    pub nxt_free: u32,
    /// Reserved.
    pub reserved2: [u8; 12],
    /// Trail signature (0xAA550000).
    pub trail_sig: u32,
}

impl Fat32FsInfo {
    /// FSInfo lead signature.
    pub const LEAD_SIG: u32 = 0x41615252;
    /// FSInfo structure signature.
    pub const STRUC_SIG: u32 = 0x61417272;
    /// FSInfo trail signature.
    pub const TRAIL_SIG: u32 = 0xAA550000;

    /// Unknown free count.
    pub const UNKNOWN_FREE: u32 = 0xFFFFFFFF;
}

/// FAT entry values.
pub mod fat_entry {
    /// Free cluster.
    pub const FREE: u32 = 0x00000000;
    /// Reserved cluster.
    pub const RESERVED: u32 = 0x00000001;
    /// Bad cluster.
    pub const BAD: u32 = 0x0FFFFFF7;
    /// End of chain minimum.
    pub const EOC_MIN: u32 = 0x0FFFFFF8;
    /// End of chain.
    pub const EOC: u32 = 0x0FFFFFFF;
    /// Mask for cluster number.
    pub const MASK: u32 = 0x0FFFFFFF;
}

/// FAT32 directory entry.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Fat32DirEntry {
    /// Short name.
    pub name: [u8; 11],
    /// Attributes.
    pub attr: u8,
    /// Reserved for Windows NT.
    pub nt_res: u8,
    /// Creation time (tenths of seconds).
    pub crt_time_tenth: u8,
    /// Creation time.
    pub crt_time: u16,
    /// Creation date.
    pub crt_date: u16,
    /// Last access date.
    pub lst_acc_date: u16,
    /// First cluster high word.
    pub fst_clus_hi: u16,
    /// Write time.
    pub wrt_time: u16,
    /// Write date.
    pub wrt_date: u16,
    /// First cluster low word.
    pub fst_clus_lo: u16,
    /// File size.
    pub file_size: u32,
}

impl Fat32DirEntry {
    /// Entry size in bytes.
    pub const SIZE: usize = 32;

    /// Attribute: Read-only.
    pub const ATTR_READ_ONLY: u8 = 0x01;
    /// Attribute: Hidden.
    pub const ATTR_HIDDEN: u8 = 0x02;
    /// Attribute: System.
    pub const ATTR_SYSTEM: u8 = 0x04;
    /// Attribute: Volume ID.
    pub const ATTR_VOLUME_ID: u8 = 0x08;
    /// Attribute: Directory.
    pub const ATTR_DIRECTORY: u8 = 0x10;
    /// Attribute: Archive.
    pub const ATTR_ARCHIVE: u8 = 0x20;
    /// Attribute: Long name.
    pub const ATTR_LONG_NAME: u8 = 0x0F;

    /// Entry marks deleted file.
    pub const DELETED: u8 = 0xE5;
    /// Entry marks last entry.
    pub const LAST: u8 = 0x00;

    /// Check if this is a free entry.
    pub fn is_free(&self) -> bool {
        self.name[0] == Self::DELETED || self.name[0] == Self::LAST
    }

    /// Check if this is the last entry.
    pub fn is_last(&self) -> bool {
        self.name[0] == Self::LAST
    }

    /// Check if this is a long name entry.
    pub fn is_long_name(&self) -> bool {
        self.attr == Self::ATTR_LONG_NAME
    }

    /// Check if this is a directory.
    pub fn is_dir(&self) -> bool {
        self.attr & Self::ATTR_DIRECTORY != 0
    }

    /// Check if this is a volume label.
    pub fn is_volume_label(&self) -> bool {
        self.attr & Self::ATTR_VOLUME_ID != 0
    }

    /// Get first cluster number.
    pub fn first_cluster(&self) -> u32 {
        ((self.fst_clus_hi as u32) << 16) | (self.fst_clus_lo as u32)
    }

    /// Get short name as string.
    pub fn short_name(&self) -> [u8; 13] {
        let mut result = [0u8; 13];
        let mut pos = 0;

        // Copy name part (first 8 bytes, trimmed)
        for i in 0..8 {
            if self.name[i] != b' ' {
                result[pos] = self.name[i];
                pos += 1;
            }
        }

        // Add dot if there's an extension
        let has_ext = self.name[8] != b' ' || self.name[9] != b' ' || self.name[10] != b' ';
        if has_ext {
            result[pos] = b'.';
            pos += 1;

            // Copy extension
            for i in 8..11 {
                if self.name[i] != b' ' {
                    result[pos] = self.name[i];
                    pos += 1;
                }
            }
        }

        result
    }

    /// Get file type.
    pub fn file_type(&self) -> FileType {
        if self.is_dir() {
            FileType::Directory
        } else {
            FileType::Regular
        }
    }

    /// Decode FAT date.
    fn decode_date(date: u16) -> (u16, u8, u8) {
        let year = 1980 + ((date >> 9) & 0x7F);
        let month = ((date >> 5) & 0x0F) as u8;
        let day = (date & 0x1F) as u8;
        (year, month, day)
    }

    /// Decode FAT time.
    fn decode_time(time: u16) -> (u8, u8, u8) {
        let hour = ((time >> 11) & 0x1F) as u8;
        let minute = ((time >> 5) & 0x3F) as u8;
        let second = ((time & 0x1F) * 2) as u8;
        (hour, minute, second)
    }

    /// Convert to DirEntry.
    pub fn to_dir_entry(&self) -> DirEntry {
        let short_name = self.short_name();
        let mut name = [0u8; 256];
        let mut len = 0;

        for &b in &short_name {
            if b == 0 {
                break;
            }
            name[len] = b;
            len += 1;
        }

        DirEntry {
            name,
            name_len: len,
            inode: self.first_cluster() as u64,
            file_type: self.file_type(),
        }
    }

    /// Convert to FileMetadata.
    pub fn to_metadata(&self) -> FileMetadata {
        let permissions = if self.attr & Self::ATTR_READ_ONLY != 0 {
            FilePermissions(0o444)
        } else {
            FilePermissions(0o644)
        };

        FileMetadata {
            file_type: self.file_type(),
            permissions,
            size: self.file_size as u64,
            nlink: 1,
            uid: 0,
            gid: 0,
            block_size: 4096,
            blocks: (self.file_size as u64 + 511) / 512,
            atime: 0, // TODO: Convert dates
            mtime: 0,
            ctime: 0,
            crtime: 0,
            inode: self.first_cluster() as u64,
            dev: 0,
            rdev: 0,
        }
    }
}

/// FAT32 long filename entry.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Fat32LfnEntry {
    /// Sequence number.
    pub ord: u8,
    /// Characters 1-5.
    pub name1: [u16; 5],
    /// Attributes (always 0x0F).
    pub attr: u8,
    /// Type (always 0).
    pub type_: u8,
    /// Checksum.
    pub chksum: u8,
    /// Characters 6-11.
    pub name2: [u16; 6],
    /// First cluster (always 0).
    pub fst_clus_lo: u16,
    /// Characters 12-13.
    pub name3: [u16; 2],
}

impl Fat32LfnEntry {
    /// Last LFN entry marker.
    pub const LAST_LFN_ENTRY: u8 = 0x40;
    /// LFN entry sequence mask.
    pub const SEQ_MASK: u8 = 0x3F;

    /// Check if this is the last LFN entry.
    pub fn is_last(&self) -> bool {
        self.ord & Self::LAST_LFN_ENTRY != 0
    }

    /// Get sequence number (1-based).
    pub fn sequence(&self) -> u8 {
        self.ord & Self::SEQ_MASK
    }

    /// Get characters from this entry.
    pub fn chars(&self) -> [u16; 13] {
        let mut result = [0u16; 13];
        // SAFETY: Read unaligned values from packed struct fields
        unsafe {
            let name1 = core::ptr::addr_of!(self.name1).read_unaligned();
            let name2 = core::ptr::addr_of!(self.name2).read_unaligned();
            let name3 = core::ptr::addr_of!(self.name3).read_unaligned();
            result[0..5].copy_from_slice(&name1);
            result[5..11].copy_from_slice(&name2);
            result[11..13].copy_from_slice(&name3);
        }
        result
    }
}

/// FAT32 filesystem.
pub struct Fat32Filesystem {
    /// BPB.
    bpb: Fat32Bpb,
    /// Is read-only.
    read_only: bool,
    /// Free cluster count.
    free_clusters: u32,
}

impl Fat32Filesystem {
    /// Create a new FAT32 filesystem (placeholder).
    pub fn new() -> Self {
        Fat32Filesystem {
            bpb: unsafe { core::mem::zeroed() },
            read_only: false,
            free_clusters: 0,
        }
    }
}

impl Default for Fat32Filesystem {
    fn default() -> Self {
        Self::new()
    }
}

impl Filesystem for Fat32Filesystem {
    fn fs_type(&self) -> &str {
        "fat32"
    }

    fn statfs(&self) -> Result<FsStats, StorageError> {
        Ok(FsStats {
            fs_type: 0x4D44, // MSDOS_SUPER_MAGIC
            block_size: self.bpb.cluster_size(),
            total_blocks: self.bpb.total_clusters() as u64,
            free_blocks: self.free_clusters as u64,
            available_blocks: self.free_clusters as u64,
            total_inodes: 0, // FAT doesn't have inodes
            free_inodes: 0,
            fs_id: self.bpb.volume_id as u64,
            max_name_len: 255, // With LFN
            fragment_size: self.bpb.cluster_size(),
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
        Err(StorageError::Unsupported) // FAT doesn't support symlinks
    }

    fn readlink(&self, _path: &str) -> Result<String, StorageError> {
        Err(StorageError::Unsupported)
    }

    fn link(&self, _old: &str, _new: &str) -> Result<(), StorageError> {
        Err(StorageError::Unsupported) // FAT doesn't support hard links
    }

    fn setattr(&self, _path: &str, _attr: &FileMetadata) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn open(&self, _path: &str, _flags: OpenFlags) -> Result<u64, StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn close(&self, _handle: u64) -> Result<(), StorageError> {
        Ok(())
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
        Err(StorageError::Unsupported) // FAT doesn't support preallocation
    }

    fn sync(&self) -> Result<(), StorageError> {
        Ok(())
    }
}
