//! Realtek RTL8111/RTL8168 Gigabit Ethernet Driver
//!
//! Supports common Realtek NICs found in many desktops and laptops.

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ptr;

use super::{
    MacAddress, NetworkDevice, NetworkError, NetworkCapabilities, NetworkStats,
    LinkStatus, LinkSpeed, LinkDuplex, NETWORK_MANAGER,
};

/// RTL8111 register offsets
#[allow(dead_code)]
mod regs {
    pub const MAC0: u32 = 0x00;         // MAC address bytes 0-3
    pub const MAC4: u32 = 0x04;         // MAC address bytes 4-5
    pub const MAR0: u32 = 0x08;         // Multicast filter 0-3
    pub const MAR4: u32 = 0x0C;         // Multicast filter 4-7
    pub const TNPDS: u32 = 0x20;        // TX Normal Priority Descriptors
    pub const THPDS: u32 = 0x28;        // TX High Priority Descriptors
    pub const CR: u32 = 0x37;           // Command Register
    pub const TPP: u32 = 0x38;          // TX Priority Polling
    pub const IMR: u32 = 0x3C;          // Interrupt Mask Register
    pub const ISR: u32 = 0x3E;          // Interrupt Status Register
    pub const TCR: u32 = 0x40;          // TX Configuration Register
    pub const RCR: u32 = 0x44;          // RX Configuration Register
    pub const TCTR: u32 = 0x48;         // Timer Count Register
    pub const MPC: u32 = 0x4C;          // Missed Packet Counter
    pub const CR9346: u32 = 0x50;       // 93C46 Command Register
    pub const CONFIG0: u32 = 0x51;      // Configuration Register 0
    pub const CONFIG1: u32 = 0x52;      // Configuration Register 1
    pub const CONFIG2: u32 = 0x53;      // Configuration Register 2
    pub const CONFIG3: u32 = 0x54;      // Configuration Register 3
    pub const CONFIG4: u32 = 0x55;      // Configuration Register 4
    pub const CONFIG5: u32 = 0x56;      // Configuration Register 5
    pub const PHYAR: u32 = 0x60;        // PHY Access Register
    pub const PHY_STATUS: u32 = 0x6C;   // PHY Status
    pub const RMS: u32 = 0xDA;          // RX Max Size
    pub const CPCR: u32 = 0xE0;         // C+ Mode Command Register
    pub const RDSAR: u32 = 0xE4;        // RX Descriptor Start Address
    pub const MTPS: u32 = 0xEC;         // Max TX Packet Size
}

/// Command register bits
#[allow(dead_code)]
mod cr {
    pub const RST: u8 = 1 << 4;         // Reset
    pub const RE: u8 = 1 << 3;          // Receiver Enable
    pub const TE: u8 = 1 << 2;          // Transmitter Enable
}

/// PHY status bits
#[allow(dead_code)]
mod phy_status {
    pub const LINK: u8 = 1 << 1;        // Link Status
    pub const FULL_DUPLEX: u8 = 1 << 0; // Full Duplex
    pub const SPEED_1000: u8 = 1 << 4;  // 1000 Mbps
    pub const SPEED_100: u8 = 1 << 3;   // 100 Mbps
    pub const SPEED_10: u8 = 1 << 2;    // 10 Mbps
}

/// Interrupt bits
#[allow(dead_code)]
mod intr {
    pub const ROK: u16 = 1 << 0;        // Receive OK
    pub const RER: u16 = 1 << 1;        // Receive Error
    pub const TOK: u16 = 1 << 2;        // Transmit OK
    pub const TER: u16 = 1 << 3;        // Transmit Error
    pub const RDU: u16 = 1 << 4;        // RX Descriptor Unavailable
    pub const LINK: u16 = 1 << 5;       // Link Change
    pub const FOVW: u16 = 1 << 6;       // RX FIFO Overflow
    pub const TDU: u16 = 1 << 7;        // TX Descriptor Unavailable
    pub const SW: u16 = 1 << 8;         // Software Interrupt
    pub const TIMEOUT: u16 = 1 << 14;   // Timer Interrupt
}

/// RX configuration bits
#[allow(dead_code)]
mod rcr {
    pub const AAP: u32 = 1 << 0;        // Accept All Packets (promisc)
    pub const APM: u32 = 1 << 1;        // Accept Physical Match
    pub const AM: u32 = 1 << 2;         // Accept Multicast
    pub const AB: u32 = 1 << 3;         // Accept Broadcast
    pub const AR: u32 = 1 << 4;         // Accept Runt
    pub const AER: u32 = 1 << 5;        // Accept Error Packet
    pub const MXDMA_UNLIMITED: u32 = 7 << 8;  // Unlimited DMA burst
    pub const RXFTH_NONE: u32 = 7 << 13;      // No RX FIFO threshold
}

