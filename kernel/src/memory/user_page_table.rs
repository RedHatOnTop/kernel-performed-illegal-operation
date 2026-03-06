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
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PhysFrame, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};

// Re-export PageTableFlags for use by syscall handlers
pub use x86_64::structures::paging::PageTableFlags;

/// Copy-on-Write marker bit.
///
/// Uses bit 9 of the PTE, which is one of the three OS-available bits
/// in the x86_64 page table entry format (bits 9, 10, 11).
/// The x86_64 crate v0.15 does not export a named constant for this.
pub const COW_BIT: PageTableFlags = PageTableFlags::from_bits_retain(1 << 9);

/// Physical memory offset (set during kernel init).
static PHYS_OFFSET: AtomicU64 = AtomicU64::new(0);

/// Saved kernel CR3 at init time (before any user page table operations).
static KERNEL_CR3: AtomicU64 = AtomicU64::new(0);

/// Initialize the user page table subsystem.
///
/// Must be called after the physical memory offset is known (from bootloader).
/// Saves the kernel's CR3 so that `create_user_page_table` always copies from
/// the pristine kernel table rather than from whatever CR3 is active at call
/// time (which could be a user process's table after a context switch).
pub fn init(phys_offset: u64) {
    PHYS_OFFSET.store(phys_offset, Ordering::Release);

    // Save the kernel's original CR3 before any user-space operations
    use x86_64::registers::control::Cr3;
    let (frame, _) = Cr3::read();
    KERNEL_CR3.store(frame.start_address().as_u64(), Ordering::Release);
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
    let l4_phys =
        crate::memory::allocate_frame().ok_or("Out of memory: cannot allocate P4 frame")?;

    // Access the new P4 via physical offset mapping
    let l4_virt = offset + l4_phys as u64;
    let new_l4: &mut PageTable = unsafe { &mut *l4_virt.as_mut_ptr::<PageTable>() };

    // Zero all entries first
    for entry in new_l4.iter_mut() {
        entry.set_unused();
    }

    // Copy P4 entries from the kernel's ORIGINAL page table (saved at
    // init time, before any user-space mapping operations).
    //
    // UPPER HALF (indices 256-511): Always copied — these contain the
    // kernel's own mappings (heap, stacks, kernel code in high
    // canonical addresses).
    //
    // LOWER HALF (indices 0-255): Only the entries the kernel needs
    // for its own operation are copied.  The bootloader places
    // certain kernel-critical mappings in the lower half:
    //   - Physical memory offset (P4 index = phys_offset >> 39)
    //   - Kernel ELF segments (typically P4[2])
    //   - Boot info (typically P4[7])
    //
    // We must NOT copy ALL lower-half entries because:
    //   1. P4 entries are shallow — they point to shared P3/P2/P1
    //      frames.  If a previous `map_user_page` call created user-
    //      accessible entries under a shared P4 entry (e.g., P4[0]),
    //      those modifications are visible through every copy.
    //   2. The bootloader may leave identity mappings at P4[0] that
    //      conflict with the standard ELF load address (0x400000).
    //
    // Solution: copy ONLY entries whose P4 index ≥ 1.  P4[0] covers
    // virtual addresses 0x0–0x7F_FFFF_FFFF, which is precisely the
    // range reserved for user-space ELF binaries and stacks.  The
    // kernel never accesses this range — all kernel infrastructure
    // sits at P4[2] (ELF), P4[5] (phys offset), P4[7] (boot info),
    // or P4[256+] (kernel half).
    //
    // Security: Ring 3 code still cannot access kernel pages because
    // the CPU enforces the page-level U/S bit.  Only pages mapped
    // with USER_ACCESSIBLE via `map_user_page` are ring-3 accessible.
    let kernel_cr3 = KERNEL_CR3.load(Ordering::Acquire);
    let source_l4 = if kernel_cr3 != 0 {
        let virt = offset + kernel_cr3;
        unsafe { &*virt.as_ptr::<PageTable>() }
    } else {
        // Fallback: use current CR3 if init() wasn't called yet
        unsafe { current_level_4_table(offset) }
    };

    // Upper half: copy unconditionally
    for i in 256..512 {
        new_l4[i] = source_l4[i].clone();
    }

    // Lower half: copy all entries EXCEPT P4[0] (user-space ELF range)
    for i in 1..256 {
        if !source_l4[i].is_unused() {
            new_l4[i] = source_l4[i].clone();
        }
    }
    // P4[0] is intentionally left zeroed — fresh for user-space mapping

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
        match mapper.map_to(page, frame, full_flags, &mut KernelFrameAllocator) {
            Ok(flush) => {
                flush.flush();
                Ok(())
            }
            Err(e) => {
                use x86_64::structures::paging::mapper::MapToError;
                match e {
                    MapToError::FrameAllocationFailed => {
                        Err("map_to: FrameAllocationFailed (KernelFrameAllocator returned None)")
                    }
                    MapToError::PageAlreadyMapped(_f) => Err("map_to: PageAlreadyMapped"),
                    MapToError::ParentEntryHugePage => Err("map_to: ParentEntryHugePage"),
                }
            }
        }
    }
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

    let (frame, flush) = mapper
        .unmap(page)
        .map_err(|_| "Failed to unmap user page")?;

    flush.flush();

    // Free the physical frame
    crate::memory::free_frame(frame.start_address().as_u64() as usize);

    Ok(())
}

