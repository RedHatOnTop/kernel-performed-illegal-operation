//! AHCI (Advanced Host Controller Interface) driver.
//!
//! This module provides support for SATA devices through the AHCI interface.

use super::BlockDevice;
use crate::{BlockDeviceInfo, StorageError};

/// AHCI generic host control registers.
#[derive(Debug)]
#[repr(C)]
pub struct AhciHostRegs {
    /// Host capabilities.
    pub cap: u32,
    /// Global host control.
    pub ghc: u32,
    /// Interrupt status.
    pub is: u32,
    /// Ports implemented.
    pub pi: u32,
    /// Version.
    pub vs: u32,
    /// Command completion coalescing control.
    pub ccc_ctl: u32,
    /// Command completion coalescing ports.
    pub ccc_ports: u32,
    /// Enclosure management location.
    pub em_loc: u32,
    /// Enclosure management control.
    pub em_ctl: u32,
    /// Host capabilities extended.
    pub cap2: u32,
    /// BIOS/OS handoff control and status.
    pub bohc: u32,
}

/// AHCI port registers.
#[derive(Debug)]
#[repr(C)]
pub struct AhciPortRegs {
    /// Command list base address (low).
    pub clb: u32,
    /// Command list base address (high).
    pub clbu: u32,
    /// FIS base address (low).
    pub fb: u32,
    /// FIS base address (high).
    pub fbu: u32,
    /// Interrupt status.
    pub is: u32,
    /// Interrupt enable.
    pub ie: u32,
    /// Command and status.
    pub cmd: u32,
    /// Reserved.
    pub reserved0: u32,
    /// Task file data.
    pub tfd: u32,
    /// Signature.
    pub sig: u32,
    /// Serial ATA status.
    pub ssts: u32,
    /// Serial ATA control.
    pub sctl: u32,
    /// Serial ATA error.
    pub serr: u32,
    /// Serial ATA active.
    pub sact: u32,
    /// Command issue.
    pub ci: u32,
    /// Serial ATA notification.
    pub sntf: u32,
    /// FIS-based switching control.
    pub fbs: u32,
    /// Device sleep.
    pub devslp: u32,
    /// Reserved.
    pub reserved1: [u32; 10],
    /// Vendor specific.
    pub vendor: [u32; 4],
}

/// AHCI command header.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AhciCommandHeader {
    /// Description information.
    pub flags: u16,
    /// Physical region descriptor table length.
    pub prdtl: u16,
    /// Physical region descriptor byte count.
    pub prdbc: u32,
    /// Command table descriptor base address (low).
    pub ctba: u32,
    /// Command table descriptor base address (high).
    pub ctbau: u32,
    /// Reserved.
    pub reserved: [u32; 4],
}

impl AhciCommandHeader {
    /// Command header size in bytes.
    pub const SIZE: usize = 32;

    /// Set command FIS length (in dwords).
    pub fn set_cfl(&mut self, len: u8) {
        self.flags = (self.flags & !0x1F) | ((len & 0x1F) as u16);
    }

    /// Set ATAPI flag.
    pub fn set_atapi(&mut self, atapi: bool) {
        if atapi {
            self.flags |= 1 << 5;
        } else {
            self.flags &= !(1 << 5);
        }
    }

    /// Set write flag.
    pub fn set_write(&mut self, write: bool) {
        if write {
            self.flags |= 1 << 6;
        } else {
            self.flags &= !(1 << 6);
        }
    }

    /// Set prefetchable flag.
    pub fn set_prefetchable(&mut self, prefetch: bool) {
        if prefetch {
            self.flags |= 1 << 7;
        } else {
            self.flags &= !(1 << 7);
        }
    }

    /// Set clear busy upon R_OK.
    pub fn set_clear_busy(&mut self, clear: bool) {
        if clear {
            self.flags |= 1 << 10;
        } else {
            self.flags &= !(1 << 10);
        }
    }
}

/// AHCI PRDT (Physical Region Descriptor Table) entry.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AhciPrdtEntry {
    /// Data base address (low).
    pub dba: u32,
    /// Data base address (high).
    pub dbau: u32,
    /// Reserved.
    pub reserved: u32,
    /// Description information.
    pub dbc: u32,
}

impl AhciPrdtEntry {
    /// PRDT entry size in bytes.
    pub const SIZE: usize = 16;

    /// Maximum bytes per PRDT entry (4MB - 2).
    pub const MAX_BYTES: u32 = 0x3FFFFF;