/// Number of RX/TX descriptors
const DESC_COUNT: usize = 64;

/// Buffer size
const BUFFER_SIZE: usize = 2048;

/// RX descriptor (C+ mode)
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct RxDescriptor {
    /// Flags and length
    opts1: u32,
    /// VLAN info
    opts2: u32,
    /// Buffer address low
    buf_lo: u32,
    /// Buffer address high
    buf_hi: u32,
}

impl RxDescriptor {
    /// Owned by NIC
    const OWN: u32 = 1 << 31;
    /// End of ring
    const EOR: u32 = 1 << 30;
    /// First segment
    const FS: u32 = 1 << 29;
    /// Last segment
    const LS: u32 = 1 << 28;
    /// Length mask
    const LEN_MASK: u32 = 0x3FFF;
}

/// TX descriptor (C+ mode)
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct TxDescriptor {
    /// Flags and length
    opts1: u32,
    /// VLAN and other options
    opts2: u32,
    /// Buffer address low
    buf_lo: u32,
    /// Buffer address high
    buf_hi: u32,
}

impl TxDescriptor {
    /// Owned by NIC
    const OWN: u32 = 1 << 31;
    /// End of ring
    const EOR: u32 = 1 << 30;
    /// First segment
    const FS: u32 = 1 << 29;
    /// Last segment
    const LS: u32 = 1 << 28;
}

/// RTL8111 device
pub struct Rtl8111Device {
    /// Device name
    name: String,
    /// MMIO base address
    mmio_base: usize,
    /// MAC address
    mac: MacAddress,
    /// RX descriptors
    rx_desc: Vec<RxDescriptor>,
    /// TX descriptors
    tx_desc: Vec<TxDescriptor>,
    /// RX buffers
    rx_buffers: Vec<Vec<u8>>,
    /// TX buffers
    tx_buffers: Vec<Vec<u8>>,
    /// Current RX index
    rx_cur: usize,
    /// Current TX index
    tx_cur: usize,
    /// Statistics
    stats: NetworkStats,
    /// Link status
    link_status: LinkStatus,
    /// Is up
    is_up: bool,
}

impl Rtl8111Device {
    /// Create new device
    pub fn new(name: &str, mmio_base: usize) -> Self {
        let mut rx_desc = Vec::with_capacity(DESC_COUNT);
        let mut tx_desc = Vec::with_capacity(DESC_COUNT);
        let mut rx_buffers = Vec::with_capacity(DESC_COUNT);
        let mut tx_buffers = Vec::with_capacity(DESC_COUNT);

        for _ in 0..DESC_COUNT {
            rx_desc.push(RxDescriptor::default());
            tx_desc.push(TxDescriptor::default());
            rx_buffers.push(alloc::vec![0u8; BUFFER_SIZE]);
            tx_buffers.push(alloc::vec![0u8; BUFFER_SIZE]);
        }

        Self {
            name: name.to_string(),
            mmio_base,
            mac: MacAddress::ZERO,
            rx_desc,
            tx_desc,
            rx_buffers,
            tx_buffers,
            rx_cur: 0,
            tx_cur: 0,
            stats: NetworkStats::default(),
            link_status: LinkStatus::default(),
            is_up: false,
        }
    }

    /// Read 8-bit register
    fn read8(&self, reg: u32) -> u8 {
        unsafe {
            ptr::read_volatile((self.mmio_base + reg as usize) as *const u8)
        }
    }

    /// Write 8-bit register
    fn write8(&mut self, reg: u32, val: u8) {
        unsafe {
            ptr::write_volatile((self.mmio_base + reg as usize) as *mut u8, val);
        }
    }

    /// Read 16-bit register
    fn read16(&self, reg: u32) -> u16 {
        unsafe {
            ptr::read_volatile((self.mmio_base + reg as usize) as *const u16)
        }
    }

    /// Write 16-bit register
    fn write16(&mut self, reg: u32, val: u16) {
        unsafe {
            ptr::write_volatile((self.mmio_base + reg as usize) as *mut u16, val);
        }
    }

    /// Read 32-bit register
    fn read32(&self, reg: u32) -> u32 {
        unsafe {
            ptr::read_volatile((self.mmio_base + reg as usize) as *const u32)
        }
    }

    /// Write 32-bit register
    fn write32(&mut self, reg: u32, val: u32) {
        unsafe {
            ptr::write_volatile((self.mmio_base + reg as usize) as *mut u32, val);
        }
    }

    /// Unlock config registers
    fn unlock_config(&mut self) {
        self.write8(regs::CR9346, 0xC0);
    }

