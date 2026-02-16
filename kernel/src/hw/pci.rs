//! Enhanced PCI Support
//!
//! PCI device enumeration and configuration space access.

use super::{DeviceResource, DeviceStatus, DeviceType, HardwareDevice};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// PCI configuration space address
const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
/// PCI configuration space data
const PCI_CONFIG_DATA: u16 = 0xCFC;

/// PCI device identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PciAddress {
    /// Bus number (0-255)
    pub bus: u8,
    /// Device number (0-31)
    pub device: u8,
    /// Function number (0-7)
    pub function: u8,
}

impl PciAddress {
    /// Create a new PCI address
    pub const fn new(bus: u8, device: u8, function: u8) -> Self {
        Self {
            bus,
            device,
            function,
        }
    }

    /// Create configuration space address
    pub fn config_address(&self, offset: u8) -> u32 {
        0x8000_0000
            | ((self.bus as u32) << 16)
            | ((self.device as u32) << 11)
            | ((self.function as u32) << 8)
            | ((offset as u32) & 0xFC)
    }
}

/// PCI device class codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PciClass {
    Unclassified = 0x00,
    MassStorage = 0x01,
    Network = 0x02,
    Display = 0x03,
    Multimedia = 0x04,
    Memory = 0x05,
    Bridge = 0x06,
    SimpleCommunication = 0x07,
    BaseSystemPeripheral = 0x08,
    InputDevice = 0x09,
    DockingStation = 0x0A,
    Processor = 0x0B,
    SerialBus = 0x0C,
    Wireless = 0x0D,
    IntelligentController = 0x0E,
    SatelliteCommunication = 0x0F,
    Encryption = 0x10,
    SignalProcessing = 0x11,
    ProcessingAccelerator = 0x12,
    NonEssentialInstrumentation = 0x13,
    Coprocessor = 0x40,
    Unknown = 0xFF,
}

/// PCI subclass for storage controllers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageSubclass {
    ScsiController,
    IdeController,
    FloppyController,
    IpiBusController,
    RaidController,
    AtaController,
    SataController,
    SasController,
    NvmeController,
    Other,
}

/// PCI subclass for network controllers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkSubclass {
    EthernetController,
    TokenRingController,
    FddiController,
    AtmController,
    IsdnController,
    WorldFipController,
    PicmgController,
    InfinibandController,
    FabricController,
    Other,
}

/// PCI device information
#[derive(Debug, Clone)]
pub struct PciDevice {
    /// PCI address
    pub address: PciAddress,
    /// Vendor ID
    pub vendor_id: u16,
    /// Device ID
    pub device_id: u16,
    /// Device class
    pub class: u8,
    /// Device subclass
    pub subclass: u8,
    /// Programming interface
    pub prog_if: u8,
    /// Revision ID
    pub revision: u8,
    /// Header type
    pub header_type: u8,
    /// Subsystem vendor ID
    pub subsystem_vendor: u16,
    /// Subsystem ID
    pub subsystem_id: u16,
    /// Interrupt line
    pub interrupt_line: u8,
    /// Interrupt pin
    pub interrupt_pin: u8,
    /// Base address registers
    pub bars: [Bar; 6],
    /// Capabilities
    pub capabilities: Vec<PciCapability>,
}

/// Base Address Register
#[derive(Debug, Clone, Copy)]
pub struct Bar {
    /// Raw value
    pub raw: u32,
    /// Is memory mapped (vs I/O port)
    pub is_memory: bool,
    /// Base address
    pub base: u64,
    /// Size in bytes
    pub size: u64,
    /// Is 64-bit (for memory BARs)
    pub is_64bit: bool,
    /// Is prefetchable (for memory BARs)
    pub prefetchable: bool,
}

impl Bar {
    /// Empty/invalid BAR
    pub const fn empty() -> Self {
        Self {
            raw: 0,
            is_memory: false,
            base: 0,
            size: 0,
            is_64bit: false,
            prefetchable: false,
        }
    }

    /// Check if BAR is present
    pub fn is_present(&self) -> bool {
        self.raw != 0
    }
}

/// PCI capability
#[derive(Debug, Clone)]
pub struct PciCapability {
    /// Capability ID
    pub id: u8,
    /// Offset in configuration space
    pub offset: u8,
    /// Capability-specific data
    pub data: Vec<u8>,
}

/// Common PCI capability IDs
pub mod capability_ids {
    pub const PM: u8 = 0x01; // Power Management
    pub const AGP: u8 = 0x02; // AGP
    pub const VPD: u8 = 0x03; // Vital Product Data
    pub const SLOT_ID: u8 = 0x04; // Slot Identification
    pub const MSI: u8 = 0x05; // Message Signaled Interrupts
    pub const PCIX: u8 = 0x07; // PCI-X
    pub const VENDOR: u8 = 0x09; // Vendor Specific
    pub const PCIE: u8 = 0x10; // PCI Express
    pub const MSIX: u8 = 0x11; // MSI-X
}

