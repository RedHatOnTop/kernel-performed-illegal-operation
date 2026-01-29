//! Hardware Detection and Management
//!
//! This module provides hardware discovery and management for KPIO OS.

pub mod detect;
pub mod acpi;
pub mod pci;
pub mod usb;

use alloc::vec::Vec;
use alloc::string::String;

/// Hardware device information
#[derive(Debug, Clone)]
pub struct HardwareDevice {
    /// Device type
    pub device_type: DeviceType,
    /// Vendor ID
    pub vendor_id: u16,
    /// Device ID
    pub device_id: u16,
    /// Device name
    pub name: String,
    /// Driver name if loaded
    pub driver: Option<String>,
    /// Device status
    pub status: DeviceStatus,
    /// Resource allocations
    pub resources: Vec<DeviceResource>,
}

/// Device types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// PCI device
    Pci,
    /// USB device
    Usb,
    /// ACPI device
    Acpi,
    /// Platform device
    Platform,
    /// Virtual device
    Virtual,
}

/// Device status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    /// Device detected but not initialized
    Detected,
    /// Device initialized and working
    Working,
    /// Device failed to initialize
    Failed,
    /// Device disabled
    Disabled,
    /// Device not present
    NotPresent,
}

/// Device resource
#[derive(Debug, Clone)]
pub enum DeviceResource {
    /// Memory mapped I/O region
    Memory {
        base: u64,
        size: u64,
    },
    /// I/O port region
    IoPort {
        base: u16,
        size: u16,
    },
    /// Interrupt line
    Irq {
        irq: u8,
        shared: bool,
    },
    /// DMA channel
    Dma {
        channel: u8,
    },
}

/// Hardware manager singleton
pub struct HardwareManager {
    /// Discovered devices
    devices: Vec<HardwareDevice>,
    /// ACPI tables loaded
    acpi_loaded: bool,
}

impl HardwareManager {
    /// Create a new hardware manager
    pub const fn new() -> Self {
        Self {
            devices: Vec::new(),
            acpi_loaded: false,
        }
    }

    /// Initialize hardware detection
    pub fn init(&mut self) {
        // Load ACPI tables
        if let Err(_) = acpi::init() {
            // ACPI not available, use fallback
        } else {
            self.acpi_loaded = true;
        }

        // Enumerate PCI devices
        pci::enumerate(&mut self.devices);

        // Enumerate USB devices
        usb::enumerate(&mut self.devices);
    }

    /// Get all detected devices
    pub fn devices(&self) -> &[HardwareDevice] {
        &self.devices
    }

    /// Find devices by type
    pub fn find_by_type(&self, device_type: DeviceType) -> Vec<&HardwareDevice> {
        self.devices.iter().filter(|d| d.device_type == device_type).collect()
    }

    /// Find device by vendor and device ID
    pub fn find_by_id(&self, vendor_id: u16, device_id: u16) -> Option<&HardwareDevice> {
        self.devices.iter().find(|d| d.vendor_id == vendor_id && d.device_id == device_id)
    }
}