    /// Lock config registers
    fn lock_config(&mut self) {
        self.write8(regs::CR9346, 0x00);
    }

    /// Reset the device
    fn reset(&mut self) {
        self.write8(regs::CR, cr::RST);
        
        // Wait for reset to complete
        for _ in 0..1000 {
            if (self.read8(regs::CR) & cr::RST) == 0 {
                break;
            }
        }
    }

    /// Read MAC address
    fn read_mac(&mut self) -> MacAddress {
        let mac0 = self.read32(regs::MAC0);
        let mac4 = self.read16(regs::MAC4);

        MacAddress::new([
            (mac0 & 0xFF) as u8,
            ((mac0 >> 8) & 0xFF) as u8,
            ((mac0 >> 16) & 0xFF) as u8,
            ((mac0 >> 24) & 0xFF) as u8,
            (mac4 & 0xFF) as u8,
            ((mac4 >> 8) & 0xFF) as u8,
        ])
    }

    /// Initialize RX ring
    fn init_rx(&mut self) {
        for i in 0..DESC_COUNT {
            let buf_addr = self.rx_buffers[i].as_ptr() as u64;
            let mut opts = RxDescriptor::OWN | (BUFFER_SIZE as u32);
            if i == DESC_COUNT - 1 {
                opts |= RxDescriptor::EOR;
            }
            self.rx_desc[i].opts1 = opts;
            self.rx_desc[i].opts2 = 0;
            self.rx_desc[i].buf_lo = buf_addr as u32;
            self.rx_desc[i].buf_hi = (buf_addr >> 32) as u32;
        }

        // Set descriptor base address
        let desc_addr = self.rx_desc.as_ptr() as u64;
        self.write32(regs::RDSAR, desc_addr as u32);
        self.write32(regs::RDSAR + 4, (desc_addr >> 32) as u32);

        self.rx_cur = 0;
    }

    /// Initialize TX ring
    fn init_tx(&mut self) {
        for i in 0..DESC_COUNT {
            let buf_addr = self.tx_buffers[i].as_ptr() as u64;
            let mut opts: u32 = 0;
            if i == DESC_COUNT - 1 {
                opts |= TxDescriptor::EOR;
            }
            self.tx_desc[i].opts1 = opts;
            self.tx_desc[i].opts2 = 0;
            self.tx_desc[i].buf_lo = buf_addr as u32;
            self.tx_desc[i].buf_hi = (buf_addr >> 32) as u32;
        }

        // Set descriptor base address
        let desc_addr = self.tx_desc.as_ptr() as u64;
        self.write32(regs::TNPDS, desc_addr as u32);
        self.write32(regs::TNPDS + 4, (desc_addr >> 32) as u32);

        self.tx_cur = 0;
    }

    /// Update link status from PHY
    fn update_link_status(&mut self) {
        let status = self.read8(regs::PHY_STATUS);

        self.link_status.up = (status & phy_status::LINK) != 0;
        self.link_status.duplex = if (status & phy_status::FULL_DUPLEX) != 0 {
            LinkDuplex::Full
        } else {
            LinkDuplex::Half
        };

        if (status & phy_status::SPEED_1000) != 0 {
            self.link_status.speed = LinkSpeed::Speed1Gbps;
        } else if (status & phy_status::SPEED_100) != 0 {
            self.link_status.speed = LinkSpeed::Speed100Mbps;
        } else if (status & phy_status::SPEED_10) != 0 {
            self.link_status.speed = LinkSpeed::Speed10Mbps;
        } else {
            self.link_status.speed = LinkSpeed::Unknown;
        }
    }

    /// Initialize device
    pub fn init(&mut self) -> Result<(), NetworkError> {
        // Reset device
        self.reset();

        // Read MAC address
        self.mac = self.read_mac();

        // Unlock config registers
        self.unlock_config();

        // Initialize RX/TX rings
        self.init_rx();
        self.init_tx();

        // Configure RX
        let rcr = rcr::APM | rcr::AM | rcr::AB | rcr::MXDMA_UNLIMITED | rcr::RXFTH_NONE;
        self.write32(regs::RCR, rcr);

        // Set max packet size
        self.write16(regs::RMS, BUFFER_SIZE as u16);
        self.write8(regs::MTPS, 0x3B); // Max TX size

        // Enable C+ mode
        self.write16(regs::CPCR, 0x00E0);

        // Enable interrupts (for polling, we'll mask them)
        self.write16(regs::IMR, 0);

        // Enable TX/RX
        self.write8(regs::CR, cr::RE | cr::TE);

        // Lock config
        self.lock_config();

        // Update link
        self.update_link_status();

        Ok(())
    }
}

