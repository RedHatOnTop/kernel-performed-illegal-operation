//! VirtIO Block device driver.
//!
//! This driver implements the VirtIO block device specification,
//! providing block-level read/write operations for virtual disks.
//!
//! # Device Discovery
//!
//! VirtIO block devices are identified by:
//! - PCI Vendor ID: 0x1AF4
//! - PCI Device ID: 0x1001 (legacy) or 0x1042 (modern)
//!
//! # Request Format
//!
//! Each block request consists of:
//! 1. Request header (type, sector)
//! 2. Data buffer
//! 3. Status byte

use crate::driver::pci::{self, PciAddress, PciDevice};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ptr;
use spin::Mutex;
use x86_64::instructions::port::Port;

/// VirtIO block request types.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum RequestType {
    /// Read sectors.
    In = 0,
    /// Write sectors.
    Out = 1,
    /// Flush (write barrier).
    Flush = 4,
    /// Get device ID.
    GetId = 8,
}

/// VirtIO block request status.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestStatus {
    /// Success.
    Ok = 0,
    /// I/O error.
    IoErr = 1,
    /// Unsupported operation.
    Unsupported = 2,
    /// Request not yet processed.
    Pending = 0xFF,
}

impl From<u8> for RequestStatus {
    fn from(value: u8) -> Self {
        match value {
            0 => RequestStatus::Ok,
            1 => RequestStatus::IoErr,
            2 => RequestStatus::Unsupported,
            _ => RequestStatus::Pending,
        }
    }
}

/// VirtIO block request header.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BlockRequestHeader {
    /// Request type.
    pub request_type: u32,
    /// Reserved.
    pub reserved: u32,
    /// Starting sector.
    pub sector: u64,
}

/// VirtIO PCI capability offsets (legacy mode).
mod legacy_regs {
    /// Device features (32-bit).
    pub const DEVICE_FEATURES: u16 = 0x00;
    /// Driver features (32-bit).
    pub const DRIVER_FEATURES: u16 = 0x04;
    /// Queue address (page-aligned).
    pub const QUEUE_ADDRESS: u16 = 0x08;
    /// Queue size.
    pub const QUEUE_SIZE: u16 = 0x0C;
    /// Queue select.
    pub const QUEUE_SELECT: u16 = 0x0E;
    /// Queue notify.
    pub const QUEUE_NOTIFY: u16 = 0x10;
    /// Device status.
    pub const DEVICE_STATUS: u16 = 0x12;
    /// ISR status.
    pub const ISR_STATUS: u16 = 0x13;
    /// Configuration space starts here (block device specific).
    pub const CONFIG: u16 = 0x14;
}

/// VirtIO device status bits.
mod device_status {
    /// Driver has acknowledged device.
    pub const ACKNOWLEDGE: u8 = 1;
    /// Driver knows how to drive the device.
    pub const DRIVER: u8 = 2;
    /// Driver is ready.
    pub const DRIVER_OK: u8 = 4;
    /// Feature negotiation complete.
    pub const FEATURES_OK: u8 = 8;
    /// Device needs reset.
    pub const NEEDS_RESET: u8 = 64;
    /// Device failed.
    pub const FAILED: u8 = 128;
}

/// Block size in bytes.
pub const BLOCK_SIZE: usize = 512;

/// VirtIO descriptor flags
const VRING_DESC_F_NEXT: u16 = 1;
const VRING_DESC_F_WRITE: u16 = 2;

