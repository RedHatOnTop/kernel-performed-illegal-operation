//! Intel E1000/E1000E Network Driver
//!
//! Supports Intel I219-LM, I217, I218, and other GbE controllers.

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ptr;

use super::{
    MacAddress, NetworkDevice, NetworkError, NetworkCapabilities, NetworkStats,
    LinkStatus, LinkSpeed, LinkDuplex, NETWORK_MANAGER,
};

/// E1000 register offsets
#[allow(dead_code)]
mod regs {
    pub const CTRL: u32 = 0x0000;       // Device Control
    pub const STATUS: u32 = 0x0008;     // Device Status
    pub const EECD: u32 = 0x0010;       // EEPROM/Flash Control
    pub const EERD: u32 = 0x0014;       // EEPROM Read
    pub const CTRL_EXT: u32 = 0x0018;   // Extended Device Control
    pub const MDIC: u32 = 0x0020;       // MDI Control
    pub const ICR: u32 = 0x00C0;        // Interrupt Cause Read
    pub const ICS: u32 = 0x00C8;        // Interrupt Cause Set
    pub const IMS: u32 = 0x00D0;        // Interrupt Mask Set
    pub const IMC: u32 = 0x00D8;        // Interrupt Mask Clear
    pub const RCTL: u32 = 0x0100;       // Receive Control
    pub const TCTL: u32 = 0x0400;       // Transmit Control
    pub const RDBAL: u32 = 0x2800;      // RX Descriptor Base Low
    pub const RDBAH: u32 = 0x2804;      // RX Descriptor Base High
    pub const RDLEN: u32 = 0x2808;      // RX Descriptor Length
    pub const RDH: u32 = 0x2810;        // RX Descriptor Head
    pub const RDT: u32 = 0x2818;        // RX Descriptor Tail
    pub const TDBAL: u32 = 0x3800;      // TX Descriptor Base Low
    pub const TDBAH: u32 = 0x3804;      // TX Descriptor Base High
    pub const TDLEN: u32 = 0x3808;      // TX Descriptor Length
    pub const TDH: u32 = 0x3810;        // TX Descriptor Head
    pub const TDT: u32 = 0x3818;        // TX Descriptor Tail
    pub const RAL0: u32 = 0x5400;       // Receive Address Low
    pub const RAH0: u32 = 0x5404;       // Receive Address High
    pub const MTA: u32 = 0x5200;        // Multicast Table Array
}

/// Control register bits
#[allow(dead_code)]
mod ctrl {
    pub const SLU: u32 = 1 << 6;        // Set Link Up
    pub const FRCSPD: u32 = 1 << 11;    // Force Speed
    pub const FRCDPLX: u32 = 1 << 12;   // Force Duplex
    pub const RST: u32 = 1 << 26;       // Device Reset
    pub const VME: u32 = 1 << 30;       // VLAN Mode Enable
    pub const PHY_RST: u32 = 1 << 31;   // PHY Reset
}

/// Status register bits
#[allow(dead_code)]
mod status {
    pub const LU: u32 = 1 << 1;         // Link Up
    pub const FD: u32 = 1 << 0;         // Full Duplex
    pub const SPEED_MASK: u32 = 0b11 << 6;
    pub const SPEED_10: u32 = 0b00 << 6;
    pub const SPEED_100: u32 = 0b01 << 6;
    pub const SPEED_1000: u32 = 0b10 << 6;
}

/// Receive control register bits
#[allow(dead_code)]
mod rctl {
    pub const EN: u32 = 1 << 1;         // Receiver Enable
    pub const SBP: u32 = 1 << 2;        // Store Bad Packets
    pub const UPE: u32 = 1 << 3;        // Unicast Promiscuous Enable
    pub const MPE: u32 = 1 << 4;        // Multicast Promiscuous Enable
    pub const LPE: u32 = 1 << 5;        // Long Packet Enable
    pub const BAM: u32 = 1 << 15;       // Broadcast Accept Mode
    pub const BSIZE_256: u32 = 0b11 << 16;
    pub const BSIZE_512: u32 = 0b10 << 16;
    pub const BSIZE_1024: u32 = 0b01 << 16;
    pub const BSIZE_2048: u32 = 0b00 << 16;
    pub const BSIZE_4096: u32 = (0b11 << 16) | (1 << 25);
    pub const SECRC: u32 = 1 << 26;     // Strip Ethernet CRC
}

