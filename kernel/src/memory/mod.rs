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

pub mod buddy;
pub mod optimization;
pub mod slab;
pub mod user_page_table;

use alloc::vec::Vec;
use bootloader_api::info::MemoryRegionKind;
use spin::Mutex;
use x86_64::{
    structures::paging::{FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB},
    PhysAddr, VirtAddr,
};

/// Page size constant (4 KiB).
const PAGE_SIZE: usize = 4096;

/// Global frame allocator for slab and buddy systems.
static GLOBAL_FRAME_ALLOCATOR: Mutex<Option<GlobalFrameAllocator>> = Mutex::new(None);

/// Stack-based free frame list for reclaiming physical frames.
static GLOBAL_FREE_FRAMES: Mutex<FreeFrameList> = Mutex::new(FreeFrameList::new());

/// A stack-based list of freed physical frames available for reuse.
struct FreeFrameList {
    frames: Vec<usize>,
}

impl FreeFrameList {
    /// Create a new empty free frame list.
    const fn new() -> Self {
        Self {
            frames: Vec::new(),
        }
    }

    /// Push a freed frame address onto the list.
    fn push(&mut self, addr: usize) {
        self.frames.push(addr);
    }

    /// Pop a frame address from the list, if any.
    fn pop(&mut self) -> Option<usize> {
        self.frames.pop()
    }

    /// Number of frames in the free list.
    fn len(&self) -> usize {
        self.frames.len()
    }
}

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
        self.next_frame += PAGE_SIZE as u64;
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
///
/// First checks the free list for recycled frames. Falls back to the
/// bump allocator when no freed frames are available.
pub fn allocate_frame() -> Option<usize> {
    // Try recycled frames first
    if let Some(addr) = GLOBAL_FREE_FRAMES.lock().pop() {
        return Some(addr);
    }

    // Fall back to bump allocator
    GLOBAL_FRAME_ALLOCATOR
        .lock()
        .as_mut()?
        .allocate()
        .map(|f| f as usize)
}

/// Free a physical frame, returning it to the free list for reuse.
///
/// # Panics
///
/// Panics if `addr` is not aligned to [`PAGE_SIZE`] (4 KiB).
pub fn free_frame(addr: usize) {
    assert!(
        addr % PAGE_SIZE == 0,
        "free_frame: address {:#x} is not page-aligned (must be aligned to {:#x})",
        addr,
        PAGE_SIZE
    );

    GLOBAL_FREE_FRAMES.lock().push(addr);
}

/// Return the number of frames currently in the free list.
pub fn free_frame_count() -> usize {
    GLOBAL_FREE_FRAMES.lock().len()
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
    // Store the physical memory offset so user_page_table (and virt_to_phys)
    // can convert between virtual and physical addresses.
    user_page_table::init(physical_memory_offset);

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

/// Translate a virtual address to its physical address by walking the
/// active page tables through the physical memory window.
///
/// Returns `None` if any page table entry along the path is not present.
///
/// # Safety
///
/// This function reads page table memory through the physical-memory mapping.
/// The caller must ensure `user_page_table::init()` has been called (which is
/// done automatically by `memory::init()`).
pub fn virt_to_phys(virt_addr: u64) -> Option<u64> {
    let phys_offset = user_page_table::get_phys_offset();
    if phys_offset == 0 {
        // Phys offset not yet initialised — fall back to identity assumption.
        return Some(virt_addr);
    }

    // Walk the 4-level page table to translate any virtual address.
    use x86_64::registers::control::Cr3;
    let cr3 = Cr3::read().0.start_address().as_u64();

    let indices = [
        ((virt_addr >> 39) & 0x1FF) as usize, // PML4
        ((virt_addr >> 30) & 0x1FF) as usize, // PDPT
        ((virt_addr >> 21) & 0x1FF) as usize, // PD
        ((virt_addr >> 12) & 0x1FF) as usize, // PT
    ];

    unsafe {
        let pml4 = (phys_offset + cr3) as *const u64;
        let pml4e = core::ptr::read_volatile(pml4.add(indices[0]));
        if pml4e & 1 == 0 {
            return None;
        }

        let pdpt = (phys_offset + (pml4e & 0x000F_FFFF_FFFF_F000)) as *const u64;
        let pdpte = core::ptr::read_volatile(pdpt.add(indices[1]));
        if pdpte & 1 == 0 {
            return None;
        }
        // 1 GiB huge page
        if pdpte & (1 << 7) != 0 {
            return Some((pdpte & 0x000F_FFFF_C000_0000) | (virt_addr & 0x3FFF_FFFF));
        }

        let pd = (phys_offset + (pdpte & 0x000F_FFFF_FFFF_F000)) as *const u64;
        let pde = core::ptr::read_volatile(pd.add(indices[2]));
        if pde & 1 == 0 {
            return None;
        }
        // 2 MiB huge page
        if pde & (1 << 7) != 0 {
            return Some((pde & 0x000F_FFFF_FFE0_0000) | (virt_addr & 0x1F_FFFF));
        }

        let pt = (phys_offset + (pde & 0x000F_FFFF_FFFF_F000)) as *const u64;
        let pte = core::ptr::read_volatile(pt.add(indices[3]));
        if pte & 1 == 0 {
            return None;
        }
        Some((pte & 0x000F_FFFF_FFFF_F000) | (virt_addr & 0xFFF))
    }
}
