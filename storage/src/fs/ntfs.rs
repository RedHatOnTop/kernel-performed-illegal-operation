//! NTFS filesystem implementation (read-only).
//!
//! This module provides read-only support for the NTFS filesystem,
//! the standard Windows filesystem.

use alloc::string::String;
use alloc::vec::Vec;

use crate::vfs::{Filesystem, FsStats};
use crate::{DirEntry, FileMetadata, FilePermissions, FileType, OpenFlags, StorageError};

/// NTFS boot sector.
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct NtfsBootSector {
    /// Jump instruction.
    pub jmp_boot: [u8; 3],
    /// OEM ID ("NTFS    ").
    pub oem_id: [u8; 8],
    /// Bytes per sector.
    pub bytes_per_sector: u16,
    /// Sectors per cluster.
    pub sectors_per_cluster: u8,
    /// Reserved sectors (always 0).
    pub reserved_sectors: u16,
    /// Always 0.
    pub reserved1: [u8; 3],
    /// Always 0.
    pub reserved2: u16,
    /// Media type.
    pub media: u8,
    /// Always 0.
    pub reserved3: u16,
    /// Sectors per track.
    pub sectors_per_track: u16,
    /// Number of heads.
    pub num_heads: u16,
    /// Hidden sectors.
    pub hidden_sectors: u32,
    /// Always 0.
    pub reserved4: u32,
    /// Always 0x80008000.
    pub reserved5: u32,
    /// Total sectors.
    pub total_sectors: u64,
    /// MFT cluster number.
    pub mft_cluster: u64,
    /// MFT mirror cluster number.
    pub mft_mirror_cluster: u64,
    /// Clusters per MFT record (signed).
    pub clusters_per_mft_record: i8,
    /// Reserved.
    pub reserved6: [u8; 3],
    /// Clusters per index record.
    pub clusters_per_index_record: i8,
    /// Reserved.
    pub reserved7: [u8; 3],
    /// Volume serial number.
    pub volume_serial: u64,
    /// Checksum.
    pub checksum: u32,
}

impl NtfsBootSector {
    /// Boot sector size.
    pub const SIZE: usize = 512;
    /// NTFS OEM ID.
    pub const OEM_ID: &'static [u8; 8] = b"NTFS    ";

    /// Get cluster size in bytes.
    pub fn cluster_size(&self) -> u32 {
        self.bytes_per_sector as u32 * self.sectors_per_cluster as u32
    }

    /// Get MFT record size in bytes.
    pub fn mft_record_size(&self) -> u32 {
        if self.clusters_per_mft_record > 0 {
            self.clusters_per_mft_record as u32 * self.cluster_size()
        } else {
            1 << (-self.clusters_per_mft_record as u32)
        }
    }

    /// Get index record size in bytes.
    pub fn index_record_size(&self) -> u32 {
        if self.clusters_per_index_record > 0 {
            self.clusters_per_index_record as u32 * self.cluster_size()
        } else {
            1 << (-self.clusters_per_index_record as u32)
        }
    }
}

/// NTFS MFT entry header.
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct NtfsMftEntry {
    /// Magic ("FILE" or "BAAD").
    pub magic: [u8; 4],
    /// Update sequence offset.
    pub update_seq_offset: u16,
    /// Update sequence size.
    pub update_seq_size: u16,
    /// Log file sequence number.
    pub lsn: u64,
    /// Sequence number.
    pub sequence: u16,
    /// Hard link count.
    pub link_count: u16,
    /// First attribute offset.
    pub attrs_offset: u16,
    /// Flags.
    pub flags: u16,
    /// Real size of record.
    pub used_size: u32,
    /// Allocated size of record.
    pub allocated_size: u32,
    /// Base record.
    pub base_record: u64,
    /// Next attribute ID.
    pub next_attr_id: u16,
    /// Alignment.
    pub align: u16,
    /// Record number.
    pub record_number: u32,
}

