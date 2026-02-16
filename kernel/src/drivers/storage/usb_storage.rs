//! USB Mass Storage Driver
//!
//! USB Mass Storage Class (MSC) driver for USB drives.

use super::{BlockDevice, StorageError, StorageInfo, StorageInterface, StorageManager};
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// USB Mass Storage subclass codes
#[repr(u8)]
pub enum MscSubclass {
    /// SCSI command set not reported
    ScsiNotReported = 0x00,
    /// RBC (Reduced Block Commands)
    Rbc = 0x01,
    /// MMC-5 (ATAPI)
    Mmc5 = 0x02,
    /// Obsolete QIC-157
    Qic157 = 0x03,
    /// UFI (USB Floppy Interface)
    Ufi = 0x04,
    /// Obsolete SFF-8070i
    Sff8070i = 0x05,
    /// SCSI transparent command set
    ScsiTransparent = 0x06,
    /// LSD FS (Lockable Storage Devices)
    LsdFs = 0x07,
    /// IEEE 1667
    Ieee1667 = 0x08,
}

/// USB Mass Storage protocol codes
#[repr(u8)]
pub enum MscProtocol {
    /// Control/Bulk/Interrupt with command completion interrupt
    Cbi = 0x00,
    /// Control/Bulk/Interrupt without command completion
    CbiNoInterrupt = 0x01,
    /// Bulk-Only Transport
    BulkOnly = 0x50,
    /// UAS (USB Attached SCSI)
    Uas = 0x62,
}

/// Command Block Wrapper (CBW) for Bulk-Only Transport
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct CommandBlockWrapper {
    /// Signature 0x43425355 ('USBC')
    pub signature: u32,
    /// Tag (echoed in CSW)
    pub tag: u32,
    /// Data transfer length
    pub data_transfer_length: u32,
    /// Flags (bit 7: direction, 0 = OUT, 1 = IN)
    pub flags: u8,
    /// Logical Unit Number (bits 0-3)
    pub lun: u8,
    /// Command block length (1-16)
    pub cb_length: u8,
    /// Command block (SCSI command)
    pub cb: [u8; 16],
}

impl CommandBlockWrapper {
    /// CBW signature
    pub const SIGNATURE: u32 = 0x43425355;

    /// Create a new CBW
    pub fn new(tag: u32, data_length: u32, direction_in: bool, lun: u8, command: &[u8]) -> Self {
        let mut cb = [0u8; 16];
        let len = command.len().min(16);
        cb[..len].copy_from_slice(&command[..len]);

        Self {
            signature: Self::SIGNATURE,
            tag,
            data_transfer_length: data_length,
            flags: if direction_in { 0x80 } else { 0x00 },
            lun: lun & 0x0F,
            cb_length: len as u8,
            cb,
        }
    }
}

/// Command Status Wrapper (CSW) for Bulk-Only Transport
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct CommandStatusWrapper {
    /// Signature 0x53425355 ('USBS')
    pub signature: u32,
    /// Tag (from CBW)
    pub tag: u32,
    /// Data residue
    pub data_residue: u32,
    /// Status (0 = passed, 1 = failed, 2 = phase error)
    pub status: u8,
}

impl CommandStatusWrapper {
    /// CSW signature
    pub const SIGNATURE: u32 = 0x53425355;

    /// Check if valid
    pub fn is_valid(&self, expected_tag: u32) -> bool {
        self.signature == Self::SIGNATURE && self.tag == expected_tag
    }

    /// Check if command passed
    pub fn passed(&self) -> bool {
        self.status == 0
    }
}

/// SCSI command codes
pub mod scsi_cmd {
    pub const TEST_UNIT_READY: u8 = 0x00;
    pub const REQUEST_SENSE: u8 = 0x03;
    pub const INQUIRY: u8 = 0x12;
    pub const MODE_SENSE_6: u8 = 0x1A;
    pub const START_STOP_UNIT: u8 = 0x1B;
    pub const PREVENT_ALLOW_MEDIUM_REMOVAL: u8 = 0x1E;
    pub const READ_CAPACITY_10: u8 = 0x25;
    pub const READ_10: u8 = 0x28;
    pub const WRITE_10: u8 = 0x2A;
    pub const READ_CAPACITY_16: u8 = 0x9E;
    pub const READ_16: u8 = 0x88;
    pub const WRITE_16: u8 = 0x8A;
}

/// SCSI Inquiry data
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ScsiInquiry {
    /// Peripheral qualifier and device type
    pub peripheral: u8,
    /// RMB and device type modifier
    pub rmb: u8,
    /// Version
    pub version: u8,
    /// Response data format
    pub response_format: u8,
    /// Additional length
    pub additional_length: u8,
    /// Flags
    pub flags: [u8; 3],
    /// Vendor identification (8 bytes)
    pub vendor: [u8; 8],
    /// Product identification (16 bytes)
    pub product: [u8; 16],
    /// Product revision level (4 bytes)
    pub revision: [u8; 4],
}

/// SCSI Read Capacity (10) response
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ScsiReadCapacity10 {
    /// Last logical block address
    pub last_lba: u32,
    /// Block length in bytes
    pub block_length: u32,
}

