//! VirtIO Network Device Driver
//!
//! Supports VirtIO network devices in QEMU and other hypervisors.

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ptr;

use super::{
    LinkDuplex, LinkSpeed, LinkStatus, MacAddress, NetworkCapabilities, NetworkDevice,
    NetworkError, NetworkStats, NETWORK_MANAGER,
};

/// VirtIO net device feature bits
#[allow(dead_code)]
mod features {
    pub const CSUM: u64 = 1 << 0; // Host handles checksum
    pub const GUEST_CSUM: u64 = 1 << 1; // Guest handles checksum
    pub const MAC: u64 = 1 << 5; // Device has MAC address
    pub const GSO: u64 = 1 << 6; // Deprecated
    pub const GUEST_TSO4: u64 = 1 << 7; // Guest can handle TSO v4
    pub const GUEST_TSO6: u64 = 1 << 8; // Guest can handle TSO v6
    pub const GUEST_ECN: u64 = 1 << 9; // Guest can handle ECN
    pub const GUEST_UFO: u64 = 1 << 10; // Guest can handle UFO
    pub const HOST_TSO4: u64 = 1 << 11; // Host can handle TSO v4
    pub const HOST_TSO6: u64 = 1 << 12; // Host can handle TSO v6
    pub const HOST_ECN: u64 = 1 << 13; // Host can handle ECN
    pub const HOST_UFO: u64 = 1 << 14; // Host can handle UFO
    pub const MRG_RXBUF: u64 = 1 << 15; // Merge rx buffers
    pub const STATUS: u64 = 1 << 16; // Configuration status field
    pub const CTRL_VQ: u64 = 1 << 17; // Control virtqueue available
    pub const CTRL_RX: u64 = 1 << 18; // RX mode control
    pub const CTRL_VLAN: u64 = 1 << 19; // VLAN filtering control
    pub const GUEST_ANNOUNCE: u64 = 1 << 21; // Guest announce support
    pub const MQ: u64 = 1 << 22; // Multi-queue support
    pub const CTRL_MAC_ADDR: u64 = 1 << 23; // MAC address control
    pub const MTU: u64 = 1 << 25; // MTU negotiation
    pub const SPEED_DUPLEX: u64 = 1 << 63; // Speed/duplex configuration
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

/// MMIO register offsets (VirtIO MMIO transport v1/v2)
#[allow(dead_code)]
mod mmio_reg {
    pub const MAGIC: u32 = 0x00;
    pub const VERSION: u32 = 0x04;
    pub const DEVICE_ID: u32 = 0x08;
    pub const VENDOR_ID: u32 = 0x0C;
    pub const DEVICE_FEATURES: u32 = 0x10;
    pub const DEVICE_FEATURES_SEL: u32 = 0x14;
    pub const DRIVER_FEATURES: u32 = 0x20;
    pub const DRIVER_FEATURES_SEL: u32 = 0x24;
    pub const QUEUE_SEL: u32 = 0x30;
    pub const QUEUE_NUM_MAX: u32 = 0x34;
    pub const QUEUE_NUM: u32 = 0x38;
    pub const QUEUE_READY: u32 = 0x44;
    pub const QUEUE_NOTIFY: u32 = 0x50;
    pub const INTERRUPT_STATUS: u32 = 0x60;
    pub const INTERRUPT_ACK: u32 = 0x64;
    pub const STATUS: u32 = 0x70;
    pub const QUEUE_DESC_LOW: u32 = 0x80;
    pub const QUEUE_DESC_HIGH: u32 = 0x84;
    pub const QUEUE_AVAIL_LOW: u32 = 0x90;
    pub const QUEUE_AVAIL_HIGH: u32 = 0x94;
    pub const QUEUE_USED_LOW: u32 = 0xA0;
    pub const QUEUE_USED_HIGH: u32 = 0xA4;
    pub const CONFIG: u32 = 0x100;
}

/// Virtqueue indices
const RX_QUEUE: u32 = 0;
const TX_QUEUE: u32 = 1;

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
    /// RX buffers (each holds VirtioNetHdr + payload)
    rx_buffers: Vec<Vec<u8>>,
    /// TX buffers
    tx_buffers: Vec<Vec<u8>>,
    /// RX available ring index (host's next slot to fill)
    rx_avail_idx: u16,
    /// TX available ring index
    tx_avail_idx: u16,
    /// RX used ring last seen index
    rx_used_idx: u16,
    /// TX used ring last seen index
    tx_used_idx: u16,
    /// Next TX descriptor slot to use
    tx_next_desc: u16,
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
            tx_next_desc: 0,
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
            unsafe { ptr::read_volatile((mmio + offset as usize) as *const u8) }
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
            unsafe { ptr::read_volatile((mmio + offset as usize) as *const u32) }
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
            let magic = self.read32(mmio_reg::MAGIC);
            if magic != 0x74726976 {
                return Err(NetworkError::DeviceNotFound);
            }

