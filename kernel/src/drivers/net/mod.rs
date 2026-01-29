//! Network Driver Abstraction Layer
//!
//! Provides unified interface for various network hardware drivers.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;
use spin::Mutex;

pub mod e1000;
pub mod rtl8111;
pub mod virtio_net;

/// Network error types
#[derive(Debug, Clone)]
pub enum NetworkError {
    /// Device not found
    DeviceNotFound,
    /// Device not initialized
    NotInitialized,
    /// Device is busy
    Busy,
    /// Invalid packet size
    InvalidSize,
    /// Transmit buffer full
    TxBufferFull,
    /// Receive buffer empty
    RxBufferEmpty,
    /// Link is down
    LinkDown,
    /// Hardware error
    HardwareError(u32),
    /// Timeout waiting for operation
    Timeout,
    /// No memory available
    OutOfMemory,
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeviceNotFound => write!(f, "Device not found"),
            Self::NotInitialized => write!(f, "Device not initialized"),
            Self::Busy => write!(f, "Device is busy"),
            Self::InvalidSize => write!(f, "Invalid packet size"),
            Self::TxBufferFull => write!(f, "Transmit buffer full"),
            Self::RxBufferEmpty => write!(f, "Receive buffer empty"),
            Self::LinkDown => write!(f, "Network link is down"),
            Self::HardwareError(code) => write!(f, "Hardware error: {:#x}", code),
            Self::Timeout => write!(f, "Operation timed out"),
            Self::OutOfMemory => write!(f, "Out of memory"),
        }
    }
}

/// MAC address
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(C)]
pub struct MacAddress([u8; 6]);

impl MacAddress {
    /// Create a new MAC address
    pub const fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    /// Get bytes
    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.0
    }

    /// Broadcast address
    pub const BROADCAST: MacAddress = MacAddress([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);

    /// Zero address
    pub const ZERO: MacAddress = MacAddress([0, 0, 0, 0, 0, 0]);

    /// Check if multicast
    pub fn is_multicast(&self) -> bool {
        (self.0[0] & 0x01) != 0
    }

    /// Check if broadcast
    pub fn is_broadcast(&self) -> bool {
        self.0 == [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    }

    /// Check if zero
    pub fn is_zero(&self) -> bool {
        self.0 == [0, 0, 0, 0, 0, 0]
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2],
            self.0[3], self.0[4], self.0[5])
    }
}

/// Link speed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSpeed {
    /// 10 Mbps
    Speed10Mbps,
    /// 100 Mbps
    Speed100Mbps,
    /// 1 Gbps
    Speed1Gbps,
    /// 2.5 Gbps
    Speed2500Mbps,
    /// 5 Gbps
    Speed5Gbps,
    /// 10 Gbps
    Speed10Gbps,
    /// 25 Gbps
    Speed25Gbps,
    /// 40 Gbps
    Speed40Gbps,
    /// 100 Gbps
    Speed100Gbps,
    /// Unknown speed
    Unknown,
}

/// Link duplex mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkDuplex {
    /// Half duplex
    Half,
    /// Full duplex
    Full,
    /// Unknown
    Unknown,
}

/// Link status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkStatus {
    /// Whether link is up
    pub up: bool,
    /// Link speed
    pub speed: LinkSpeed,
    /// Duplex mode
    pub duplex: LinkDuplex,
}

impl Default for LinkStatus {
    fn default() -> Self {
        Self {
            up: false,
            speed: LinkSpeed::Unknown,
            duplex: LinkDuplex::Unknown,
        }
    }
}

/// Network device capabilities
#[derive(Debug, Clone, Copy, Default)]
pub struct NetworkCapabilities {
    /// Supports checksum offload for TX
    pub tx_checksum: bool,
    /// Supports checksum offload for RX
    pub rx_checksum: bool,
    /// Supports TCP segmentation offload
    pub tso: bool,
    /// Supports large receive offload
    pub lro: bool,
    /// Supports scatter/gather I/O
    pub scatter_gather: bool,
    /// Supports VLAN tagging
    pub vlan: bool,
    /// Maximum transmission unit
    pub mtu: u16,
    /// Maximum number of TX queues
    pub max_tx_queues: u8,
    /// Maximum number of RX queues
    pub max_rx_queues: u8,
}

