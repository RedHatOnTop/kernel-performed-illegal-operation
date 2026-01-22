//! Boot information structures.
//!
//! This module defines the data structures passed from the bootloader
//! to the kernel, containing essential information about the system
//! configuration discovered during early boot.

use core::ops::Range;

/// Boot information passed from the UEFI bootloader.
///
/// This structure contains all information the kernel needs to
/// initialize itself, including memory maps, framebuffer info,
/// and ACPI table locations.
#[repr(C)]
pub struct BootInfo {
    /// Memory map describing all physical memory regions.
    pub memory_map: MemoryMap,
    
    /// Framebuffer information for early console output.
    pub framebuffer: Option<FramebufferInfo>,
    
    /// ACPI RSDP (Root System Description Pointer) address.
    pub rsdp_addr: Option<u64>,
    
    /// Kernel physical address range.
    pub kernel_phys: Range<u64>,
    
    /// Kernel virtual address range.
    pub kernel_virt: Range<u64>,
    
    /// Physical memory offset (for direct mapping).
    pub physical_memory_offset: u64,
}

/// Memory map containing all memory region descriptors.
#[repr(C)]
pub struct MemoryMap {
    /// Pointer to the array of memory region descriptors.
    pub entries: *const MemoryRegion,
    
    /// Number of entries in the memory map.
    pub entry_count: usize,
}

impl MemoryMap {
    /// Returns an iterator over memory regions.
    ///
    /// # Safety
    ///
    /// The caller must ensure the memory map entries are valid
    /// and accessible.
    pub unsafe fn iter(&self) -> impl Iterator<Item = &MemoryRegion> {
        (0..self.entry_count).map(move |i| unsafe {
            &*self.entries.add(i)
        })
    }
    
    /// Returns an iterator over usable memory regions.
    ///
    /// # Safety
    ///
    /// The caller must ensure the memory map entries are valid
    /// and accessible.
    pub unsafe fn usable_regions(&self) -> impl Iterator<Item = &MemoryRegion> {
        unsafe { self.iter() }.filter(|r| r.kind == MemoryRegionKind::Usable)
    }
}

/// A single memory region descriptor.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    /// Physical start address of the region.
    pub start: u64,
    
    /// Physical end address of the region (exclusive).
    pub end: u64,
    
    /// Type of memory region.
    pub kind: MemoryRegionKind,
}

impl MemoryRegion {
    /// Returns the size of the region in bytes.
    pub fn size(&self) -> u64 {
        self.end - self.start
    }
    
    /// Returns true if the region is usable for allocation.
    pub fn is_usable(&self) -> bool {
        self.kind == MemoryRegionKind::Usable
    }
}

/// Type of memory region.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegionKind {
    /// Usable RAM that can be freely allocated.
    Usable = 0,
    
    /// Reserved memory that must not be used.
    Reserved = 1,
    
    /// ACPI reclaimable memory.
    AcpiReclaimable = 2,
    
    /// ACPI NVS (Non-Volatile Storage).
    AcpiNvs = 3,
    
    /// Memory-mapped I/O.
    Mmio = 4,
    
    /// Memory used by the bootloader.
    BootloaderReclaimable = 5,
    
    /// Memory used by the kernel.
    Kernel = 6,
    
    /// Memory used by the page tables.
    PageTables = 7,
    
    /// Framebuffer memory.
    Framebuffer = 8,
}

/// Framebuffer information for graphics output.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FramebufferInfo {
    /// Physical address of the framebuffer.
    pub address: u64,
    
    /// Width in pixels.
    pub width: u32,
    
    /// Height in pixels.
    pub height: u32,
    
    /// Bytes per scanline (pitch).
    pub pitch: u32,
    
    /// Bits per pixel.
    pub bpp: u8,
    
    /// Pixel format.
    pub format: PixelFormat,
}

/// Pixel format for the framebuffer.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// RGB with 8 bits per channel.
    Rgb = 0,
    
    /// BGR with 8 bits per channel.
    Bgr = 1,
    
    /// Unknown format.
    Unknown = 255,
}
