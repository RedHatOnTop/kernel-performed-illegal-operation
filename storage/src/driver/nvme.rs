//! NVMe driver.
//!
//! This module provides support for NVMe (Non-Volatile Memory Express) devices,
//! the standard interface for modern SSDs.

use crate::{BlockDeviceInfo, StorageError};
use super::BlockDevice;

/// NVMe controller capabilities register.
#[derive(Debug, Clone, Copy)]
pub struct NvmeCapabilities {
    /// Maximum queue entries supported (0-based).
    pub mqes: u16,
    /// Contiguous queues required.
    pub cqr: bool,
    /// Arbitration mechanism supported.
    pub ams: u8,
    /// Timeout (in 500ms units).
    pub to: u8,
    /// Doorbell stride (2^(2+dstrd) bytes).
    pub dstrd: u8,
    /// NVM subsystem reset supported.
    pub nssrs: bool,
    /// Command sets supported.
    pub css: u8,
    /// Boot partition support.
    pub bps: bool,
    /// Memory page size minimum (2^(12+mpsmin) bytes).
    pub mpsmin: u8,
    /// Memory page size maximum (2^(12+mpsmax) bytes).
    pub mpsmax: u8,
}

impl NvmeCapabilities {
    /// Read from a 64-bit register value.
    pub fn from_raw(value: u64) -> Self {
        NvmeCapabilities {
            mqes: (value & 0xFFFF) as u16,
            cqr: (value >> 16) & 1 != 0,
            ams: ((value >> 17) & 0x3) as u8,
            to: ((value >> 24) & 0xFF) as u8,
            dstrd: ((value >> 32) & 0xF) as u8,
            nssrs: (value >> 36) & 1 != 0,
            css: ((value >> 37) & 0xFF) as u8,
            bps: (value >> 45) & 1 != 0,
            mpsmin: ((value >> 48) & 0xF) as u8,
            mpsmax: ((value >> 52) & 0xF) as u8,
        }
    }

    /// Get the maximum queue size.
    pub fn max_queue_size(&self) -> u16 {
        self.mqes + 1
    }

    /// Get the doorbell stride in bytes.
    pub fn doorbell_stride(&self) -> usize {
        4 << self.dstrd
    }

    /// Get the minimum memory page size.
    pub fn min_page_size(&self) -> usize {
        1 << (12 + self.mpsmin)
    }

    /// Get the maximum memory page size.
    pub fn max_page_size(&self) -> usize {
        1 << (12 + self.mpsmax)
    }
}

/// NVMe controller status register.
#[derive(Debug, Clone, Copy)]
pub struct NvmeControllerStatus {
    /// Ready.
    pub rdy: bool,
    /// Controller fatal status.
    pub cfs: bool,
    /// Shutdown status.
    pub shst: u8,
    /// NVM subsystem reset occurred.
    pub nssro: bool,
    /// Processing paused.
    pub pp: bool,
}

impl NvmeControllerStatus {
    /// Read from a 32-bit register value.
    pub fn from_raw(value: u32) -> Self {
        NvmeControllerStatus {
            rdy: value & 1 != 0,
            cfs: (value >> 1) & 1 != 0,
            shst: ((value >> 2) & 0x3) as u8,
            nssro: (value >> 4) & 1 != 0,
            pp: (value >> 5) & 1 != 0,
        }
    }
}

/// NVMe submission queue entry (64 bytes).
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct NvmeSubmissionEntry {
    /// Command dword 0.
    pub cdw0: u32,
    /// Namespace identifier.
    pub nsid: u32,
    /// Reserved.
    pub reserved: [u32; 2],
    /// Metadata pointer.
    pub mptr: u64,
    /// Data pointer (PRP or SGL).
    pub dptr: [u64; 2],
    /// Command dwords 10-15.
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
}

impl NvmeSubmissionEntry {
    /// Entry size in bytes.
    pub const SIZE: usize = 64;

    /// Set the opcode.
    pub fn set_opcode(&mut self, opcode: u8) {
        self.cdw0 = (self.cdw0 & !0xFF) | (opcode as u32);
    }

    /// Set the command ID.
    pub fn set_cid(&mut self, cid: u16) {
        self.cdw0 = (self.cdw0 & 0xFFFF) | ((cid as u32) << 16);
    }

    /// Set the fused operation.
    pub fn set_fused(&mut self, fused: u8) {
        self.cdw0 = (self.cdw0 & !(0x3 << 8)) | ((fused as u32 & 0x3) << 8);
    }
}

/// NVMe completion queue entry (16 bytes).
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct NvmeCompletionEntry {
    /// Command-specific.
    pub dw0: u32,
    /// Reserved.
    pub dw1: u32,
    /// Submission queue head pointer.
    pub sqhd: u16,
    /// Submission queue identifier.
    pub sqid: u16,
    /// Command identifier.
    pub cid: u16,
    /// Status field.
    pub status: u16,
}

impl NvmeCompletionEntry {
    /// Entry size in bytes.
    pub const SIZE: usize = 16;

