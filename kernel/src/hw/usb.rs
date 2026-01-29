//! USB Support
//!
//! USB device enumeration and driver support.

use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
use super::{HardwareDevice, DeviceType, DeviceStatus, DeviceResource};

/// USB speed modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbSpeed {
    /// Low speed (1.5 Mbps)
    Low,
    /// Full speed (12 Mbps)
    Full,
    /// High speed (480 Mbps) - USB 2.0
    High,
    /// Super speed (5 Gbps) - USB 3.0
    Super,
    /// Super speed+ (10 Gbps) - USB 3.1 Gen 2
    SuperPlus,
    /// Super speed+ (20 Gbps) - USB 3.2 Gen 2x2
    SuperPlus20,
}

/// USB device class codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UsbClass {
    /// Device class defined at interface level
    PerInterface = 0x00,
    /// Audio devices
    Audio = 0x01,
    /// Communications and CDC control
    CdcControl = 0x02,
    /// Human Interface Device (HID)
    Hid = 0x03,
    /// Physical
    Physical = 0x05,
    /// Image (still imaging)
    Image = 0x06,
    /// Printer
    Printer = 0x07,
    /// Mass Storage
    MassStorage = 0x08,
    /// Hub
    Hub = 0x09,
    /// CDC Data
    CdcData = 0x0A,
    /// Smart Card
    SmartCard = 0x0B,
    /// Content Security
    ContentSecurity = 0x0D,
    /// Video
    Video = 0x0E,
    /// Personal Healthcare
    PersonalHealthcare = 0x0F,
    /// Audio/Video Devices
    AudioVideo = 0x10,
    /// Billboard Device
    Billboard = 0x11,
    /// Type-C Bridge
    TypeCBridge = 0x12,
    /// Diagnostic Device
    Diagnostic = 0xDC,
    /// Wireless Controller
    Wireless = 0xE0,
    /// Miscellaneous
    Misc = 0xEF,
    /// Application Specific
    Application = 0xFE,
    /// Vendor Specific
    VendorSpecific = 0xFF,
}

/// USB device descriptor
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct UsbDeviceDescriptor {
    /// Length of this descriptor (18 bytes)
    pub length: u8,
    /// Descriptor type (1 for device)
    pub descriptor_type: u8,
    /// USB specification version (BCD)
    pub usb_version: u16,
    /// Device class
    pub device_class: u8,
    /// Device subclass
    pub device_subclass: u8,
    /// Device protocol
    pub device_protocol: u8,
    /// Maximum packet size for endpoint 0
    pub max_packet_size: u8,
    /// Vendor ID
    pub vendor_id: u16,
    /// Product ID
    pub product_id: u16,
    /// Device release number (BCD)
    pub device_version: u16,
    /// Index of manufacturer string
    pub manufacturer_index: u8,
    /// Index of product string
    pub product_index: u8,
    /// Index of serial number string
    pub serial_number_index: u8,
    /// Number of configurations
    pub num_configurations: u8,
}

/// USB configuration descriptor
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct UsbConfigDescriptor {
    /// Length of this descriptor (9 bytes)
    pub length: u8,
    /// Descriptor type (2 for configuration)
    pub descriptor_type: u8,
    /// Total length of all descriptors
    pub total_length: u16,
    /// Number of interfaces
    pub num_interfaces: u8,
    /// Configuration value for SET_CONFIGURATION
    pub configuration_value: u8,
    /// Index of string descriptor
    pub configuration_index: u8,
    /// Configuration attributes (bit 6: self-powered, bit 5: remote wakeup)
    pub attributes: u8,
    /// Maximum power consumption (2mA units)
    pub max_power: u8,
}

/// USB interface descriptor
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct UsbInterfaceDescriptor {
    /// Length of this descriptor (9 bytes)
    pub length: u8,
    /// Descriptor type (4 for interface)
    pub descriptor_type: u8,
    /// Interface number
    pub interface_number: u8,
    /// Alternate setting
    pub alternate_setting: u8,
    /// Number of endpoints
    pub num_endpoints: u8,
    /// Interface class
    pub interface_class: u8,
    /// Interface subclass
    pub interface_subclass: u8,
    /// Interface protocol
    pub interface_protocol: u8,
    /// Index of string descriptor
    pub interface_index: u8,
}

