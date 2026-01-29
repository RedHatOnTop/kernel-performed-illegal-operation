//! VirtIO Network Device Driver
//!
//! Supports VirtIO network devices in QEMU and other hypervisors.

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ptr;

use super::{
    MacAddress, NetworkDevice, NetworkError, NetworkCapabilities, NetworkStats,
    LinkStatus, LinkSpeed, LinkDuplex, NETWORK_MANAGER,
};

/// VirtIO net device feature bits
#[allow(dead_code)]
mod features {
    pub const CSUM: u64 = 1 << 0;           // Host handles checksum
    pub const GUEST_CSUM: u64 = 1 << 1;     // Guest handles checksum
    pub const MAC: u64 = 1 << 5;            // Device has MAC address
    pub const GSO: u64 = 1 << 6;            // Deprecated
    pub const GUEST_TSO4: u64 = 1 << 7;     // Guest can handle TSO v4
    pub const GUEST_TSO6: u64 = 1 << 8;     // Guest can handle TSO v6
    pub const GUEST_ECN: u64 = 1 << 9;      // Guest can handle ECN
    pub const GUEST_UFO: u64 = 1 << 10;     // Guest can handle UFO
    pub const HOST_TSO4: u64 = 1 << 11;     // Host can handle TSO v4
    pub const HOST_TSO6: u64 = 1 << 12;     // Host can handle TSO v6
    pub const HOST_ECN: u64 = 1 << 13;      // Host can handle ECN
    pub const HOST_UFO: u64 = 1 << 14;      // Host can handle UFO
    pub const MRG_RXBUF: u64 = 1 << 15;     // Merge rx buffers
    pub const STATUS: u64 = 1 << 16;        // Configuration status field
    pub const CTRL_VQ: u64 = 1 << 17;       // Control virtqueue available
    pub const CTRL_RX: u64 = 1 << 18;       // RX mode control
    pub const CTRL_VLAN: u64 = 1 << 19;     // VLAN filtering control
    pub const GUEST_ANNOUNCE: u64 = 1 << 21; // Guest announce support
    pub const MQ: u64 = 1 << 22;            // Multi-queue support
    pub const CTRL_MAC_ADDR: u64 = 1 << 23; // MAC address control
    pub const MTU: u64 = 1 << 25;           // MTU negotiation
    pub const SPEED_DUPLEX: u64 = 1 << 63;  // Speed/duplex configuration
}

/// VirtIO net header
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct VirtioNetHdr {
    /// Header flags
    flags: u8,
    /// GSO type
    gso_type: u8,
    /// Header length
    hdr_len: u16,
    /// GSO size
    gso_size: u16,
    /// Checksum start
    csum_start: u16,
    /// Checksum offset
    csum_offset: u16,
    /// Number of buffers (if MRG_RXBUF)
    num_buffers: u16,
}

impl VirtioNetHdr {
    const SIZE: usize = 12;

    /// No checksum needed
    const NEEDS_CSUM: u8 = 1;
    /// Data is valid
    const DATA_VALID: u8 = 2;

    /// No GSO
    const GSO_NONE: u8 = 0;
    /// TCPv4 GSO
    const GSO_TCPV4: u8 = 1;
    /// UDP GSO
    const GSO_UDP: u8 = 3;
    /// TCPv6 GSO
    const GSO_TCPV6: u8 = 4;
}

/// VirtIO queue descriptor
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct VirtqDesc {
    /// Buffer address
    addr: u64,
    /// Buffer length
    len: u32,
    /// Flags
    flags: u16,
    /// Next descriptor in chain
    next: u16,
}

impl VirtqDesc {
    /// This marks a buffer as continuing via the next field
    const NEXT: u16 = 1;
    /// This marks a buffer as write-only (for device)
    const WRITE: u16 = 2;
    /// This means the buffer contains a list of buffer descriptors
    const INDIRECT: u16 = 4;
}

/// VirtIO available ring
#[repr(C, packed)]
struct VirtqAvail {
    flags: u16,
    idx: u16,
    ring: [u16; 256], // Variable size, using 256 as max
}

/// VirtIO used ring element
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct VirtqUsedElem {
    id: u32,
    len: u32,
}

/// VirtIO used ring
#[repr(C, packed)]
struct VirtqUsed {
    flags: u16,
    idx: u16,
    ring: [VirtqUsedElem; 256], // Variable size
}

/// Number of descriptors per queue
const QUEUE_SIZE: usize = 128;

/// RX buffer size
const RX_BUFFER_SIZE: usize = 2048;