impl NtfsMftEntry {
    /// MFT entry magic ("FILE").
    pub const MAGIC_FILE: &'static [u8; 4] = b"FILE";
    /// Bad MFT entry magic ("BAAD").
    pub const MAGIC_BAAD: &'static [u8; 4] = b"BAAD";

    /// Entry is in use.
    pub const FLAG_IN_USE: u16 = 0x0001;
    /// Entry is a directory.
    pub const FLAG_DIRECTORY: u16 = 0x0002;

    /// Check if entry is in use.
    pub fn is_in_use(&self) -> bool {
        self.flags & Self::FLAG_IN_USE != 0
    }

    /// Check if entry is a directory.
    pub fn is_directory(&self) -> bool {
        self.flags & Self::FLAG_DIRECTORY != 0
    }

    /// Check if magic is valid.
    pub fn is_valid(&self) -> bool {
        &self.magic == Self::MAGIC_FILE
    }
}

/// Well-known MFT entry numbers.
pub mod mft_entries {
    /// $MFT - Master File Table.
    pub const MFT: u64 = 0;
    /// $MFTMirr - MFT mirror.
    pub const MFT_MIRROR: u64 = 1;
    /// $LogFile - Transaction log.
    pub const LOG_FILE: u64 = 2;
    /// $Volume - Volume information.
    pub const VOLUME: u64 = 3;
    /// $AttrDef - Attribute definitions.
    pub const ATTR_DEF: u64 = 4;
    /// Root directory (.).
    pub const ROOT: u64 = 5;
    /// $Bitmap - Cluster allocation bitmap.
    pub const BITMAP: u64 = 6;
    /// $Boot - Boot sector.
    pub const BOOT: u64 = 7;
    /// $BadClus - Bad cluster list.
    pub const BAD_CLUS: u64 = 8;
    /// $Secure - Security descriptors.
    pub const SECURE: u64 = 9;
    /// $UpCase - Uppercase table.
    pub const UPCASE: u64 = 10;
    /// $Extend - Extended metadata.
    pub const EXTEND: u64 = 11;
    /// First user file record.
    pub const FIRST_USER: u64 = 16;
}

/// NTFS attribute types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum NtfsAttrType {
    /// $STANDARD_INFORMATION.
    StandardInformation = 0x10,
    /// $ATTRIBUTE_LIST.
    AttributeList = 0x20,
    /// $FILE_NAME.
    FileName = 0x30,
    /// $OBJECT_ID.
    ObjectId = 0x40,
    /// $SECURITY_DESCRIPTOR.
    SecurityDescriptor = 0x50,
    /// $VOLUME_NAME.
    VolumeName = 0x60,
    /// $VOLUME_INFORMATION.
    VolumeInformation = 0x70,
    /// $DATA.
    Data = 0x80,
    /// $INDEX_ROOT.
    IndexRoot = 0x90,
    /// $INDEX_ALLOCATION.
    IndexAllocation = 0xA0,
    /// $BITMAP.
    Bitmap = 0xB0,
    /// $REPARSE_POINT.
    ReparsePoint = 0xC0,
    /// $EA_INFORMATION.
    EaInformation = 0xD0,
    /// $EA.
    Ea = 0xE0,
    /// End marker.
    End = 0xFFFFFFFF,
}

/// NTFS attribute header (common part).
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct NtfsAttrHeader {
    /// Attribute type.
    pub attr_type: u32,
    /// Record length.
    pub length: u32,
    /// Non-resident flag.
    pub non_resident: u8,
    /// Name length.
    pub name_length: u8,
    /// Name offset.
    pub name_offset: u16,
    /// Flags.
    pub flags: u16,
    /// Attribute ID.
    pub attr_id: u16,
}

impl NtfsAttrHeader {
    /// Attribute is compressed.
    pub const FLAG_COMPRESSED: u16 = 0x0001;
    /// Attribute is encrypted.
    pub const FLAG_ENCRYPTED: u16 = 0x4000;
    /// Attribute is sparse.
    pub const FLAG_SPARSE: u16 = 0x8000;

