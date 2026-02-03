//! NVMe Driver
//!
//! Non-Volatile Memory Express driver for NVMe SSDs.

#![allow(clippy::while_immutable_condition)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::boxed::Box;
use alloc::format;
use core::ptr;
use spin::Mutex;
use super::{BlockDevice, StorageError, StorageInfo, StorageInterface, StorageManager};

/// NVMe Controller Registers (BAR0 memory-mapped)
#[repr(C)]
pub struct NvmeRegisters {
    /// Controller Capabilities
    pub cap: u64,
    /// Version
    pub vs: u32,
    /// Interrupt Mask Set
    pub intms: u32,
    /// Interrupt Mask Clear
    pub intmc: u32,
    /// Controller Configuration
    pub cc: u32,
    /// Reserved
    pub _reserved1: u32,
    /// Controller Status
    pub csts: u32,
    /// NVM Subsystem Reset
    pub nssr: u32,
    /// Admin Queue Attributes
    pub aqa: u32,
    /// Admin Submission Queue Base Address
    pub asq: u64,
    /// Admin Completion Queue Base Address
    pub acq: u64,
    /// Controller Memory Buffer Location
    pub cmbloc: u32,
    /// Controller Memory Buffer Size
    pub cmbsz: u32,
    /// Boot Partition Info
    pub bpinfo: u32,
    /// Boot Partition Read Select
    pub bprsel: u32,
    /// Boot Partition Memory Buffer Location
    pub bpmbl: u64,
    /// Controller Memory Buffer Memory Space Control
    pub cmbmsc: u64,
    /// Controller Memory Buffer Status
    pub cmbsts: u32,
    /// Persistent Memory Capabilities
    pub pmrcap: u32,
    /// Persistent Memory Region Control
    pub pmrctl: u32,
    /// Persistent Memory Region Status
    pub pmrsts: u32,
    /// Persistent Memory Region Elasticity Buffer Size
    pub pmrebs: u32,
    /// Persistent Memory Region Sustained Write Throughput
    pub pmrswtp: u32,
    /// Persistent Memory Region Controller Memory Space Control
    pub pmrmsc: u64,
    /// Reserved
    pub _reserved2: [u8; 0xE00 - 0x58],
    /// Submission Queue 0 Tail Doorbell
    pub sq0tdbl: u32,
    /// Completion Queue 0 Head Doorbell
    pub cq0hdbl: u32,
}

/// NVMe Submission Queue Entry (Command)
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct NvmeCommand {
    /// Command Dword 0 (opcode, fuse, command id)
    pub cdw0: u32,
    /// Namespace ID
    pub nsid: u32,
    /// Reserved
    pub cdw2: u32,
    /// Reserved
    pub cdw3: u32,
    /// Metadata Pointer
    pub mptr: u64,
    /// Data Pointer PRP1
    pub prp1: u64,
    /// Data Pointer PRP2 (or PRP list pointer)
    pub prp2: u64,
    /// Command specific DWORDs 10-15
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
}

/// NVMe Completion Queue Entry
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct NvmeCompletion {
    /// Command specific
    pub cdw0: u32,
    /// Reserved
    pub cdw1: u32,
    /// Submission Queue Head Pointer
    pub sq_head: u16,
    /// Submission Queue Identifier
    pub sq_id: u16,
    /// Command Identifier
    pub cid: u16,
    /// Phase bit and Status Field
    pub status: u16,
}

impl NvmeCompletion {
    /// Check if this is a new completion (phase bit matches)
    pub fn is_valid(&self, expected_phase: bool) -> bool {
        ((self.status & 1) != 0) == expected_phase
    }

    /// Get status code
    pub fn status_code(&self) -> u8 {
        ((self.status >> 1) & 0xFF) as u8
    }

    /// Get status code type
    pub fn status_code_type(&self) -> u8 {
        ((self.status >> 9) & 0x7) as u8
    }

    /// Check if command succeeded
    pub fn succeeded(&self) -> bool {
        self.status_code() == 0 && self.status_code_type() == 0
    }
}

/// NVMe Admin Opcodes
#[repr(u8)]
pub enum AdminOpcode {
    DeleteSubmissionQueue = 0x00,
    CreateSubmissionQueue = 0x01,
    GetLogPage = 0x02,
    DeleteCompletionQueue = 0x04,
    CreateCompletionQueue = 0x05,
    Identify = 0x06,
    Abort = 0x08,
    SetFeatures = 0x09,
    GetFeatures = 0x0A,
    AsyncEventRequest = 0x0C,
    NamespaceManagement = 0x0D,
    FirmwareActivate = 0x10,
    FirmwareDownload = 0x11,
    NamespaceAttachment = 0x15,
    FormatNvm = 0x80,
    SecuritySend = 0x81,
    SecurityReceive = 0x82,
}