/// USB endpoint descriptor
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct UsbEndpointDescriptor {
    /// Length of this descriptor (7 bytes)
    pub length: u8,
    /// Descriptor type (5 for endpoint)
    pub descriptor_type: u8,
    /// Endpoint address (bit 7: direction, bits 0-3: endpoint number)
    pub endpoint_address: u8,
    /// Endpoint attributes (bits 0-1: transfer type)
    pub attributes: u8,
    /// Maximum packet size
    pub max_packet_size: u16,
    /// Polling interval
    pub interval: u8,
}

/// USB device information
#[derive(Debug, Clone)]
pub struct UsbDevice {
    /// Host controller index
    pub controller: u8,
    /// Port number on root hub or parent hub
    pub port: u8,
    /// Device address
    pub address: u8,
    /// Device speed
    pub speed: UsbSpeed,
    /// Vendor ID
    pub vendor_id: u16,
    /// Product ID
    pub product_id: u16,
    /// Device class
    pub device_class: u8,
    /// Device subclass
    pub device_subclass: u8,
    /// Manufacturer string
    pub manufacturer: String,
    /// Product string
    pub product: String,
    /// Serial number
    pub serial_number: String,
    /// Number of configurations
    pub num_configurations: u8,
    /// Current configuration
    pub current_configuration: u8,
    /// Interfaces
    pub interfaces: Vec<UsbInterface>,
}

/// USB interface
#[derive(Debug, Clone)]
pub struct UsbInterface {
    /// Interface number
    pub number: u8,
    /// Alternate setting
    pub alt_setting: u8,
    /// Interface class
    pub class: u8,
    /// Interface subclass
    pub subclass: u8,
    /// Interface protocol
    pub protocol: u8,
    /// Endpoints
    pub endpoints: Vec<UsbEndpoint>,
}

/// USB endpoint
#[derive(Debug, Clone)]
pub struct UsbEndpoint {
    /// Endpoint address
    pub address: u8,
    /// Direction (true = IN, false = OUT)
    pub direction_in: bool,
    /// Transfer type
    pub transfer_type: UsbTransferType,
    /// Maximum packet size
    pub max_packet_size: u16,
    /// Polling interval (for interrupt/isochronous)
    pub interval: u8,
}

/// USB transfer types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbTransferType {
    Control,
    Isochronous,
    Bulk,
    Interrupt,
}

/// USB host controller types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostControllerType {
    /// Universal Host Controller Interface (USB 1.x)
    Uhci,
    /// Open Host Controller Interface (USB 1.x)
    Ohci,
    /// Enhanced Host Controller Interface (USB 2.0)
    Ehci,
    /// eXtensible Host Controller Interface (USB 3.x)
    Xhci,
}

/// USB host controller
pub struct UsbHostController {
    /// Controller type
    pub controller_type: HostControllerType,
    /// PCI address (if PCI device)
    pub pci_address: Option<super::pci::PciAddress>,
    /// Base address for MMIO
    pub mmio_base: u64,
    /// Number of ports
    pub num_ports: u8,
    /// Discovered devices
    pub devices: Vec<UsbDevice>,
}

impl UsbHostController {
    /// Create a new host controller
    pub fn new(controller_type: HostControllerType, mmio_base: u64, num_ports: u8) -> Self {
        Self {
            controller_type,
            pci_address: None,
            mmio_base,
            num_ports,
            devices: Vec::new(),
        }
    }
}

/// USB request types
#[repr(u8)]
pub enum UsbRequestType {
    /// Standard request to device
    StandardDevice = 0x00,
    /// Standard request to interface
    StandardInterface = 0x01,
    /// Standard request to endpoint
    StandardEndpoint = 0x02,
    /// Class request to device
    ClassDevice = 0x20,
    /// Class request to interface
    ClassInterface = 0x21,
    /// Vendor request to device
    VendorDevice = 0x40,
}

/// Standard USB request codes
#[repr(u8)]
pub enum UsbRequest {
    GetStatus = 0x00,
    ClearFeature = 0x01,
    SetFeature = 0x03,
    SetAddress = 0x05,
    GetDescriptor = 0x06,
    SetDescriptor = 0x07,
    GetConfiguration = 0x08,
    SetConfiguration = 0x09,
    GetInterface = 0x0A,
    SetInterface = 0x0B,
    SynchFrame = 0x0C,
}