/// SCSI Read Capacity (16) response
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ScsiReadCapacity16 {
    /// Last logical block address
    pub last_lba: u64,
    /// Block length in bytes
    pub block_length: u32,
    /// Protection info and flags
    pub prot_info: u8,
    /// Logical blocks per physical block exponent
    pub lb_per_pb_exp: u8,
    /// Lowest aligned LBA
    pub lowest_aligned: u16,
    /// Reserved
    pub reserved: [u8; 16],
}

/// USB Mass Storage device
pub struct UsbStorageDevice {
    /// Device name
    name: String,
    /// USB device address
    device_address: u8,
    /// Bulk-In endpoint
    bulk_in_ep: u8,
    /// Bulk-Out endpoint
    bulk_out_ep: u8,
    /// LUN (Logical Unit Number)
    lun: u8,
    /// Block size
    block_size: u32,
    /// Total blocks
    block_count: u64,
    /// Vendor string
    vendor: String,
    /// Product string
    product: String,
    /// Is removable
    removable: bool,
    /// Current tag for CBW/CSW
    tag: u32,
}

impl BlockDevice for UsbStorageDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn size(&self) -> u64 {
        self.block_count * self.block_size as u64
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn read_blocks(
        &self,
        start_block: u64,
        count: u32,
        buffer: &mut [u8],
    ) -> Result<(), StorageError> {
        let expected_size = count as usize * self.block_size as usize;
        if buffer.len() < expected_size {
            return Err(StorageError::BufferTooSmall);
        }

        if start_block + count as u64 > self.block_count {
            return Err(StorageError::InvalidAddress);
        }

        // In real implementation:
        // 1. Build SCSI READ(10) or READ(16) command
        // 2. Send CBW via bulk-out endpoint
        // 3. Receive data via bulk-in endpoint
        // 4. Receive CSW via bulk-in endpoint
        // 5. Check CSW status

        Err(StorageError::NotSupported)
    }

    fn write_blocks(
        &self,
        start_block: u64,
        count: u32,
        buffer: &[u8],
    ) -> Result<(), StorageError> {
        let expected_size = count as usize * self.block_size as usize;
        if buffer.len() < expected_size {
            return Err(StorageError::BufferTooSmall);
        }

        if start_block + count as u64 > self.block_count {
            return Err(StorageError::InvalidAddress);
        }

        Err(StorageError::NotSupported)
    }

    fn flush(&self) -> Result<(), StorageError> {
        // USB storage doesn't have an explicit flush command
        // Data is typically written synchronously
        Ok(())
    }
}

/// Build SCSI TEST UNIT READY command
pub fn scsi_test_unit_ready() -> [u8; 6] {
    [scsi_cmd::TEST_UNIT_READY, 0, 0, 0, 0, 0]
}

/// Build SCSI INQUIRY command
pub fn scsi_inquiry(allocation_length: u8) -> [u8; 6] {
    [scsi_cmd::INQUIRY, 0, 0, 0, allocation_length, 0]
}

/// Build SCSI READ CAPACITY (10) command
pub fn scsi_read_capacity_10() -> [u8; 10] {
    [scsi_cmd::READ_CAPACITY_10, 0, 0, 0, 0, 0, 0, 0, 0, 0]
}

/// Build SCSI READ (10) command
pub fn scsi_read_10(lba: u32, block_count: u16) -> [u8; 10] {
    [
        scsi_cmd::READ_10,
        0,
        (lba >> 24) as u8,
        (lba >> 16) as u8,
        (lba >> 8) as u8,
        lba as u8,
        0,
        (block_count >> 8) as u8,
        block_count as u8,
        0,
    ]
}

/// Build SCSI WRITE (10) command
pub fn scsi_write_10(lba: u32, block_count: u16) -> [u8; 10] {
    [
        scsi_cmd::WRITE_10,
        0,
        (lba >> 24) as u8,
        (lba >> 16) as u8,
        (lba >> 8) as u8,
        lba as u8,
        0,
        (block_count >> 8) as u8,
        block_count as u8,
        0,
    ]
}

/// Build SCSI READ (16) command for large drives
pub fn scsi_read_16(lba: u64, block_count: u32) -> [u8; 16] {
    [
        scsi_cmd::READ_16,
        0,
        (lba >> 56) as u8,
        (lba >> 48) as u8,
        (lba >> 40) as u8,
        (lba >> 32) as u8,
        (lba >> 24) as u8,
        (lba >> 16) as u8,
        (lba >> 8) as u8,
        lba as u8,
        (block_count >> 24) as u8,
        (block_count >> 16) as u8,
        (block_count >> 8) as u8,
        block_count as u8,
        0,
        0,
    ]
}

/// Probe for USB mass storage devices
pub fn probe(manager: &mut StorageManager) {
    // In real implementation:
    // 1. Enumerate USB devices
    // 2. Find devices with class 0x08 (Mass Storage)
    // 3. Check subclass (preferably 0x06 SCSI transparent)
    // 4. Check protocol (0x50 Bulk-Only or 0x62 UAS)
    // 5. Get max LUN via class request
    // 6. For each LUN:
    //    a. Send INQUIRY
    //    b. Send TEST UNIT READY
    //    c. Send READ CAPACITY
    //    d. Create UsbStorageDevice
    //    e. Register with storage manager
}
