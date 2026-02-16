//! AHCI (SATA) Driver
//!
//! Advanced Host Controller Interface driver for SATA devices.

use super::{BlockDevice, StorageError, StorageInfo, StorageInterface, StorageManager};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ptr;
use spin::Mutex;

/// AHCI Generic Host Control registers
#[repr(C)]
pub struct AhciHba {
    /// Host Capabilities
    pub cap: u32,
    /// Global Host Control
    pub ghc: u32,
    /// Interrupt Status
    pub is: u32,
    /// Ports Implemented
    pub pi: u32,
    /// Version
    pub vs: u32,
    /// Command Completion Coalescing Control
    pub ccc_ctl: u32,
    /// Command Completion Coalescing Ports
    pub ccc_ports: u32,
    /// Enclosure Management Location
    pub em_loc: u32,
    /// Enclosure Management Control
    pub em_ctl: u32,
    /// Host Capabilities Extended
    pub cap2: u32,
    /// BIOS/OS Handoff Control and Status
    pub bohc: u32,
    /// Reserved
    pub _reserved: [u8; 0xA0 - 0x2C],
    /// Vendor Specific
    pub vendor: [u8; 0x100 - 0xA0],
    /// Port registers (up to 32 ports)
    pub ports: [AhciPort; 32],
}

/// AHCI Port registers
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AhciPort {
    /// Command List Base Address (lower 32 bits)
    pub clb: u32,
    /// Command List Base Address (upper 32 bits)
    pub clbu: u32,
    /// FIS Base Address (lower 32 bits)
    pub fb: u32,
    /// FIS Base Address (upper 32 bits)
    pub fbu: u32,
    /// Interrupt Status
    pub is: u32,
    /// Interrupt Enable
    pub ie: u32,
    /// Command and Status
    pub cmd: u32,
    /// Reserved
    pub _reserved0: u32,
    /// Task File Data
    pub tfd: u32,
    /// Signature
    pub sig: u32,
    /// SATA Status
    pub ssts: u32,
    /// SATA Control
    pub sctl: u32,
    /// SATA Error
    pub serr: u32,
    /// SATA Active
    pub sact: u32,
    /// Command Issue
    pub ci: u32,
    /// SATA Notification
    pub sntf: u32,
    /// FIS-based Switching Control
    pub fbs: u32,
    /// Device Sleep
    pub devslp: u32,
    /// Reserved
    pub _reserved1: [u8; 0x70 - 0x48],
    /// Vendor Specific
    pub vendor: [u8; 0x80 - 0x70],
}

/// Port signature values
pub mod port_sig {
    pub const SATA_ATA: u32 = 0x00000101;
    pub const SATA_ATAPI: u32 = 0xEB140101;
    pub const SATA_SEMB: u32 = 0xC33C0101; // Enclosure Management Bridge
    pub const SATA_PM: u32 = 0x96690101; // Port Multiplier
}

/// AHCI Command Header
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct AhciCommandHeader {
    /// Description Information (PRDTL, PMP, CFL, etc.)
    pub dw0: u32,
    /// PRD Byte Count
    pub prdbc: u32,
    /// Command Table Base Address (lower)
    pub ctba: u32,
    /// Command Table Base Address (upper)
    pub ctbau: u32,
    /// Reserved
    pub _reserved: [u32; 4],
}

impl AhciCommandHeader {
    /// Create a new command header
    pub fn new(cfl: u8, atapi: bool, write: bool, prefetch: bool, prdtl: u16) -> Self {
        let mut dw0 = (cfl & 0x1F) as u32;
        if atapi {
            dw0 |= 1 << 5;
        }
        if write {
            dw0 |= 1 << 6;
        }
        if prefetch {
            dw0 |= 1 << 7;
        }
        dw0 |= (prdtl as u32) << 16;

        Self {
            dw0,
            prdbc: 0,
            ctba: 0,
            ctbau: 0,
            _reserved: [0; 4],
        }
    }
}

/// AHCI Physical Region Descriptor
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct AhciPrd {
    /// Data Base Address (lower)
    pub dba: u32,
    /// Data Base Address (upper)
    pub dbau: u32,
    /// Reserved
    pub _reserved: u32,
    /// Data Byte Count and Interrupt on Completion
    pub dbc_i: u32,
}

/// AHCI Command Table
#[repr(C)]
pub struct AhciCommandTable {
    /// Command FIS
    pub cfis: [u8; 64],
    /// ATAPI Command
    pub acmd: [u8; 16],
    /// Reserved
    pub _reserved: [u8; 48],
    /// Physical Region Descriptor Table
    pub prdt: [AhciPrd; 65535],
}

