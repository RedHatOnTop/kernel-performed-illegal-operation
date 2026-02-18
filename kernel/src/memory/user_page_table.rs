//! Per-Process User Page Table Management
//!
//! Creates and manages individual page tables for user-space processes.
//! Each process gets its own P4 (PML4) table with:
//! - Indices 0-255: User space (unique per process)
//! - Indices 256-511: Kernel space (shared, copied from kernel P4)
//!
//! # Address Space Layout
//!
//! ```text
//! 0x0000_0000_0000 - 0x7FFF_FFFF_FFFF  User space (128 TiB, per-process)
//! 0xFFFF_8000_0000_0000 - ...           Kernel space (shared)
//! ```

use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};

/// Physical memory offset (set during kernel init).
static PHYS_OFFSET: AtomicU64 = AtomicU64::new(0);

/// Initialize the user page table subsystem.
///
/// Must be called after the physical memory offset is known (from bootloader).
pub fn init(phys_offset: u64) {
    PHYS_OFFSET.store(phys_offset, Ordering::Release);
}

/// Get the stored physical memory offset.
fn phys_offset() -> VirtAddr {
    VirtAddr::new(PHYS_OFFSET.load(Ordering::Acquire))
}

/// Get the raw physical memory offset value (for other modules).
pub fn get_phys_offset() -> u64 {
    PHYS_OFFSET.load(Ordering::Acquire)
}

/// Adapter to use our global frame allocator with x86_64 crate's Mapper.
struct KernelFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for KernelFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        crate::memory::allocate_frame()
            .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr as u64)))
    }
}

/// Create a new user-space page table.
///
/// Allocates a P4 frame, copies kernel half-space entries (indices 256-511)
/// from the current CR3, and zeroes user-space entries (indices 0-255).
///
/// # Returns
///
/// Physical address of the new P4 frame (to be stored in CR3).
///
/// # Errors
///
/// Returns error if frame allocation fails.
pub fn create_user_page_table() -> Result<u64, &'static str> {
    let offset = phys_offset();
    if offset.as_u64() == 0 {
        return Err("User page table subsystem not initialized");
    }

    // Allocate a physical frame for the new P4 table
    let l4_phys = crate::memory::allocate_frame().ok_or("Out of memory: cannot allocate P4 frame")?;

    // Access the new P4 via physical offset mapping
    let l4_virt = offset + l4_phys as u64;
    let new_l4: &mut PageTable = unsafe { &mut *l4_virt.as_mut_ptr::<PageTable>() };

    // Zero all entries first (user space: indices 0-255)
    for entry in new_l4.iter_mut() {
        entry.set_unused();
    }

    // Copy kernel half-space entries (indices 256-511) from current P4
    let current_l4 = unsafe { current_level_4_table(offset) };
    for i in 256..512 {
        new_l4[i] = current_l4[i].clone();
    }

    Ok(l4_phys as u64)
}

/// Map a single 4KB user-space page.
///
/// Allocates a physical frame for the page data and maps it into the given
/// page table at the specified virtual address.
///
/// # Arguments
///
/// * `cr3_phys` - Physical address of the process's P4 table
/// * `virt_addr` - Virtual address to map (will be page-aligned)
/// * `flags` - Page table flags (USER_ACCESSIBLE is always added)
///
/// # Returns
///
/// Physical address of the allocated frame on success.
pub fn map_user_page(
    cr3_phys: u64,
    virt_addr: u64,
    flags: PageTableFlags,
) -> Result<u64, &'static str> {
    let offset = phys_offset();

    // Allocate a physical frame for the page data
    let frame_phys =
        crate::memory::allocate_frame().ok_or("Out of memory: cannot allocate user page frame")?;


    // Zero the frame
    let frame_virt = offset + frame_phys as u64;
    unsafe {
        core::ptr::write_bytes(frame_virt.as_mut_ptr::<u8>(), 0, 4096);
    }

    // Map the page in the user's page table
    map_user_page_at(cr3_phys, virt_addr, frame_phys as u64, flags)?;

    Ok(frame_phys as u64)
}