/// Destroy a user-space page table and free all user-space frames.
///
/// Walks P4 entries 0-255 (user space) and recursively frees all page table
/// frames and mapped data frames. CoW-shared frames are only freed when
/// their reference count drops to 0.
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
                if l2_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                    continue;
                }

                let l1_phys = l2_entry.addr();
                let l1_virt = offset + l1_phys.as_u64();
                let l1_table: &PageTable = unsafe { &*l1_virt.as_mut_ptr::<PageTable>() };

                // Free or decrement refcount for mapped data frames in L1
                for l in 0..512 {
                    let l1_entry = &l1_table[l];
                    if l1_entry.flags().contains(PageTableFlags::PRESENT) {
                        let data_phys = l1_entry.addr().as_u64();
                        let new_refcount = crate::memory::refcount::decrement(data_phys);
                        if new_refcount == 0 {
                            crate::memory::free_frame(data_phys as usize);
                        }
                    }
                }

                // Free L1 table frame (always unique per process)
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

/// Clone a user-space page table with Copy-on-Write semantics.
///
/// Creates a new P4 table where:
/// - Kernel-half entries (256-511) are shared (copied from parent)
/// - User-half entries (0-255) are deeply copied at the page-table
///   structure level (L3/L2/L1 frames are fresh allocations).
/// - Leaf data frames are **shared** via CoW: both parent and child
///   PTEs point to the same physical frame, the WRITABLE bit is
///   cleared, and the COW_BIT marker is set on both PTEs.
///
/// When either process writes to a CoW page, the page fault handler
/// allocates a private copy and restores the WRITABLE bit.
///
/// # Arguments
///
/// * `parent_cr3` - Physical address of the parent's P4 table
///
/// # Returns
///
/// Physical address of the new (child) P4 table.
pub fn clone_user_page_table(parent_cr3: u64) -> Result<u64, &'static str> {
    let offset = phys_offset();
    if offset.as_u64() == 0 {
        return Err("User page table subsystem not initialized");
    }

    // Allocate a new P4 frame for the child
    let child_l4_phys =
        crate::memory::allocate_frame().ok_or("Out of memory: cannot allocate child P4 frame")?;
    let child_l4_virt = offset + child_l4_phys as u64;
    let child_l4: &mut PageTable = unsafe { &mut *child_l4_virt.as_mut_ptr::<PageTable>() };

    // Access parent P4 — mutable so we can update PTEs for CoW
    let parent_l4_virt = offset + parent_cr3;
    let parent_l4: &mut PageTable = unsafe { &mut *parent_l4_virt.as_mut_ptr::<PageTable>() };

    // Copy kernel half (indices 256-511) directly — shared across all processes
    for i in 256..512 {
        child_l4[i] = parent_l4[i].clone();
    }

    // Copy kernel lower-half entries (indices 1-255) as shallow copies.
    // These contain bootloader/kernel infrastructure (ELF, phys offset,
    // boot info) and must NOT be deep-cloned — they share page table
    // structure with the kernel.  Only P4[0] contains user-space pages
    // that need CoW treatment.
    for i in 1..256 {
        child_l4[i] = parent_l4[i].clone();
    }

    let mut shared_pages: u64 = 0;

    // Deep-clone P4[0] only (user-space ELF range: 0x0–0x7F_FFFF_FFFF)
    {
        let i = 0;
        let parent_entry = &parent_l4[i];
        if !parent_entry.flags().contains(PageTableFlags::PRESENT) {
            child_l4[i].set_unused();
        } else {

        // Allocate child L3
        let child_l3_phys = crate::memory::allocate_frame()
            .ok_or("Out of memory: cannot allocate child L3 frame")?;
        let child_l3_virt = offset + child_l3_phys as u64;
        let child_l3: &mut PageTable = unsafe { &mut *child_l3_virt.as_mut_ptr::<PageTable>() };

        let parent_l3_phys = parent_entry.addr();
        let parent_l3_virt = offset + parent_l3_phys.as_u64();
        let parent_l3: &mut PageTable = unsafe { &mut *parent_l3_virt.as_mut_ptr::<PageTable>() };

        for j in 0..512 {
            let parent_l3e = &parent_l3[j];
            if !parent_l3e.flags().contains(PageTableFlags::PRESENT) {
                child_l3[j].set_unused();
                continue;
            }
            // Skip huge pages (1GB)
            if parent_l3e.flags().contains(PageTableFlags::HUGE_PAGE) {
                child_l3[j] = parent_l3e.clone();
                continue;
            }

            // Allocate child L2
            let child_l2_phys = crate::memory::allocate_frame()
                .ok_or("Out of memory: cannot allocate child L2 frame")?;
            let child_l2_virt = offset + child_l2_phys as u64;
            let child_l2: &mut PageTable = unsafe { &mut *child_l2_virt.as_mut_ptr::<PageTable>() };

            let parent_l2_phys = parent_l3e.addr();
            let parent_l2_virt = offset + parent_l2_phys.as_u64();
            let parent_l2: &mut PageTable =
                unsafe { &mut *parent_l2_virt.as_mut_ptr::<PageTable>() };

            for k in 0..512 {
                let parent_l2e = &parent_l2[k];
                if !parent_l2e.flags().contains(PageTableFlags::PRESENT) {
                    child_l2[k].set_unused();
                    continue;
                }
                // Skip huge pages (2MB)
                if parent_l2e.flags().contains(PageTableFlags::HUGE_PAGE) {
                    child_l2[k] = parent_l2e.clone();
                    continue;
                }

                // Allocate child L1
                let child_l1_phys = crate::memory::allocate_frame()
                    .ok_or("Out of memory: cannot allocate child L1 frame")?;
                let child_l1_virt = offset + child_l1_phys as u64;
                let child_l1: &mut PageTable =
                    unsafe { &mut *child_l1_virt.as_mut_ptr::<PageTable>() };

                let parent_l1_phys = parent_l2e.addr();
                let parent_l1_virt = offset + parent_l1_phys.as_u64();
                let parent_l1: &mut PageTable =
                    unsafe { &mut *parent_l1_virt.as_mut_ptr::<PageTable>() };

                // CoW: share data frames between parent and child.
                // Both PTEs are marked read-only + COW_BIT.
                for l in 0..512 {
                    if parent_l1[l].flags().contains(PageTableFlags::PRESENT) {
                        let data_phys = parent_l1[l].addr().as_u64();
                        let mut flags = parent_l1[l].flags();

                        // Mark CoW: clear WRITABLE, set COW_BIT on both PTEs.
                        // If the page was already CoW (e.g., chained forks),
                        // just increment the refcount.
                        let was_writable = flags.contains(PageTableFlags::WRITABLE);
                        if was_writable {
                            flags = (flags & !PageTableFlags::WRITABLE) | COW_BIT;
                            // Update parent PTE to read-only + CoW
                            parent_l1[l].set_addr(PhysAddr::new(data_phys), flags);
                        }
                        // If already CoW (not writable + COW_BIT set), just
                        // share with the same flags.

                        // Child gets same flags (read-only + CoW marker)
                        child_l1[l].set_addr(PhysAddr::new(data_phys), flags);

                        // Increment reference count for the shared frame
                        crate::memory::refcount::increment(data_phys);
                        shared_pages += 1;
                    } else {
                        child_l1[l].set_unused();
                    }
                }

                // Set child L2 entry to point to the new L1
                child_l2[k] = parent_l2[k].clone();
                child_l2[k].set_addr(PhysAddr::new(child_l1_phys as u64), parent_l2[k].flags());
            }

            // Set child L3 entry to point to the new L2
            child_l3[j] = parent_l3[j].clone();
            child_l3[j].set_addr(PhysAddr::new(child_l2_phys as u64), parent_l3[j].flags());
        }

        // Set child L4 entry to point to the new L3
        child_l4[i] = parent_l4[i].clone();
        child_l4[i].set_addr(PhysAddr::new(child_l3_phys as u64), parent_l4[i].flags());
        } // else (P4[0] present)
    } // Deep-clone P4[0] block

    // Flush TLB for the parent — PTEs were changed (WRITABLE cleared)
    // SAFETY: we only modified user-space PTEs; a full TLB flush is safe.
    unsafe {
        core::arch::asm!("mov cr3, {}", in(reg) parent_cr3, options(nostack, preserves_flags));
    }

    crate::serial_println!("[CoW] fork shared {} pages (refcounted)", shared_pages);

    Ok(child_l4_phys as u64)
}