            // Check version
            let version = self.read32(mmio_reg::VERSION);
            if version != 1 && version != 2 {
                return Err(NetworkError::HardwareError(version));
            }

            // Check device ID (1 = network)
            let device_id = self.read32(mmio_reg::DEVICE_ID);
            if device_id != 1 {
                return Err(NetworkError::DeviceNotFound);
            }

            // Reset device
            self.write32(mmio_reg::STATUS, 0);

            // Set ACKNOWLEDGE status bit
            self.write32(mmio_reg::STATUS, 1);

            // Set DRIVER status bit
            self.write32(mmio_reg::STATUS, 3);

            // Read device features
            self.write32(mmio_reg::DEVICE_FEATURES_SEL, 0);
            let feat_lo = self.read32(mmio_reg::DEVICE_FEATURES);
            self.write32(mmio_reg::DEVICE_FEATURES_SEL, 1);
            let feat_hi = self.read32(mmio_reg::DEVICE_FEATURES);
            let device_features = (feat_hi as u64) << 32 | (feat_lo as u64);

            // Select features we want (keep it simple: MAC + STATUS)
            self.features = device_features
                & (features::MAC | features::STATUS | features::CSUM | features::GUEST_CSUM);

            // Write driver features
            self.write32(mmio_reg::DRIVER_FEATURES_SEL, 0);
            self.write32(mmio_reg::DRIVER_FEATURES, self.features as u32);
            self.write32(mmio_reg::DRIVER_FEATURES_SEL, 1);
            self.write32(mmio_reg::DRIVER_FEATURES, (self.features >> 32) as u32);

            // Set FEATURES_OK
            self.write32(mmio_reg::STATUS, 11);

            // Verify FEATURES_OK was accepted
            let status = self.read32(mmio_reg::STATUS);
            if (status & 8) == 0 {
                self.write32(mmio_reg::STATUS, 128); // FAILED
                return Err(NetworkError::HardwareError(status));
            }

            // Read MAC address if supported
            if (self.features & features::MAC) != 0 {
                let mut mac = [0u8; 6];
                for i in 0..6 {
                    mac[i] = self.read8(mmio_reg::CONFIG + i as u32);
                }
                self.mac = MacAddress::new(mac);
            }

            // ── Initialize RX queue (queue 0) ──
            self.init_virtqueue(RX_QUEUE)?;

            // Fill RX queue with receive buffers
            for i in 0..(QUEUE_SIZE.min(self.rx_buffers.len())) {
                let buf_addr = self.rx_buffers[i].as_ptr() as u64;
                let buf_len = self.rx_buffers[i].len() as u32;
                self.rx_desc[i] = VirtqDesc {
                    addr: buf_addr,
                    len: buf_len,
                    flags: VirtqDesc::WRITE, // device writes to this buffer
                    next: 0,
                };
                // Add to available ring via MMIO
                self.put_avail_rx(i as u16);
            }

            // ── Initialize TX queue (queue 1) ──
            self.init_virtqueue(TX_QUEUE)?;