    /// Check if attribute is resident.
    pub fn is_resident(&self) -> bool {
        self.non_resident == 0
    }
}

/// NTFS resident attribute header.
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct NtfsResidentAttr {
    /// Common header.
    pub header: NtfsAttrHeader,
    /// Value length.
    pub value_length: u32,
    /// Value offset.
    pub value_offset: u16,
    /// Indexed flag.
    pub indexed_flag: u8,
    /// Padding.
    pub padding: u8,
}

/// NTFS non-resident attribute header.
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct NtfsNonResidentAttr {
    /// Common header.
    pub header: NtfsAttrHeader,
    /// Lowest VCN.
    pub lowest_vcn: u64,
    /// Highest VCN.
    pub highest_vcn: u64,
    /// Data runs offset.
    pub data_runs_offset: u16,
    /// Compression unit size.
    pub compression_unit_size: u16,
    /// Padding.
    pub padding: u32,
    /// Allocated size.
    pub allocated_size: u64,
    /// Data size.
    pub data_size: u64,
    /// Initialized size.
    pub initialized_size: u64,
}

/// NTFS $FILE_NAME attribute.
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct NtfsFileName {
    /// Parent directory reference.
    pub parent_ref: u64,
    /// Creation time.
    pub creation_time: u64,
    /// Modification time.
    pub modification_time: u64,
    /// MFT modification time.
    pub mft_modification_time: u64,
    /// Access time.
    pub access_time: u64,
    /// Allocated size.
    pub allocated_size: u64,
    /// Data size.
    pub data_size: u64,
    /// File flags.
    pub flags: u32,
    /// Reparse value.
    pub reparse_value: u32,
    /// Name length.
    pub name_length: u8,
    /// Namespace.
    pub namespace: u8,
    // Followed by name (UTF-16LE)
}

impl NtfsFileName {
    /// POSIX namespace.
    pub const NAMESPACE_POSIX: u8 = 0;
    /// Win32 namespace.
    pub const NAMESPACE_WIN32: u8 = 1;
    /// DOS namespace.
    pub const NAMESPACE_DOS: u8 = 2;
    /// Win32 and DOS namespace.
    pub const NAMESPACE_WIN32_AND_DOS: u8 = 3;

    /// File is read-only.
    pub const FLAG_READ_ONLY: u32 = 0x0001;
    /// File is hidden.
    pub const FLAG_HIDDEN: u32 = 0x0002;
    /// File is system.
    pub const FLAG_SYSTEM: u32 = 0x0004;
    /// File is a directory.
    pub const FLAG_DIRECTORY: u32 = 0x10000000;
}

/// NTFS $STANDARD_INFORMATION attribute.
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct NtfsStandardInfo {
    /// Creation time.
    pub creation_time: u64,
    /// Modification time.
    pub modification_time: u64,
    /// MFT modification time.
    pub mft_modification_time: u64,
    /// Access time.
    pub access_time: u64,
    /// File flags.
    pub flags: u32,
    /// Maximum versions.
    pub max_versions: u32,
    /// Version number.
    pub version_number: u32,
    /// Class ID.
    pub class_id: u32,
    /// Owner ID.
    pub owner_id: u32,
    /// Security ID.
    pub security_id: u32,
    /// Quota charged.
    pub quota_charged: u64,
    /// USN.
    pub usn: u64,
}

/// Convert NTFS timestamp (100ns since 1601-01-01) to Unix timestamp.
fn ntfs_time_to_unix(ntfs_time: u64) -> u64 {
    // Number of 100ns intervals between 1601-01-01 and 1970-01-01
    const EPOCH_DIFF: u64 = 116444736000000000;

    if ntfs_time < EPOCH_DIFF {
        0
    } else {
        (ntfs_time - EPOCH_DIFF) * 100 // Convert to nanoseconds
    }
}

/// NTFS filesystem (read-only).
pub struct NtfsFilesystem {
    /// Boot sector.
    boot: NtfsBootSector,
    /// Volume serial number.
    serial: u64,
}