/// Read 32-bit value from PCI configuration space
pub fn read_config_u32(addr: PciAddress, offset: u8) -> u32 {
    let config_addr = addr.config_address(offset);
    unsafe {
        // Write address to CONFIG_ADDRESS
        core::arch::asm!(
            "out dx, eax",
            in("dx") PCI_CONFIG_ADDRESS,
            in("eax") config_addr,
        );
        // Read data from CONFIG_DATA
        let value: u32;
        core::arch::asm!(
            "in eax, dx",
            in("dx") PCI_CONFIG_DATA,
            out("eax") value,
        );
        value
    }
}

/// Write 32-bit value to PCI configuration space
pub fn write_config_u32(addr: PciAddress, offset: u8, value: u32) {
    let config_addr = addr.config_address(offset);
    unsafe {
        // Write address to CONFIG_ADDRESS
        core::arch::asm!(
            "out dx, eax",
            in("dx") PCI_CONFIG_ADDRESS,
            in("eax") config_addr,
        );
        // Write data to CONFIG_DATA
        core::arch::asm!(
            "out dx, eax",
            in("dx") PCI_CONFIG_DATA,
            in("eax") value,
        );
    }
}

/// Read 16-bit value from PCI configuration space
pub fn read_config_u16(addr: PciAddress, offset: u8) -> u16 {
    let value = read_config_u32(addr, offset & 0xFC);
    ((value >> ((offset & 2) * 8)) & 0xFFFF) as u16
}

/// Read 8-bit value from PCI configuration space
pub fn read_config_u8(addr: PciAddress, offset: u8) -> u8 {
    let value = read_config_u32(addr, offset & 0xFC);
    ((value >> ((offset & 3) * 8)) & 0xFF) as u8
}

/// Check if device exists at address
pub fn device_exists(addr: PciAddress) -> bool {
    read_config_u16(addr, 0) != 0xFFFF
}

/// Scan a single PCI function
pub fn scan_function(addr: PciAddress) -> Option<PciDevice> {
    if !device_exists(addr) {
        return None;
    }

    let vendor_device = read_config_u32(addr, 0x00);
    let vendor_id = (vendor_device & 0xFFFF) as u16;
    let device_id = ((vendor_device >> 16) & 0xFFFF) as u16;

    let class_rev = read_config_u32(addr, 0x08);
    let revision = (class_rev & 0xFF) as u8;
    let prog_if = ((class_rev >> 8) & 0xFF) as u8;
    let subclass = ((class_rev >> 16) & 0xFF) as u8;
    let class = ((class_rev >> 24) & 0xFF) as u8;

    let header_type = read_config_u8(addr, 0x0E);

    let subsys = read_config_u32(addr, 0x2C);
    let subsystem_vendor = (subsys & 0xFFFF) as u16;
    let subsystem_id = ((subsys >> 16) & 0xFFFF) as u16;

    let int_info = read_config_u32(addr, 0x3C);
    let interrupt_line = (int_info & 0xFF) as u8;
    let interrupt_pin = ((int_info >> 8) & 0xFF) as u8;

    // Read BARs (only for type 0 headers)
    let mut bars = [Bar::empty(); 6];
    if (header_type & 0x7F) == 0 {
        for i in 0..6 {
            let bar_offset = 0x10 + (i * 4) as u8;
            let bar_value = read_config_u32(addr, bar_offset);

            if bar_value == 0 {
                continue;
            }

            let is_memory = (bar_value & 1) == 0;

            if is_memory {
                let is_64bit = ((bar_value >> 1) & 3) == 2;
                let prefetchable = ((bar_value >> 3) & 1) != 0;

                // Determine size by writing all 1s and reading back
                write_config_u32(addr, bar_offset, 0xFFFFFFFF);
                let size_mask = read_config_u32(addr, bar_offset);
                write_config_u32(addr, bar_offset, bar_value);

                let size = if size_mask != 0 {
                    let size = !(size_mask & 0xFFFFFFF0) + 1;
                    size as u64
                } else {
                    0
                };

                bars[i] = Bar {
                    raw: bar_value,
                    is_memory: true,
                    base: (bar_value & 0xFFFFFFF0) as u64,
                    size,
                    is_64bit,
                    prefetchable,
                };

                if is_64bit && i < 5 {
                    let high_value = read_config_u32(addr, bar_offset + 4);
                    bars[i].base |= (high_value as u64) << 32;
                }
            } else {
                // I/O BAR
                bars[i] = Bar {
                    raw: bar_value,
                    is_memory: false,
                    base: (bar_value & 0xFFFFFFFC) as u64,
                    size: 256, // Typical I/O port range
                    is_64bit: false,
                    prefetchable: false,
                };
            }
        }
    }

    // Parse capabilities
    let mut capabilities = Vec::new();
    let status = read_config_u16(addr, 0x06);
    if (status & 0x10) != 0 {
        // Capabilities list present
        let mut cap_offset = read_config_u8(addr, 0x34);
        while cap_offset != 0 && cap_offset != 0xFF {
            let cap_header = read_config_u32(addr, cap_offset);
            let cap_id = (cap_header & 0xFF) as u8;
            let next_offset = ((cap_header >> 8) & 0xFF) as u8;

            capabilities.push(PciCapability {
                id: cap_id,
                offset: cap_offset,
                data: Vec::new(), // Could read capability-specific data
            });

            cap_offset = next_offset;
        }
    }

    Some(PciDevice {
        address: addr,
        vendor_id,
        device_id,
        class,
        subclass,
        prog_if,
        revision,
        header_type,
        subsystem_vendor,
        subsystem_id,
        interrupt_line,
        interrupt_pin,
        bars,
        capabilities,
    })
}