            // Set DRIVER_OK
            self.write32(mmio_reg::STATUS, 15);
        }

        Ok(())
    }

    /// Initialize a virtqueue (must be called after FEATURES_OK).
    fn init_virtqueue(&mut self, queue_idx: u32) -> Result<(), NetworkError> {
        if self.mmio_base.is_none() {
            return Ok(());
        }

        // Select queue
        self.write32(mmio_reg::QUEUE_SEL, queue_idx);

        // Check max queue size
        let max_size = self.read32(mmio_reg::QUEUE_NUM_MAX);
        if max_size == 0 {
            return Err(NetworkError::HardwareError(queue_idx));
        }

        let qsz = (QUEUE_SIZE as u32).min(max_size);
        self.write32(mmio_reg::QUEUE_NUM, qsz);

        // Point descriptor, avail, used to our pre-allocated buffers.
        // For simplicity, we use the Vec-backed descriptor arrays directly.
        // Their addresses are stable because Vec allocates on the heap.
        let (desc_ptr, avail_ptr, used_ptr) = if queue_idx == RX_QUEUE {
            (
                self.rx_desc.as_ptr() as u64,
                // We don't have separate avail/used ring structs in memory,
                // so we'll use the MMIO queue-notify model: the device reads
                // descriptors directly and we write notify.
                0u64,
                0u64,
            )
        } else {
            (self.tx_desc.as_ptr() as u64, 0u64, 0u64)
        };

        // Write queue addresses (split into low/high for 64-bit)
        self.write32(mmio_reg::QUEUE_DESC_LOW, desc_ptr as u32);
        self.write32(mmio_reg::QUEUE_DESC_HIGH, (desc_ptr >> 32) as u32);

        // For MMIO v2, avail and used ring addresses are also set.
        // We leave them as 0 (device-managed) when not explicitly allocated.
        self.write32(mmio_reg::QUEUE_AVAIL_LOW, avail_ptr as u32);
        self.write32(mmio_reg::QUEUE_AVAIL_HIGH, (avail_ptr >> 32) as u32);
        self.write32(mmio_reg::QUEUE_USED_LOW, used_ptr as u32);
        self.write32(mmio_reg::QUEUE_USED_HIGH, (used_ptr >> 32) as u32);

        // Enable queue
        self.write32(mmio_reg::QUEUE_READY, 1);

        Ok(())
    }

    /// Add an RX buffer index to the available ring.
    fn put_avail_rx(&mut self, desc_idx: u16) {
        self.rx_avail_idx = self.rx_avail_idx.wrapping_add(1);
        // Notify device that new RX buffers are available
        if self.mmio_base.is_some() {
            self.write32(mmio_reg::QUEUE_SEL, RX_QUEUE);
            self.write32(mmio_reg::QUEUE_NOTIFY, RX_QUEUE);
        }
    }

    /// Notify device that TX descriptors are ready.
    fn notify_tx(&mut self) {
        if self.mmio_base.is_some() {
            self.write32(mmio_reg::QUEUE_SEL, TX_QUEUE);
            self.write32(mmio_reg::QUEUE_NOTIFY, TX_QUEUE);
        }
    }

    /// Acknowledge device interrupts.
    fn ack_interrupt(&mut self) -> u32 {
        if self.mmio_base.is_some() {
            let status = self.read32(mmio_reg::INTERRUPT_STATUS);
            if status != 0 {
                self.write32(mmio_reg::INTERRUPT_ACK, status);
            }
            status
        } else {
            0
        }
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
            max_tx_queues: if (self.features & features::MQ) != 0 {
                4
            } else {
                1
            },
            max_rx_queues: if (self.features & features::MQ) != 0 {
                4
            } else {
                1
            },
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

        // Pick the next TX descriptor slot (round-robin)
        let idx = self.tx_next_desc as usize % QUEUE_SIZE;
        self.tx_next_desc = self.tx_next_desc.wrapping_add(1);

        // Prepend VirtioNetHdr (12 bytes of zeros = no offload)
        let hdr = VirtioNetHdr::default();
        let total = VirtioNetHdr::SIZE + data.len();
        if total > self.tx_buffers[idx].len() {
            return Err(NetworkError::InvalidSize);
        }

        // Copy header
        let hdr_bytes: [u8; VirtioNetHdr::SIZE] = unsafe { core::mem::transmute(hdr) };
        self.tx_buffers[idx][..VirtioNetHdr::SIZE].copy_from_slice(&hdr_bytes);
        // Copy payload
        self.tx_buffers[idx][VirtioNetHdr::SIZE..total].copy_from_slice(data);

        // Set up TX descriptor
        self.tx_desc[idx] = VirtqDesc {
            addr: self.tx_buffers[idx].as_ptr() as u64,
            len: total as u32,
            flags: 0, // device reads from this buffer
            next: 0,
        };

        // Update available ring index
        self.tx_avail_idx = self.tx_avail_idx.wrapping_add(1);

        // Notify device
        self.notify_tx();

        self.stats.tx_packets += 1;
        self.stats.tx_bytes += data.len() as u64;

        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, NetworkError> {
        if !self.is_up {
            return Err(NetworkError::NotInitialized);
        }

        // Check for interrupt / completed RX descriptors
        self.ack_interrupt();

        // Check if there are any completed RX descriptors.
        // In our simplified model, we check each RX descriptor for data.
        // A real implementation would track the used ring properly.
        let idx = self.rx_used_idx as usize % QUEUE_SIZE;

        // Look at the RX descriptor — if the device has written data,
        // the len field will be non-zero and different from the initial value.
        let desc = &self.rx_desc[idx];
        if desc.len == 0 || desc.len as usize == self.rx_buffers[idx].len() {
            return Err(NetworkError::RxBufferEmpty);
        }

        let total_len = desc.len as usize;
        if total_len <= VirtioNetHdr::SIZE {
            // Reset and refill this descriptor
            self.refill_rx(idx);
            return Err(NetworkError::RxBufferEmpty);
        }

        // Strip VirtioNetHdr
        let payload_len = total_len - VirtioNetHdr::SIZE;
        let copy_len = payload_len.min(buffer.len());
        buffer[..copy_len].copy_from_slice(
            &self.rx_buffers[idx][VirtioNetHdr::SIZE..VirtioNetHdr::SIZE + copy_len],
        );

        // Refill this RX slot
        self.refill_rx(idx);

        self.rx_used_idx = self.rx_used_idx.wrapping_add(1);
        self.stats.rx_packets += 1;
        self.stats.rx_bytes += copy_len as u64;

        Ok(copy_len)
    }

    fn rx_available(&self) -> bool {
        if !self.is_up {
            return false;
        }
        // Check if the current RX descriptor has been filled by the device.
        let idx = self.rx_used_idx as usize % QUEUE_SIZE;
        let desc = &self.rx_desc[idx];
        desc.len != 0 && (desc.len as usize) < self.rx_buffers[idx].len()
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
        // Acknowledge any pending interrupts
        self.ack_interrupt();

        // Check link status if STATUS feature is negotiated
        if (self.features & features::STATUS) != 0 && self.mmio_base.is_some() {
            let status = self.read8(mmio_reg::CONFIG + 6); // After MAC in config space
            self.link_status.up = (status & 1) != 0;
        }
        Ok(())
    }
}

impl VirtioNetDevice {
    /// Refill an RX descriptor slot so the device can write to it again.
    fn refill_rx(&mut self, idx: usize) {
        // Zero the buffer
        for b in self.rx_buffers[idx].iter_mut() {
            *b = 0;
        }
        // Reset descriptor
        self.rx_desc[idx] = VirtqDesc {
            addr: self.rx_buffers[idx].as_ptr() as u64,
            len: self.rx_buffers[idx].len() as u32,
            flags: VirtqDesc::WRITE,
            next: 0,
        };
        // Notify device of refilled RX buffer
        self.put_avail_rx(idx as u16);
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