    /// Check if this is a phase bit match.
    pub fn phase(&self) -> bool {
        self.status & 1 != 0
    }

    /// Get the status code.
    pub fn status_code(&self) -> u8 {
        ((self.status >> 1) & 0xFF) as u8
    }

    /// Get the status code type.
    pub fn status_code_type(&self) -> u8 {
        ((self.status >> 9) & 0x7) as u8
    }

    /// Check if the command completed successfully.
    pub fn success(&self) -> bool {
        self.status_code() == 0 && self.status_code_type() == 0
    }
}

/// NVMe admin command opcodes.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum NvmeAdminOpcode {
    /// Delete I/O submission queue.
    DeleteIoSq = 0x00,
    /// Create I/O submission queue.
    CreateIoSq = 0x01,
    /// Get log page.
    GetLogPage = 0x02,
    /// Delete I/O completion queue.
    DeleteIoCq = 0x04,
    /// Create I/O completion queue.
    CreateIoCq = 0x05,
    /// Identify.
    Identify = 0x06,
    /// Abort.
    Abort = 0x08,
    /// Set features.
    SetFeatures = 0x09,
    /// Get features.
    GetFeatures = 0x0A,
    /// Asynchronous event request.
    AsyncEventReq = 0x0C,
    /// Namespace management.
    NsManagement = 0x0D,
    /// Firmware commit.
    FirmwareCommit = 0x10,
    /// Firmware image download.
    FirmwareDownload = 0x11,
    /// Device self-test.
    DeviceSelfTest = 0x14,
    /// Namespace attachment.
    NsAttachment = 0x15,
    /// Keep alive.
    KeepAlive = 0x18,
    /// Directive send.
    DirectiveSend = 0x19,
    /// Directive receive.
    DirectiveRecv = 0x1A,
    /// Virtualization management.
    VirtMgmt = 0x1C,
    /// NVMe-MI send.
    NvmeMiSend = 0x1D,
    /// NVMe-MI receive.
    NvmeMiRecv = 0x1E,
    /// Doorbell buffer config.
    DoorbellBufferConfig = 0x7C,
    /// Format NVM.
    FormatNvm = 0x80,
    /// Security send.
    SecuritySend = 0x81,
    /// Security receive.
    SecurityRecv = 0x82,
    /// Sanitize.
    Sanitize = 0x84,
    /// Get LBA status.
    GetLbaStatus = 0x86,
}

/// NVMe I/O command opcodes.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum NvmeIoOpcode {
    /// Flush.
    Flush = 0x00,
    /// Write.
    Write = 0x01,
    /// Read.
    Read = 0x02,
    /// Write uncorrectable.
    WriteUncorrectable = 0x04,
    /// Compare.
    Compare = 0x05,
    /// Write zeroes.
    WriteZeroes = 0x08,
    /// Dataset management.
    DatasetManagement = 0x09,
    /// Verify.
    Verify = 0x0C,
    /// Reservation register.
    ReservationRegister = 0x0D,
    /// Reservation report.
    ReservationReport = 0x0E,
    /// Reservation acquire.
    ReservationAcquire = 0x11,
    /// Reservation release.
    ReservationRelease = 0x15,
}

/// NVMe queue.
pub struct NvmeQueue {
    /// Submission queue entries.
    sq_entries: *mut NvmeSubmissionEntry,
    /// Completion queue entries.
    cq_entries: *mut NvmeCompletionEntry,
    /// Submission queue size.
    sq_size: u16,
    /// Completion queue size.
    cq_size: u16,
    /// Submission queue tail.
    sq_tail: u16,
    /// Completion queue head.
    cq_head: u16,
    /// Expected phase bit.
    cq_phase: bool,
    /// Queue ID.
    qid: u16,
    /// Doorbell stride.
    db_stride: usize,
    /// Submission queue doorbell.
    sq_doorbell: *mut u32,
    /// Completion queue doorbell.
    cq_doorbell: *mut u32,
}

impl NvmeQueue {
    /// Create a new NVMe queue.
    pub fn new(
        qid: u16,
        sq_entries: *mut NvmeSubmissionEntry,
        cq_entries: *mut NvmeCompletionEntry,
        sq_size: u16,
        cq_size: u16,
        sq_doorbell: *mut u32,
        cq_doorbell: *mut u32,
        db_stride: usize,
    ) -> Self {
        NvmeQueue {
            sq_entries,
            cq_entries,
            sq_size,
            cq_size,
            sq_tail: 0,
            cq_head: 0,
            cq_phase: true,
            qid,
            db_stride,
            sq_doorbell,
            cq_doorbell,
        }
    }

    /// Submit a command.
    pub fn submit(&mut self, cmd: NvmeSubmissionEntry) -> Result<u16, StorageError> {
        let next_tail = (self.sq_tail + 1) % self.sq_size;

        // Check if queue is full
        // Note: This is simplified; real implementation needs to track head
        if next_tail == 0 {
            // Queue might be full - simplified check
        }

        unsafe {
            let entry = self.sq_entries.add(self.sq_tail as usize);
            core::ptr::write_volatile(entry, cmd);
        }

        let cid = self.sq_tail;
        self.sq_tail = next_tail;

        // Ring doorbell
        unsafe {
            core::ptr::write_volatile(self.sq_doorbell, self.sq_tail as u32);
        }

        Ok(cid)
    }