/// NVMe I/O Opcodes
#[repr(u8)]
pub enum IoOpcode {
    Flush = 0x00,
    Write = 0x01,
    Read = 0x02,
    WriteUncorrectable = 0x04,
    Compare = 0x05,
    WriteZeroes = 0x08,
    DatasetManagement = 0x09,
    Verify = 0x0C,
    ReservationRegister = 0x0D,
    ReservationReport = 0x0E,
    ReservationAcquire = 0x11,
    ReservationRelease = 0x15,
}

/// Identify Controller data structure
#[repr(C)]
#[derive(Clone)]
pub struct IdentifyController {
    /// PCI Vendor ID
    pub vid: u16,
    /// PCI Subsystem Vendor ID
    pub ssvid: u16,
    /// Serial Number
    pub sn: [u8; 20],
    /// Model Number
    pub mn: [u8; 40],
    /// Firmware Revision
    pub fr: [u8; 8],
    /// Recommended Arbitration Burst
    pub rab: u8,
    /// IEEE OUI Identifier
    pub ieee: [u8; 3],
    /// Controller Multi-Path I/O and Namespace Sharing
    pub cmic: u8,
    /// Maximum Data Transfer Size
    pub mdts: u8,
    /// Controller ID
    pub cntlid: u16,
    /// Version
    pub ver: u32,
    /// RTD3 Resume Latency
    pub rtd3r: u32,
    /// RTD3 Entry Latency
    pub rtd3e: u32,
    /// Optional Async Events Supported
    pub oaes: u32,
    /// Controller Attributes
    pub ctratt: u32,
    /// Reserved/Extended
    pub _reserved: [u8; 4096 - 0x100],
}

/// Identify Namespace data structure
#[repr(C)]
#[derive(Clone)]
pub struct IdentifyNamespace {
    /// Namespace Size (in logical blocks)
    pub nsze: u64,
    /// Namespace Capacity
    pub ncap: u64,
    /// Namespace Utilization
    pub nuse: u64,
    /// Namespace Features
    pub nsfeat: u8,
    /// Number of LBA Formats
    pub nlbaf: u8,
    /// Formatted LBA Size
    pub flbas: u8,
    /// Metadata Capabilities
    pub mc: u8,
    /// End-to-End Data Protection Capabilities
    pub dpc: u8,
    /// End-to-End Data Protection Type Settings
    pub dps: u8,
    /// Namespace Multi-path I/O and Namespace Sharing
    pub nmic: u8,
    /// Reservation Capabilities
    pub rescap: u8,
    /// Format Progress Indicator
    pub fpi: u8,
    /// Deallocate Logical Block Features
    pub dlfeat: u8,
    /// Namespace Atomic Write Unit Normal
    pub nawun: u16,
    /// Namespace Atomic Write Unit Power Fail
    pub nawupf: u16,
    /// Namespace Atomic Compare & Write Unit
    pub nacwu: u16,
    /// Namespace Atomic Boundary Size Normal
    pub nabsn: u16,
    /// Namespace Atomic Boundary Offset
    pub nabo: u16,
    /// Namespace Atomic Boundary Size Power Fail
    pub nabspf: u16,
    /// Namespace Optimal I/O Boundary
    pub noiob: u16,
    /// NVM Capacity
    pub nvmcap: [u8; 16],
    /// Reserved
    pub _reserved1: [u8; 40],
    /// Namespace Globally Unique Identifier
    pub nguid: [u8; 16],
    /// IEEE Extended Unique Identifier
    pub eui64: u64,
    /// LBA Formats
    pub lbaf: [LbaFormat; 16],
    /// Reserved
    pub _reserved2: [u8; 4096 - 0xC0],
}

/// LBA Format
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LbaFormat {
    /// Metadata Size
    pub ms: u16,
    /// LBA Data Size (power of 2)
    pub lbads: u8,
    /// Relative Performance
    pub rp: u8,
}

/// NVMe Queue
pub struct NvmeQueue {
    /// Submission queue entries
    sq_base: u64,
    /// Completion queue entries
    cq_base: u64,
    /// Queue depth
    depth: u16,
    /// Submission queue tail
    sq_tail: u16,
    /// Completion queue head
    cq_head: u16,
    /// Phase bit for completion checking
    cq_phase: bool,
    /// Current command ID
    cid: u16,
}

impl NvmeQueue {
    /// Create a new queue
    pub fn new(sq_base: u64, cq_base: u64, depth: u16) -> Self {
        Self {
            sq_base,
            cq_base,
            depth,
            sq_tail: 0,
            cq_head: 0,
            cq_phase: true,
            cid: 0,
        }
    }
}