/// Update page table entry flags in-place for a mapped page.
///
/// Walks the page table for `virt_addr` and sets new flags on the
/// leaf (L1) PTE.  Returns `Ok(())` if the page was found and updated,
/// or `Err` if the page is not mapped.
///
/// # Arguments
///
/// * `cr3_phys` - Physical address of the P4 table
/// * `virt_addr` - Virtual address of the page to update
/// * `new_flags` - New page table flags (PRESENT + USER_ACCESSIBLE are forced)
pub fn update_pte_flags(
    cr3_phys: u64,
    virt_addr: u64,
    new_flags: PageTableFlags,
) -> Result<(), &'static str> {
    let offset = phys_offset();

    let l4_index = ((virt_addr >> 39) & 0x1FF) as usize;
    let l3_index = ((virt_addr >> 30) & 0x1FF) as usize;
    let l2_index = ((virt_addr >> 21) & 0x1FF) as usize;
    let l1_index = ((virt_addr >> 12) & 0x1FF) as usize;

    let l4_virt = offset + cr3_phys;
    let l4: &PageTable = unsafe { &*l4_virt.as_mut_ptr::<PageTable>() };

    let l4e = &l4[l4_index];
    if !l4e.flags().contains(PageTableFlags::PRESENT) {
        return Err("L4 entry not present");
    }

    let l3_virt = offset + l4e.addr().as_u64();
    let l3: &PageTable = unsafe { &*l3_virt.as_mut_ptr::<PageTable>() };

    let l3e = &l3[l3_index];
    if !l3e.flags().contains(PageTableFlags::PRESENT) {
        return Err("L3 entry not present");
    }

    let l2_virt = offset + l3e.addr().as_u64();
    let l2: &PageTable = unsafe { &*l2_virt.as_mut_ptr::<PageTable>() };

    let l2e = &l2[l2_index];
    if !l2e.flags().contains(PageTableFlags::PRESENT) {
        return Err("L2 entry not present");
    }

    let l1_virt = offset + l2e.addr().as_u64();
    let l1: &mut PageTable = unsafe { &mut *l1_virt.as_mut_ptr::<PageTable>() };

    let l1e = &mut l1[l1_index];
    if !l1e.flags().contains(PageTableFlags::PRESENT) {
        return Err("L1 entry not present (page not mapped)");
    }

    // Preserve the physical address, update flags
    let phys = l1e.addr();
    let full_flags = new_flags | PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
    l1e.set_addr(phys, full_flags);

    Ok(())
}

