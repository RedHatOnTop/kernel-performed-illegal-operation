//! PCI (Peripheral Component Interconnect) bus driver.
//!
//! This module provides PCI bus enumeration and configuration space access.
//! 
//! # PCI Configuration Space
//! 
//! PCI devices expose a 256-byte configuration space accessible via:
//! - Legacy I/O ports (0xCF8/0xCFC) for Configuration Mechanism #1
//! - Memory-mapped configuration (PCIe ECAM) for extended config space
//!
//! # Bus Topology
//!
//! PCI uses a hierarchical addressing scheme:
//! - Bus (0-255): Up to 256 buses
//! - Device (0-31): Up to 32 devices per bus
//! - Function (0-7): Up to 8 functions per device
//!
//! # References
//!
//! - PCI Local Bus Specification 3.0
//! - PCI Express Base Specification

use alloc::vec::Vec;
use core::fmt;
use spin::Mutex;
use x86_64::instructions::port::Port;

/// PCI configuration space I/O ports (Configuration Mechanism #1).
const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

/// Global list of discovered PCI devices.
static PCI_DEVICES: Mutex<Vec<PciDevice>> = Mutex::new(Vec::new());

/// PCI device address (Bus:Device:Function).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PciAddress {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

impl PciAddress {
    /// Create a new PCI address.
    pub fn new(bus: u8, device: u8, function: u8) -> Self {
        debug_assert!(device < 32, "Device must be 0-31");
        debug_assert!(function < 8, "Function must be 0-7");
        Self { bus, device, function }
    }
    
    /// Convert to configuration address for I/O port access.
    fn to_config_address(&self, offset: u8) -> u32 {
        debug_assert!(offset & 0x3 == 0, "Offset must be 4-byte aligned");
        
        (1 << 31) // Enable bit
            | ((self.bus as u32) << 16)
            | ((self.device as u32) << 11)
            | ((self.function as u32) << 8)
            | (offset as u32)
    }
}

impl fmt::Display for PciAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02x}:{:02x}.{}", self.bus, self.device, self.function)
    }
}

/// PCI device header types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HeaderType {
    /// Standard device (Type 0).
    Standard = 0x00,
    /// PCI-to-PCI bridge (Type 1).
    PciBridge = 0x01,
    /// CardBus bridge (Type 2).
    CardBusBridge = 0x02,
    /// Unknown header type.
    Unknown = 0xFF,
}

impl From<u8> for HeaderType {
    fn from(value: u8) -> Self {
        match value & 0x7F {
            0x00 => HeaderType::Standard,
            0x01 => HeaderType::PciBridge,
            0x02 => HeaderType::CardBusBridge,
            _ => HeaderType::Unknown,
        }
    }
}

/// PCI device class codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PciClass {
    pub class: u8,
    pub subclass: u8,
    pub prog_if: u8,
}

impl PciClass {
    /// Mass storage controller.
    pub const MASS_STORAGE: u8 = 0x01;
    /// Network controller.
    pub const NETWORK: u8 = 0x02;
    /// Display controller.
    pub const DISPLAY: u8 = 0x03;
    /// Bridge device.
    pub const BRIDGE: u8 = 0x06;
    
    /// Check if this is a storage device.
    pub fn is_storage(&self) -> bool {
        self.class == Self::MASS_STORAGE
    }
    
    /// Check if this is a network device.
    pub fn is_network(&self) -> bool {
        self.class == Self::NETWORK
    }
}

impl fmt::Display for PciClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02x}:{:02x}:{:02x}", self.class, self.subclass, self.prog_if)
    }
}

/// Discovered PCI device information.
#[derive(Debug, Clone)]
pub struct PciDevice {
    /// Bus:Device:Function address.
    pub address: PciAddress,
    /// Vendor ID.
    pub vendor_id: u16,
    /// Device ID.
    pub device_id: u16,
    /// Device class.
    pub class: PciClass,
    /// Header type.
    pub header_type: HeaderType,
    /// Subsystem vendor ID (if available).
    pub subsystem_vendor_id: u16,
    /// Subsystem ID (if available).
    pub subsystem_id: u16,
    /// Interrupt line.
    pub interrupt_line: u8,
    /// Interrupt pin.
    pub interrupt_pin: u8,
    /// Base Address Registers (BARs).
    pub bars: [u32; 6],
}