/// NVMe Controller
pub struct NvmeController {
    /// Memory-mapped registers base
    regs: u64,
    /// Controller ID
    id: u8,
    /// Admin queue
    admin_queue: Mutex<NvmeQueue>,
    /// I/O queues
    io_queues: Vec<Mutex<NvmeQueue>>,
    /// Controller info
    info: IdentifyController,
    /// Maximum transfer size in bytes
    max_transfer_size: u32,
    /// Stride between doorbells (in u32s)
    doorbell_stride: u32,
}

/// NVMe Namespace (represents a single drive/partition)
pub struct NvmeNamespace {
    /// Controller reference
    controller_id: u8,
    /// Namespace ID
    nsid: u32,
    /// Block size
    block_size: u32,
    /// Total blocks
    block_count: u64,
    /// Device name
    name: String,
}

impl BlockDevice for NvmeNamespace {
    fn name(&self) -> &str {
        &self.name
    }

    fn size(&self) -> u64 {
        self.block_count * self.block_size as u64
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn read_blocks(&self, start_block: u64, count: u32, buffer: &mut [u8]) -> Result<(), StorageError> {
        let expected_size = count as usize * self.block_size as usize;
        if buffer.len() < expected_size {
            return Err(StorageError::BufferTooSmall);
        }

        if start_block + count as u64 > self.block_count {
            return Err(StorageError::InvalidAddress);
        }

        // In real implementation: submit read command to controller
        // For now, return error as controller not fully implemented
        Err(StorageError::NotSupported)
    }

    fn write_blocks(&self, start_block: u64, count: u32, buffer: &[u8]) -> Result<(), StorageError> {
        let expected_size = count as usize * self.block_size as usize;
        if buffer.len() < expected_size {
            return Err(StorageError::BufferTooSmall);
        }

        if start_block + count as u64 > self.block_count {
            return Err(StorageError::InvalidAddress);
        }

        // In real implementation: submit write command to controller
        Err(StorageError::NotSupported)
    }

    fn flush(&self) -> Result<(), StorageError> {
        // In real implementation: submit flush command
        Ok(())
    }

    fn supports_trim(&self) -> bool {
        true // NVMe SSDs typically support TRIM
    }

    fn trim(&self, start_block: u64, count: u64) -> Result<(), StorageError> {
        if start_block + count > self.block_count {
            return Err(StorageError::InvalidAddress);
        }
        // In real implementation: submit Dataset Management command with Deallocate
        Err(StorageError::NotSupported)
    }
}

/// Probe for NVMe controllers
pub fn probe(manager: &mut StorageManager) {
    // Find NVMe controllers via PCI enumeration
    // Class 0x01 (Storage), Subclass 0x08 (NVMe), ProgIF 0x02 (NVM Express)
    
    // In real implementation:
    // 1. Enumerate PCI for NVMe controllers
    // 2. Map BAR0 to get controller registers
    // 3. Initialize controller (reset, configure queues)
    // 4. Identify controller and namespaces
    // 5. Create NvmeNamespace for each namespace
    // 6. Register with storage manager
}

/// Initialize an NVMe controller at the given MMIO base
pub unsafe fn init_controller(mmio_base: u64) -> Result<NvmeController, &'static str> {
    let regs = mmio_base as *mut NvmeRegisters;
    
    // Read capabilities
    let cap = unsafe { (*regs).cap };
    let mqes = (cap & 0xFFFF) as u16;          // Maximum Queue Entries Supported
    let cqr = ((cap >> 16) & 1) != 0;          // Contiguous Queues Required
    let _ams = ((cap >> 17) & 3) as u8;        // Arbitration Mechanism Supported
    let _to = ((cap >> 24) & 0xFF) as u8;      // Timeout (500ms units)
    let dstrd = ((cap >> 32) & 0xF) as u32;    // Doorbell Stride
    let mpsmin = ((cap >> 48) & 0xF) as u8;    // Memory Page Size Minimum
    let mpsmax = ((cap >> 52) & 0xF) as u8;    // Memory Page Size Maximum
    
    // Check controller version
    let vs = unsafe { (*regs).vs };
    let major = (vs >> 16) & 0xFFFF;
    let minor = (vs >> 8) & 0xFF;
    let _tertiary = vs & 0xFF;
    
    if major < 1 {
        return Err("Unsupported NVMe version");
    }

    // Disable controller
    unsafe { (*regs).cc = 0 };
    
    // Wait for controller to be ready (with timeout)
    let mut timeout = 10_000_000u32; // Arbitrary timeout count
    while unsafe { (*regs).csts } & 1 != 0 {
        timeout = timeout.saturating_sub(1);
        if timeout == 0 {
            return Err("NVMe controller timeout waiting for disable");
        }
        core::hint::spin_loop();
    }

    // For now, return an error as full initialization not implemented
    Err("NVMe controller initialization not fully implemented")
}