/// Destroy user-space mappings (entries 0-255) without freeing the P4 frame.
///
/// This is used by `execve()` to clear the old address space while
/// keeping the same P4 frame (CR3 stays the same).
/// CoW-shared frames are only freed when their reference count drops to 0.
pub fn destroy_user_mappings(cr3_phys: u64) -> Result<(), &'static str> {
    let offset = phys_offset();
    let l4_virt = offset + cr3_phys;
    let l4_table: &mut PageTable = unsafe { &mut *l4_virt.as_mut_ptr::<PageTable>() };

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
                if l2_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                    continue;
                }

                let l1_phys = l2_entry.addr();
                let l1_virt = offset + l1_phys.as_u64();
                let l1_table: &PageTable = unsafe { &*l1_virt.as_mut_ptr::<PageTable>() };

                for l in 0..512 {
                    let l1_entry = &l1_table[l];
                    if l1_entry.flags().contains(PageTableFlags::PRESENT) {
                        let data_phys = l1_entry.addr().as_u64();
                        let new_refcount = crate::memory::refcount::decrement(data_phys);
                        if new_refcount == 0 {
                            crate::memory::free_frame(data_phys as usize);
                        }
                    }
                }

                crate::memory::free_frame(l1_phys.as_u64() as usize);
            }

            crate::memory::free_frame(l2_phys.as_u64() as usize);
        }

        crate::memory::free_frame(l3_phys.as_u64() as usize);

        // Clear the L4 entry
        l4_table[i].set_unused();
    }

    Ok(())
}