    /// Poll for completion.
    pub fn poll_completion(&mut self) -> Option<NvmeCompletionEntry> {
        unsafe {
            let entry = self.cq_entries.add(self.cq_head as usize);
            let cqe = core::ptr::read_volatile(entry);

            if cqe.phase() != self.cq_phase {
                return None;
            }

            // Advance completion queue head
            self.cq_head += 1;
            if self.cq_head >= self.cq_size {
                self.cq_head = 0;
                self.cq_phase = !self.cq_phase;
            }

            // Ring doorbell
            core::ptr::write_volatile(self.cq_doorbell, self.cq_head as u32);

            Some(cqe)
        }
    }
}

/// NVMe device.
pub struct NvmeDevice {
    /// Base address of the controller registers.
    base_addr: usize,
    /// Controller capabilities.
    caps: NvmeCapabilities,
    /// Admin queue.
    admin_queue: Option<NvmeQueue>,
    /// I/O queues.
    io_queues: [Option<NvmeQueue>; 16],
    /// Number of namespaces.
    num_namespaces: u32,
    /// Block size (LBA size).
    block_size: u32,
    /// Total number of blocks.
    total_blocks: u64,
    /// Device is initialized.
    initialized: bool,
}

impl NvmeDevice {
    /// Create a new NVMe device.
    pub fn new(base_addr: usize) -> Self {
        NvmeDevice {
            base_addr,
            caps: NvmeCapabilities::from_raw(0),
            admin_queue: None,
            io_queues: Default::default(),
            num_namespaces: 0,
            block_size: 512,
            total_blocks: 0,
            initialized: false,
        }
    }

    /// Initialize the device.
    pub fn init(&mut self) -> Result<(), StorageError> {
        // Read capabilities
        let cap = unsafe {
            core::ptr::read_volatile(self.base_addr as *const u64)
        };
        self.caps = NvmeCapabilities::from_raw(cap);

        // TODO: Full NVMe initialization sequence
        // 1. Disable controller (clear CC.EN)
        // 2. Wait for CSTS.RDY to become 0
        // 3. Configure admin queue
        // 4. Set memory page size in CC
        // 5. Set command set in CC
        // 6. Enable controller (set CC.EN)
        // 7. Wait for CSTS.RDY to become 1
        // 8. Send Identify Controller command
        // 9. Create I/O queues

        self.initialized = true;
        Ok(())
    }

    /// Read from namespace.
    fn read_lba(&self, nsid: u32, lba: u64, count: u16, buffer: &mut [u8]) -> Result<(), StorageError> {
        if !self.initialized {
            return Err(StorageError::NotReady);
        }

        let _ = nsid;
        let _ = lba;
        let _ = count;
        let _ = buffer;

        // TODO: Build and submit read command
        Ok(())
    }

    /// Write to namespace.
    fn write_lba(&self, nsid: u32, lba: u64, count: u16, data: &[u8]) -> Result<(), StorageError> {
        if !self.initialized {
            return Err(StorageError::NotReady);
        }

        let _ = nsid;
        let _ = lba;
        let _ = count;
        let _ = data;

        // TODO: Build and submit write command
        Ok(())
    }
}

impl BlockDevice for NvmeDevice {
    fn info(&self) -> BlockDeviceInfo {
        let mut name = [0u8; 32];
        name[..4].copy_from_slice(b"nvme");

        BlockDeviceInfo {
            name,
            name_len: 4,
            block_size: self.block_size,
            total_blocks: self.total_blocks,
            read_only: false,
            supports_trim: true,
            optimal_io_size: 128, // 64KB with 512B blocks
            physical_block_size: self.block_size,
        }
    }

    fn read_blocks(&self, start_block: u64, buffer: &mut [u8]) -> Result<usize, StorageError> {
        let blocks = buffer.len() / self.block_size as usize;
        self.read_lba(1, start_block, blocks as u16, buffer)?;
        Ok(buffer.len())
    }

    fn write_blocks(&self, start_block: u64, data: &[u8]) -> Result<usize, StorageError> {
        let blocks = data.len() / self.block_size as usize;
        self.write_lba(1, start_block, blocks as u16, data)?;
        Ok(data.len())
    }

    fn flush(&self) -> Result<(), StorageError> {
        // TODO: Submit flush command
        Ok(())
    }

    fn discard(&self, _start_block: u64, _num_blocks: u64) -> Result<(), StorageError> {
        // TODO: Submit dataset management command with deallocate
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.initialized
    }
}

/// Probe for NVMe devices.
pub fn probe() -> Result<(), StorageError> {
    // TODO: Scan PCI for NVMe controllers (class 01, subclass 08, prog-if 02)
    Ok(())
}
