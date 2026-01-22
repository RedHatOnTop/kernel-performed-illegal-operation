//! Filesystem implementations.
//!
//! This module provides filesystem implementations:
//! - ext4 (Linux extended filesystem)
//! - FAT32 (File Allocation Table)
//! - NTFS (Windows NT filesystem, read-only)

pub mod ext4;
pub mod fat32;
pub mod ntfs;

use crate::StorageError;

/// Detect filesystem type from a block device.
pub fn detect_filesystem(device: &dyn super::driver::BlockDevice) -> Option<&'static str> {
    let mut buffer = [0u8; 4096];

    // Read first sectors
    if device.read_blocks(0, &mut buffer).is_err() {
        return None;
    }

    // Check for FAT32/FAT16/FAT12
    // FAT BPB at offset 0, check for valid jump instruction and filesystem type
    if buffer[0] == 0xEB || buffer[0] == 0xE9 {
        // Check FAT32 specific field at offset 82
        if &buffer[82..87] == b"FAT32" {
            return Some("fat32");
        }
        // Check FAT16/FAT12 at offset 54
        if &buffer[54..59] == b"FAT16" || &buffer[54..59] == b"FAT12" {
            return Some("fat");
        }
    }

    // Check for NTFS
    // NTFS has "NTFS    " at offset 3
    if &buffer[3..11] == b"NTFS    " {
        return Some("ntfs");
    }

    // Check for ext2/3/4
    // Superblock is at offset 1024, magic is at offset 56 of superblock
    let mut sb_buffer = [0u8; 1024];
    if device.read_blocks(2, &mut sb_buffer).is_ok() {
        let magic = u16::from_le_bytes([sb_buffer[56], sb_buffer[57]]);
        if magic == 0xEF53 {
            // Check for ext4 specific features
            let compat = u32::from_le_bytes([sb_buffer[92], sb_buffer[93], sb_buffer[94], sb_buffer[95]]);
            let incompat = u32::from_le_bytes([sb_buffer[96], sb_buffer[97], sb_buffer[98], sb_buffer[99]]);

            // EXT4_FEATURE_INCOMPAT_EXTENTS = 0x0040
            if incompat & 0x0040 != 0 {
                return Some("ext4");
            }

            // EXT3_FEATURE_COMPAT_HAS_JOURNAL = 0x0004
            if compat & 0x0004 != 0 {
                return Some("ext3");
            }

            return Some("ext2");
        }
    }

    None
}

/// Filesystem type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemType {
    /// ext4 filesystem.
    Ext4,
    /// FAT32 filesystem.
    Fat32,
    /// NTFS filesystem (read-only).
    Ntfs,
    /// Unknown filesystem.
    Unknown,
}

impl FilesystemType {
    /// Get filesystem type from string.
    pub fn from_str(s: &str) -> Self {
        match s {
            "ext4" | "ext3" | "ext2" => FilesystemType::Ext4,
            "fat32" | "fat" | "vfat" => FilesystemType::Fat32,
            "ntfs" => FilesystemType::Ntfs,
            _ => FilesystemType::Unknown,
        }
    }

    /// Get filesystem name.
    pub fn name(&self) -> &'static str {
        match self {
            FilesystemType::Ext4 => "ext4",
            FilesystemType::Fat32 => "fat32",
            FilesystemType::Ntfs => "ntfs",
            FilesystemType::Unknown => "unknown",
        }
    }
}
