//! Partition table parsing.
//!
//! This module provides support for parsing partition tables:
//! - MBR (Master Boot Record)
//! - GPT (GUID Partition Table)

use crate::StorageError;

/// Partition type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionType {
    /// Empty/unused partition.
    Empty,
    /// FAT12.
    Fat12,
    /// FAT16 (small).
    Fat16Small,
    /// Extended partition (CHS).
    Extended,
    /// FAT16 (large).
    Fat16,
    /// NTFS.
    Ntfs,
    /// FAT32 (CHS).
    Fat32Chs,
    /// FAT32 (LBA).
    Fat32Lba,
    /// FAT16 (LBA).
    Fat16Lba,
    /// Extended partition (LBA).
    ExtendedLba,
    /// Linux.
    Linux,
    /// Linux swap.
    LinuxSwap,
    /// Linux LVM.
    LinuxLvm,
    /// EFI System Partition.
    EfiSystem,
    /// GPT protective MBR.
    GptProtective,
    /// Unknown type.
    Unknown(u8),
}

impl From<u8> for PartitionType {
    fn from(value: u8) -> Self {
        match value {
            0x00 => PartitionType::Empty,
            0x01 => PartitionType::Fat12,
            0x04 => PartitionType::Fat16Small,
            0x05 => PartitionType::Extended,
            0x06 => PartitionType::Fat16,
            0x07 => PartitionType::Ntfs,
            0x0B => PartitionType::Fat32Chs,
            0x0C => PartitionType::Fat32Lba,
            0x0E => PartitionType::Fat16Lba,
            0x0F => PartitionType::ExtendedLba,
            0x82 => PartitionType::LinuxSwap,
            0x83 => PartitionType::Linux,
            0x8E => PartitionType::LinuxLvm,
            0xEE => PartitionType::GptProtective,
            0xEF => PartitionType::EfiSystem,
            other => PartitionType::Unknown(other),
        }
    }
}

impl PartitionType {
    /// Check if this is an extended partition.
    pub fn is_extended(&self) -> bool {
        matches!(self, PartitionType::Extended | PartitionType::ExtendedLba)
    }

    /// Check if this is a GPT protective MBR.
    pub fn is_gpt(&self) -> bool {
        matches!(self, PartitionType::GptProtective)
    }
}

/// MBR partition entry.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct MbrPartitionEntry {
    /// Boot indicator (0x80 = bootable).
    pub boot_indicator: u8,
    /// Starting head.
    pub start_head: u8,
    /// Starting sector (bits 0-5) and cylinder high (bits 6-7).
    pub start_sector: u8,
    /// Starting cylinder low.
    pub start_cylinder: u8,
    /// Partition type.
    pub partition_type: u8,
    /// Ending head.
    pub end_head: u8,
    /// Ending sector (bits 0-5) and cylinder high (bits 6-7).
    pub end_sector: u8,
    /// Ending cylinder low.
    pub end_cylinder: u8,
    /// Starting LBA.
    pub start_lba: u32,
    /// Number of sectors.
    pub num_sectors: u32,
}

impl MbrPartitionEntry {
    /// Entry size in bytes.
    pub const SIZE: usize = 16;

    /// Check if partition is bootable.
    pub fn is_bootable(&self) -> bool {
        self.boot_indicator == 0x80
    }

    /// Check if partition is valid (non-empty).
    pub fn is_valid(&self) -> bool {
        self.partition_type != 0 && self.num_sectors > 0
    }

    /// Get partition type.
    pub fn get_type(&self) -> PartitionType {
        PartitionType::from(self.partition_type)
    }

    /// Get starting sector (CHS).
    pub fn start_sector_chs(&self) -> u8 {
        self.start_sector & 0x3F
    }

    /// Get starting cylinder (CHS).
    pub fn start_cylinder_chs(&self) -> u16 {
        ((self.start_sector as u16 & 0xC0) << 2) | self.start_cylinder as u16
    }
}

/// MBR (Master Boot Record).
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct Mbr {
    /// Bootstrap code.
    pub bootstrap: [u8; 446],
    /// Partition entries.
    pub partitions: [MbrPartitionEntry; 4],
    /// Boot signature (0xAA55).
    pub signature: u16,
}

impl Mbr {
    /// MBR size in bytes.
    pub const SIZE: usize = 512;
    /// MBR boot signature.
    pub const SIGNATURE: u16 = 0xAA55;

    /// Parse MBR from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, StorageError> {
        if data.len() < Self::SIZE {
            return Err(StorageError::InvalidArgument);
        }

        let mut bootstrap = [0u8; 446];
        bootstrap.copy_from_slice(&data[0..446]);

        let mut partitions = [MbrPartitionEntry {
            boot_indicator: 0,
            start_head: 0,
            start_sector: 0,
            start_cylinder: 0,
            partition_type: 0,
            end_head: 0,
            end_sector: 0,
            end_cylinder: 0,
            start_lba: 0,
            num_sectors: 0,
        }; 4];