/// Map a pre-allocated physical frame at a user-space virtual address.
///
/// # Arguments
///
/// * `cr3_phys` - Physical address of the process's P4 table
/// * `virt_addr` - Virtual address to map
/// * `frame_phys` - Physical address of the frame to map
/// * `flags` - Page table flags (USER_ACCESSIBLE + PRESENT always added)
pub fn map_user_page_at(
    cr3_phys: u64,
    virt_addr: u64,
    frame_phys: u64,
    flags: PageTableFlags,
) -> Result<(), &'static str> {
    let offset = phys_offset();

    // Get the process's L4 table
    let l4_virt = offset + cr3_phys;
    let l4_table: &mut PageTable = unsafe { &mut *l4_virt.as_mut_ptr::<PageTable>() };

    // Create a temporary OffsetPageTable mapper
    let mut mapper = unsafe { OffsetPageTable::new(l4_table, offset) };

    let page = Page::<Size4KiB>::containing_address(VirtAddr::new(virt_addr));
    let frame: PhysFrame<Size4KiB> = PhysFrame::containing_address(PhysAddr::new(frame_phys));
    let full_flags = flags | PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;

    unsafe {
        mapper
            .map_to(page, frame, full_flags, &mut KernelFrameAllocator)
            .map_err(|_| "Failed to map user page")?
            .flush();
    }

    Ok(())
}

/// Map a contiguous range of user-space pages.
///
/// Allocates physical frames and maps them at consecutive virtual addresses.
///
/// # Arguments
///
/// * `cr3_phys` - Physical address of the process's P4 table
/// * `virt_start` - Starting virtual address (page-aligned)
/// * `size` - Size in bytes (will be rounded up to page boundary)
/// * `flags` - Page table flags
///
/// # Returns
///
/// Vector of allocated physical frame addresses.
pub fn map_user_range(
    cr3_phys: u64,
    virt_start: u64,
    size: u64,
    flags: PageTableFlags,
) -> Result<alloc::vec::Vec<u64>, &'static str> {
    let page_start = virt_start & !0xFFF;
    let page_end = (virt_start + size + 0xFFF) & !0xFFF;
    let mut frames = alloc::vec::Vec::new();

    for page_addr in (page_start..page_end).step_by(4096) {
        let frame_phys = map_user_page(cr3_phys, page_addr, flags)?;
        frames.push(frame_phys);
    }

    Ok(frames)
}

/// Unmap a single user-space page and free its physical frame.
///
/// # Arguments
///
/// * `cr3_phys` - Physical address of the process's P4 table
/// * `virt_addr` - Virtual address to unmap
pub fn unmap_user_page(cr3_phys: u64, virt_addr: u64) -> Result<(), &'static str> {
    let offset = phys_offset();

    let l4_virt = offset + cr3_phys;
    let l4_table: &mut PageTable = unsafe { &mut *l4_virt.as_mut_ptr::<PageTable>() };

    let mut mapper = unsafe { OffsetPageTable::new(l4_table, offset) };

    let page = Page::<Size4KiB>::containing_address(VirtAddr::new(virt_addr));

    let (frame, flush) = mapper.unmap(page).map_err(|_| "Failed to unmap user page")?;

    flush.flush();

    // Free the physical frame
    crate::memory::free_frame(frame.start_address().as_u64() as usize);

    Ok(())
}

/// Destroy a user-space page table and free all user-space frames.
///
/// Walks P4 entries 0-255 (user space) and recursively frees all page table
/// frames and mapped data frames. Does NOT free kernel half-space entries.
///
/// # Arguments
///
/// * `cr3_phys` - Physical address of the P4 frame to destroy
pub fn destroy_user_page_table(cr3_phys: u64) -> Result<(), &'static str> {
    let offset = phys_offset();
    let l4_virt = offset + cr3_phys;
    let l4_table: &PageTable = unsafe { &*l4_virt.as_mut_ptr::<PageTable>() };

    // Walk user-space entries (indices 0-255)
    for i in 0..256 {
        let l4_entry = &l4_table[i];
        if !l4_entry.flags().contains(PageTableFlags::PRESENT) {
            continue;
        }

        let l3_phys = l4_entry.addr();
        let l3_virt = offset + l3_phys.as_u64();
        let l3_table: &PageTable = unsafe { &*l3_virt.as_mut_ptr::<PageTable>() };

        for j in 0..512 {
            let l3_entry = &l3_table[j];
            if !l3_entry.flags().contains(PageTableFlags::PRESENT) {
                continue;
            }
            // Skip huge pages (1GB)
            if l3_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                continue;
            }

            let l2_phys = l3_entry.addr();
            let l2_virt = offset + l2_phys.as_u64();
            let l2_table: &PageTable = unsafe { &*l2_virt.as_mut_ptr::<PageTable>() };

            for k in 0..512 {
                let l2_entry = &l2_table[k];
                if !l2_entry.flags().contains(PageTableFlags::PRESENT) {
                    continue;
                }
                // Skip huge pages (2MB)
                if l2_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                    continue;
                }

                let l1_phys = l2_entry.addr();
                let l1_virt = offset + l1_phys.as_u64();
                let l1_table: &PageTable = unsafe { &*l1_virt.as_mut_ptr::<PageTable>() };

                // Free all mapped data frames in L1
                for l in 0..512 {
                    let l1_entry = &l1_table[l];
                    if l1_entry.flags().contains(PageTableFlags::PRESENT) {
                        crate::memory::free_frame(l1_entry.addr().as_u64() as usize);
                    }
                }

                // Free L1 table frame
                crate::memory::free_frame(l1_phys.as_u64() as usize);
            }

            // Free L2 table frame
            crate::memory::free_frame(l2_phys.as_u64() as usize);
        }

        // Free L3 table frame
        crate::memory::free_frame(l3_phys.as_u64() as usize);
    }

    // Free the P4 frame itself
    crate::memory::free_frame(cr3_phys as usize);

    Ok(())
}

