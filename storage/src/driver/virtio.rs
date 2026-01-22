//! VirtIO block device driver.
//!
//! This module provides support for VirtIO block devices commonly used
//! in virtual machines (QEMU, KVM, etc.).

use crate::{BlockDeviceInfo, StorageError};
use super::BlockDevice;

/// VirtIO block device feature flags.
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum VirtioBlkFeature {
    /// Device supports request barriers.
    Barrier = 1 << 0,
    /// Maximum size of any single segment is in size_max.
    SizeMax = 1 << 1,
    /// Maximum number of segments in a request is in seg_max.
    SegMax = 1 << 2,
    /// Disk-style geometry specified in geometry.
    Geometry = 1 << 4,
    /// Device is read-only.
    ReadOnly = 1 << 5,
    /// Block size of disk is in blk_size.
    BlkSize = 1 << 6,
    /// Device supports scsi packet commands.
    Scsi = 1 << 7,
    /// Cache flush command support.
    Flush = 1 << 9,
    /// Device exports information on optimal I/O alignment.
    Topology = 1 << 10,
    /// Device can toggle its cache between writeback and writethrough modes.
    ConfigWce = 1 << 11,
    /// Device supports discard command.
    Discard = 1 << 13,
    /// Device supports write zeroes command.
    WriteZeroes = 1 << 14,
}

/// VirtIO block request types.
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum VirtioBlkRequestType {
    /// Read request.
    Read = 0,
    /// Write request.
    Write = 1,
    /// Flush request.
    Flush = 4,
    /// Discard request.
    Discard = 11,
    /// Write zeroes request.
    WriteZeroes = 13,
}

/// VirtIO block request header.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct VirtioBlkReqHeader {
    /// Request type.
    pub type_: u32,
    /// Reserved.
    pub reserved: u32,
    /// Sector number.
    pub sector: u64,
}

impl VirtioBlkReqHeader {
    /// Header size in bytes.
    pub const SIZE: usize = 16;

    /// Create a new read request header.
    pub fn read(sector: u64) -> Self {
        VirtioBlkReqHeader {
            type_: VirtioBlkRequestType::Read as u32,
            reserved: 0,
            sector,
        }
    }

    /// Create a new write request header.
    pub fn write(sector: u64) -> Self {
        VirtioBlkReqHeader {
            type_: VirtioBlkRequestType::Write as u32,
            reserved: 0,
            sector,
        }
    }

    /// Create a new flush request header.
    pub fn flush() -> Self {
        VirtioBlkReqHeader {
            type_: VirtioBlkRequestType::Flush as u32,
            reserved: 0,
            sector: 0,
        }
    }
}

/// VirtIO block device configuration.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct VirtioBlkConfig {
    /// Capacity in 512-byte sectors.
    pub capacity: u64,
    /// Maximum segment size.
    pub size_max: u32,
    /// Maximum number of segments.
    pub seg_max: u32,
    /// Geometry - cylinders.
    pub cylinders: u16,
    /// Geometry - heads.
    pub heads: u8,
    /// Geometry - sectors.
    pub sectors: u8,
    /// Block size.
    pub blk_size: u32,
    /// Topology - physical block exponent.
    pub physical_block_exp: u8,
    /// Topology - alignment offset.
    pub alignment_offset: u8,
    /// Topology - minimum I/O size.
    pub min_io_size: u16,
    /// Topology - optimal I/O size.
    pub opt_io_size: u32,
    /// Writeback mode.
    pub writeback: u8,
    /// Reserved.
    pub reserved0: u8,
    /// Number of queues.
    pub num_queues: u16,
    /// Maximum discard sectors.
    pub max_discard_sectors: u32,
    /// Maximum discard segment count.
    pub max_discard_seg: u32,
    /// Discard sector alignment.
    pub discard_sector_alignment: u32,
    /// Maximum write zeroes sectors.
    pub max_write_zeroes_sectors: u32,
    /// Maximum write zeroes segment count.
    pub max_write_zeroes_seg: u32,
    /// Write zeroes may unmap.
    pub write_zeroes_may_unmap: u8,
    /// Reserved.
    pub reserved1: [u8; 3],
}

/// VirtIO block device status codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VirtioBlkStatus {
    /// Success.
    Ok = 0,
    /// I/O error.
    IoError = 1,
    /// Unsupported request.
    Unsupported = 2,
}

/// VirtIO block device.
pub struct VirtioBlkDevice {
    /// Base address of the device.
    base_addr: usize,
    /// Device configuration.
    config: VirtioBlkConfig,
    /// Negotiated features.
    features: u64,
    /// Block size in bytes.
    block_size: u32,
    /// Is the device read-only.
    read_only: bool,
    /// Device is initialized.
    initialized: bool,
}

impl VirtioBlkDevice {
    /// VirtIO block device ID.
    pub const DEVICE_ID: u32 = 2;

    /// Create a new VirtIO block device.
    pub fn new(base_addr: usize) -> Self {
        VirtioBlkDevice {
            base_addr,
            config: VirtioBlkConfig {
                capacity: 0,
                size_max: 0,
                seg_max: 0,
                cylinders: 0,
                heads: 0,
                sectors: 0,
                blk_size: 512,
                physical_block_exp: 0,
                alignment_offset: 0,
                min_io_size: 0,
                opt_io_size: 0,
                writeback: 0,
                reserved0: 0,
                num_queues: 0,
                max_discard_sectors: 0,
                max_discard_seg: 0,
                discard_sector_alignment: 0,
                max_write_zeroes_sectors: 0,
                max_write_zeroes_seg: 0,
                write_zeroes_may_unmap: 0,
                reserved1: [0; 3],
            },
            features: 0,
            block_size: 512,
            read_only: false,
            initialized: false,
        }
    }