/// Enumerate all PCI devices
pub fn enumerate(devices: &mut Vec<HardwareDevice>) {
    for bus in 0..=255u8 {
        for device in 0..32u8 {
            let addr = PciAddress::new(bus, device, 0);

            if !device_exists(addr) {
                continue;
            }

            // Check if multi-function device
            let header_type = read_config_u8(addr, 0x0E);
            let is_multi_function = (header_type & 0x80) != 0;

            let max_function = if is_multi_function { 8 } else { 1 };

            for function in 0..max_function {
                let addr = PciAddress::new(bus, device, function);
                if let Some(pci_dev) = scan_function(addr) {
                    // Convert to HardwareDevice
                    let name = get_device_name(
                        pci_dev.vendor_id,
                        pci_dev.device_id,
                        pci_dev.class,
                        pci_dev.subclass,
                    );

                    let mut resources = Vec::new();
                    for bar in &pci_dev.bars {
                        if bar.is_present() {
                            if bar.is_memory {
                                resources.push(DeviceResource::Memory {
                                    base: bar.base,
                                    size: bar.size,
                                });
                            } else {
                                resources.push(DeviceResource::IoPort {
                                    base: bar.base as u16,
                                    size: bar.size as u16,
                                });
                            }
                        }
                    }

                    if pci_dev.interrupt_pin != 0 {
                        resources.push(DeviceResource::Irq {
                            irq: pci_dev.interrupt_line,
                            shared: true,
                        });
                    }

                    devices.push(HardwareDevice {
                        device_type: DeviceType::Pci,
                        vendor_id: pci_dev.vendor_id,
                        device_id: pci_dev.device_id,
                        name,
                        driver: None,
                        status: DeviceStatus::Detected,
                        resources,
                    });
                }
            }
        }
    }
}

/// Get human-readable device name from IDs
fn get_device_name(vendor_id: u16, device_id: u16, class: u8, subclass: u8) -> String {
    // Common vendor names
    let vendor_name = match vendor_id {
        0x8086 => "Intel",
        0x10DE => "NVIDIA",
        0x1002 => "AMD",
        0x1022 => "AMD",
        0x10EC => "Realtek",
        0x14E4 => "Broadcom",
        0x168C => "Qualcomm Atheros",
        0x1D6A => "Aquantia",
        0x8087 => "Intel",
        0x1B4B => "Marvell",
        0x197B => "JMicron",
        0x1AF4 => "VirtIO",
        _ => "Unknown",
    };

    // Class-based description
    let class_name = match (class, subclass) {
        (0x01, 0x06) => "SATA Controller",
        (0x01, 0x08) => "NVMe Controller",
        (0x02, 0x00) => "Ethernet Controller",
        (0x02, 0x80) => "Network Controller",
        (0x03, 0x00) => "VGA Controller",
        (0x03, 0x02) => "3D Controller",
        (0x04, 0x03) => "Audio Device",
        (0x06, 0x00) => "Host Bridge",
        (0x06, 0x01) => "ISA Bridge",
        (0x06, 0x04) => "PCI-to-PCI Bridge",
        (0x0C, 0x03) => "USB Controller",
        (0x0D, 0x00) => "WiFi Controller",
        _ => "Device",
    };

    format!("{} {}", vendor_name, class_name)
}

/// Find all devices of a specific class
pub fn find_by_class(devices: &[PciDevice], class: u8) -> Vec<&PciDevice> {
    devices.iter().filter(|d| d.class == class).collect()
}

/// Find device by vendor and device ID
pub fn find_by_id(devices: &[PciDevice], vendor_id: u16, device_id: u16) -> Option<&PciDevice> {
    devices
        .iter()
        .find(|d| d.vendor_id == vendor_id && d.device_id == device_id)
}

/// Enable bus mastering for DMA
pub fn enable_bus_master(addr: PciAddress) {
    let command = read_config_u16(addr, 0x04);
    let new_command = command | 0x04; // Set Bus Master bit
    write_config_u32(addr, 0x04, new_command as u32);
}

/// Enable memory space access
pub fn enable_memory_space(addr: PciAddress) {
    let command = read_config_u16(addr, 0x04);
    let new_command = command | 0x02; // Set Memory Space bit
    write_config_u32(addr, 0x04, new_command as u32);
}

/// Enable I/O space access
pub fn enable_io_space(addr: PciAddress) {
    let command = read_config_u16(addr, 0x04);
    let new_command = command | 0x01; // Set I/O Space bit
    write_config_u32(addr, 0x04, new_command as u32);
}