impl PciDevice {
    /// Check if this is a VirtIO device.
    /// VirtIO devices use vendor ID 0x1AF4.
    pub fn is_virtio(&self) -> bool {
        self.vendor_id == 0x1AF4
    }
    
    /// Get VirtIO device type if this is a VirtIO device.
    /// Device ID 0x1000-0x103F are transitional VirtIO devices.
    /// Device ID 0x1040+ are modern VirtIO devices.
    pub fn virtio_device_type(&self) -> Option<VirtioDeviceType> {
        if !self.is_virtio() {
            return None;
        }
        
        match self.device_id {
            0x1001 | 0x1042 => Some(VirtioDeviceType::Block),
            0x1000 | 0x1041 => Some(VirtioDeviceType::Network),
            0x1003 | 0x1043 => Some(VirtioDeviceType::Console),
            0x1005 | 0x1044 => Some(VirtioDeviceType::Entropy),
            0x1009 | 0x1049 => Some(VirtioDeviceType::Filesystem),
            _ => Some(VirtioDeviceType::Unknown(self.device_id)),
        }
    }
}

impl fmt::Display for PciDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {:04x}:{:04x} class={} {}",
            self.address,
            self.vendor_id,
            self.device_id,
            self.class,
            if self.is_virtio() { "(VirtIO)" } else { "" }
        )
    }
}

/// VirtIO device types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtioDeviceType {
    Network,
    Block,
    Console,
    Entropy,
    Filesystem,
    Unknown(u16),
}

impl fmt::Display for VirtioDeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VirtioDeviceType::Network => write!(f, "Network"),
            VirtioDeviceType::Block => write!(f, "Block"),
            VirtioDeviceType::Console => write!(f, "Console"),
            VirtioDeviceType::Entropy => write!(f, "Entropy"),
            VirtioDeviceType::Filesystem => write!(f, "Filesystem"),
            VirtioDeviceType::Unknown(id) => write!(f, "Unknown({:#x})", id),
        }
    }
}

/// Read a 32-bit value from PCI configuration space.
pub fn config_read32(addr: PciAddress, offset: u8) -> u32 {
    let config_addr = addr.to_config_address(offset);
    
    unsafe {
        let mut addr_port: Port<u32> = Port::new(CONFIG_ADDRESS);
        let mut data_port: Port<u32> = Port::new(CONFIG_DATA);
        
        addr_port.write(config_addr);
        data_port.read()
    }
}

/// Read a 16-bit value from PCI configuration space.
pub fn config_read16(addr: PciAddress, offset: u8) -> u16 {
    let value = config_read32(addr, offset & !0x3);
    ((value >> ((offset & 0x2) * 8)) & 0xFFFF) as u16
}

/// Read an 8-bit value from PCI configuration space.
pub fn config_read8(addr: PciAddress, offset: u8) -> u8 {
    let value = config_read32(addr, offset & !0x3);
    ((value >> ((offset & 0x3) * 8)) & 0xFF) as u8
}

/// Write a 32-bit value to PCI configuration space.
pub fn config_write32(addr: PciAddress, offset: u8, value: u32) {
    let config_addr = addr.to_config_address(offset);
    
    unsafe {
        let mut addr_port: Port<u32> = Port::new(CONFIG_ADDRESS);
        let mut data_port: Port<u32> = Port::new(CONFIG_DATA);
        
        addr_port.write(config_addr);
        data_port.write(value);
    }
}

/// Write a 16-bit value to PCI configuration space.
pub fn config_write16(addr: PciAddress, offset: u8, value: u16) {
    let aligned_offset = offset & !0x3;
    let shift = (offset & 0x2) * 8;
    
    let mut current = config_read32(addr, aligned_offset);
    current &= !(0xFFFF << shift);
    current |= (value as u32) << shift;
    
    config_write32(addr, aligned_offset, current);
}

/// Check if a device exists at the given address.
fn device_exists(addr: PciAddress) -> bool {
    let vendor_id = config_read16(addr, 0x00);
    vendor_id != 0xFFFF
}

/// Check if a device is multi-function.
fn is_multifunction(addr: PciAddress) -> bool {
    let header_type = config_read8(addr, 0x0E);
    (header_type & 0x80) != 0
}