    /// Set the byte count (must be even, set bit 0 for interrupt).
    pub fn set_byte_count(&mut self, count: u32, interrupt: bool) {
        self.dbc = (count - 1) & Self::MAX_BYTES;
        if interrupt {
            self.dbc |= 1 << 31;
        }
    }
}

/// AHCI command table.
#[repr(C)]
pub struct AhciCommandTable {
    /// Command FIS.
    pub cfis: [u8; 64],
    /// ATAPI command.
    pub acmd: [u8; 16],
    /// Reserved.
    pub reserved: [u8; 48],
    /// Physical region descriptor table entries.
    pub prdt: [AhciPrdtEntry; 8],
}

/// FIS (Frame Information Structure) types.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum FisType {
    /// Register FIS - host to device.
    RegH2D = 0x27,
    /// Register FIS - device to host.
    RegD2H = 0x34,
    /// DMA activate FIS - device to host.
    DmaActivate = 0x39,
    /// DMA setup FIS - bidirectional.
    DmaSetup = 0x41,
    /// Data FIS - bidirectional.
    Data = 0x46,
    /// BIST activate FIS - bidirectional.
    Bist = 0x58,
    /// PIO setup FIS - device to host.
    PioSetup = 0x5F,
    /// Set device bits FIS - device to host.
    SetDeviceBits = 0xA1,
}

/// Register FIS - Host to Device.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
pub struct FisRegH2D {
    /// FIS type.
    pub fis_type: u8,
    /// Flags (port multiplier, command/control).
    pub flags: u8,
    /// Command register.
    pub command: u8,
    /// Feature register, 7:0.
    pub featurel: u8,
    /// LBA low register, 7:0.
    pub lba0: u8,
    /// LBA mid register, 15:8.
    pub lba1: u8,
    /// LBA high register, 23:16.
    pub lba2: u8,
    /// Device register.
    pub device: u8,
    /// LBA register, 31:24.
    pub lba3: u8,
    /// LBA register, 39:32.
    pub lba4: u8,
    /// LBA register, 47:40.
    pub lba5: u8,
    /// Feature register, 15:8.
    pub featureh: u8,
    /// Count register, 7:0.
    pub countl: u8,
    /// Count register, 15:8.
    pub counth: u8,
    /// Isochronous command completion.
    pub icc: u8,
    /// Control register.
    pub control: u8,
    /// Reserved.
    pub reserved: [u8; 4],
}

impl FisRegH2D {
    /// FIS size in bytes.
    pub const SIZE: usize = 20;

    /// Create a new register FIS.
    pub fn new() -> Self {
        FisRegH2D {
            fis_type: FisType::RegH2D as u8,
            flags: 0x80, // Command bit set
            ..Default::default()
        }
    }

    /// Set LBA48 address.
    pub fn set_lba(&mut self, lba: u64) {
        self.lba0 = (lba >> 0) as u8;
        self.lba1 = (lba >> 8) as u8;
        self.lba2 = (lba >> 16) as u8;
        self.lba3 = (lba >> 24) as u8;
        self.lba4 = (lba >> 32) as u8;
        self.lba5 = (lba >> 40) as u8;
        self.device = 0x40; // LBA mode
    }

    /// Set sector count.
    pub fn set_count(&mut self, count: u16) {
        self.countl = (count & 0xFF) as u8;
        self.counth = ((count >> 8) & 0xFF) as u8;
    }
}

/// ATA commands.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum AtaCommand {
    /// Read sectors.
    ReadSectors = 0x20,
    /// Read sectors (extended).
    ReadSectorsExt = 0x24,
    /// Read DMA.
    ReadDma = 0xC8,
    /// Read DMA (extended).
    ReadDmaExt = 0x25,
    /// Write sectors.
    WriteSectors = 0x30,
    /// Write sectors (extended).
    WriteSectorsExt = 0x34,
    /// Write DMA.
    WriteDma = 0xCA,
    /// Write DMA (extended).
    WriteDmaExt = 0x35,
    /// Identify device.
    IdentifyDevice = 0xEC,
    /// Identify packet device.
    IdentifyPacketDevice = 0xA1,
    /// Flush cache.
    FlushCache = 0xE7,
    /// Flush cache (extended).
    FlushCacheExt = 0xEA,
    /// Data set management (TRIM).
    DataSetManagement = 0x06,
}