/// FIS (Frame Information Structure) types
#[repr(u8)]
pub enum FisType {
    RegH2D = 0x27,      // Register FIS - Host to Device
    RegD2H = 0x34,      // Register FIS - Device to Host
    DmaActivate = 0x39, // DMA Activate FIS
    DmaSetup = 0x41,    // DMA Setup FIS
    Data = 0x46,        // Data FIS
    Bist = 0x58,        // BIST Activate FIS
    PioSetup = 0x5F,    // PIO Setup FIS
    DevBits = 0xA1,     // Set Device Bits FIS
}

/// Register FIS - Host to Device
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FisRegH2D {
    pub fis_type: u8,
    pub pm_port_c: u8, // Port multiplier and command bit
    pub command: u8,
    pub feature_low: u8,

    pub lba0: u8,
    pub lba1: u8,
    pub lba2: u8,
    pub device: u8,

    pub lba3: u8,
    pub lba4: u8,
    pub lba5: u8,
    pub feature_high: u8,

    pub count_low: u8,
    pub count_high: u8,
    pub icc: u8,
    pub control: u8,

    pub _reserved: [u8; 4],
}

impl FisRegH2D {
    /// Create a new H2D FIS
    pub fn new_command(command: u8) -> Self {
        Self {
            fis_type: FisType::RegH2D as u8,
            pm_port_c: 0x80, // Command bit set
            command,
            device: 0xE0, // LBA mode
            ..Default::default()
        }
    }

    /// Set LBA address
    pub fn set_lba(&mut self, lba: u64) {
        self.lba0 = (lba & 0xFF) as u8;
        self.lba1 = ((lba >> 8) & 0xFF) as u8;
        self.lba2 = ((lba >> 16) & 0xFF) as u8;
        self.lba3 = ((lba >> 24) & 0xFF) as u8;
        self.lba4 = ((lba >> 32) & 0xFF) as u8;
        self.lba5 = ((lba >> 40) & 0xFF) as u8;
        self.device = 0x40; // LBA mode
    }

    /// Set sector count
    pub fn set_count(&mut self, count: u16) {
        self.count_low = (count & 0xFF) as u8;
        self.count_high = ((count >> 8) & 0xFF) as u8;
    }
}

/// ATA Commands
pub mod ata_cmd {
    pub const IDENTIFY_DEVICE: u8 = 0xEC;
    pub const IDENTIFY_PACKET_DEVICE: u8 = 0xA1;
    pub const READ_DMA_EXT: u8 = 0x25;
    pub const WRITE_DMA_EXT: u8 = 0x35;
    pub const READ_FPDMA_QUEUED: u8 = 0x60;
    pub const WRITE_FPDMA_QUEUED: u8 = 0x61;
    pub const FLUSH_CACHE: u8 = 0xE7;
    pub const FLUSH_CACHE_EXT: u8 = 0xEA;
    pub const DATA_SET_MANAGEMENT: u8 = 0x06;
}