/// Read device information from configuration space.
fn read_device(addr: PciAddress) -> Option<PciDevice> {
    let vendor_id = config_read16(addr, 0x00);
    if vendor_id == 0xFFFF {
        return None;
    }
    
    let device_id = config_read16(addr, 0x02);
    let class_reg = config_read32(addr, 0x08);
    let header_type_raw = config_read8(addr, 0x0E);
    
    let class = PciClass {
        class: ((class_reg >> 24) & 0xFF) as u8,
        subclass: ((class_reg >> 16) & 0xFF) as u8,
        prog_if: ((class_reg >> 8) & 0xFF) as u8,
    };
    
    let header_type = HeaderType::from(header_type_raw);
    
    // Read BARs (only for Type 0 headers)
    let mut bars = [0u32; 6];
    if header_type == HeaderType::Standard {
        for i in 0..6 {
            bars[i] = config_read32(addr, 0x10 + (i as u8 * 4));
        }
    }
    
    // Read subsystem info
    let (subsystem_vendor_id, subsystem_id) = if header_type == HeaderType::Standard {
        (
            config_read16(addr, 0x2C),
            config_read16(addr, 0x2E),
        )
    } else {
        (0, 0)
    };
    
    // Read interrupt info
    let interrupt_line = config_read8(addr, 0x3C);
    let interrupt_pin = config_read8(addr, 0x3D);
    
    Some(PciDevice {
        address: addr,
        vendor_id,
        device_id,
        class,
        header_type,
        subsystem_vendor_id,
        subsystem_id,
        interrupt_line,
        interrupt_pin,
        bars,
    })
}

/// Scan a single bus for devices.
fn scan_bus(bus: u8, devices: &mut Vec<PciDevice>) {
    for device in 0..32 {
        let addr = PciAddress::new(bus, device, 0);
        
        if !device_exists(addr) {
            continue;
        }
        
        // Check function 0
        if let Some(dev) = read_device(addr) {
            let multifunction = is_multifunction(addr);
            
            // Check for PCI-to-PCI bridge
            if dev.header_type == HeaderType::PciBridge {
                let secondary_bus = config_read8(addr, 0x19);
                scan_bus(secondary_bus, devices);
            }
            
            devices.push(dev);
            
            // Scan other functions if multifunction device
            if multifunction {
                for function in 1..8 {
                    let func_addr = PciAddress::new(bus, device, function);
                    if let Some(func_dev) = read_device(func_addr) {
                        devices.push(func_dev);
                    }
                }
            }
        }
    }
}

/// Enumerate all PCI devices in the system.
pub fn enumerate() {
    let mut devices = PCI_DEVICES.lock();
    devices.clear();
    
    crate::serial_println!("[PCI] Enumerating PCI bus...");
    
    // Check if multiple PCI host controllers exist
    let host_addr = PciAddress::new(0, 0, 0);
    if is_multifunction(host_addr) {
        // Multiple host controllers
        for function in 0..8 {
            let addr = PciAddress::new(0, 0, function);
            if device_exists(addr) {
                scan_bus(function, &mut devices);
            }
        }
    } else {
        // Single host controller
        scan_bus(0, &mut devices);
    }
    
    crate::serial_println!("[PCI] Found {} devices:", devices.len());
    for dev in devices.iter() {
        crate::serial_println!("  {}", dev);
    }
}

/// Get a list of all discovered PCI devices.
pub fn devices() -> Vec<PciDevice> {
    PCI_DEVICES.lock().clone()
}

/// Find VirtIO block devices.
pub fn find_virtio_block() -> Vec<PciDevice> {
    PCI_DEVICES
        .lock()
        .iter()
        .filter(|dev| dev.virtio_device_type() == Some(VirtioDeviceType::Block))
        .cloned()
        .collect()
}

/// Find VirtIO network devices.
pub fn find_virtio_network() -> Vec<PciDevice> {
    PCI_DEVICES
        .lock()
        .iter()
        .filter(|dev| dev.virtio_device_type() == Some(VirtioDeviceType::Network))
        .cloned()
        .collect()
}

/// Enable bus mastering for a device.
pub fn enable_bus_master(addr: PciAddress) {
    let command = config_read16(addr, 0x04);
    config_write16(addr, 0x04, command | 0x04);
}

/// Enable memory space access for a device.
pub fn enable_memory_space(addr: PciAddress) {
    let command = config_read16(addr, 0x04);
    config_write16(addr, 0x04, command | 0x02);
}

/// Enable I/O space access for a device.
pub fn enable_io_space(addr: PciAddress) {
    let command = config_read16(addr, 0x04);
    config_write16(addr, 0x04, command | 0x01);
}