impl NtfsFilesystem {
    /// Create a new NTFS filesystem (placeholder).
    pub fn new() -> Self {
        NtfsFilesystem {
            boot: unsafe { core::mem::zeroed() },
            serial: 0,
        }
    }
}

impl Default for NtfsFilesystem {
    fn default() -> Self {
        Self::new()
    }
}

impl Filesystem for NtfsFilesystem {
    fn fs_type(&self) -> &str {
        "ntfs"
    }

    fn statfs(&self) -> Result<FsStats, StorageError> {
        let cluster_size = self.boot.cluster_size();
        let total_clusters = self.boot.total_sectors / self.boot.sectors_per_cluster as u64;

        Ok(FsStats {
            fs_type: 0x5346544E, // "NTFS" in little-endian
            block_size: cluster_size,
            total_blocks: total_clusters,
            free_blocks: 0, // Would need to read $Bitmap
            available_blocks: 0,
            total_inodes: 0, // NTFS doesn't expose this
            free_inodes: 0,
            fs_id: self.boot.volume_serial,
            max_name_len: 255,
            fragment_size: cluster_size,
            flags: crate::MountFlags::READ_ONLY,
        })
    }

    fn lookup(&self, _path: &str) -> Result<FileMetadata, StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn readdir(&self, _path: &str, _offset: u64) -> Result<Vec<DirEntry>, StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn create(&self, _path: &str, _mode: u16) -> Result<u64, StorageError> {
        Err(StorageError::ReadOnly)
    }

    fn mkdir(&self, _path: &str, _mode: u16) -> Result<(), StorageError> {
        Err(StorageError::ReadOnly)
    }

    fn unlink(&self, _path: &str) -> Result<(), StorageError> {
        Err(StorageError::ReadOnly)
    }

    fn rmdir(&self, _path: &str) -> Result<(), StorageError> {
        Err(StorageError::ReadOnly)
    }

    fn rename(&self, _old: &str, _new: &str) -> Result<(), StorageError> {
        Err(StorageError::ReadOnly)
    }

    fn symlink(&self, _target: &str, _link: &str) -> Result<(), StorageError> {
        Err(StorageError::ReadOnly)
    }

    fn readlink(&self, _path: &str) -> Result<String, StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn link(&self, _old: &str, _new: &str) -> Result<(), StorageError> {
        Err(StorageError::ReadOnly)
    }

    fn setattr(&self, _path: &str, _attr: &FileMetadata) -> Result<(), StorageError> {
        Err(StorageError::ReadOnly)
    }

    fn open(&self, _path: &str, flags: OpenFlags) -> Result<u64, StorageError> {
        if flags.contains(OpenFlags::WRITE) || flags.contains(OpenFlags::CREATE) {
            return Err(StorageError::ReadOnly);
        }
        Err(StorageError::NotImplemented)
    }

    fn close(&self, _handle: u64) -> Result<(), StorageError> {
        Ok(())
    }

    fn read(&self, _handle: u64, _offset: u64, _buffer: &mut [u8]) -> Result<usize, StorageError> {
        Err(StorageError::NotImplemented)
    }

    fn write(&self, _handle: u64, _offset: u64, _data: &[u8]) -> Result<usize, StorageError> {
        Err(StorageError::ReadOnly)
    }

    fn flush(&self, _handle: u64) -> Result<(), StorageError> {
        Ok(())
    }

    fn fsync(&self, _handle: u64, _data_only: bool) -> Result<(), StorageError> {
        Ok(())
    }

    fn truncate(&self, _path: &str, _size: u64) -> Result<(), StorageError> {
        Err(StorageError::ReadOnly)
    }

    fn fallocate(&self, _handle: u64, _offset: u64, _len: u64) -> Result<(), StorageError> {
        Err(StorageError::ReadOnly)
    }

    fn sync(&self) -> Result<(), StorageError> {
        Ok(())
    }
}