/// VirtIO network device
pub struct VirtioNetDevice {
    /// Device name
    name: String,
    /// Base I/O address
    io_base: u16,
    /// MMIO base (for MMIO devices)
    mmio_base: Option<usize>,
    /// MAC address
    mac: MacAddress,
    /// Negotiated features
    features: u64,
    /// RX queue descriptors
    rx_desc: Vec<VirtqDesc>,
    /// TX queue descriptors
    tx_desc: Vec<VirtqDesc>,
    /// RX buffers
    rx_buffers: Vec<Vec<u8>>,
    /// TX buffers
    tx_buffers: Vec<Vec<u8>>,
    /// RX available ring index
    rx_avail_idx: u16,
    /// TX available ring index
    tx_avail_idx: u16,
    /// RX used ring last seen index
    rx_used_idx: u16,
    /// TX used ring last seen index
    tx_used_idx: u16,
    /// Statistics
    stats: NetworkStats,
    /// Link status
    link_status: LinkStatus,
    /// Is up
    is_up: bool,
}

impl VirtioNetDevice {
    /// Create new device (PIO mode)
    pub fn new_pio(name: &str, io_base: u16) -> Self {
        Self::new_internal(name, io_base, None)
    }

    /// Create new device (MMIO mode)
    pub fn new_mmio(name: &str, mmio_base: usize) -> Self {
        Self::new_internal(name, 0, Some(mmio_base))
    }

    fn new_internal(name: &str, io_base: u16, mmio_base: Option<usize>) -> Self {
        let mut rx_desc = Vec::with_capacity(QUEUE_SIZE);
        let mut tx_desc = Vec::with_capacity(QUEUE_SIZE);
        let mut rx_buffers = Vec::with_capacity(QUEUE_SIZE);
        let mut tx_buffers = Vec::with_capacity(QUEUE_SIZE);

        for _ in 0..QUEUE_SIZE {
            rx_desc.push(VirtqDesc::default());
            tx_desc.push(VirtqDesc::default());
            // RX buffers need header space
            rx_buffers.push(alloc::vec![0u8; RX_BUFFER_SIZE + VirtioNetHdr::SIZE]);
            tx_buffers.push(alloc::vec![0u8; RX_BUFFER_SIZE + VirtioNetHdr::SIZE]);
        }

        Self {
            name: name.to_string(),
            io_base,
            mmio_base,
            mac: MacAddress::ZERO,
            features: 0,
            rx_desc,
            tx_desc,
            rx_buffers,
            tx_buffers,
            rx_avail_idx: 0,
            tx_avail_idx: 0,
            rx_used_idx: 0,
            tx_used_idx: 0,
            stats: NetworkStats::default(),
            link_status: LinkStatus {
                up: true,
                speed: LinkSpeed::Speed10Gbps,
                duplex: LinkDuplex::Full,
            },
            is_up: false,
        }
    }

    /// Read 8-bit from device
    fn read8(&self, offset: u32) -> u8 {
        if let Some(mmio) = self.mmio_base {
            unsafe {
                ptr::read_volatile((mmio + offset as usize) as *const u8)
            }
        } else {
            // PIO mode - would use port I/O
            0
        }
    }

    /// Write 8-bit to device
    fn write8(&mut self, offset: u32, val: u8) {
        if let Some(mmio) = self.mmio_base {
            unsafe {
                ptr::write_volatile((mmio + offset as usize) as *mut u8, val);
            }
        }
        // PIO mode would use port I/O
    }

    /// Read 32-bit from device
    fn read32(&self, offset: u32) -> u32 {
        if let Some(mmio) = self.mmio_base {
            unsafe {
                ptr::read_volatile((mmio + offset as usize) as *const u32)
            }
        } else {
            0
        }
    }

    /// Write 32-bit to device
    fn write32(&mut self, offset: u32, val: u32) {
        if let Some(mmio) = self.mmio_base {
            unsafe {
                ptr::write_volatile((mmio + offset as usize) as *mut u32, val);
            }
        }
    }

    /// Initialize device
    pub fn init(&mut self) -> Result<(), NetworkError> {
        // VirtIO MMIO initialization
        if self.mmio_base.is_some() {
            // Check magic value (0x74726976)
            let magic = self.read32(0x00);
            if magic != 0x74726976 {
                return Err(NetworkError::DeviceNotFound);
            }

            // Check version
            let version = self.read32(0x04);
            if version != 1 && version != 2 {
                return Err(NetworkError::HardwareError(version));
            }

            // Check device ID (1 = network)
            let device_id = self.read32(0x08);
            if device_id != 1 {
                return Err(NetworkError::DeviceNotFound);
            }

            // Reset device
            self.write32(0x70, 0);

            // Set ACKNOWLEDGE status bit
            self.write32(0x70, 1);

            // Set DRIVER status bit
            self.write32(0x70, 3);

            // Read device features
            self.write32(0x14, 0); // Select feature set 0
            let feat_lo = self.read32(0x10);
            self.write32(0x14, 1); // Select feature set 1
            let feat_hi = self.read32(0x10);
            let device_features = (feat_hi as u64) << 32 | (feat_lo as u64);

            // Select features we want
            self.features = device_features & (
                features::MAC | 
                features::STATUS | 
                features::CSUM | 
                features::GUEST_CSUM
            );

            // Write driver features
            self.write32(0x24, 0); // Select feature set 0
            self.write32(0x20, self.features as u32);
            self.write32(0x24, 1); // Select feature set 1
            self.write32(0x20, (self.features >> 32) as u32);

            // Set FEATURES_OK
            self.write32(0x70, 11);

            // Read MAC address if supported
            if (self.features & features::MAC) != 0 {
                let mut mac = [0u8; 6];
                for i in 0..6 {
                    mac[i] = self.read8(0x100 + i as u32);
                }
                self.mac = MacAddress::new(mac);
            }

            // Initialize queues would go here...

            // Set DRIVER_OK
            self.write32(0x70, 15);
        }

        Ok(())
    }
}