        for i in 0..4 {
            let offset = 446 + i * 16;
            partitions[i] = MbrPartitionEntry {
                boot_indicator: data[offset],
                start_head: data[offset + 1],
                start_sector: data[offset + 2],
                start_cylinder: data[offset + 3],
                partition_type: data[offset + 4],
                end_head: data[offset + 5],
                end_sector: data[offset + 6],
                end_cylinder: data[offset + 7],
                start_lba: u32::from_le_bytes([
                    data[offset + 8],
                    data[offset + 9],
                    data[offset + 10],
                    data[offset + 11],
                ]),
                num_sectors: u32::from_le_bytes([
                    data[offset + 12],
                    data[offset + 13],
                    data[offset + 14],
                    data[offset + 15],
                ]),
            };
        }

        let signature = u16::from_le_bytes([data[510], data[511]]);

        if signature != Self::SIGNATURE {
            return Err(StorageError::InvalidFilesystem);
        }

        Ok(Mbr {
            bootstrap,
            partitions,
            signature,
        })
    }

    /// Check if this is a GPT disk.
    pub fn is_gpt(&self) -> bool {
        self.partitions[0].get_type().is_gpt()
    }
}

/// GPT header.
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct GptHeader {
    /// Signature ("EFI PART").
    pub signature: [u8; 8],
    /// Revision.
    pub revision: u32,
    /// Header size.
    pub header_size: u32,
    /// Header CRC32.
    pub header_crc32: u32,
    /// Reserved.
    pub reserved: u32,
    /// Current LBA.
    pub current_lba: u64,
    /// Backup LBA.
    pub backup_lba: u64,
    /// First usable LBA.
    pub first_usable_lba: u64,
    /// Last usable LBA.
    pub last_usable_lba: u64,
    /// Disk GUID.
    pub disk_guid: [u8; 16],
    /// Partition entry LBA.
    pub partition_entry_lba: u64,
    /// Number of partition entries.
    pub num_partition_entries: u32,
    /// Size of partition entry.
    pub partition_entry_size: u32,
    /// Partition entry array CRC32.
    pub partition_entries_crc32: u32,
}

impl GptHeader {
    /// GPT header size.
    pub const SIZE: usize = 92;
    /// GPT signature.
    pub const SIGNATURE: &'static [u8; 8] = b"EFI PART";

    /// Parse GPT header from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, StorageError> {
        if data.len() < Self::SIZE {
            return Err(StorageError::InvalidArgument);
        }

        let mut signature = [0u8; 8];
        signature.copy_from_slice(&data[0..8]);

        if &signature != Self::SIGNATURE {
            return Err(StorageError::InvalidFilesystem);
        }

        let mut disk_guid = [0u8; 16];
        disk_guid.copy_from_slice(&data[56..72]);

        Ok(GptHeader {
            signature,
            revision: u32::from_le_bytes([data[8], data[9], data[10], data[11]]),
            header_size: u32::from_le_bytes([data[12], data[13], data[14], data[15]]),
            header_crc32: u32::from_le_bytes([data[16], data[17], data[18], data[19]]),
            reserved: u32::from_le_bytes([data[20], data[21], data[22], data[23]]),
            current_lba: u64::from_le_bytes([
                data[24], data[25], data[26], data[27],
                data[28], data[29], data[30], data[31],
            ]),
            backup_lba: u64::from_le_bytes([
                data[32], data[33], data[34], data[35],
                data[36], data[37], data[38], data[39],
            ]),
            first_usable_lba: u64::from_le_bytes([
                data[40], data[41], data[42], data[43],
                data[44], data[45], data[46], data[47],
            ]),
            last_usable_lba: u64::from_le_bytes([
                data[48], data[49], data[50], data[51],
                data[52], data[53], data[54], data[55],
            ]),
            disk_guid,
            partition_entry_lba: u64::from_le_bytes([
                data[72], data[73], data[74], data[75],
                data[76], data[77], data[78], data[79],
            ]),
            num_partition_entries: u32::from_le_bytes([data[80], data[81], data[82], data[83]]),
            partition_entry_size: u32::from_le_bytes([data[84], data[85], data[86], data[87]]),
            partition_entries_crc32: u32::from_le_bytes([data[88], data[89], data[90], data[91]]),
        })
    }
}

/// GPT partition entry.
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct GptPartitionEntry {
    /// Partition type GUID.
    pub type_guid: [u8; 16],
    /// Partition GUID.
    pub partition_guid: [u8; 16],
    /// Starting LBA.
    pub start_lba: u64,
    /// Ending LBA.
    pub end_lba: u64,
    /// Attributes.
    pub attributes: u64,
    /// Partition name (UTF-16LE).
    pub name: [u16; 36],
}

impl GptPartitionEntry {
    /// Entry size in bytes.
    pub const SIZE: usize = 128;

    /// Empty type GUID.
    pub const TYPE_EMPTY: [u8; 16] = [0; 16];