/// ATA Identify Device data
#[repr(C)]
#[derive(Clone)]
pub struct AtaIdentify {
    /// General configuration
    pub general_config: u16,
    /// Obsolete
    pub _obsolete1: u16,
    /// Specific configuration
    pub specific_config: u16,
    /// Obsolete
    pub _obsolete2: u16,
    /// Retired
    pub _retired1: [u16; 2],
    /// Obsolete
    pub _obsolete3: u16,
    /// Reserved for CompactFlash
    pub _reserved_cf: [u16; 2],
    /// Retired
    pub _retired2: u16,
    /// Serial number (20 ASCII chars)
    pub serial: [u8; 20],
    /// Retired
    pub _retired3: [u16; 2],
    /// Obsolete
    pub _obsolete4: u16,
    /// Firmware revision (8 ASCII chars)
    pub firmware: [u8; 8],
    /// Model number (40 ASCII chars)
    pub model: [u8; 40],
    /// Maximum sectors per interrupt on READ/WRITE MULTIPLE
    pub max_sectors_multiple: u16,
    /// Trusted Computing feature set options
    pub trusted_computing: u16,
    /// Capabilities
    pub capabilities: [u16; 2],
    /// Obsolete
    pub _obsolete5: [u16; 2],
    /// Free-fall control sensitivity
    pub free_fall: u16,
    /// Obsolete
    pub _obsolete6: [u16; 5],
    /// Current sectors per interrupt on READ/WRITE MULTIPLE
    pub current_sectors_multiple: u16,
    /// Total sectors (28-bit LBA)
    pub total_sectors_28: u32,
    /// Obsolete
    pub _obsolete7: u16,
    /// Multiword DMA modes
    pub multiword_dma: u16,
    /// PIO modes supported
    pub pio_modes: u16,
    /// Minimum Multiword DMA cycle time
    pub min_mwdma_cycle: u16,
    /// Recommended Multiword DMA cycle time
    pub rec_mwdma_cycle: u16,
    /// Minimum PIO cycle time without IORDY
    pub min_pio_cycle_no_iordy: u16,
    /// Minimum PIO cycle time with IORDY
    pub min_pio_cycle_iordy: u16,
    /// Additional supported
    pub additional_supported: u16,
    /// Reserved
    pub _reserved1: [u16; 5],
    /// Queue depth
    pub queue_depth: u16,
    /// SATA capabilities
    pub sata_capabilities: [u16; 2],
    /// SATA features supported
    pub sata_features_supported: u16,
    /// SATA features enabled
    pub sata_features_enabled: u16,
    /// Major version
    pub major_version: u16,
    /// Minor version
    pub minor_version: u16,
    /// Command sets supported
    pub command_sets: [u16; 3],
    /// Command sets enabled
    pub command_sets_enabled: [u16; 3],
    /// Ultra DMA modes
    pub udma_modes: u16,
    /// Time for SECURITY ERASE
    pub security_erase_time: u16,
    /// Time for ENHANCED SECURITY ERASE
    pub enhanced_security_erase_time: u16,
    /// Current APM value
    pub current_apm: u16,
    /// Master password revision
    pub master_password_rev: u16,
    /// Hardware reset result
    pub hardware_reset_result: u16,
    /// Acoustic management value
    pub acoustic_management: u16,
    /// Stream minimum request size
    pub stream_min_request: u16,
    /// Stream transfer time DMA
    pub stream_transfer_time_dma: u16,
    /// Stream access latency
    pub stream_access_latency: u16,
    /// Stream performance granularity
    pub stream_performance_granularity: [u16; 2],
    /// Total sectors (48-bit LBA)
    pub total_sectors_48: u64,
    /// Stream transfer time PIO
    pub stream_transfer_time_pio: u16,
    /// Max data set management blocks
    pub max_dsm_blocks: u16,
    /// Physical/logical sector size
    pub sector_size: u16,
    /// Inter-seek delay
    pub inter_seek_delay: u16,
    /// World Wide Name
    pub wwn: [u16; 4],
    /// Reserved
    pub _reserved2: [u16; 5],
    /// Logical sector size
    pub logical_sector_size: u32,
    /// Command sets supported ext
    pub command_sets_ext: u16,
    /// Command sets enabled ext
    pub command_sets_enabled_ext: u16,
    /// Reserved
    pub _reserved3: [u16; 6],
    /// Obsolete
    pub _obsolete8: u16,
    /// Security status
    pub security_status: u16,
    /// Vendor specific
    pub vendor_specific: [u16; 31],
    /// CFA power mode
    pub cfa_power_mode: u16,
    /// Reserved for CompactFlash
    pub _reserved_cf2: [u16; 7],
    /// Device nominal form factor
    pub form_factor: u16,
    /// Data set management support
    pub dsm_support: u16,
    /// Additional product identifier
    pub additional_product_id: [u16; 4],
    /// Reserved
    pub _reserved4: [u16; 2],
    /// Current media serial number
    pub media_serial: [u8; 60],
    /// SCT command transport
    pub sct_command_transport: u16,
    /// Reserved
    pub _reserved5: [u16; 2],
    /// Logical sector alignment
    pub sector_alignment: u16,
    /// Write-read-verify sector count mode 3
    pub wrv_sector_count_mode3: u32,
    /// Write-read-verify sector count mode 2
    pub wrv_sector_count_mode2: u32,
    /// Obsolete
    pub _obsolete9: [u16; 3],
    /// Nominal media rotation rate
    pub rotation_rate: u16,
    /// Reserved
    pub _reserved6: u16,
    /// Obsolete
    pub _obsolete10: u16,
    /// Write-read-verify feature set current mode
    pub wrv_current_mode: u16,
    /// Reserved
    pub _reserved7: u16,
    /// Transport major version
    pub transport_major: u16,
    /// Transport minor version
    pub transport_minor: u16,
    /// Reserved
    pub _reserved8: [u16; 6],
    /// Extended number of user addressable sectors
    pub extended_sectors: u64,
    /// Minimum blocks per DOWNLOAD MICROCODE
    pub min_microcode_blocks: u16,
    /// Maximum blocks per DOWNLOAD MICROCODE
    pub max_microcode_blocks: u16,
    /// Reserved
    pub _reserved9: [u16; 19],
    /// Integrity word
    pub integrity: u16,
}