impl NetworkDevice for Rtl8111Device {
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
            tx_checksum: true,
            rx_checksum: true,
            tso: true,
            lro: false,
            scatter_gather: true,
            vlan: true,
            mtu: 1500,
            max_tx_queues: 1,
            max_rx_queues: 1,
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

        // Disable TX/RX
        self.write8(regs::CR, 0);
        self.is_up = false;
        Ok(())
    }

    fn set_mtu(&mut self, mtu: u16) -> Result<(), NetworkError> {
        if mtu > 9000 {
            return Err(NetworkError::InvalidSize);
        }
        // Would reconfigure buffers
        Ok(())
    }

    fn transmit(&mut self, data: &[u8]) -> Result<(), NetworkError> {
        if !self.is_up {
            return Err(NetworkError::NotInitialized);
        }

        if !self.link_status.up {
            return Err(NetworkError::LinkDown);
        }

        if data.len() > BUFFER_SIZE {
            return Err(NetworkError::InvalidSize);
        }

        // Check if descriptor available
        let opts1 = self.tx_desc[self.tx_cur].opts1;
        if (opts1 & TxDescriptor::OWN) != 0 {
            return Err(NetworkError::TxBufferFull);
        }

        // Copy data
        let len = data.len();
        self.tx_buffers[self.tx_cur][..len].copy_from_slice(data);

        // Set up descriptor
        let mut opts = TxDescriptor::OWN | TxDescriptor::FS | TxDescriptor::LS | (len as u32);
        if self.tx_cur == DESC_COUNT - 1 {
            opts |= TxDescriptor::EOR;
        }
        self.tx_desc[self.tx_cur].opts1 = opts;

        // Trigger TX
        self.write8(regs::TPP, 0x40);

        // Advance
        self.tx_cur = (self.tx_cur + 1) % DESC_COUNT;

        self.stats.tx_packets += 1;
        self.stats.tx_bytes += len as u64;

        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, NetworkError> {
        if !self.is_up {
            return Err(NetworkError::NotInitialized);
        }

        // Check if packet available
        let opts1 = self.rx_desc[self.rx_cur].opts1;
        if (opts1 & RxDescriptor::OWN) != 0 {
            return Err(NetworkError::RxBufferEmpty);
        }

        let len = (opts1 & RxDescriptor::LEN_MASK) as usize;
        if buffer.len() < len {
            return Err(NetworkError::InvalidSize);
        }

        // Copy data
        buffer[..len].copy_from_slice(&self.rx_buffers[self.rx_cur][..len]);

        // Reset descriptor
        let mut opts = RxDescriptor::OWN | (BUFFER_SIZE as u32);
        if self.rx_cur == DESC_COUNT - 1 {
            opts |= RxDescriptor::EOR;
        }
        self.rx_desc[self.rx_cur].opts1 = opts;

        self.rx_cur = (self.rx_cur + 1) % DESC_COUNT;

        self.stats.rx_packets += 1;
        self.stats.rx_bytes += len as u64;

        Ok(len)
    }

    fn rx_available(&self) -> bool {
        (self.rx_desc[self.rx_cur].opts1 & RxDescriptor::OWN) == 0
    }

    fn set_promiscuous(&mut self, enabled: bool) -> Result<(), NetworkError> {
        let mut rcr_val = self.read32(regs::RCR);
        if enabled {
            rcr_val |= rcr::AAP;
        } else {
            rcr_val &= !rcr::AAP;
        }
        self.write32(regs::RCR, rcr_val);
        Ok(())
    }

    fn add_multicast(&mut self, _addr: MacAddress) -> Result<(), NetworkError> {
        // Would update MAR registers
        Ok(())
    }

    fn remove_multicast(&mut self, _addr: MacAddress) -> Result<(), NetworkError> {
        Ok(())
    }

    fn poll(&mut self) -> Result<(), NetworkError> {
        // Read and clear interrupt status
        let _isr = self.read16(regs::ISR);
        self.write16(regs::ISR, 0xFFFF);

        self.update_link_status();
        Ok(())
    }
}

/// Probe for RTL8111 devices
pub fn probe() {
    // Scan PCI for:
    // - Vendor ID: 0x10EC (Realtek)
    // - Device IDs: 0x8168, 0x8136, etc.
}

/// Initialize RTL8111 device at MMIO address
pub fn init_device(mmio_base: usize) -> Result<(), NetworkError> {
    let name = {
        let manager = NETWORK_MANAGER.lock();
        alloc::format!("eth{}", manager.device_count())
    };

    let mut device = Rtl8111Device::new(&name, mmio_base);
    device.init()?;

    NETWORK_MANAGER.lock().register(Box::new(device));

    Ok(())
}