/// Raw virtqueue descriptor for direct memory-mapped access.
#[repr(C)]
struct VirtqDescRaw {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

/// VirtIO Block device driver.
pub struct VirtioBlock {
    /// PCI address.
    pci_addr: PciAddress,
    /// I/O base port.
    io_base: u16,
    /// Device capacity in sectors.
    capacity: u64,
    /// Queue size.
    queue_size: u16,
    /// Descriptor table — physical address (for DMA / device).
    desc_phys: u64,
    /// Descriptor table — virtual address (for CPU access).
    desc_virt: u64,
    /// Available ring — physical address.
    avail_phys: u64,
    /// Available ring — virtual address.
    avail_virt: u64,
    /// Used ring — physical address.
    used_phys: u64,
    /// Used ring — virtual address.
    used_virt: u64,
    /// Request header buffer.
    header_buf: Box<BlockRequestHeader>,
    /// Status buffer.
    status_buf: Box<u8>,
    /// Data buffer for single-sector operations.
    data_buf: Box<[u8; BLOCK_SIZE]>,
}

impl VirtioBlock {
    /// Initialize a VirtIO block device.
    ///
    /// # Safety
    ///
    /// Must be called with a valid VirtIO block PCI device.
    pub unsafe fn new(device: &PciDevice) -> Option<Self> {
        let pci_addr = device.address;

        // Enable PCI bus mastering and I/O space
        pci::enable_bus_master(pci_addr);
        pci::enable_io_space(pci_addr);

        // Get I/O base from BAR0
        let bar0 = device.bars[0];
        if (bar0 & 0x1) == 0 {
            // Memory-mapped - not supported in legacy mode
            crate::serial_println!("[VirtIO] Memory-mapped BAR not supported");
            return None;
        }
        let io_base = (bar0 & !0x3) as u16;

        crate::serial_println!("[VirtIO-Blk] I/O base: {:#x}", io_base);

        // Reset device
        Self::write_status_raw(io_base, 0);

        // Acknowledge device
        Self::write_status_raw(io_base, device_status::ACKNOWLEDGE);

        // Indicate we know how to drive it
        Self::write_status_raw(io_base, device_status::ACKNOWLEDGE | device_status::DRIVER);

        // Read features (we don't need any special features for basic I/O)
        let mut features_port: Port<u32> = Port::new(io_base + legacy_regs::DEVICE_FEATURES);
        let _features = unsafe { features_port.read() };

        // Accept no features (basic operation)
        let mut driver_features_port: Port<u32> = Port::new(io_base + legacy_regs::DRIVER_FEATURES);
        unsafe { driver_features_port.write(0) };

        // Mark features OK
        Self::write_status_raw(
            io_base,
            device_status::ACKNOWLEDGE | device_status::DRIVER | device_status::FEATURES_OK,
        );

        // Check that features OK was accepted
        let status = Self::read_status_raw(io_base);
        if (status & device_status::FEATURES_OK) == 0 {
            crate::serial_println!("[VirtIO-Blk] Feature negotiation failed");
            Self::write_status_raw(io_base, device_status::FAILED);
            return None;
        }

        // Set up virtqueue 0
        let mut queue_select: Port<u16> = Port::new(io_base + legacy_regs::QUEUE_SELECT);
        unsafe { queue_select.write(0) };

        let mut queue_size_port: Port<u16> = Port::new(io_base + legacy_regs::QUEUE_SIZE);
        let queue_size = unsafe { queue_size_port.read() };

        if queue_size == 0 {
            crate::serial_println!("[VirtIO-Blk] Queue not available");
            Self::write_status_raw(io_base, device_status::FAILED);
            return None;
        }

        crate::serial_println!("[VirtIO-Blk] Queue size: {}", queue_size);

        // Queue memory layout (VirtIO legacy spec §2.6.2):
        //   Descriptor table:  16 bytes × queue_size
        //   Available ring:    6 + 2 × queue_size bytes
        //   (pad to next page boundary)
        //   Used ring:         6 + 8 × queue_size bytes
        let desc_size = core::mem::size_of::<super::queue::VirtqDesc>() * queue_size as usize;
        let avail_size = 6 + 2 * queue_size as usize;
        let _used_size = 6 + 8 * queue_size as usize;

        // Allocate page-aligned memory so the PFN calculation is correct.
        let layout = alloc::alloc::Layout::from_size_align(4096 * 4, 4096)
            .expect("[VirtIO-Blk] Layout error");
        let queue_virt = unsafe { alloc::alloc::alloc_zeroed(layout) } as u64;
        if queue_virt == 0 {
            crate::serial_println!("[VirtIO-Blk] Queue allocation failed");
            Self::write_status_raw(io_base, device_status::FAILED);
            return None;
        }

        // Translate virtual → physical for DMA.
        let queue_phys = crate::memory::virt_to_phys(queue_virt)
            .expect("[VirtIO-Blk] virt_to_phys failed for queue memory");

        let desc_virt = queue_virt;
        let desc_phys = queue_phys;
        let avail_virt = desc_virt + desc_size as u64;
        let avail_phys = desc_phys + desc_size as u64;
        // Used ring must start on a page boundary (legacy spec).
        let used_virt = (avail_virt + avail_size as u64 + 4095) & !4095;
        let used_phys = (avail_phys + avail_size as u64 + 4095) & !4095;

        // Tell device the queue address — PHYSICAL page frame number.
        let mut queue_addr: Port<u32> = Port::new(io_base + legacy_regs::QUEUE_ADDRESS);
        unsafe { queue_addr.write((desc_phys / 4096) as u32) };

        crate::serial_println!(
            "[VirtIO-Blk] Queue mem virt={:#x} phys={:#x} pfn={:#x}",
            queue_virt, queue_phys, desc_phys / 4096
        );

        // Read device capacity
        let mut cap_low: Port<u32> = Port::new(io_base + legacy_regs::CONFIG);
        let mut cap_high: Port<u32> = Port::new(io_base + legacy_regs::CONFIG + 4);
        let capacity = unsafe { cap_low.read() as u64 | ((cap_high.read() as u64) << 32) };

        crate::serial_println!(
            "[VirtIO-Blk] Capacity: {} sectors ({} MB)",
            capacity,
            capacity * 512 / 1024 / 1024
        );

        // Mark driver OK
        Self::write_status_raw(
            io_base,
            device_status::ACKNOWLEDGE
                | device_status::DRIVER
                | device_status::FEATURES_OK
                | device_status::DRIVER_OK,
        );

        crate::serial_println!("[VirtIO-Blk] Initialization complete");

        Some(VirtioBlock {
            pci_addr,
            io_base,
            capacity,
            queue_size,
            desc_phys,
            desc_virt,
            avail_phys,
            avail_virt,
            used_phys,
            used_virt,
            header_buf: Box::new(BlockRequestHeader {
                request_type: 0,
                reserved: 0,
                sector: 0,
            }),
            status_buf: Box::new(RequestStatus::Pending as u8),
            data_buf: Box::new([0u8; BLOCK_SIZE]),
        })
    }

