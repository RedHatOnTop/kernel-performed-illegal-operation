//! VirtIO Network Device Driver
//!
//! Supports VirtIO network devices in QEMU and other hypervisors.
//! Implements both MMIO and PIO (legacy) transport modes.
//!
//! The PIO mode uses `x86_64::instructions::port::Port` for register access,
//! following the same proven pattern as the VirtIO block driver
//! (`kernel/src/driver/virtio/block.rs`).

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ptr;
use x86_64::instructions::port::Port;

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

/// Number of descriptors per queue.
///
/// Legacy VirtIO PCI: the QUEUE_SIZE register is read-only.  The device
/// dictates the size (usually 256 on QEMU).  We MUST match it, otherwise
/// the avail/used ring offsets inside the virtqueue page won't line up
/// with what the device expects.
const QUEUE_SIZE: usize = 256;

/// RX buffer size
const RX_BUFFER_SIZE: usize = 2048;

/// Tracks the physical addresses of virtqueue ring memory (PIO mode).
///
/// `*_phys` fields hold the *physical* addresses seen by the device for DMA.
/// `*_virt` fields hold the *virtual* addresses the CPU uses to read/write.
struct VirtqRings {
    /// Physical address of descriptor table (for device DMA)
    desc_phys: u64,
    /// Virtual address of descriptor table (for CPU access)
    desc_virt: u64,
    /// Physical address of available ring
    avail_phys: u64,
    /// Virtual address of available ring
    avail_virt: u64,
    /// Physical address of used ring
    used_phys: u64,
    /// Virtual address of used ring
    used_virt: u64,
    /// Queue size reported by device
    queue_size: u16,
}

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

/// PIO (legacy) register offsets for VirtIO PCI transport (VirtIO 1.0 §4.1.4.8)
#[allow(dead_code)]
mod pio_reg {
    /// Device features (4 bytes, read-only)
    pub const DEVICE_FEATURES: u16 = 0x00;
    /// Driver (guest) features (4 bytes, read-write)
    pub const DRIVER_FEATURES: u16 = 0x04;
    /// Queue address — physical page number (4 bytes)
    pub const QUEUE_ADDRESS: u16 = 0x08;
    /// Queue size (2 bytes, read-only)
    pub const QUEUE_SIZE: u16 = 0x0C;
    /// Queue select (2 bytes, read-write)
    pub const QUEUE_SELECT: u16 = 0x0E;
    /// Queue notify (2 bytes, write-only)
    pub const QUEUE_NOTIFY: u16 = 0x10;
    /// Device status (1 byte, read-write)
    pub const DEVICE_STATUS: u16 = 0x12;
    /// ISR status (1 byte, read-only)
    pub const ISR_STATUS: u16 = 0x13;
    /// MAC address byte 0 (device-specific config starts at 0x14 for net)
    pub const MAC0: u16 = 0x14;
    /// Network status (2 bytes, at offset 0x1A)
    pub const NET_STATUS: u16 = 0x1A;
}