    /// EFI System Partition type GUID.
    pub const TYPE_EFI_SYSTEM: [u8; 16] = [
        0x28, 0x73, 0x2A, 0xC1, 0x1F, 0xF8, 0xD2, 0x11,
        0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E, 0xC9, 0x3B,
    ];

    /// Microsoft Basic Data type GUID.
    pub const TYPE_MICROSOFT_BASIC: [u8; 16] = [
        0xA2, 0xA0, 0xD0, 0xEB, 0xE5, 0xB9, 0x33, 0x44,
        0x87, 0xC0, 0x68, 0xB6, 0xB7, 0x26, 0x99, 0xC7,
    ];

    /// Linux filesystem type GUID.
    pub const TYPE_LINUX_FS: [u8; 16] = [
        0xAF, 0x3D, 0xC6, 0x0F, 0x83, 0x84, 0x72, 0x47,
        0x8E, 0x79, 0x3D, 0x69, 0xD8, 0x47, 0x7D, 0xE4,
    ];

    /// Linux swap type GUID.
    pub const TYPE_LINUX_SWAP: [u8; 16] = [
        0x6D, 0xFD, 0x57, 0x06, 0xAB, 0xA4, 0xC4, 0x43,
        0x84, 0xE5, 0x09, 0x33, 0xC8, 0x4B, 0x4F, 0x4F,
    ];

    /// Parse GPT partition entry from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, StorageError> {
        if data.len() < Self::SIZE {
            return Err(StorageError::InvalidArgument);
        }

        let mut type_guid = [0u8; 16];
        type_guid.copy_from_slice(&data[0..16]);

        let mut partition_guid = [0u8; 16];
        partition_guid.copy_from_slice(&data[16..32]);

        let mut name = [0u16; 36];
        for i in 0..36 {
            let offset = 56 + i * 2;
            name[i] = u16::from_le_bytes([data[offset], data[offset + 1]]);
        }

        Ok(GptPartitionEntry {
            type_guid,
            partition_guid,
            start_lba: u64::from_le_bytes([
                data[32], data[33], data[34], data[35],
                data[36], data[37], data[38], data[39],
            ]),
            end_lba: u64::from_le_bytes([
                data[40], data[41], data[42], data[43],
                data[44], data[45], data[46], data[47],
            ]),
            attributes: u64::from_le_bytes([
                data[48], data[49], data[50], data[51],
                data[52], data[53], data[54], data[55],
            ]),
            name,
        })
    }

    /// Check if partition is valid (non-empty).
    pub fn is_valid(&self) -> bool {
        self.type_guid != Self::TYPE_EMPTY
    }

    /// Get partition size in sectors.
    pub fn size(&self) -> u64 {
        if self.end_lba >= self.start_lba {
            self.end_lba - self.start_lba + 1
        } else {
            0
        }
    }

    /// Check if this is an EFI System Partition.
    pub fn is_efi_system(&self) -> bool {
        self.type_guid == Self::TYPE_EFI_SYSTEM
    }

    /// Check if this is a Linux filesystem partition.
    pub fn is_linux_fs(&self) -> bool {
        self.type_guid == Self::TYPE_LINUX_FS
    }

    /// Check if this is a Linux swap partition.
    pub fn is_linux_swap(&self) -> bool {
        self.type_guid == Self::TYPE_LINUX_SWAP
    }

    /// Check if this is a Microsoft Basic Data partition.
    pub fn is_microsoft_basic(&self) -> bool {
        self.type_guid == Self::TYPE_MICROSOFT_BASIC
    }

    /// Required for system partition.
    pub fn is_required(&self) -> bool {
        self.attributes & 1 != 0
    }

    /// Partition should not be automounted.
    pub fn no_block_io(&self) -> bool {
        self.attributes & 2 != 0
    }

    /// Legacy BIOS bootable.
    pub fn legacy_bootable(&self) -> bool {
        self.attributes & 4 != 0
    }
}

/// Partition information.
#[derive(Debug, Clone)]
pub struct PartitionInfo {
    /// Partition index.
    pub index: u8,
    /// Partition type.
    pub part_type: PartitionType,
    /// Starting LBA.
    pub start_lba: u64,
    /// Number of sectors.
    pub sectors: u64,
    /// Is bootable.
    pub bootable: bool,
    /// Type GUID (for GPT).
    pub type_guid: Option<[u8; 16]>,
    /// Partition name (for GPT).
    pub name: Option<[u16; 36]>,
}

/// Partition table type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionTableType {
    /// MBR partition table.
    Mbr,
    /// GPT partition table.
    Gpt,
    /// No partition table found.
    None,
}

/// Detect partition table type.
pub fn detect_table_type(sector0: &[u8], sector1: &[u8]) -> PartitionTableType {
    // Try to parse MBR
    if let Ok(mbr) = Mbr::from_bytes(sector0) {
        if mbr.is_gpt() {
            // Check for GPT header at LBA 1
            if GptHeader::from_bytes(sector1).is_ok() {
                return PartitionTableType::Gpt;
            }
        }
        return PartitionTableType::Mbr;
    }

    PartitionTableType::None
}