    /// Read device status register.
    fn read_status_raw(io_base: u16) -> u8 {
        let mut port: Port<u8> = Port::new(io_base + legacy_regs::DEVICE_STATUS);
        unsafe { port.read() }
    }

    /// Write device status register.
    fn write_status_raw(io_base: u16, status: u8) {
        let mut port: Port<u8> = Port::new(io_base + legacy_regs::DEVICE_STATUS);
        unsafe { port.write(status) }
    }

    /// Notify the device about available buffers.
    fn notify(&self, queue: u16) {
        let mut port: Port<u16> = Port::new(self.io_base + legacy_regs::QUEUE_NOTIFY);
        unsafe { port.write(queue) };
    }

    /// Get device capacity in sectors.
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Get device capacity in bytes.
    pub fn capacity_bytes(&self) -> u64 {
        self.capacity * BLOCK_SIZE as u64
    }

    /// Read a single sector using VirtQueue descriptor chain.
    ///
    /// Submits a 3-descriptor chain:
    /// - Desc 0: request header (device-readable)
    /// - Desc 1: data buffer (device-writable)
    /// - Desc 2: status byte (device-writable)
    pub fn read_sector(
        &mut self,
        sector: u64,
        buffer: &mut [u8; BLOCK_SIZE],
    ) -> Result<(), RequestStatus> {
        if sector >= self.capacity {
            return Err(RequestStatus::IoErr);
        }

        // Set up request header
        self.header_buf.request_type = RequestType::In as u32;
        self.header_buf.reserved = 0;
        self.header_buf.sector = sector;

        // Reset status
        *self.status_buf = RequestStatus::Pending as u8;

        // Clear data buffer
        self.data_buf.fill(0);

        // Translate DMA buffer addresses: virtual → physical.
        let hdr_virt = &*self.header_buf as *const BlockRequestHeader as u64;
        let hdr_phys = crate::memory::virt_to_phys(hdr_virt).unwrap_or(hdr_virt);
        let data_virt = self.data_buf.as_ptr() as u64;
        let data_phys = crate::memory::virt_to_phys(data_virt).unwrap_or(data_virt);
        let status_virt = &*self.status_buf as *const u8 as u64;
        let status_phys = crate::memory::virt_to_phys(status_virt).unwrap_or(status_virt);

        // Build 3-descriptor chain — CPU writes via VIRTUAL desc base,
        // but addr fields inside descriptors must be PHYSICAL (DMA).
        let desc_base = self.desc_virt as *mut VirtqDescRaw;
        let avail_base = self.avail_virt as *mut u16;

        unsafe {
            // Descriptor 0: header (device-readable)
            let d0 = &mut *desc_base.add(0);
            d0.addr = hdr_phys;
            d0.len = core::mem::size_of::<BlockRequestHeader>() as u32;
            d0.flags = VRING_DESC_F_NEXT;
            d0.next = 1;

            // Descriptor 1: data buffer (device-writable for read)
            let d1 = &mut *desc_base.add(1);
            d1.addr = data_phys;
            d1.len = BLOCK_SIZE as u32;
            d1.flags = VRING_DESC_F_WRITE | VRING_DESC_F_NEXT;
            d1.next = 2;

            // Descriptor 2: status (device-writable)
            let d2 = &mut *desc_base.add(2);
            d2.addr = status_phys;
            d2.len = 1;
            d2.flags = VRING_DESC_F_WRITE;
            d2.next = 0;

            // Add to available ring (CPU access via VIRTUAL avail base)
            // avail ring layout: flags(u16), idx(u16), ring[queue_size](u16), used_event(u16)
            let avail_idx_ptr = avail_base.add(1);
            let avail_idx = core::ptr::read_volatile(avail_idx_ptr);
            let ring_entry = avail_base.add(2 + (avail_idx as usize % self.queue_size as usize));
            core::ptr::write_volatile(ring_entry, 0); // head descriptor index = 0

            // Memory barrier
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

            // Increment avail idx
            core::ptr::write_volatile(avail_idx_ptr, avail_idx.wrapping_add(1));

            // Memory barrier
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        }

        // Notify device (queue 0)
        self.notify(0);

        // Poll for completion — CPU reads used ring via VIRTUAL address.
        // used ring layout: flags(u16), idx(u16), ring[queue_size](VirtqUsedElem), avail_event(u16)
        let used_idx_ptr = unsafe { (self.used_virt as *mut u16).add(1) };
        let start_idx = unsafe { core::ptr::read_volatile(used_idx_ptr) };
        let mut timeout = 1_000_000u32;
        loop {
            let current_idx = unsafe { core::ptr::read_volatile(used_idx_ptr) };
            if current_idx != start_idx {
                break;
            }
            timeout -= 1;
            if timeout == 0 {
                crate::serial_println!("[VirtIO-Blk] Read timeout (sector {})", sector);
                return Err(RequestStatus::IoErr);
            }
            core::hint::spin_loop();
        }

        // Check status
        let status = RequestStatus::from(*self.status_buf);
        if status == RequestStatus::Ok {
            buffer.copy_from_slice(&*self.data_buf);
            Ok(())
        } else {
            Err(status)
        }
    }

