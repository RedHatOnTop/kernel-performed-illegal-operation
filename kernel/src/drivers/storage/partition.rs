//! Partition Table Support
//!
//! GPT and MBR partition table parsing.

use super::{BlockDevice, StorageError};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// Partition information
#[derive(Debug, Clone)]
pub struct Partition {
    /// Partition number (1-based)
    pub number: u8,
    /// Partition name (from GPT or generated)
    pub name: String,
    /// Partition type GUID (for GPT) or type byte (for MBR)
    pub partition_type: PartitionType,
    /// Start LBA
    pub start_lba: u64,
    /// Size in sectors
    pub sector_count: u64,
    /// Is bootable
    pub bootable: bool,
    /// Partition GUID (GPT only)
    pub guid: Option<Guid>,
}

/// Partition type
#[derive(Debug, Clone)]
pub enum PartitionType {
    /// MBR partition type byte
    Mbr(u8),
    /// GPT partition type GUID
    Gpt(Guid),
}

/// GUID structure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl Guid {
    /// Empty/null GUID
    pub const EMPTY: Guid = Guid {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };

    /// EFI System Partition GUID
    pub const EFI_SYSTEM: Guid = Guid {
        data1: 0xC12A7328,
        data2: 0xF81F,
        data3: 0x11D2,
        data4: [0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E, 0xC9, 0x3B],
    };

    /// Microsoft Basic Data GUID (for NTFS, FAT32)
    pub const MICROSOFT_BASIC_DATA: Guid = Guid {
        data1: 0xEBD0A0A2,
        data2: 0xB9E5,
        data3: 0x4433,
        data4: [0x87, 0xC0, 0x68, 0xB6, 0xB7, 0x26, 0x99, 0xC7],
    };

    /// Linux Filesystem GUID
    pub const LINUX_FILESYSTEM: Guid = Guid {
        data1: 0x0FC63DAF,
        data2: 0x8483,
        data3: 0x4772,
        data4: [0x8E, 0x79, 0x3D, 0x69, 0xD8, 0x47, 0x7D, 0xE4],
    };

    /// Linux Swap GUID
    pub const LINUX_SWAP: Guid = Guid {
        data1: 0x0657FD6D,
        data2: 0xA4AB,
        data3: 0x43C4,
        data4: [0x84, 0xE5, 0x09, 0x33, 0xC8, 0x4B, 0x4F, 0x4F],
    };

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        *self == Self::EMPTY
    }

    /// Parse from bytes (little-endian)
    pub fn from_bytes(bytes: &[u8; 16]) -> Self {
        Self {
            data1: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            data2: u16::from_le_bytes([bytes[4], bytes[5]]),
            data3: u16::from_le_bytes([bytes[6], bytes[7]]),
            data4: [
                bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14],
                bytes[15],
            ],
        }
    }

    /// Convert to bytes
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&self.data1.to_le_bytes());
        bytes[4..6].copy_from_slice(&self.data2.to_le_bytes());
        bytes[6..8].copy_from_slice(&self.data3.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.data4);
        bytes
    }
}

/// MBR Partition Entry (16 bytes)
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct MbrPartitionEntry {
    /// Boot indicator (0x80 = bootable)
    pub boot_indicator: u8,
    /// Starting head
    pub starting_head: u8,
    /// Starting sector (bits 0-5) and cylinder high (bits 6-7)
    pub starting_sector_cyl: u8,
    /// Starting cylinder low
    pub starting_cylinder: u8,
    /// Partition type
    pub partition_type: u8,
    /// Ending head
    pub ending_head: u8,
    /// Ending sector (bits 0-5) and cylinder high (bits 6-7)
    pub ending_sector_cyl: u8,
    /// Ending cylinder low
    pub ending_cylinder: u8,
    /// Starting LBA
    pub starting_lba: u32,
    /// Size in sectors
    pub size_in_sectors: u32,
}