    /// Initialize the device.
    pub fn init(&mut self) -> Result<(), StorageError> {
        // TODO: Implement VirtIO device initialization
        // 1. Reset device
        // 2. Set ACKNOWLEDGE status bit
        // 3. Set DRIVER status bit
        // 4. Read feature bits
        // 5. Negotiate features
        // 6. Set FEATURES_OK status bit
        // 7. Re-read status to verify FEATURES_OK
        // 8. Perform device-specific setup
        // 9. Set DRIVER_OK status bit

        self.initialized = true;
        Ok(())
    }

    /// Read the device configuration.
    fn read_config(&self) -> VirtioBlkConfig {
        // TODO: Read from MMIO config space
        self.config.clone()
    }

    /// Negotiate features with the device.
    fn negotiate_features(&mut self, offered: u64) -> u64 {
        // Accept basic features
        let mut features = 0u64;

        if offered & (VirtioBlkFeature::BlkSize as u64) != 0 {
            features |= VirtioBlkFeature::BlkSize as u64;
        }

        if offered & (VirtioBlkFeature::Flush as u64) != 0 {
            features |= VirtioBlkFeature::Flush as u64;
        }

        if offered & (VirtioBlkFeature::Discard as u64) != 0 {
            features |= VirtioBlkFeature::Discard as u64;
        }

        if offered & (VirtioBlkFeature::ReadOnly as u64) != 0 {
            self.read_only = true;
        }

        self.features = features;
        features
    }

    /// Submit a read request.
    fn submit_read(&self, sector: u64, buffer: &mut [u8]) -> Result<(), StorageError> {
        if !self.initialized {
            return Err(StorageError::NotReady);
        }

        let _header = VirtioBlkReqHeader::read(sector);

        // TODO: Submit request to virtqueue
        // 1. Allocate descriptor chain
        // 2. Set up header descriptor (device readable)
        // 3. Set up data descriptor (device writable)
        // 4. Set up status descriptor (device writable)
        // 5. Add to available ring
        // 6. Notify device
        // 7. Wait for completion
        // 8. Check status byte

        let _ = buffer;
        Ok(())
    }

    /// Submit a write request.
    fn submit_write(&self, sector: u64, data: &[u8]) -> Result<(), StorageError> {
        if !self.initialized {
            return Err(StorageError::NotReady);
        }

        if self.read_only {
            return Err(StorageError::ReadOnly);
        }

        let _header = VirtioBlkReqHeader::write(sector);

        // TODO: Submit request to virtqueue
        let _ = data;
        Ok(())
    }

    /// Submit a flush request.
    fn submit_flush(&self) -> Result<(), StorageError> {
        if !self.initialized {
            return Err(StorageError::NotReady);
        }

        if self.features & (VirtioBlkFeature::Flush as u64) == 0 {
            return Ok(()); // Flush not supported, treat as success
        }

        let _header = VirtioBlkReqHeader::flush();

        // TODO: Submit request to virtqueue
        Ok(())
    }
}

impl BlockDevice for VirtioBlkDevice {
    fn info(&self) -> BlockDeviceInfo {
        let mut name = [0u8; 32];
        name[..6].copy_from_slice(b"virtio");

        BlockDeviceInfo {
            name,
            name_len: 6,
            block_size: self.block_size,
            total_blocks: self.config.capacity,
            read_only: self.read_only,
            supports_trim: self.features & (VirtioBlkFeature::Discard as u64) != 0,
            optimal_io_size: self.config.opt_io_size,
            physical_block_size: 512 << self.config.physical_block_exp,
        }
    }

    fn read_blocks(&self, start_block: u64, buffer: &mut [u8]) -> Result<usize, StorageError> {
        let blocks = buffer.len() as u64 / self.block_size as u64;

        if start_block + blocks > self.config.capacity {
            return Err(StorageError::InvalidBlock);
        }

        for i in 0..blocks {
            let offset = (i * self.block_size as u64) as usize;
            let sector = start_block + i;
            self.submit_read(sector, &mut buffer[offset..offset + self.block_size as usize])?;
        }

        Ok(buffer.len())
    }

    fn write_blocks(&self, start_block: u64, data: &[u8]) -> Result<usize, StorageError> {
        let blocks = data.len() as u64 / self.block_size as u64;

        if start_block + blocks > self.config.capacity {
            return Err(StorageError::InvalidBlock);
        }

        for i in 0..blocks {
            let offset = (i * self.block_size as u64) as usize;
            let sector = start_block + i;
            self.submit_write(sector, &data[offset..offset + self.block_size as usize])?;
        }

        Ok(data.len())
    }

    fn flush(&self) -> Result<(), StorageError> {
        self.submit_flush()
    }

    fn discard(&self, start_block: u64, num_blocks: u64) -> Result<(), StorageError> {
        if self.features & (VirtioBlkFeature::Discard as u64) == 0 {
            return Err(StorageError::Unsupported);
        }

        if start_block + num_blocks > self.config.capacity {
            return Err(StorageError::InvalidBlock);
        }

        // TODO: Submit discard request
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.initialized
    }
}

/// Probe for VirtIO block devices.
pub fn probe() -> Result<(), StorageError> {
    // TODO: Scan for VirtIO devices on the system
    // For x86_64, check PCI configuration space for VirtIO vendor ID (0x1AF4)
    // and device ID for block device (0x1001 or 0x1042 for modern)

    Ok(())
}