impl NetworkDevice for VirtioNetDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn mac_address(&self) -> MacAddress {
        self.mac
    }

    fn link_status(&self) -> LinkStatus {
        self.link_status
    }

    fn capabilities(&self) -> NetworkCapabilities {
        NetworkCapabilities {
            tx_checksum: (self.features & features::CSUM) != 0,
            rx_checksum: (self.features & features::GUEST_CSUM) != 0,
            tso: (self.features & features::HOST_TSO4) != 0,
            lro: false,
            scatter_gather: true,
            vlan: (self.features & features::CTRL_VLAN) != 0,
            mtu: 1500,
            max_tx_queues: if (self.features & features::MQ) != 0 { 4 } else { 1 },
            max_rx_queues: if (self.features & features::MQ) != 0 { 4 } else { 1 },
        }
    }

    fn stats(&self) -> NetworkStats {
        self.stats
    }

    fn up(&mut self) -> Result<(), NetworkError> {
        if self.is_up {
            return Ok(());
        }

        self.init()?;
        self.is_up = true;
        Ok(())
    }

    fn down(&mut self) -> Result<(), NetworkError> {
        if !self.is_up {
            return Ok(());
        }

        // Reset device
        if self.mmio_base.is_some() {
            self.write32(0x70, 0);
        }

        self.is_up = false;
        Ok(())
    }

    fn set_mtu(&mut self, mtu: u16) -> Result<(), NetworkError> {
        if mtu > 9000 {
            return Err(NetworkError::InvalidSize);
        }
        Ok(())
    }

    fn transmit(&mut self, data: &[u8]) -> Result<(), NetworkError> {
        if !self.is_up {
            return Err(NetworkError::NotInitialized);
        }

        if data.len() > RX_BUFFER_SIZE {
            return Err(NetworkError::InvalidSize);
        }

        // In a real implementation, we would:
        // 1. Get a free TX descriptor
        // 2. Copy virtio_net_hdr + data to buffer
        // 3. Add to available ring
        // 4. Notify device

        self.stats.tx_packets += 1;
        self.stats.tx_bytes += data.len() as u64;

        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, NetworkError> {
        if !self.is_up {
            return Err(NetworkError::NotInitialized);
        }

        // In a real implementation, we would:
        // 1. Check used ring for completed RX
        // 2. Copy data (skipping virtio_net_hdr)
        // 3. Refill RX buffer to available ring

        Err(NetworkError::RxBufferEmpty)
    }

    fn rx_available(&self) -> bool {
        // Would check used ring
        false
    }

    fn set_promiscuous(&mut self, _enabled: bool) -> Result<(), NetworkError> {
        // Would use control virtqueue if CTRL_RX feature
        Ok(())
    }

    fn add_multicast(&mut self, _addr: MacAddress) -> Result<(), NetworkError> {
        Ok(())
    }

    fn remove_multicast(&mut self, _addr: MacAddress) -> Result<(), NetworkError> {
        Ok(())
    }

    fn poll(&mut self) -> Result<(), NetworkError> {
        // Check for completed TX/RX
        if (self.features & features::STATUS) != 0 && self.mmio_base.is_some() {
            let status = self.read8(0x100 + 6); // After MAC in config space
            self.link_status.up = (status & 1) != 0;
        }
        Ok(())
    }
}

/// Probe for VirtIO network devices
pub fn probe() {
    // Would scan PCI for VirtIO devices:
    // - Vendor ID: 0x1AF4 (Red Hat)
    // - Device ID: 0x1000 (network - legacy) or 0x1041 (network - modern)
    //
    // Or scan MMIO regions for VirtIO MMIO devices
}

/// Initialize VirtIO MMIO network device
pub fn init_mmio(mmio_base: usize) -> Result<(), NetworkError> {
    let name = {
        let manager = NETWORK_MANAGER.lock();
        alloc::format!("virtio{}", manager.device_count())
    };

    let mut device = VirtioNetDevice::new_mmio(&name, mmio_base);
    device.init()?;

    NETWORK_MANAGER.lock().register(Box::new(device));

    Ok(())
}

/// Initialize VirtIO PIO network device
pub fn init_pio(io_base: u16) -> Result<(), NetworkError> {
    let name = {
        let manager = NETWORK_MANAGER.lock();
        alloc::format!("virtio{}", manager.device_count())
    };

    let mut device = VirtioNetDevice::new_pio(&name, io_base);
    device.init()?;

    NETWORK_MANAGER.lock().register(Box::new(device));

    Ok(())
}
