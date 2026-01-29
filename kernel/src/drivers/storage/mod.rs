//! Storage Drivers
//!
//! Block device drivers for various storage interfaces.

pub mod nvme;
pub mod ahci;
pub mod usb_storage;
pub mod partition;

use alloc::vec::Vec;
use alloc::string::String;
use alloc::boxed::Box;

/// Block device trait for storage abstraction
pub trait BlockDevice: Send + Sync {
    /// Get device name
    fn name(&self) -> &str;
    
    /// Get total size in bytes
    fn size(&self) -> u64;
    
    /// Get block size in bytes
    fn block_size(&self) -> u32;
    
    /// Get number of blocks
    fn block_count(&self) -> u64 {
        self.size() / self.block_size() as u64
    }
    
    /// Read blocks into buffer
    fn read_blocks(&self, start_block: u64, count: u32, buffer: &mut [u8]) -> Result<(), StorageError>;
    
    /// Write blocks from buffer
    fn write_blocks(&self, start_block: u64, count: u32, buffer: &[u8]) -> Result<(), StorageError>;
    
    /// Flush any cached writes
    fn flush(&self) -> Result<(), StorageError>;
    
    /// Check if device is read-only
    fn is_read_only(&self) -> bool {
        false
    }
    
    /// Check if device supports TRIM/discard
    fn supports_trim(&self) -> bool {
        false
    }
    
    /// Perform TRIM/discard operation
    fn trim(&self, _start_block: u64, _count: u64) -> Result<(), StorageError> {
        Err(StorageError::NotSupported)
    }
}

/// Storage error types
#[derive(Debug, Clone)]
pub enum StorageError {
    /// Device not found
    DeviceNotFound,
    /// I/O error during read/write
    IoError(String),
    /// Invalid block address
    InvalidAddress,
    /// Buffer too small
    BufferTooSmall,
    /// Operation not supported
    NotSupported,
    /// Device busy
    DeviceBusy,
    /// Timeout
    Timeout,
    /// Hardware failure
    HardwareFailure,
    /// Write protected
    WriteProtected,
    /// Medium not present (removable media)
    MediumNotPresent,
}

/// Storage device information
#[derive(Debug, Clone)]
pub struct StorageInfo {
    /// Device model
    pub model: String,
    /// Serial number
    pub serial: String,
    /// Firmware version
    pub firmware: String,
    /// Interface type
    pub interface: StorageInterface,
    /// Capacity in bytes
    pub capacity: u64,
    /// Block size
    pub block_size: u32,
    /// Is SSD
    pub is_ssd: bool,
    /// Supports NCQ
    pub ncq_support: bool,
    /// Maximum queue depth
    pub max_queue_depth: u8,
}

/// Storage interface types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageInterface {
    /// NVMe SSD
    NVMe,
    /// SATA (AHCI)
    Sata,
    /// PATA/IDE (legacy)
    Ide,
    /// USB Mass Storage
    Usb,
    /// SD/MMC card
    SdMmc,
    /// Virtual/RAM disk
    Virtual,
}

/// Storage subsystem manager
pub struct StorageManager {
    /// Registered block devices
    devices: Vec<Box<dyn BlockDevice>>,
    /// Device info cache
    device_info: Vec<StorageInfo>,
}

impl StorageManager {
    /// Create a new storage manager
    pub const fn new() -> Self {
        Self {
            devices: Vec::new(),
            device_info: Vec::new(),
        }
    }

    /// Initialize storage subsystem
    pub fn init(&mut self) {
        // Probe for NVMe controllers
        nvme::probe(self);
        
        // Probe for AHCI/SATA controllers
        ahci::probe(self);
        
        // Probe for USB mass storage
        usb_storage::probe(self);
    }

    /// Register a block device
    pub fn register(&mut self, device: Box<dyn BlockDevice>, info: StorageInfo) {
        self.devices.push(device);
        self.device_info.push(info);
    }

    /// Get device count
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Get device by index
    pub fn get_device(&self, index: usize) -> Option<&dyn BlockDevice> {
        self.devices.get(index).map(|d| d.as_ref())
    }

    /// Get device info by index
    pub fn get_info(&self, index: usize) -> Option<&StorageInfo> {
        self.device_info.get(index)
    }

    /// Find device by name
    pub fn find_by_name(&self, name: &str) -> Option<&dyn BlockDevice> {
        self.devices.iter().find(|d| d.name() == name).map(|d| d.as_ref())
    }
}

/// Global storage manager
static mut STORAGE_MANAGER: Option<StorageManager> = None;

/// Initialize global storage manager
pub fn init() {
    unsafe {
        let mut manager = StorageManager::new();
        manager.init();
        STORAGE_MANAGER = Some(manager);
    }
}

/// Get storage manager reference
pub fn manager() -> Option<&'static StorageManager> {
    unsafe { STORAGE_MANAGER.as_ref() }
}