/// MBR partition type codes
pub mod mbr_types {
    pub const EMPTY: u8 = 0x00;
    pub const FAT12: u8 = 0x01;
    pub const FAT16_SMALL: u8 = 0x04;
    pub const EXTENDED: u8 = 0x05;
    pub const FAT16: u8 = 0x06;
    pub const NTFS: u8 = 0x07;
    pub const FAT32: u8 = 0x0B;
    pub const FAT32_LBA: u8 = 0x0C;
    pub const FAT16_LBA: u8 = 0x0E;
    pub const EXTENDED_LBA: u8 = 0x0F;
    pub const LINUX: u8 = 0x83;
    pub const LINUX_SWAP: u8 = 0x82;
    pub const LINUX_LVM: u8 = 0x8E;
    pub const EFI_PROTECTIVE: u8 = 0xEE;
    pub const EFI_SYSTEM: u8 = 0xEF;
}

/// Master Boot Record
#[repr(C, packed)]
pub struct Mbr {
    /// Bootstrap code (446 bytes)
    pub bootstrap: [u8; 446],
    /// Partition entries (4 x 16 bytes)
    pub partitions: [MbrPartitionEntry; 4],
    /// Boot signature (0x55AA)
    pub signature: u16,
}

impl Mbr {
    /// Expected signature
    pub const SIGNATURE: u16 = 0xAA55;

    /// Check if valid MBR
    pub fn is_valid(&self) -> bool {
        self.signature == Self::SIGNATURE
    }

    /// Check if this is a protective MBR (GPT disk)
    pub fn is_protective(&self) -> bool {
        self.is_valid() && self.partitions[0].partition_type == mbr_types::EFI_PROTECTIVE
    }
}

/// GPT Header
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct GptHeader {
    /// Signature "EFI PART"
    pub signature: [u8; 8],
    /// Revision (usually 0x00010000)
    pub revision: u32,
    /// Header size (usually 92)
    pub header_size: u32,
    /// CRC32 of header
    pub header_crc32: u32,
    /// Reserved
    pub reserved: u32,
    /// Current LBA (location of this header)
    pub current_lba: u64,
    /// Backup LBA (location of other header)
    pub backup_lba: u64,
    /// First usable LBA
    pub first_usable_lba: u64,
    /// Last usable LBA
    pub last_usable_lba: u64,
    /// Disk GUID
    pub disk_guid: [u8; 16],
    /// Partition entries starting LBA
    pub partition_entries_lba: u64,
    /// Number of partition entries
    pub num_partition_entries: u32,
    /// Size of each partition entry (usually 128)
    pub partition_entry_size: u32,
    /// CRC32 of partition entries
    pub partition_entries_crc32: u32,
}

impl GptHeader {
    /// Expected signature
    pub const SIGNATURE: &'static [u8; 8] = b"EFI PART";

    /// Check if valid
    pub fn is_valid(&self) -> bool {
        &self.signature == Self::SIGNATURE
    }
}

/// GPT Partition Entry
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct GptPartitionEntry {
    /// Partition type GUID
    pub type_guid: [u8; 16],
    /// Unique partition GUID
    pub partition_guid: [u8; 16],
    /// Starting LBA
    pub starting_lba: u64,
    /// Ending LBA (inclusive)
    pub ending_lba: u64,
    /// Attributes
    pub attributes: u64,
    /// Partition name (UTF-16LE, up to 36 chars)
    pub name: [u16; 36],
}

impl GptPartitionEntry {
    /// Check if entry is used
    pub fn is_used(&self) -> bool {
        !Guid::from_bytes(&self.type_guid).is_empty()
    }

    /// Get partition name as String
    pub fn get_name(&self) -> String {
        let mut name = String::new();
        // Copy the name array to avoid unaligned access
        let name_copy: [u16; 36] = self.name;
        for c in name_copy {
            if c == 0 {
                break;
            }
            if let Some(ch) = char::from_u32(c as u32) {
                name.push(ch);
            }
        }
        name
    }
}

/// Partition table type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionTableType {
    /// No partition table found
    None,
    /// Master Boot Record
    Mbr,
    /// GUID Partition Table
    Gpt,
}

/// Parse partition table from block device
pub fn parse_partitions(device: &dyn BlockDevice) -> Result<Vec<Partition>, StorageError> {
    let block_size = device.block_size();
    let mut sector = vec![0u8; block_size as usize];

    // Read MBR (sector 0)
    device.read_blocks(0, 1, &mut sector)?;

    let mbr = unsafe { &*(sector.as_ptr() as *const Mbr) };

    if !mbr.is_valid() {
        return Ok(Vec::new()); // No valid partition table
    }

    if mbr.is_protective() {
        // GPT disk, parse GPT
        parse_gpt(device)
    } else {
        // MBR disk
        parse_mbr(mbr)
    }
}