/// Write data to a physical address (via offset mapping).
///
/// Used to write ELF segment data directly to physical frames
/// without switching page tables.
///
/// # Safety
///
/// The caller must ensure `phys_addr` points to a valid mapped frame
/// and `data` does not exceed the frame boundary.
pub unsafe fn write_to_phys(phys_addr: u64, offset_in_page: usize, data: &[u8]) {
    let offset = phys_offset();
    let dst = (offset + phys_addr + offset_in_page as u64).as_mut_ptr::<u8>();
    unsafe {
        core::ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
    }
}

/// Read the active level 4 page table.
///
/// # Safety
///
/// All physical memory must be mapped at `physical_memory_offset`.
unsafe fn current_level_4_table(physical_memory_offset: VirtAddr) -> &'static PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    unsafe { &*virt.as_ptr() }
}

extern crate alloc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phys_offset_init() {
        // Test that init stores the offset correctly
        let test_offset = 0xFFFF_8880_0000_0000u64;
        PHYS_OFFSET.store(test_offset, Ordering::SeqCst);
        assert_eq!(phys_offset().as_u64(), test_offset);
        // Reset
        PHYS_OFFSET.store(0, Ordering::SeqCst);
    }

    #[test]
    fn test_page_alignment() {
        // virt_start=0x401000, size=0x2000 should cover pages 0x401000 and 0x402000
        let start = 0x401000u64;
        let size = 0x2000u64;
        let page_start = start & !0xFFF;
        let page_end = (start + size + 0xFFF) & !0xFFF;
        assert_eq!(page_start, 0x401000);
        assert_eq!(page_end, 0x403000);
        assert_eq!((page_end - page_start) / 4096, 2);
    }

    #[test]
    fn test_unaligned_range() {
        // virt_start=0x400100, size=0x100 should still map one page at 0x400000
        let start = 0x400100u64;
        let size = 0x100u64;
        let page_start = start & !0xFFF;
        let page_end = (start + size + 0xFFF) & !0xFFF;
        assert_eq!(page_start, 0x400000);
        assert_eq!(page_end, 0x401000);
        assert_eq!((page_end - page_start) / 4096, 1);
    }

    #[test]
    fn test_kernel_half_space_indices() {
        // Kernel space starts at 0xFFFF_8000_0000_0000
        // P4 index for this address: bits 47:39
        let kernel_start: u64 = 0xFFFF_8000_0000_0000;
        let p4_index = (kernel_start >> 39) & 0x1FF;
        assert_eq!(p4_index, 256);
    }

    #[test]
    fn test_user_space_index_range() {
        // User space: 0x0 to 0x7FFF_FFFF_FFFF
        // Max P4 index for user space
        let user_max: u64 = 0x7FFF_FFFF_FFFF;
        let p4_index = (user_max >> 39) & 0x1FF;
        assert_eq!(p4_index, 255);
    }

    #[test]
    fn test_page_table_flags() {
        let flags =
            PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE;
        assert!(flags.contains(PageTableFlags::PRESENT));
        assert!(flags.contains(PageTableFlags::USER_ACCESSIBLE));
        assert!(flags.contains(PageTableFlags::WRITABLE));
        assert!(!flags.contains(PageTableFlags::NO_EXECUTE));
    }
}