/// AHCI port device type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AhciDeviceType {
    /// No device.
    None,
    /// SATA drive.
    Sata,
    /// SATAPI drive (e.g., CD/DVD).
    Satapi,
    /// Enclosure management bridge.
    Semb,
    /// Port multiplier.
    Pm,
}

/// AHCI port.
pub struct AhciPort {
    /// Port number.
    port_num: u8,
    /// Device type.
    device_type: AhciDeviceType,
    /// Port registers.
    regs: *mut AhciPortRegs,
    /// Command list.
    cmd_list: *mut AhciCommandHeader,
    /// Command tables.
    cmd_tables: [*mut AhciCommandTable; 32],
    /// Total sectors.
    total_sectors: u64,
    /// Sector size.
    sector_size: u32,
    /// Is initialized.
    initialized: bool,
}

// SAFETY: AhciPort access is synchronized through the storage subsystem.
unsafe impl Send for AhciPort {}
unsafe impl Sync for AhciPort {}

impl AhciPort {
    /// SATA signature.
    pub const SATA_SIG_ATA: u32 = 0x00000101;
    /// SATAPI signature.
    pub const SATA_SIG_ATAPI: u32 = 0xEB140101;
    /// SEMB signature.
    pub const SATA_SIG_SEMB: u32 = 0xC33C0101;
    /// PM signature.
    pub const SATA_SIG_PM: u32 = 0x96690101;

    /// Create a new AHCI port.
    pub fn new(port_num: u8, regs: *mut AhciPortRegs) -> Self {
        AhciPort {
            port_num,
            device_type: AhciDeviceType::None,
            regs,
            cmd_list: core::ptr::null_mut(),
            cmd_tables: [core::ptr::null_mut(); 32],
            total_sectors: 0,
            sector_size: 512,
            initialized: false,
        }
    }

    /// Detect device type.
    pub fn detect_device_type(&self) -> AhciDeviceType {
        unsafe {
            let ssts = core::ptr::read_volatile(&(*self.regs).ssts);
            let det = ssts & 0x0F;
            let ipm = (ssts >> 8) & 0x0F;

            // Check if device is present and active
            if det != 3 || ipm != 1 {
                return AhciDeviceType::None;
            }

            let sig = core::ptr::read_volatile(&(*self.regs).sig);
            match sig {
                Self::SATA_SIG_ATA => AhciDeviceType::Sata,
                Self::SATA_SIG_ATAPI => AhciDeviceType::Satapi,
                Self::SATA_SIG_SEMB => AhciDeviceType::Semb,
                Self::SATA_SIG_PM => AhciDeviceType::Pm,
                _ => AhciDeviceType::None,
            }
        }
    }

    /// Initialize the port.
    pub fn init(&mut self) -> Result<(), StorageError> {
        self.device_type = self.detect_device_type();

        if self.device_type == AhciDeviceType::None {
            return Err(StorageError::DeviceNotFound);
        }

        // TODO: Allocate command list and FIS buffer
        // TODO: Start command engine
        // TODO: Send IDENTIFY command

        self.initialized = true;
        Ok(())
    }

    /// Stop command engine.
    fn stop_cmd(&mut self) -> Result<(), StorageError> {
        unsafe {
            // Clear ST (bit 0)
            let mut cmd = core::ptr::read_volatile(&(*self.regs).cmd);
            cmd &= !(1 << 0);
            core::ptr::write_volatile(&mut (*self.regs).cmd, cmd);

            // Wait for CR (bit 15) to clear
            for _ in 0..1000000 {
                let cmd = core::ptr::read_volatile(&(*self.regs).cmd);
                if cmd & (1 << 15) == 0 {
                    break;
                }
            }

            // Clear FRE (bit 4)
            cmd = core::ptr::read_volatile(&(*self.regs).cmd);
            cmd &= !(1 << 4);
            core::ptr::write_volatile(&mut (*self.regs).cmd, cmd);

            // Wait for FR (bit 14) to clear
            for _ in 0..1000000 {
                let cmd = core::ptr::read_volatile(&(*self.regs).cmd);
                if cmd & (1 << 14) == 0 {
                    return Ok(());
                }
            }
        }

        Err(StorageError::IoError)
    }