/// Read the L1 (leaf) PTE flags and physical address for a given virtual address.
///
/// Walks the page table for `cr3_phys` and returns `(phys_addr, flags)` of
/// the L1 entry. Returns `None` if any level is not present.
pub fn read_pte(cr3_phys: u64, virt_addr: u64) -> Option<(u64, PageTableFlags)> {
    let offset = phys_offset();

    let l4_index = ((virt_addr >> 39) & 0x1FF) as usize;
    let l3_index = ((virt_addr >> 30) & 0x1FF) as usize;
    let l2_index = ((virt_addr >> 21) & 0x1FF) as usize;
    let l1_index = ((virt_addr >> 12) & 0x1FF) as usize;

    // SAFETY: we access page tables via the kernel's physical memory mapping.
    unsafe {
        let l4: &PageTable = &*(offset + cr3_phys).as_ptr::<PageTable>();
        let l4e = &l4[l4_index];
        if !l4e.flags().contains(PageTableFlags::PRESENT) {
            return None;
        }

        let l3: &PageTable = &*(offset + l4e.addr().as_u64()).as_ptr::<PageTable>();
        let l3e = &l3[l3_index];
        if !l3e.flags().contains(PageTableFlags::PRESENT) {
            return None;
        }

        let l2: &PageTable = &*(offset + l3e.addr().as_u64()).as_ptr::<PageTable>();
        let l2e = &l2[l2_index];
        if !l2e.flags().contains(PageTableFlags::PRESENT) {
            return None;
        }

        let l1: &PageTable = &*(offset + l2e.addr().as_u64()).as_ptr::<PageTable>();
        let l1e = &l1[l1_index];
        if !l1e.flags().contains(PageTableFlags::PRESENT) {
            return None;
        }

        Some((l1e.addr().as_u64(), l1e.flags()))
    }
}