/// AHCI Drive (SATA device)
pub struct AhciDrive {
    /// Controller index
    controller_id: u8,
    /// Port number
    port: u8,
    /// Device name
    name: String,
    /// Block size
    block_size: u32,
    /// Total blocks
    block_count: u64,
    /// Is ATAPI device
    is_atapi: bool,
    /// NCQ queue depth
    queue_depth: u8,
    /// Is SSD (rotation rate = 1)
    is_ssd: bool,
}

impl BlockDevice for AhciDrive {
    fn name(&self) -> &str {
        &self.name
    }

    fn size(&self) -> u64 {
        self.block_count * self.block_size as u64
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn read_blocks(
        &self,
        start_block: u64,
        count: u32,
        buffer: &mut [u8],
    ) -> Result<(), StorageError> {
        let expected_size = count as usize * self.block_size as usize;
        if buffer.len() < expected_size {
            return Err(StorageError::BufferTooSmall);
        }

        if start_block + count as u64 > self.block_count {
            return Err(StorageError::InvalidAddress);
        }

        // In real implementation: build FIS and submit command
        Err(StorageError::NotSupported)
    }

    fn write_blocks(
        &self,
        start_block: u64,
        count: u32,
        buffer: &[u8],
    ) -> Result<(), StorageError> {
        let expected_size = count as usize * self.block_size as usize;
        if buffer.len() < expected_size {
            return Err(StorageError::BufferTooSmall);
        }

        if start_block + count as u64 > self.block_count {
            return Err(StorageError::InvalidAddress);
        }

        Err(StorageError::NotSupported)
    }

    fn flush(&self) -> Result<(), StorageError> {
        // Submit FLUSH CACHE command
        Err(StorageError::NotSupported)
    }

    fn supports_trim(&self) -> bool {
        self.is_ssd
    }
}

/// AHCI Controller
pub struct AhciController {
    /// MMIO base address
    mmio_base: u64,
    /// Controller ID
    id: u8,
    /// Number of ports
    num_ports: u8,
    /// Ports implemented bitmap
    ports_implemented: u32,
    /// Supports NCQ
    ncq_support: bool,
    /// Supports 64-bit addressing
    addr64_support: bool,
}

/// Probe for AHCI controllers
pub fn probe(manager: &mut StorageManager) {
    // Find AHCI controllers via PCI
    // Class 0x01 (Storage), Subclass 0x06 (SATA), ProgIF 0x01 (AHCI)

    // In real implementation:
    // 1. Find AHCI controller in PCI config space
    // 2. Map BAR5 (ABAR) for HBA memory registers
    // 3. Take ownership from BIOS (BOHC register)
    // 4. Enable AHCI mode
    // 5. Scan ports for connected devices
    // 6. Issue IDENTIFY to each device
    // 7. Create AhciDrive and register
}

/// Initialize AHCI controller
pub unsafe fn init_controller(mmio_base: u64) -> Result<AhciController, &'static str> {
    let hba = mmio_base as *mut AhciHba;

    // Read capabilities
    let cap = unsafe { (*hba).cap };
    let num_ports = ((cap & 0x1F) + 1) as u8;
    let ncq_support = (cap & (1 << 30)) != 0;
    let addr64_support = (cap & (1 << 31)) != 0;

    // Read ports implemented
    let pi = unsafe { (*hba).pi };

    // Enable AHCI mode
    let ghc = unsafe { (*hba).ghc };
    if (ghc & (1 << 31)) == 0 {
        unsafe { (*hba).ghc = ghc | (1 << 31) }; // Set AE (AHCI Enable)
    }

    Ok(AhciController {
        mmio_base,
        id: 0,
        num_ports,
        ports_implemented: pi,
        ncq_support,
        addr64_support,
    })
}

/// Check what type of device is connected to a port
pub unsafe fn check_port_type(port: &AhciPort) -> Option<&'static str> {
    let ssts = port.ssts;
    let det = ssts & 0xF; // Device detection
    let ipm = (ssts >> 8) & 0xF; // Interface power management

    // Check if device present and PHY communication established
    if det != 3 || ipm != 1 {
        return None;
    }

    match port.sig {
        port_sig::SATA_ATA => Some("SATA"),
        port_sig::SATA_ATAPI => Some("ATAPI"),
        port_sig::SATA_SEMB => Some("SEMB"),
        port_sig::SATA_PM => Some("Port Multiplier"),
        _ => Some("Unknown"),
    }
}

/// Parse ATA identify string (byte-swapped)
pub fn parse_ata_string(data: &[u8]) -> String {
    let mut result = String::with_capacity(data.len());
    for chunk in data.chunks(2) {
        if chunk.len() == 2 {
            if chunk[1] != 0 && chunk[1] != b' ' {
                result.push(chunk[1] as char);
            }
            if chunk[0] != 0 && chunk[0] != b' ' {
                result.push(chunk[0] as char);
            }
        }
    }
    result.trim().to_string()
}