    /// Start command engine.
    fn start_cmd(&mut self) -> Result<(), StorageError> {
        unsafe {
            // Wait for CR (bit 15) to clear
            for _ in 0..1000000 {
                let cmd = core::ptr::read_volatile(&(*self.regs).cmd);
                if cmd & (1 << 15) == 0 {
                    break;
                }
            }

            // Set FRE (bit 4)
            let mut cmd = core::ptr::read_volatile(&(*self.regs).cmd);
            cmd |= 1 << 4;
            core::ptr::write_volatile(&mut (*self.regs).cmd, cmd);

            // Set ST (bit 0)
            cmd |= 1 << 0;
            core::ptr::write_volatile(&mut (*self.regs).cmd, cmd);
        }

        Ok(())
    }

    /// Read sectors.
    fn read_sectors(&self, lba: u64, count: u16, buffer: &mut [u8]) -> Result<(), StorageError> {
        if !self.initialized {
            return Err(StorageError::NotReady);
        }

        let _ = lba;
        let _ = count;
        let _ = buffer;

        // TODO: Build read command and submit
        Ok(())
    }

    /// Write sectors.
    fn write_sectors(&self, lba: u64, count: u16, data: &[u8]) -> Result<(), StorageError> {
        if !self.initialized {
            return Err(StorageError::NotReady);
        }

        let _ = lba;
        let _ = count;
        let _ = data;

        // TODO: Build write command and submit
        Ok(())
    }
}

impl BlockDevice for AhciPort {
    fn info(&self) -> BlockDeviceInfo {
        let mut name = [0u8; 32];
        name[..4].copy_from_slice(b"ahci");
        name[4] = b'0' + self.port_num;

        BlockDeviceInfo {
            name,
            name_len: 5,
            block_size: self.sector_size,
            total_blocks: self.total_sectors,
            read_only: false,
            supports_trim: true,
            optimal_io_size: 256, // 128KB with 512B sectors
            physical_block_size: self.sector_size,
        }
    }

    fn read_blocks(&self, start_block: u64, buffer: &mut [u8]) -> Result<usize, StorageError> {
        let sectors = buffer.len() / self.sector_size as usize;
        self.read_sectors(start_block, sectors as u16, buffer)?;
        Ok(buffer.len())
    }

    fn write_blocks(&self, start_block: u64, data: &[u8]) -> Result<usize, StorageError> {
        let sectors = data.len() / self.sector_size as usize;
        self.write_sectors(start_block, sectors as u16, data)?;
        Ok(data.len())
    }

    fn flush(&self) -> Result<(), StorageError> {
        // TODO: Send FLUSH CACHE command
        Ok(())
    }

    fn discard(&self, _start_block: u64, _num_blocks: u64) -> Result<(), StorageError> {
        // TODO: Send DATA SET MANAGEMENT command
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.initialized
    }
}

/// AHCI controller.
pub struct AhciController {
    /// Base address.
    base_addr: usize,
    /// Host registers.
    host_regs: *mut AhciHostRegs,
    /// Ports.
    ports: [Option<AhciPort>; 32],
    /// Number of ports.
    num_ports: u8,
}

impl AhciController {
    /// Create a new AHCI controller.
    pub fn new(base_addr: usize) -> Self {
        AhciController {
            base_addr,
            host_regs: base_addr as *mut AhciHostRegs,
            ports: Default::default(),
            num_ports: 0,
        }
    }

    /// Initialize the controller.
    pub fn init(&mut self) -> Result<(), StorageError> {
        unsafe {
            // Read capabilities
            let cap = core::ptr::read_volatile(&(*self.host_regs).cap);
            self.num_ports = ((cap & 0x1F) + 1) as u8;

            // Enable AHCI mode
            let mut ghc = core::ptr::read_volatile(&(*self.host_regs).ghc);
            ghc |= 1 << 31; // AHCI enable
            core::ptr::write_volatile(&mut (*self.host_regs).ghc, ghc);

            // Read ports implemented
            let pi = core::ptr::read_volatile(&(*self.host_regs).pi);

            // Initialize implemented ports
            for i in 0..32 {
                if pi & (1 << i) != 0 {
                    let port_base = self.base_addr + 0x100 + (i * 0x80);
                    let mut port = AhciPort::new(i as u8, port_base as *mut AhciPortRegs);

                    if port.detect_device_type() != AhciDeviceType::None {
                        if port.init().is_ok() {
                            self.ports[i] = Some(port);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Probe for AHCI controllers.
pub fn probe() -> Result<(), StorageError> {
    // TODO: Scan PCI for AHCI controllers (class 01, subclass 06, prog-if 01)
    Ok(())
}