/// Parse MBR partition table
fn parse_mbr(mbr: &Mbr) -> Result<Vec<Partition>, StorageError> {
    let mut partitions = Vec::new();

    for (i, entry) in mbr.partitions.iter().enumerate() {
        if entry.partition_type == mbr_types::EMPTY {
            continue;
        }

        // Skip extended partitions (would need additional parsing)
        if entry.partition_type == mbr_types::EXTENDED
            || entry.partition_type == mbr_types::EXTENDED_LBA
        {
            continue;
        }

        partitions.push(Partition {
            number: (i + 1) as u8,
            name: format!("Partition {}", i + 1),
            partition_type: PartitionType::Mbr(entry.partition_type),
            start_lba: entry.starting_lba as u64,
            sector_count: entry.size_in_sectors as u64,
            bootable: entry.boot_indicator == 0x80,
            guid: None,
        });
    }

    Ok(partitions)
}

/// Parse GPT partition table
fn parse_gpt(device: &dyn BlockDevice) -> Result<Vec<Partition>, StorageError> {
    let block_size = device.block_size();
    let mut sector = vec![0u8; block_size as usize];

    // Read GPT header (sector 1)
    device.read_blocks(1, 1, &mut sector)?;

    let header = unsafe { &*(sector.as_ptr() as *const GptHeader) };

    if !header.is_valid() {
        return Err(StorageError::IoError(String::from("Invalid GPT header")));
    }

    let mut partitions = Vec::new();
    let entries_per_sector = block_size / header.partition_entry_size;
    let mut partition_number = 1u8;

    // Read partition entries
    let mut current_lba = header.partition_entries_lba;
    let mut entries_read = 0u32;

    while entries_read < header.num_partition_entries {
        device.read_blocks(current_lba, 1, &mut sector)?;

        for i in 0..entries_per_sector {
            if entries_read >= header.num_partition_entries {
                break;
            }

            let offset = (i * header.partition_entry_size) as usize;
            let entry = unsafe { &*(sector[offset..].as_ptr() as *const GptPartitionEntry) };

            if entry.is_used() {
                let type_guid = Guid::from_bytes(&entry.type_guid);
                let partition_guid = Guid::from_bytes(&entry.partition_guid);

                partitions.push(Partition {
                    number: partition_number,
                    name: entry.get_name(),
                    partition_type: PartitionType::Gpt(type_guid),
                    start_lba: entry.starting_lba,
                    sector_count: entry.ending_lba - entry.starting_lba + 1,
                    bootable: (entry.attributes & 4) != 0, // Legacy BIOS bootable
                    guid: Some(partition_guid),
                });

                partition_number += 1;
            }

            entries_read += 1;
        }

        current_lba += 1;
    }

    Ok(partitions)
}

/// Get partition type name
pub fn partition_type_name(ptype: &PartitionType) -> &'static str {
    match ptype {
        PartitionType::Mbr(t) => match *t {
            mbr_types::FAT12 => "FAT12",
            mbr_types::FAT16 | mbr_types::FAT16_SMALL | mbr_types::FAT16_LBA => "FAT16",
            mbr_types::FAT32 | mbr_types::FAT32_LBA => "FAT32",
            mbr_types::NTFS => "NTFS",
            mbr_types::LINUX => "Linux",
            mbr_types::LINUX_SWAP => "Linux Swap",
            mbr_types::EFI_SYSTEM => "EFI System",
            _ => "Unknown",
        },
        PartitionType::Gpt(g) => {
            if *g == Guid::EFI_SYSTEM {
                "EFI System"
            } else if *g == Guid::MICROSOFT_BASIC_DATA {
                "Basic Data"
            } else if *g == Guid::LINUX_FILESYSTEM {
                "Linux"
            } else if *g == Guid::LINUX_SWAP {
                "Linux Swap"
            } else {
                "Unknown"
            }
        }
    }
}

use alloc::vec;