/// VirtIO device status bits (shared between PIO and MMIO)
mod device_status {
    /// Driver has acknowledged the device
    pub const ACKNOWLEDGE: u8 = 1;
    /// Driver knows how to drive the device
    pub const DRIVER: u8 = 2;
    /// Driver is ready
    pub const DRIVER_OK: u8 = 4;
    /// Feature negotiation complete
    pub const FEATURES_OK: u8 = 8;
    /// Device has experienced an error and needs reset
    #[allow(dead_code)]
    pub const NEEDS_RESET: u8 = 64;
    /// Something went wrong — device is unusable
    pub const FAILED: u8 = 128;
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
    /// PIO-mode RX virtqueue ring addresses
    rx_rings: Option<VirtqRings>,
    /// PIO-mode TX virtqueue ring addresses
    tx_rings: Option<VirtqRings>,
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
            rx_rings: None,
            tx_rings: None,
        }
    }

    /// Read 8-bit from device register
    fn read8(&self, offset: u32) -> u8 {
        if let Some(mmio) = self.mmio_base {
            unsafe { ptr::read_volatile((mmio + offset as usize) as *const u8) }
        } else {
            let port = self.io_base + offset as u16;
            unsafe { Port::<u8>::new(port).read() }
        }
    }

    /// Write 8-bit to device register
    fn write8(&mut self, offset: u32, val: u8) {
        if let Some(mmio) = self.mmio_base {
            unsafe {
                ptr::write_volatile((mmio + offset as usize) as *mut u8, val);
            }
        } else {
            let port = self.io_base + offset as u16;
            unsafe { Port::<u8>::new(port).write(val) }
        }
    }

    /// Read 16-bit from device register (PIO only, used for queue size etc.)
    fn read16(&self, offset: u32) -> u16 {
        if let Some(mmio) = self.mmio_base {
            unsafe { ptr::read_volatile((mmio + offset as usize) as *const u16) }
        } else {
            let port = self.io_base + offset as u16;
            unsafe { Port::<u16>::new(port).read() }
        }
    }

    /// Write 16-bit to device register
    fn write16(&mut self, offset: u32, val: u16) {
        if let Some(mmio) = self.mmio_base {
            unsafe {
                ptr::write_volatile((mmio + offset as usize) as *mut u16, val);
            }
        } else {
            let port = self.io_base + offset as u16;
            unsafe { Port::<u16>::new(port).write(val) }
        }
    }

    /// Read 32-bit from device register
    fn read32(&self, offset: u32) -> u32 {
        if let Some(mmio) = self.mmio_base {
            unsafe { ptr::read_volatile((mmio + offset as usize) as *const u32) }
        } else {
            let port = self.io_base + offset as u16;
            unsafe { Port::<u32>::new(port).read() }
        }
    }

    /// Write 32-bit to device register
    fn write32(&mut self, offset: u32, val: u32) {
        if let Some(mmio) = self.mmio_base {
            unsafe {
                ptr::write_volatile((mmio + offset as usize) as *mut u32, val);
            }
        } else {
            let port = self.io_base + offset as u16;
            unsafe { Port::<u32>::new(port).write(val) }
        }
    }

    /// Initialize device (dispatches to MMIO or PIO path)
    pub fn init(&mut self) -> Result<(), NetworkError> {
        if self.mmio_base.is_some() {
            self.init_mmio_device()
        } else {
            self.init_pio_device()
        }
    }

    /// Initialize device via PIO (legacy VirtIO PCI transport).
    ///
    /// Follows the same proven sequence as the VirtIO block driver
    /// (`kernel/src/driver/virtio/block.rs`).
    fn init_pio_device(&mut self) -> Result<(), NetworkError> {
        let io = self.io_base;
        crate::serial_println!("[VirtIO Net] PIO init starting (io_base={:#x})", io);

        // 1. Reset device
        self.write8(pio_reg::DEVICE_STATUS as u32, 0);

        // 2. Set ACKNOWLEDGE status bit
        self.write8(pio_reg::DEVICE_STATUS as u32, device_status::ACKNOWLEDGE);

        // 3. Set DRIVER status bit
        self.write8(
            pio_reg::DEVICE_STATUS as u32,
            device_status::ACKNOWLEDGE | device_status::DRIVER,
        );

        // 4. Read device features
        let device_features = self.read32(pio_reg::DEVICE_FEATURES as u32) as u64;
        crate::serial_println!("[VirtIO Net] Device features: {:#x}", device_features);

        // 5. Select features we want.
        //    MAC + STATUS for basic operation.
        //    MRG_RXBUF is REQUIRED because our VirtioNetHdr is 12 bytes (includes
        //    num_buffers field).  Without MRG_RXBUF the device uses a 10-byte
        //    header, causing a 2-byte offset mismatch that corrupts every packet.
        self.features = device_features
            & (features::MAC
                | features::STATUS
                | features::MRG_RXBUF
                | features::CSUM
                | features::GUEST_CSUM);

        // 6. Write driver features
        self.write32(pio_reg::DRIVER_FEATURES as u32, self.features as u32);

        // 7. Set FEATURES_OK
        self.write8(
            pio_reg::DEVICE_STATUS as u32,
            device_status::ACKNOWLEDGE | device_status::DRIVER | device_status::FEATURES_OK,
        );

        // 8. Verify FEATURES_OK was accepted
        let status = self.read8(pio_reg::DEVICE_STATUS as u32);
        if (status & device_status::FEATURES_OK) == 0 {
            crate::serial_println!("[VirtIO Net] Feature negotiation failed (status={:#x})", status);
            self.write8(pio_reg::DEVICE_STATUS as u32, device_status::FAILED);
            return Err(NetworkError::HardwareError(status as u32));
        }

        // 9. Read MAC address from device-specific config space (offset 0x14..0x19)
        if (self.features & features::MAC) != 0 {
            let mut mac = [0u8; 6];
            for i in 0..6 {
                mac[i] = self.read8((pio_reg::MAC0 + i as u16) as u32);
            }
            self.mac = MacAddress::new(mac);
            crate::serial_println!(
                "[VirtIO Net] MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
            );
        } else {
            crate::serial_println!("[VirtIO Net] No MAC feature — using zero MAC");
        }

        // 10. Initialize RX queue (queue 0) via PIO
        self.init_virtqueue_pio(0)?;

        // Fill RX queue with receive buffers — descriptors must carry PHYSICAL
        // addresses because the VirtIO device does DMA.
        for i in 0..(QUEUE_SIZE.min(self.rx_buffers.len())) {
            let buf_virt = self.rx_buffers[i].as_ptr() as u64;
            let buf_phys = crate::memory::virt_to_phys(buf_virt).unwrap_or(buf_virt);
            let buf_len = self.rx_buffers[i].len() as u32;
            self.rx_desc[i] = VirtqDesc {
                addr: buf_phys,
                len: buf_len,
                flags: VirtqDesc::WRITE, // device writes to this buffer
                next: 0,
            };
        }

        // Write all RX descriptors to the available ring
        if let Some(ref rings) = self.rx_rings {
            unsafe {
                // Write descriptors to device-visible memory
                let desc_base = rings.desc_virt as *mut VirtqDesc;
                for i in 0..QUEUE_SIZE {
                    ptr::write_volatile(desc_base.add(i), self.rx_desc[i]);
                }

                let avail_ptr = rings.avail_virt as *mut u16;
                for i in 0..QUEUE_SIZE as u16 {
                    // ring[i] = descriptor index
                    ptr::write_volatile(avail_ptr.add(2 + i as usize), i);
                }
                // Set avail idx = QUEUE_SIZE (all buffers available)
                core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
                ptr::write_volatile(avail_ptr.add(1), QUEUE_SIZE as u16);
                core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            }
        }
        self.rx_avail_idx = QUEUE_SIZE as u16;

        // Notify device that RX buffers are available
        self.write16(pio_reg::QUEUE_NOTIFY as u32, 0);

        // 11. Initialize TX queue (queue 1) via PIO
        self.init_virtqueue_pio(1)?;

        // 12. Set DRIVER_OK — device is live
        self.write8(
            pio_reg::DEVICE_STATUS as u32,
            device_status::ACKNOWLEDGE
                | device_status::DRIVER
                | device_status::FEATURES_OK
                | device_status::DRIVER_OK,
        );

        let final_status = self.read8(pio_reg::DEVICE_STATUS as u32);
        crate::serial_println!(
            "[VirtIO Net] PIO init complete — status={:#x} (DRIVER_OK={})",
            final_status,
            (final_status & device_status::DRIVER_OK) != 0
        );

        self.is_up = true;
        Ok(())
    }

    /// Initialize device via MMIO transport.
    fn init_mmio_device(&mut self) -> Result<(), NetworkError> {
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

        // Select features we want.
        //    MRG_RXBUF is REQUIRED because our VirtioNetHdr is 12 bytes (includes
        //    num_buffers field).  Without MRG_RXBUF the device uses a 10-byte
        //    header, causing a 2-byte offset mismatch that corrupts every packet.
        self.features = device_features
            & (features::MAC
                | features::STATUS
                | features::MRG_RXBUF
                | features::CSUM
                | features::GUEST_CSUM);

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

        self.is_up = true;
        Ok(())
    }

    /// Initialize a virtqueue via PIO (legacy VirtIO PCI transport).
    ///
    /// Allocates descriptor table, available ring, and used ring in memory,
    /// then writes the physical page number to the QUEUE_ADDRESS register.
    /// This follows the same pattern as `kernel/src/driver/virtio/block.rs`.
    fn init_virtqueue_pio(&mut self, queue_idx: u16) -> Result<(), NetworkError> {
        // Select queue
        self.write16(pio_reg::QUEUE_SELECT as u32, queue_idx);

        // Read max queue size from device
        let max_size = self.read16(pio_reg::QUEUE_SIZE as u32);
        if max_size == 0 {
            crate::serial_println!(
                "[VirtIO Net] Queue {} not available (size=0)",
                queue_idx
            );
            return Err(NetworkError::HardwareError(queue_idx as u32));
        }

        let qsz = (QUEUE_SIZE as u16).min(max_size);
        crate::serial_println!(
            "[VirtIO Net] Queue {} size: {} (max={})",
            queue_idx, qsz, max_size
        );

        // Calculate sizes for the three ring areas.
        // Legacy VirtIO layout (spec §2.4.2):
        //   desc table:  16 bytes × queue_size
        //   avail ring:  6 + 2 × queue_size bytes
        //   (pad to next page boundary)
        //   used ring:   6 + 8 × queue_size bytes
        let desc_size = 16 * qsz as usize; // sizeof(VirtqDesc) = 16
        let avail_size = 6 + 2 * qsz as usize;
        let _used_size = 6 + 8 * qsz as usize;

        // Allocate heap memory (4 pages, 16 KiB — plenty for 128-entry queues)
        // and translate the virtual address to physical for DMA.
        //
        // CRITICAL: the legacy VirtIO QUEUE_ADDRESS register takes a physical
        // page frame number (PFN), so the queue memory must start at a
        // page-aligned address.  Use Layout with 4096-byte alignment.
        let layout = alloc::alloc::Layout::from_size_align(4096 * 4, 4096)
            .expect("[VirtIO Net] Failed to create page-aligned layout");
        let queue_virt = unsafe { alloc::alloc::alloc_zeroed(layout) } as u64;
        if queue_virt == 0 {
            return Err(NetworkError::HardwareError(0xDEAD));
        }

        let queue_phys = crate::memory::virt_to_phys(queue_virt)
            .expect("[VirtIO Net] Failed to translate queue memory to physical address");

        let desc_virt = queue_virt;
        let desc_phys = queue_phys;
        let avail_virt = desc_virt + desc_size as u64;
        let avail_phys = desc_phys + desc_size as u64;
        // Used ring must be page-aligned (legacy spec requirement)
        let used_virt = (avail_virt + avail_size as u64 + 4095) & !4095;
        let used_phys = (avail_phys + avail_size as u64 + 4095) & !4095;

        // Write queue PHYSICAL page number to device (legacy: pfn = phys_addr >> 12)
        let mut queue_addr_port: Port<u32> =
            Port::new(self.io_base + pio_reg::QUEUE_ADDRESS);
        unsafe { queue_addr_port.write((desc_phys / 4096) as u32) };

        let rings = VirtqRings {
            desc_phys,
            desc_virt,
            avail_phys,
            avail_virt,
            used_phys,
            used_virt,
            queue_size: qsz,
        };

        if queue_idx == 0 {
            // Copy our pre-allocated descriptor data into the device memory
            let desc_base = desc_virt as *mut VirtqDesc;
            for i in 0..qsz as usize {
                unsafe {
                    ptr::write_volatile(desc_base.add(i), self.rx_desc[i]);
                }
            }
            self.rx_rings = Some(rings);
        } else {
            let desc_base = desc_virt as *mut VirtqDesc;
            for i in 0..qsz as usize {
                unsafe {
                    ptr::write_volatile(desc_base.add(i), self.tx_desc[i]);
                }
            }
            self.tx_rings = Some(rings);
        }

        crate::serial_println!(
            "[VirtIO Net] Queue {} configured: phys={:#x} (virt={:#x})",
            queue_idx, desc_phys, desc_virt
        );
        Ok(())
    }

    /// Initialize a virtqueue (must be called after FEATURES_OK) — MMIO path.
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
        // For MMIO v2, the device requires physical addresses for DMA.
        // Allocate a contiguous page-aligned region and translate to physical.
        let desc_virt = if queue_idx == RX_QUEUE {
            self.rx_desc.as_ptr() as u64
        } else {
            self.tx_desc.as_ptr() as u64
        };

        let desc_phys = crate::memory::virt_to_phys(desc_virt)
            .unwrap_or(desc_virt);

        // Allocate avail and used rings (simple bump allocation from heap)
        // For our current implementation, we use MMIO queue-notify model.
        // Set avail/used to same base with proper offsets if not separately allocated.
        let avail_phys = 0u64;
        let used_phys = 0u64;

        // Write queue addresses (split into low/high for 64-bit)
        self.write32(mmio_reg::QUEUE_DESC_LOW, desc_phys as u32);
        self.write32(mmio_reg::QUEUE_DESC_HIGH, (desc_phys >> 32) as u32);

        // For MMIO v2, avail and used ring addresses are also set.
        // We leave them as 0 (device-managed) when not explicitly allocated.
        self.write32(mmio_reg::QUEUE_AVAIL_LOW, avail_phys as u32);
        self.write32(mmio_reg::QUEUE_AVAIL_HIGH, (avail_phys >> 32) as u32);
        self.write32(mmio_reg::QUEUE_USED_LOW, used_phys as u32);
        self.write32(mmio_reg::QUEUE_USED_HIGH, (used_phys >> 32) as u32);

        // Enable queue
        self.write32(mmio_reg::QUEUE_READY, 1);

        Ok(())
    }

    /// Add an RX buffer index to the available ring.
    fn put_avail_rx(&mut self, desc_idx: u16) {
        if self.mmio_base.is_some() {
            self.rx_avail_idx = self.rx_avail_idx.wrapping_add(1);
            // Notify device that new RX buffers are available
            self.write32(mmio_reg::QUEUE_SEL, RX_QUEUE);
            self.write32(mmio_reg::QUEUE_NOTIFY, RX_QUEUE);
        } else if let Some(ref rings) = self.rx_rings {
            unsafe {
                let avail_ptr = rings.avail_virt as *mut u16;
                let avail_idx = ptr::read_volatile(avail_ptr.add(1));
                let ring_slot = avail_ptr.add(2 + (avail_idx as usize % rings.queue_size as usize));
                ptr::write_volatile(ring_slot, desc_idx);
                core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
                ptr::write_volatile(avail_ptr.add(1), avail_idx.wrapping_add(1));
                core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            }
            self.rx_avail_idx = self.rx_avail_idx.wrapping_add(1);
            // Notify device
            self.write16(pio_reg::QUEUE_NOTIFY as u32, 0); // queue 0 = RX
        }
    }

    /// Notify device that TX descriptors are ready.
    fn notify_tx(&mut self) {
        if self.mmio_base.is_some() {
            self.write32(mmio_reg::QUEUE_SEL, TX_QUEUE);
            self.write32(mmio_reg::QUEUE_NOTIFY, TX_QUEUE);
        } else {
            self.write16(pio_reg::QUEUE_NOTIFY as u32, 1); // queue 1 = TX
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
            // PIO: read ISR status register (automatically clears on read)
            self.read8(pio_reg::ISR_STATUS as u32) as u32
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
            self.write32(mmio_reg::STATUS, 0);
        } else {
            self.write8(pio_reg::DEVICE_STATUS as u32, 0);
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

        // Set up TX descriptor in our local array — addr must be PHYSICAL
        let buf_virt = self.tx_buffers[idx].as_ptr() as u64;
        let buf_phys = crate::memory::virt_to_phys(buf_virt).unwrap_or(buf_virt);
        self.tx_desc[idx] = VirtqDesc {
            addr: buf_phys,
            len: total as u32,
            flags: 0, // device reads from this buffer
            next: 0,
        };

        // For PIO mode: write descriptor to device memory and update available ring
        if let Some(ref rings) = self.tx_rings {
            unsafe {
                // Write descriptor to device-visible descriptor table (CPU access via virt)
                let desc_base = rings.desc_virt as *mut VirtqDesc;
                ptr::write_volatile(desc_base.add(idx), self.tx_desc[idx]);

                // Add to available ring (CPU access via virt)
                let avail_ptr = rings.avail_virt as *mut u16;
                let avail_idx = ptr::read_volatile(avail_ptr.add(1));
                let ring_slot =
                    avail_ptr.add(2 + (avail_idx as usize % rings.queue_size as usize));
                ptr::write_volatile(ring_slot, idx as u16);
                core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
                ptr::write_volatile(avail_ptr.add(1), avail_idx.wrapping_add(1));
                core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            }
        }

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

        // PIO mode: check the used ring for completed RX descriptors
        if let Some(ref rings) = self.rx_rings {
            let used_idx_ptr = unsafe { (rings.used_virt as *mut u16).add(1) };
            let current_used = unsafe { ptr::read_volatile(used_idx_ptr) };

            if current_used == self.rx_used_idx {
                return Err(NetworkError::RxBufferEmpty);
            }

            // Read the used ring element:  ring[used_idx % qsz] → (id: u32, len: u32)
            let used_ring_base = unsafe { (rings.used_virt as *mut u8).add(4) }; // skip flags+idx
            let elem_offset = (self.rx_used_idx as usize % rings.queue_size as usize) * 8;
            let elem_ptr = unsafe { used_ring_base.add(elem_offset) };
            let desc_id = unsafe { ptr::read_volatile(elem_ptr as *const u32) } as usize;
            let total_len = unsafe { ptr::read_volatile((elem_ptr as *const u32).add(1)) } as usize;

            if total_len <= VirtioNetHdr::SIZE || desc_id >= QUEUE_SIZE {
                self.refill_rx(desc_id % QUEUE_SIZE);
                self.rx_used_idx = self.rx_used_idx.wrapping_add(1);
                return Err(NetworkError::RxBufferEmpty);
            }

            // Strip VirtioNetHdr
            let payload_len = total_len - VirtioNetHdr::SIZE;
            let copy_len = payload_len.min(buffer.len());
            buffer[..copy_len].copy_from_slice(
                &self.rx_buffers[desc_id][VirtioNetHdr::SIZE..VirtioNetHdr::SIZE + copy_len],
            );

            // Refill this RX slot
            self.refill_rx(desc_id);

            self.rx_used_idx = self.rx_used_idx.wrapping_add(1);
            self.stats.rx_packets += 1;
            self.stats.rx_bytes += copy_len as u64;



            return Ok(copy_len);
        }

        // MMIO fallback: check descriptor directly (original logic)
        let idx = self.rx_used_idx as usize % QUEUE_SIZE;
        let desc = &self.rx_desc[idx];
        if desc.len == 0 || desc.len as usize == self.rx_buffers[idx].len() {
            return Err(NetworkError::RxBufferEmpty);
        }

        let total_len = desc.len as usize;
        if total_len <= VirtioNetHdr::SIZE {
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
        // PIO mode: check used ring
        if let Some(ref rings) = self.rx_rings {
            let used_idx_ptr = unsafe { (rings.used_virt as *mut u16).add(1) };
            let current_used = unsafe { ptr::read_volatile(used_idx_ptr) };
            return current_used != self.rx_used_idx;
        }
        // MMIO fallback: check descriptor directly
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
        if (self.features & features::STATUS) != 0 {
            if self.mmio_base.is_some() {
                let status = self.read8(mmio_reg::CONFIG + 6); // After MAC in config space
                self.link_status.up = (status & 1) != 0;
            } else {
                let status = self.read16(pio_reg::NET_STATUS as u32);
                self.link_status.up = (status & 1) != 0;
            }
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
        // Reset descriptor — addr must be PHYSICAL for DMA
        let buf_virt = self.rx_buffers[idx].as_ptr() as u64;
        let buf_phys = crate::memory::virt_to_phys(buf_virt).unwrap_or(buf_virt);
        self.rx_desc[idx] = VirtqDesc {
            addr: buf_phys,
            len: self.rx_buffers[idx].len() as u32,
            flags: VirtqDesc::WRITE,
            next: 0,
        };
        // Write updated descriptor to device-visible memory (PIO mode, via virt)
        if let Some(ref rings) = self.rx_rings {
            unsafe {
                let desc_base = rings.desc_virt as *mut VirtqDesc;
                ptr::write_volatile(desc_base.add(idx), self.rx_desc[idx]);
            }
        }
        // Notify device of refilled RX buffer
        self.put_avail_rx(idx as u16);
    }
}

/// Probe for VirtIO network devices on the PCI bus, initialize each one,
/// and register them in the `NETWORK_MANAGER`.
///
/// For each discovered device the function:
/// 1. Enables PCI bus mastering and I/O space access
/// 2. Extracts the I/O base from BAR0
/// 3. Calls `init_pio()` which performs the full VirtIO legacy init sequence
pub fn probe() {
    let network_devs = crate::driver::pci::find_virtio_network();
    if network_devs.is_empty() {
        crate::serial_println!("[VirtIO Net] No VirtIO network devices found");
        return;
    }

    crate::serial_println!(
        "[VirtIO Net] {} device(s) discovered — initializing",
        network_devs.len()
    );

    for dev in &network_devs {
        // Enable PCI bus mastering and I/O space (required for DMA + port access)
        crate::driver::pci::enable_bus_master(dev.address);
        crate::driver::pci::enable_io_space(dev.address);

        let bar0 = dev.bars[0];
        if (bar0 & 0x1) == 0 {
            crate::serial_println!(
                "[VirtIO Net] Device at {} has memory-mapped BAR0 — skipping (PIO only)",
                dev.address
            );
            continue;
        }
        let io_base = (bar0 & 0xFFFC) as u16;
        crate::serial_println!(
            "[VirtIO Net] Found NIC at {} (IO base={:#x})",
            dev.address,
            io_base
        );

        match init_pio(io_base) {
            Ok(()) => crate::serial_println!("[VirtIO Net] NIC initialized successfully"),
            Err(e) => crate::serial_println!("[VirtIO Net] NIC init failed: {:?}", e),
        }
    }
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

/// Returns `true` if at least one VirtIO NIC has been initialized and
/// registered in the `NETWORK_MANAGER`.
pub fn is_initialized() -> bool {
    NETWORK_MANAGER.lock().device_count() > 0
}

/// Returns the cumulative TX packet count across all registered NICs.
pub fn tx_packet_count() -> u64 {
    let mgr = NETWORK_MANAGER.lock();
    let mut total = 0u64;
    for dev in mgr.enumerate() {
        total += dev.stats().tx_packets;
    }
    total
}
