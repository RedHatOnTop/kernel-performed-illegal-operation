//! Intel E1000 network driver.
//!
//! This module implements the Intel E1000 (82540EM) network device driver.

use super::NetworkDevice;
use crate::{MacAddr, NetworkError};

/// E1000 register offsets.
mod regs {
    pub const CTRL: u32 = 0x0000;
    pub const STATUS: u32 = 0x0008;
    pub const EECD: u32 = 0x0010;
    pub const EERD: u32 = 0x0014;
    pub const ICR: u32 = 0x00C0;
    pub const IMS: u32 = 0x00D0;
    pub const IMC: u32 = 0x00D8;
    pub const RCTL: u32 = 0x0100;
    pub const TCTL: u32 = 0x0400;
    pub const RDBAL: u32 = 0x2800;
    pub const RDBAH: u32 = 0x2804;
    pub const RDLEN: u32 = 0x2808;
    pub const RDH: u32 = 0x2810;
    pub const RDT: u32 = 0x2818;
    pub const TDBAL: u32 = 0x3800;
    pub const TDBAH: u32 = 0x3804;
    pub const TDLEN: u32 = 0x3808;
    pub const TDH: u32 = 0x3810;
    pub const TDT: u32 = 0x3818;
    pub const RAL: u32 = 0x5400;
    pub const RAH: u32 = 0x5404;
}

/// E1000 control register flags.
mod ctrl {
    pub const SLU: u32 = 1 << 6;
    pub const RST: u32 = 1 << 26;
}

/// E1000 receive control flags.
mod rctl {
    pub const EN: u32 = 1 << 1;
    pub const BAM: u32 = 1 << 15;
    pub const BSIZE_4096: u32 = 3 << 16;
    pub const SECRC: u32 = 1 << 26;
}

/// E1000 transmit control flags.
mod tctl {
    pub const EN: u32 = 1 << 1;
    pub const PSP: u32 = 1 << 3;
}

/// E1000 network device.
pub struct E1000 {
    /// Base MMIO address.
    base: u64,
    /// MAC address.
    mac: MacAddr,
    /// MTU.
    mtu: u16,
    /// Link status.
    link_up: bool,
}

impl E1000 {
    /// Create a new E1000 device.
    pub fn new(base: u64) -> Result<Self, NetworkError> {
        let mut dev = E1000 {
            base,
            mac: MacAddr::new(0, 0, 0, 0, 0, 0),
            mtu: 1500,
            link_up: false,
        };
        
        dev.reset()?;
        dev.read_mac()?;
        dev.init_rx()?;
        dev.init_tx()?;
        dev.link_up = dev.check_link();
        
        Ok(dev)
    }
    
    /// Reset the device.
    fn reset(&mut self) -> Result<(), NetworkError> {
        self.write_reg(regs::CTRL, ctrl::RST);
        // Wait for reset to complete
        for _ in 0..1000 {
            if self.read_reg(regs::CTRL) & ctrl::RST == 0 {
                return Ok(());
            }
        }
        Err(NetworkError::DriverError("E1000 reset timeout".into()))
    }
    
    /// Read the MAC address.
    fn read_mac(&mut self) -> Result<(), NetworkError> {
        let ral = self.read_reg(regs::RAL);
        let rah = self.read_reg(regs::RAH);
        
        self.mac = MacAddr::new(
            (ral & 0xFF) as u8,
            ((ral >> 8) & 0xFF) as u8,
            ((ral >> 16) & 0xFF) as u8,
            ((ral >> 24) & 0xFF) as u8,
            (rah & 0xFF) as u8,
            ((rah >> 8) & 0xFF) as u8,
        );
        
        Ok(())
    }
    
    /// Initialize receive descriptors.
    fn init_rx(&mut self) -> Result<(), NetworkError> {
        // Would allocate and set up RX descriptor ring
        self.write_reg(regs::RCTL, rctl::EN | rctl::BAM | rctl::BSIZE_4096 | rctl::SECRC);
        Ok(())
    }
    
    /// Initialize transmit descriptors.
    fn init_tx(&mut self) -> Result<(), NetworkError> {
        // Would allocate and set up TX descriptor ring
        self.write_reg(regs::TCTL, tctl::EN | tctl::PSP);
        Ok(())
    }
    
    /// Check link status.
    fn check_link(&self) -> bool {
        let status = self.read_reg(regs::STATUS);
        (status & 0x02) != 0 // Link up bit
    }
    
    /// Read a register.
    fn read_reg(&self, offset: u32) -> u32 {
        unsafe {
            let ptr = (self.base + offset as u64) as *const u32;
            ptr.read_volatile()
        }
    }
    
    /// Write a register.
    fn write_reg(&self, offset: u32, value: u32) {
        unsafe {
            let ptr = (self.base + offset as u64) as *mut u32;
            ptr.write_volatile(value);
        }
    }
}

impl NetworkDevice for E1000 {
    fn mac_address(&self) -> MacAddr {
        self.mac
    }
    
    fn mtu(&self) -> u16 {
        self.mtu
    }
    
    fn link_up(&self) -> bool {
        self.link_up
    }
    
    fn transmit(&mut self, _packet: &[u8]) -> Result<(), NetworkError> {
        // Would add to TX descriptor ring and notify hardware
        Ok(())
    }
    
    fn receive(&mut self, _buffer: &mut [u8]) -> Result<usize, NetworkError> {
        // Would check RX descriptor ring for received packets
        Err(NetworkError::WouldBlock)
    }
    
    fn can_receive(&self) -> bool {
        false // Placeholder
    }
    
    fn link_speed(&self) -> u32 {
        1000 // 1 Gbps
    }
}

/// Probe for E1000 devices.
pub fn probe() -> Result<(), NetworkError> {
    // Would scan PCI bus for E1000 devices (vendor 0x8086, device 0x100E)
    Ok(())
}