    /// Write a single sector using VirtQueue descriptor chain.
    pub fn write_sector(
        &mut self,
        sector: u64,
        buffer: &[u8; BLOCK_SIZE],
    ) -> Result<(), RequestStatus> {
        if sector >= self.capacity {
            return Err(RequestStatus::IoErr);
        }

        // Set up request header
        self.header_buf.request_type = RequestType::Out as u32;
        self.header_buf.reserved = 0;
        self.header_buf.sector = sector;

        // Reset status
        *self.status_buf = RequestStatus::Pending as u8;

        // Copy data to our buffer
        self.data_buf.copy_from_slice(buffer);

        // Translate DMA buffer addresses: virtual → physical.
        let hdr_virt = &*self.header_buf as *const BlockRequestHeader as u64;
        let hdr_phys = crate::memory::virt_to_phys(hdr_virt).unwrap_or(hdr_virt);
        let data_virt = self.data_buf.as_ptr() as u64;
        let data_phys = crate::memory::virt_to_phys(data_virt).unwrap_or(data_virt);
        let status_virt = &*self.status_buf as *const u8 as u64;
        let status_phys = crate::memory::virt_to_phys(status_virt).unwrap_or(status_virt);

        // Build 3-descriptor chain — PHYSICAL addresses in descriptors.
        let desc_base = self.desc_virt as *mut VirtqDescRaw;
        let avail_base = self.avail_virt as *mut u16;

        unsafe {
            // Descriptor 0: header (device-readable)
            let d0 = &mut *desc_base.add(0);
            d0.addr = hdr_phys;
            d0.len = core::mem::size_of::<BlockRequestHeader>() as u32;
            d0.flags = VRING_DESC_F_NEXT;
            d0.next = 1;

            // Descriptor 1: data buffer (device-readable for write)
            let d1 = &mut *desc_base.add(1);
            d1.addr = data_phys;
            d1.len = BLOCK_SIZE as u32;
            d1.flags = VRING_DESC_F_NEXT; // NOT writable — device reads from this
            d1.next = 2;

            // Descriptor 2: status (device-writable)
            let d2 = &mut *desc_base.add(2);
            d2.addr = status_phys;
            d2.len = 1;
            d2.flags = VRING_DESC_F_WRITE;
            d2.next = 0;

            // Add to available ring (CPU access via VIRTUAL avail base)
            let avail_idx_ptr = avail_base.add(1);
            let avail_idx = core::ptr::read_volatile(avail_idx_ptr);
            let ring_entry = avail_base.add(2 + (avail_idx as usize % self.queue_size as usize));
            core::ptr::write_volatile(ring_entry, 0);

            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            core::ptr::write_volatile(avail_idx_ptr, avail_idx.wrapping_add(1));
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        }

        // Notify device
        self.notify(0);

        // Poll for completion — CPU reads used ring via VIRTUAL address.
        let used_idx_ptr = unsafe { (self.used_virt as *mut u16).add(1) };
        let start_idx = unsafe { core::ptr::read_volatile(used_idx_ptr) };
        let mut timeout = 1_000_000u32;
        loop {
            let current_idx = unsafe { core::ptr::read_volatile(used_idx_ptr) };
            if current_idx != start_idx {
                break;
            }
            timeout -= 1;
            if timeout == 0 {
                crate::serial_println!("[VirtIO-Blk] Write timeout (sector {})", sector);
                return Err(RequestStatus::IoErr);
            }
            core::hint::spin_loop();
        }

        let status = RequestStatus::from(*self.status_buf);
        if status == RequestStatus::Ok {
            Ok(())
        } else {
            Err(status)
        }
    }
}

/// Global VirtIO block devices.
static VIRTIO_BLOCK_DEVICES: Mutex<Vec<VirtioBlock>> = Mutex::new(Vec::new());

/// Initialize VirtIO block devices.
pub fn init() {
    let devices = pci::find_virtio_block();

    if devices.is_empty() {
        crate::serial_println!("[VirtIO-Blk] No VirtIO block devices found");
        return;
    }

    crate::serial_println!(
        "[VirtIO-Blk] Found {} VirtIO block device(s)",
        devices.len()
    );

    for device in devices {
        match unsafe { VirtioBlock::new(&device) } {
            Some(blk) => {
                VIRTIO_BLOCK_DEVICES.lock().push(blk);
            }
            None => {
                crate::serial_println!(
                    "[VirtIO-Blk] Failed to initialize device at {}",
                    device.address
                );
            }
        }
    }
}

/// Get the number of initialized VirtIO block devices.
pub fn device_count() -> usize {
    VIRTIO_BLOCK_DEVICES.lock().len()
}

/// Get info about VirtIO block devices (index, capacity_sectors, capacity_mb).
pub fn device_info() -> Vec<(usize, u64, u64)> {
    VIRTIO_BLOCK_DEVICES
        .lock()
        .iter()
        .enumerate()
        .map(|(i, dev)| (i, dev.capacity(), dev.capacity_bytes() / (1024 * 1024)))
        .collect()
}

/// Read one 512-byte sector from a VirtIO block device.
pub fn read_sector(device_index: usize, sector: u64, buffer: &mut [u8; BLOCK_SIZE]) -> bool {
    let mut devices = VIRTIO_BLOCK_DEVICES.lock();
    match devices.get_mut(device_index) {
        Some(dev) => dev.read_sector(sector, buffer).is_ok(),
        None => false,
    }
}

/// Write one 512-byte sector to a VirtIO block device.
pub fn write_sector(device_index: usize, sector: u64, buffer: &[u8; BLOCK_SIZE]) -> bool {
    let mut devices = VIRTIO_BLOCK_DEVICES.lock();
    match devices.get_mut(device_index) {
        Some(dev) => dev.write_sector(sector, buffer).is_ok(),
        None => false,
    }
}