/// Descriptor types
#[repr(u8)]
pub enum UsbDescriptorType {
    Device = 1,
    Configuration = 2,
    String = 3,
    Interface = 4,
    Endpoint = 5,
    DeviceQualifier = 6,
    OtherSpeedConfig = 7,
    InterfacePower = 8,
    Otg = 9,
    Debug = 10,
    InterfaceAssociation = 11,
    Bos = 15,
    DeviceCapability = 16,
    HidReport = 0x22,
}

/// Global list of USB controllers
static mut USB_CONTROLLERS: Vec<UsbHostController> = Vec::new();

/// Initialize USB subsystem
pub fn init() {
    // In a real implementation, this would:
    // 1. Find USB host controllers via PCI enumeration
    // 2. Initialize each controller (UHCI, OHCI, EHCI, xHCI)
    // 3. Enable port power and scan for devices
}

/// Enumerate USB devices
pub fn enumerate(devices: &mut Vec<HardwareDevice>) {
    // In a real implementation, this would enumerate all USB devices
    // and add them to the device list
    
    // For now, just scan for USB controllers in PCI
    let mut pci_devices = Vec::new();
    super::pci::enumerate(&mut pci_devices);
    
    for dev in &pci_devices {
        // Class 0x0C, Subclass 0x03 = USB Controller
        if dev.device_type == DeviceType::Pci {
            // Check class code from PCI device info
            // This is simplified - real code would check class/subclass
        }
    }
}

/// Get list of USB controllers
pub fn controllers() -> &'static [UsbHostController] {
    unsafe { &USB_CONTROLLERS }
}

/// Get human-readable name for USB class
pub fn class_name(class: u8, subclass: u8, protocol: u8) -> String {
    match class {
        0x01 => String::from("Audio"),
        0x02 => String::from("Communications"),
        0x03 => match (subclass, protocol) {
            (1, 1) => String::from("Keyboard"),
            (1, 2) => String::from("Mouse"),
            (1, _) => String::from("HID Device"),
            _ => String::from("HID"),
        },
        0x05 => String::from("Physical"),
        0x06 => String::from("Image"),
        0x07 => String::from("Printer"),
        0x08 => match subclass {
            0x01 => String::from("RBC Storage"),
            0x02 => String::from("ATAPI Storage"),
            0x04 => String::from("UFI Floppy"),
            0x06 => String::from("SCSI Storage"),
            _ => String::from("Mass Storage"),
        },
        0x09 => match protocol {
            0x00 => String::from("Full-Speed Hub"),
            0x01 => String::from("High-Speed Hub (single TT)"),
            0x02 => String::from("High-Speed Hub (multi TT)"),
            0x03 => String::from("SuperSpeed Hub"),
            _ => String::from("USB Hub"),
        },
        0x0E => String::from("Video"),
        0xE0 => match (subclass, protocol) {
            (1, 1) => String::from("Bluetooth Adapter"),
            _ => String::from("Wireless Controller"),
        },
        0xEF => String::from("Miscellaneous"),
        0xFF => String::from("Vendor Specific"),
        _ => String::from("Unknown USB Device"),
    }
}

/// USB Setup packet for control transfers
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct UsbSetupPacket {
    /// Request type
    pub request_type: u8,
    /// Request code
    pub request: u8,
    /// Value (varies by request)
    pub value: u16,
    /// Index (varies by request)
    pub index: u16,
    /// Data length
    pub length: u16,
}

impl UsbSetupPacket {
    /// Create GET_DESCRIPTOR request
    pub fn get_descriptor(desc_type: u8, desc_index: u8, length: u16) -> Self {
        Self {
            request_type: 0x80,  // Device to Host, Standard, Device
            request: UsbRequest::GetDescriptor as u8,
            value: ((desc_type as u16) << 8) | (desc_index as u16),
            index: 0,
            length,
        }
    }

    /// Create SET_ADDRESS request
    pub fn set_address(address: u8) -> Self {
        Self {
            request_type: 0x00,  // Host to Device, Standard, Device
            request: UsbRequest::SetAddress as u8,
            value: address as u16,
            index: 0,
            length: 0,
        }
    }

    /// Create SET_CONFIGURATION request
    pub fn set_configuration(config_value: u8) -> Self {
        Self {
            request_type: 0x00,
            request: UsbRequest::SetConfiguration as u8,
            value: config_value as u16,
            index: 0,
            length: 0,
        }
    }
}