/// Transmit control register bits
#[allow(dead_code)]
mod tctl {
    pub const EN: u32 = 1 << 1;         // Transmit Enable
    pub const PSP: u32 = 1 << 3;        // Pad Short Packets
    pub const CT_SHIFT: u32 = 4;        // Collision Threshold
    pub const COLD_SHIFT: u32 = 12;     // Collision Distance
    pub const COLD_FD: u32 = 0x40 << 12; // Full Duplex value
    pub const COLD_HD: u32 = 0x200 << 12; // Half Duplex value
}

/// Number of RX descriptors
const RX_DESC_COUNT: usize = 32;

/// Number of TX descriptors
const TX_DESC_COUNT: usize = 32;

/// Receive buffer size
const RX_BUFFER_SIZE: usize = 2048;

/// Receive descriptor
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct RxDescriptor {
    /// Buffer address
    buffer_addr: u64,
    /// Length
    length: u16,
    /// Checksum
    checksum: u16,
    /// Status
    status: u8,
    /// Errors
    errors: u8,
    /// Special
    special: u16,
}

impl RxDescriptor {
    /// Descriptor done bit
    const DD: u8 = 1 << 0;
    /// End of packet
    const EOP: u8 = 1 << 1;
}

/// Transmit descriptor
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct TxDescriptor {
    /// Buffer address
    buffer_addr: u64,
    /// Length
    length: u16,
    /// Checksum offset
    cso: u8,
    /// Command
    cmd: u8,
    /// Status
    status: u8,
    /// Checksum start
    css: u8,
    /// Special
    special: u16,
}

impl TxDescriptor {
    /// End of packet
    const EOP: u8 = 1 << 0;
    /// Insert FCS
    const IFCS: u8 = 1 << 1;
    /// Report status
    const RS: u8 = 1 << 3;
    /// Descriptor done
    const DD: u8 = 1 << 0;
}

/// E1000 device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum E1000Type {
    /// Original E1000
    E1000,
    /// E1000E (PCIe)
    E1000E,
    /// I217/I218/I219 (newer Intel GbE)
    I21x,
}

/// E1000 network driver
pub struct E1000Device {
    /// Device name
    name: String,
    /// Device type
    device_type: E1000Type,
    /// Base MMIO address
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
    /// Current RX descriptor index
    rx_cur: usize,
    /// Current TX descriptor index
    tx_cur: usize,
    /// Statistics
    stats: NetworkStats,
    /// Link status
    link_status: LinkStatus,
    /// Is up
    is_up: bool,
}

