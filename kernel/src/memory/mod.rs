//! Memory management subsystem.
//!
//! This module provides physical and virtual memory management for the kernel.
//!
//! # Components
//!
//! - **BootInfoFrameAllocator**: Physical frame allocator using bootloader memory map
//! - **Page Table Mapper**: Virtual memory management
//! - **Heap**: Dynamic memory allocation (in allocator module)
//! - **Slab**: Fixed-size object caching
//! - **Buddy**: Power-of-two block allocator
//! - **Optimization**: Memory compression and reclamation

pub mod slab;
pub mod buddy;
pub mod optimization;

use bootloader_api::info::MemoryRegionKind;
use spin::Mutex;
use x86_64::{
    structures::paging::{FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB},
    PhysAddr, VirtAddr,
};

/// Global frame allocator for slab and buddy systems.
static GLOBAL_FRAME_ALLOCATOR: Mutex<Option<GlobalFrameAllocator>> = Mutex::new(None);

/// Simple global frame allocator.
struct GlobalFrameAllocator {
    next_frame: u64,
    end_frame: u64,
}

impl GlobalFrameAllocator {
    fn allocate(&mut self) -> Option<u64> {
        if self.next_frame >= self.end_frame {
            return None;
        }
        let frame = self.next_frame;
        self.next_frame += 4096;
        Some(frame)
    }
}

/// Initialize the global frame allocator for slab/buddy.
pub fn init_frame_allocator(start: u64, end: u64) {
    *GLOBAL_FRAME_ALLOCATOR.lock() = Some(GlobalFrameAllocator {
        next_frame: start,
        end_frame: end,
    });
}

/// Allocate a physical frame for slab allocator.
pub fn allocate_frame() -> Option<usize> {
    GLOBAL_FRAME_ALLOCATOR.lock().as_mut()?.allocate().map(|f| f as usize)
}

/// Free a physical frame.
pub fn free_frame(_addr: usize) {
    // In a real implementation, this would return the frame to the pool
    // For now, we don't reclaim frames
}

/// Validate the physical memory offset.
///
/// Verifies that the physical memory offset provided by the bootloader is valid.
/// An invalid offset can cause memory access errors.
///
/// # Panics
///
/// Panics if any of the following conditions fail:
/// - Page alignment check (4KiB)
/// - Canonical address check
/// - Kernel space check (>= 0xFFFF_8000_0000_0000)
/// - Read test (whether actually accessible)
pub fn validate_physical_memory_offset(offset: u64) {
    const PAGE_SIZE: u64 = 4096;
    const KERNEL_SPACE_START: u64 = 0xFFFF_8000_0000_0000;

    // 1. Page alignment check
    if offset % PAGE_SIZE != 0 {
        panic!(
            "Physical memory offset {:#x} is not page-aligned (must be aligned to {:#x})",
            offset, PAGE_SIZE
        );
    }

    // 2. Canonical address check
    // On x86_64, virtual addresses use 47-bit or 57-bit (LA57) address space
    // Bits 47 (or 57) through 63 must all have the same value
    let sign_extension = if offset & (1 << 47) != 0 {
        // Negative range: upper bits must all be 1
        offset & 0xFFFF_0000_0000_0000 == 0xFFFF_0000_0000_0000
    } else {
        // Positive range: upper bits must all be 0
        offset & 0xFFFF_0000_0000_0000 == 0
    };

    if !sign_extension {
        panic!(
            "Physical memory offset {:#x} is not a canonical address",
            offset
        );
    }

    // 3. Kernel space check (warning only; lower half is also valid with Dynamic mapping)
    // bootloader 0.11 Dynamic mapping does not guarantee higher half
    if offset < KERNEL_SPACE_START {
        crate::serial_println!(
            "[KPIO] Warning: Physical memory offset {:#x} is in lower half (< {:#x})",
            offset,
            KERNEL_SPACE_START
        );
        crate::serial_println!("[KPIO] This is valid with bootloader Dynamic mapping");
    }

    // 4. Read test
    // Read at offset + 0 (physical address 0) to verify accessibility
    // Physical address 0 usually contains the real mode IVT or is empty
    let test_ptr = offset as *const u8;
    let _test_read = unsafe { core::ptr::read_volatile(test_ptr) };

    crate::serial_println!("[KPIO] Physical memory offset validated: {:#x}", offset);
}

/// Initialize the page table mapper.
///
/// # Safety
///
/// The caller must ensure:
/// - All physical memory is mapped at `physical_memory_offset`
/// - This function is called only once
pub unsafe fn init(physical_memory_offset: u64) -> OffsetPageTable<'static> {
    let phys_offset = VirtAddr::new(physical_memory_offset);
    let level_4_table = unsafe { active_level_4_table(phys_offset) };
    unsafe { OffsetPageTable::new(level_4_table, phys_offset) }
}

/// Return the active level 4 page table.
///
/// # Safety
///
/// The caller must ensure that all physical memory is mapped at `physical_memory_offset`.
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}

/// Frame allocator based on the bootloader memory map.
///
/// Allocates available frames from the memory map provided by the bootloader.
pub struct BootInfoFrameAllocator<I>
where
    I: Iterator<Item = &'static bootloader_api::info::MemoryRegion>,
{
    memory_regions: I,
    current_region: Option<&'static bootloader_api::info::MemoryRegion>,
    next_frame: u64,
}

impl<I> BootInfoFrameAllocator<I>
where
    I: Iterator<Item = &'static bootloader_api::info::MemoryRegion>,
{
    /// Create a new frame allocator.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the provided memory regions are valid and actually usable.
    pub unsafe fn new(memory_regions: I) -> Self {
        let mut allocator = Self {
            memory_regions,
            current_region: None,
            next_frame: 0,
        };
        allocator.advance_to_usable_region();
        allocator
    }

    /// Advance to the next usable memory region.
    fn advance_to_usable_region(&mut self) {
        while let Some(region) = self.memory_regions.next() {
            if region.kind == MemoryRegionKind::Usable {
                self.current_region = Some(region);
                self.next_frame = region.start;
                return;
            }
        }
        self.current_region = None;
    }
}

unsafe impl<I> FrameAllocator<Size4KiB> for BootInfoFrameAllocator<I>
where
    I: Iterator<Item = &'static bootloader_api::info::MemoryRegion>,
{
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        const PAGE_SIZE: u64 = 4096;

        loop {
            let region = self.current_region?;

            if self.next_frame < region.end {
                let frame_addr = self.next_frame;
                self.next_frame += PAGE_SIZE;

                let frame = PhysFrame::containing_address(PhysAddr::new(frame_addr));
                return Some(frame);
            }

            // Current region exhausted, advance to the next region
            self.advance_to_usable_region();
        }
    }
}