/// Handle a Copy-on-Write page fault.
///
/// Called when a user-mode process writes to a CoW-shared page
/// (PROTECTION_VIOLATION + CAUSED_BY_WRITE + USER_MODE + COW_BIT set).
///
/// 1. Allocates a new physical frame.
/// 2. Copies the 4 KiB data from the shared frame.
/// 3. Maps the new frame with WRITABLE, clears COW_BIT.
/// 4. Decrements the old frame's refcount (frees if count reaches 0).
/// 5. Invalidates the TLB entry for the faulting address.
///
/// # Returns
///
/// `true` if the CoW fault was handled successfully, `false` if it failed.
pub fn handle_cow_fault(cr3_phys: u64, fault_addr: u64) -> bool {
    let offset = phys_offset();

    let l4_index = ((fault_addr >> 39) & 0x1FF) as usize;
    let l3_index = ((fault_addr >> 30) & 0x1FF) as usize;
    let l2_index = ((fault_addr >> 21) & 0x1FF) as usize;
    let l1_index = ((fault_addr >> 12) & 0x1FF) as usize;

    // Navigate to the L1 PTE.
    // SAFETY: we access page tables via the kernel's physical memory mapping.
    let l1e_flags;
    let old_phys;
    let l1_table_ptr;

    unsafe {
        let l4: &PageTable = &*(offset + cr3_phys).as_ptr::<PageTable>();
        let l4e = &l4[l4_index];
        if !l4e.flags().contains(PageTableFlags::PRESENT) {
            return false;
        }

        let l3: &PageTable = &*(offset + l4e.addr().as_u64()).as_ptr::<PageTable>();
        let l3e = &l3[l3_index];
        if !l3e.flags().contains(PageTableFlags::PRESENT) {
            return false;
        }

        let l2: &PageTable = &*(offset + l3e.addr().as_u64()).as_ptr::<PageTable>();
        let l2e = &l2[l2_index];
        if !l2e.flags().contains(PageTableFlags::PRESENT) {
            return false;
        }

        let l1: &PageTable = &*(offset + l2e.addr().as_u64()).as_ptr::<PageTable>();
        let l1e = &l1[l1_index];
        if !l1e.flags().contains(PageTableFlags::PRESENT) {
            return false;
        }

        l1e_flags = l1e.flags();
        old_phys = l1e.addr().as_u64();
        l1_table_ptr = (offset + l2e.addr().as_u64()).as_u64();
    }

    // Verify this is actually a CoW page.
    if !l1e_flags.contains(COW_BIT) {
        return false;
    }

    // Check if we're the only reference (refcount == 1).
    // In that case, just restore WRITABLE and clear COW_BIT — no copy needed.
    let refcount = crate::memory::refcount::get(old_phys);
    if refcount <= 1 {
        // We're the sole owner — just make writable again.
        let new_flags = (l1e_flags | PageTableFlags::WRITABLE) & !COW_BIT;
        // SAFETY: updating a valid L1 PTE.
        unsafe {
            let l1: &mut PageTable = &mut *(l1_table_ptr as *mut PageTable);
            l1[l1_index].set_addr(PhysAddr::new(old_phys), new_flags);
        }
        // Invalidate TLB for this page.
        let page_addr = fault_addr & !0xFFF;
        unsafe {
            core::arch::asm!("invlpg [{}]", in(reg) page_addr, options(nostack, preserves_flags));
        }
        crate::serial_println!(
            "[CoW] fault handled (sole owner, no copy needed) at {:#x}",
            fault_addr
        );
        return true;
    }

    // Allocate a new frame for the private copy.
    let new_frame = match crate::memory::allocate_frame() {
        Some(f) => f as u64,
        None => {
            crate::serial_println!(
                "[CoW] FATAL: out of memory during CoW fault at {:#x}",
                fault_addr
            );
            return false;
        }
    };

    // Copy 4 KiB from old frame to new frame.
    // SAFETY: both frames are valid mapped physical memory.
    unsafe {
        let src = (offset + old_phys).as_ptr::<u8>();
        let dst = (offset + new_frame).as_mut_ptr::<u8>();
        core::ptr::copy_nonoverlapping(src, dst, 4096);
    }

    // Update the L1 PTE: point to new frame, restore WRITABLE, clear COW_BIT.
    let new_flags = (l1e_flags | PageTableFlags::WRITABLE) & !COW_BIT;
    // SAFETY: updating a valid L1 PTE with the new private frame.
    unsafe {
        let l1: &mut PageTable = &mut *(l1_table_ptr as *mut PageTable);
        l1[l1_index].set_addr(PhysAddr::new(new_frame), new_flags);
    }

    // Decrement refcount on the old shared frame.
    let new_refcount = crate::memory::refcount::decrement(old_phys);
    if new_refcount == 0 {
        crate::memory::free_frame(old_phys as usize);
    }

    // Invalidate TLB for this page.
    let page_addr = fault_addr & !0xFFF;
    unsafe {
        core::arch::asm!("invlpg [{}]", in(reg) page_addr, options(nostack, preserves_flags));
    }

    crate::serial_println!(
        "[CoW] fault handled at {:#x} (copied frame, old refcount={})",
        fault_addr,
        new_refcount + 1
    );

    true
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