/// Network device statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct NetworkStats {
    /// Packets transmitted
    pub tx_packets: u64,
    /// Bytes transmitted
    pub tx_bytes: u64,
    /// Packets received
    pub rx_packets: u64,
    /// Bytes received
    pub rx_bytes: u64,
    /// Transmit errors
    pub tx_errors: u64,
    /// Receive errors
    pub rx_errors: u64,
    /// Dropped packets
    pub dropped: u64,
    /// Multicast packets received
    pub multicast: u64,
    /// Collisions
    pub collisions: u64,
}

/// Network device interface
pub trait NetworkDevice: Send + Sync {
    /// Get device name
    fn name(&self) -> &str;

    /// Get MAC address
    fn mac_address(&self) -> MacAddress;

    /// Get link status
    fn link_status(&self) -> LinkStatus;

    /// Get device capabilities
    fn capabilities(&self) -> NetworkCapabilities;

    /// Get statistics
    fn stats(&self) -> NetworkStats;

    /// Bring interface up
    fn up(&mut self) -> Result<(), NetworkError>;

    /// Bring interface down
    fn down(&mut self) -> Result<(), NetworkError>;

    /// Set MTU
    fn set_mtu(&mut self, mtu: u16) -> Result<(), NetworkError>;

    /// Transmit a packet
    fn transmit(&mut self, data: &[u8]) -> Result<(), NetworkError>;

    /// Receive a packet
    /// Returns the number of bytes received
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, NetworkError>;

    /// Check if there are packets available to receive
    fn rx_available(&self) -> bool;

    /// Enable promiscuous mode
    fn set_promiscuous(&mut self, enabled: bool) -> Result<(), NetworkError>;

    /// Add multicast address
    fn add_multicast(&mut self, addr: MacAddress) -> Result<(), NetworkError>;

    /// Remove multicast address
    fn remove_multicast(&mut self, addr: MacAddress) -> Result<(), NetworkError>;

    /// Poll for interrupts (for polling mode)
    fn poll(&mut self) -> Result<(), NetworkError>;
}

/// Network device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkDeviceType {
    /// Ethernet
    Ethernet,
    /// WiFi
    WiFi,
    /// Loopback
    Loopback,
    /// Virtual
    Virtual,
    /// Unknown
    Unknown,
}

/// Network device info
#[derive(Debug, Clone)]
pub struct NetworkDeviceInfo {
    /// Device name
    pub name: String,
    /// Device type
    pub device_type: NetworkDeviceType,
    /// MAC address
    pub mac: MacAddress,
    /// Vendor ID
    pub vendor_id: u16,
    /// Device ID
    pub device_id: u16,
    /// Driver name
    pub driver: String,
}

/// Network manager
pub struct NetworkManager {
    /// Registered network devices
    devices: BTreeMap<String, Box<dyn NetworkDevice>>,
    /// Device counter for naming
    device_counter: u32,
}

impl NetworkManager {
    /// Create a new network manager
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            device_counter: 0,
        }
    }

    /// Register a network device
    pub fn register(&mut self, device: Box<dyn NetworkDevice>) -> String {
        let name = device.name().to_string();
        self.devices.insert(name.clone(), device);
        self.device_counter += 1;
        name
    }

    /// Unregister a device
    pub fn unregister(&mut self, name: &str) -> Option<Box<dyn NetworkDevice>> {
        self.devices.remove(name)
    }

    /// Get a device by name
    pub fn device(&self, name: &str) -> Option<&dyn NetworkDevice> {
        self.devices.get(name).map(|d| d.as_ref())
    }

    /// Get a mutable device by name
    pub fn device_mut(&mut self, name: &str) -> Option<&mut dyn NetworkDevice> {
        match self.devices.get_mut(name) {
            Some(d) => Some(d.as_mut()),
            None => None,
        }
    }

    /// List all device names
    pub fn device_names(&self) -> Vec<String> {
        self.devices.keys().cloned().collect()
    }

    /// Enumerate all devices
    pub fn enumerate(&self) -> impl Iterator<Item = &dyn NetworkDevice> {
        self.devices.values().map(|d| d.as_ref())
    }

    /// Get device count
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

impl Default for NetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global network manager
pub static NETWORK_MANAGER: Mutex<NetworkManager> = Mutex::new(NetworkManager::new());

/// Initialize network subsystem
pub fn init() {
    // Probe for network devices
    e1000::probe();
    rtl8111::probe();
    virtio_net::probe();
}

/// Probe for network devices on PCI bus
pub fn probe_pci() {
    // This would iterate PCI devices and initialize appropriate drivers
    // For now, just initialize known drivers
    init();
}
