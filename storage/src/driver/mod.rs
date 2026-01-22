//! Block device drivers.
//!
//! This module provides block device driver implementations including:
//! - VirtIO-Blk for virtual environments
//! - NVMe for modern SSDs
//! - AHCI/SATA for traditional drives

pub mod virtio;
pub mod nvme;
pub mod ahci;

use crate::{BlockDeviceInfo, StorageError};

/// Block device trait.
///
/// All block device drivers must implement this trait.
pub trait BlockDevice: Send + Sync {
    /// Get device information.
    fn info(&self) -> BlockDeviceInfo;

    /// Read blocks from the device.
    fn read_blocks(&self, start_block: u64, buffer: &mut [u8]) -> Result<usize, StorageError>;

    /// Write blocks to the device.
    fn write_blocks(&self, start_block: u64, data: &[u8]) -> Result<usize, StorageError>;

    /// Flush device buffers.
    fn flush(&self) -> Result<(), StorageError>;

    /// Discard/TRIM blocks.
    fn discard(&self, start_block: u64, num_blocks: u64) -> Result<(), StorageError>;

    /// Check if the device is ready.
    fn is_ready(&self) -> bool;
}

/// Maximum number of block devices.
const MAX_DEVICES: usize = 16;

/// Global block device registry.
static mut DEVICES: [Option<*const dyn BlockDevice>; MAX_DEVICES] = [None; MAX_DEVICES];
static mut DEVICE_COUNT: usize = 0;

/// Initialize block device drivers.
pub fn init() -> Result<(), StorageError> {
    // Probe for VirtIO-Blk devices
    virtio::probe()?;

    // Probe for NVMe devices
    nvme::probe()?;

    // Probe for AHCI devices
    ahci::probe()?;

    Ok(())
}

/// Register a block device.
pub fn register_device(device: &'static dyn BlockDevice) -> Result<usize, StorageError> {
    unsafe {
        if DEVICE_COUNT >= MAX_DEVICES {
            return Err(StorageError::NoSpace);
        }

        let index = DEVICE_COUNT;
        DEVICES[index] = Some(device as *const dyn BlockDevice);
        DEVICE_COUNT += 1;

        Ok(index)
    }
}

/// Get a block device by index.
pub fn get_device(index: usize) -> Option<&'static dyn BlockDevice> {
    unsafe {
        DEVICES.get(index)?.as_ref().map(|ptr| &**ptr)
    }
}

/// Get the number of registered devices.
pub fn device_count() -> usize {
    unsafe { DEVICE_COUNT }
}

/// Find a device by name.
pub fn find_device(name: &str) -> Option<usize> {
    unsafe {
        for i in 0..DEVICE_COUNT {
            if let Some(ptr) = DEVICES[i] {
                let device = &*ptr;
                if device.info().name_str() == name {
                    return Some(i);
                }
            }
        }
        None
    }
}