impl E1000Device {
    /// Create a new E1000 device
    pub fn new(name: &str, mmio_base: usize, device_type: E1000Type) -> Self {
        // Allocate descriptors
        let mut rx_desc = Vec::with_capacity(RX_DESC_COUNT);
        let mut tx_desc = Vec::with_capacity(TX_DESC_COUNT);
        let mut rx_buffers = Vec::with_capacity(RX_DESC_COUNT);
        let mut tx_buffers = Vec::with_capacity(TX_DESC_COUNT);

        for _ in 0..RX_DESC_COUNT {
            rx_desc.push(RxDescriptor::default());
            rx_buffers.push(alloc::vec![0u8; RX_BUFFER_SIZE]);
        }

        for _ in 0..TX_DESC_COUNT {
            tx_desc.push(TxDescriptor::default());
            tx_buffers.push(alloc::vec![0u8; RX_BUFFER_SIZE]);
        }

        Self {
            name: name.to_string(),
            device_type,
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

    /// Read register
    fn read_reg(&self, reg: u32) -> u32 {
        unsafe {
            ptr::read_volatile((self.mmio_base + reg as usize) as *const u32)
        }
    }

    /// Write register
    fn write_reg(&mut self, reg: u32, value: u32) {
        unsafe {
            ptr::write_volatile((self.mmio_base + reg as usize) as *mut u32, value);
        }
    }

    /// Read MAC address from EEPROM
    fn read_mac_from_eeprom(&mut self) -> MacAddress {
        let mut mac = [0u8; 6];

        for i in 0..3 {
            // Request EEPROM read
            self.write_reg(regs::EERD, 1 | ((i as u32) << 8));

            // Wait for read complete
            let mut timeout = 10000;
            loop {
                let val = self.read_reg(regs::EERD);
                if (val & (1 << 4)) != 0 {
                    let data = (val >> 16) as u16;
                    mac[i * 2] = data as u8;
                    mac[i * 2 + 1] = (data >> 8) as u8;
                    break;
                }
                timeout -= 1;
                if timeout == 0 {
                    break;
                }
            }
        }

        MacAddress::new(mac)
    }

    /// Read MAC address from RAL/RAH registers
    fn read_mac_from_ral(&self) -> MacAddress {
        let ral = self.read_reg(regs::RAL0);
        let rah = self.read_reg(regs::RAH0);

        MacAddress::new([
            (ral & 0xFF) as u8,
            ((ral >> 8) & 0xFF) as u8,
            ((ral >> 16) & 0xFF) as u8,
            ((ral >> 24) & 0xFF) as u8,
            (rah & 0xFF) as u8,
            ((rah >> 8) & 0xFF) as u8,
        ])
    }

    /// Reset the device
    fn reset(&mut self) {
        // Set RST bit
        let ctrl = self.read_reg(regs::CTRL);
        self.write_reg(regs::CTRL, ctrl | ctrl::RST);

        // Wait for reset to complete
        for _ in 0..1000 {
            if (self.read_reg(regs::CTRL) & ctrl::RST) == 0 {
                break;
            }
        }

        // Disable interrupts
        self.write_reg(regs::IMC, 0xFFFFFFFF);
        let _ = self.read_reg(regs::ICR);
    }

    /// Initialize RX ring
    fn init_rx(&mut self) {
        // Set up RX descriptor buffers
        for i in 0..RX_DESC_COUNT {
            self.rx_desc[i].buffer_addr = self.rx_buffers[i].as_ptr() as u64;
            self.rx_desc[i].status = 0;
        }

        // Set descriptor base address
        let desc_addr = self.rx_desc.as_ptr() as u64;
        self.write_reg(regs::RDBAL, desc_addr as u32);
        self.write_reg(regs::RDBAH, (desc_addr >> 32) as u32);

        // Set descriptor length
        self.write_reg(regs::RDLEN, (RX_DESC_COUNT * core::mem::size_of::<RxDescriptor>()) as u32);

        // Set head and tail
        self.write_reg(regs::RDH, 0);
        self.write_reg(regs::RDT, (RX_DESC_COUNT - 1) as u32);

        // Enable receiver
        let rctl = rctl::EN | rctl::BAM | rctl::BSIZE_2048 | rctl::SECRC;
        self.write_reg(regs::RCTL, rctl);

        self.rx_cur = 0;
    }

    /// Initialize TX ring
    fn init_tx(&mut self) {
        // Set up TX descriptor buffers
        for i in 0..TX_DESC_COUNT {
            self.tx_desc[i].buffer_addr = self.tx_buffers[i].as_ptr() as u64;
            self.tx_desc[i].status = TxDescriptor::DD; // Mark as done initially
            self.tx_desc[i].cmd = 0;
        }

        // Set descriptor base address
        let desc_addr = self.tx_desc.as_ptr() as u64;
        self.write_reg(regs::TDBAL, desc_addr as u32);
        self.write_reg(regs::TDBAH, (desc_addr >> 32) as u32);

        // Set descriptor length
        self.write_reg(regs::TDLEN, (TX_DESC_COUNT * core::mem::size_of::<TxDescriptor>()) as u32);

        // Set head and tail
        self.write_reg(regs::TDH, 0);
        self.write_reg(regs::TDT, 0);

        // Enable transmitter
        let tctl = tctl::EN | tctl::PSP | (15 << tctl::CT_SHIFT) | tctl::COLD_FD;
        self.write_reg(regs::TCTL, tctl);

        self.tx_cur = 0;
    }

    /// Update link status
    fn update_link_status(&mut self) {
        let status = self.read_reg(regs::STATUS);

        self.link_status.up = (status & status::LU) != 0;
        self.link_status.duplex = if (status & status::FD) != 0 {
            LinkDuplex::Full
        } else {
            LinkDuplex::Half
        };

        self.link_status.speed = match status & status::SPEED_MASK {
            status::SPEED_10 => LinkSpeed::Speed10Mbps,
            status::SPEED_100 => LinkSpeed::Speed100Mbps,
            status::SPEED_1000 => LinkSpeed::Speed1Gbps,
            _ => LinkSpeed::Unknown,
        };
    }

    /// Initialize the device
    pub fn init(&mut self) -> Result<(), NetworkError> {
        // Reset device
        self.reset();

        // Read MAC address
        self.mac = self.read_mac_from_ral();
        if self.mac.is_zero() {
            self.mac = self.read_mac_from_eeprom();
        }

        // Set link up
        let ctrl = self.read_reg(regs::CTRL);
        self.write_reg(regs::CTRL, ctrl | ctrl::SLU);

        // Clear multicast table
        for i in 0..128 {
            self.write_reg(regs::MTA + i * 4, 0);
        }

        // Initialize RX/TX rings
        self.init_rx();
        self.init_tx();

        // Update link status
        self.update_link_status();

        Ok(())
    }
}

impl NetworkDevice for E1000Device {
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
            tso: self.device_type != E1000Type::E1000,
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

        // Disable RX/TX
        self.write_reg(regs::RCTL, 0);
        self.write_reg(regs::TCTL, 0);

        self.is_up = false;
        Ok(())
    }

    fn set_mtu(&mut self, mtu: u16) -> Result<(), NetworkError> {
        if mtu > 9000 {
            return Err(NetworkError::InvalidSize);
        }
        // Would need to reconfigure buffer sizes
        Ok(())
    }

    fn transmit(&mut self, data: &[u8]) -> Result<(), NetworkError> {
        if !self.is_up {
            return Err(NetworkError::NotInitialized);
        }

        if !self.link_status.up {
            return Err(NetworkError::LinkDown);
        }

        if data.len() > RX_BUFFER_SIZE {
            return Err(NetworkError::InvalidSize);
        }

        // Check if descriptor is available
        if (self.tx_desc[self.tx_cur].status & TxDescriptor::DD) == 0 {
            return Err(NetworkError::TxBufferFull);
        }

        // Copy data to buffer
        let len = data.len();
        self.tx_buffers[self.tx_cur][..len].copy_from_slice(data);

        // Set up descriptor
        self.tx_desc[self.tx_cur].length = len as u16;
        self.tx_desc[self.tx_cur].cmd = TxDescriptor::EOP | TxDescriptor::IFCS | TxDescriptor::RS;
        self.tx_desc[self.tx_cur].status = 0;

        // Advance tail
        let old_tail = self.tx_cur;
        self.tx_cur = (self.tx_cur + 1) % TX_DESC_COUNT;
        self.write_reg(regs::TDT, self.tx_cur as u32);

        // Update stats
        self.stats.tx_packets += 1;
        self.stats.tx_bytes += len as u64;

        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, NetworkError> {
        if !self.is_up {
            return Err(NetworkError::NotInitialized);
        }

        // Check if packet available
        if (self.rx_desc[self.rx_cur].status & RxDescriptor::DD) == 0 {
            return Err(NetworkError::RxBufferEmpty);
        }

        // Copy length from packed struct safely
        let length = self.rx_desc[self.rx_cur].length as usize;
        
        if buffer.len() < length {
            return Err(NetworkError::InvalidSize);
        }

        // Copy data
        buffer[..length].copy_from_slice(&self.rx_buffers[self.rx_cur][..length]);

        // Reset descriptor
        self.rx_desc[self.rx_cur].status = 0;

        // Update tail
        let old_cur = self.rx_cur;
        self.rx_cur = (self.rx_cur + 1) % RX_DESC_COUNT;
        self.write_reg(regs::RDT, old_cur as u32);

        // Update stats
        self.stats.rx_packets += 1;
        self.stats.rx_bytes += length as u64;

        Ok(length)
    }

    fn rx_available(&self) -> bool {
        (self.rx_desc[self.rx_cur].status & RxDescriptor::DD) != 0
    }

    fn set_promiscuous(&mut self, enabled: bool) -> Result<(), NetworkError> {
        let mut rctl = self.read_reg(regs::RCTL);
        if enabled {
            rctl |= rctl::UPE | rctl::MPE;
        } else {
            rctl &= !(rctl::UPE | rctl::MPE);
        }
        self.write_reg(regs::RCTL, rctl);
        Ok(())
    }

    fn add_multicast(&mut self, _addr: MacAddress) -> Result<(), NetworkError> {
        // Would update MTA table
        Ok(())
    }

    fn remove_multicast(&mut self, _addr: MacAddress) -> Result<(), NetworkError> {
        // Would update MTA table
        Ok(())
    }

    fn poll(&mut self) -> Result<(), NetworkError> {
        // Read and clear interrupt status
        let _ = self.read_reg(regs::ICR);

        // Update link status
        self.update_link_status();

        Ok(())
    }
}

/// Probe for E1000 devices
pub fn probe() {
    // In a real implementation, this would scan PCI bus for:
    // - Vendor ID: 0x8086 (Intel)
    // - Device IDs: 0x100E (E1000), 0x10D3 (E1000E), 0x153A/0x15B8 (I219), etc.
}

/// Initialize E1000 device at given MMIO address
pub fn init_device(mmio_base: usize, device_type: E1000Type) -> Result<(), NetworkError> {
    let name = {
        let manager = NETWORK_MANAGER.lock();
        alloc::format!("eth{}", manager.device_count())
    };

    let mut device = E1000Device::new(&name, mmio_base, device_type);
    device.init()?;

    NETWORK_MANAGER.lock().register(Box::new(device));

    Ok(())
}
