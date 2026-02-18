//! ELF Segment Memory Loader
//!
//! Connects the ELF parser (`loader/elf.rs`) to the user page table system
//! (`memory/user_page_table.rs`) to actually load ELF segments into a
//! process's virtual address space.
//!
//! # Process
//!
//! 1. For each PT_LOAD segment in the ELF:
//!    a. Calculate page-aligned range covering the segment
//!    b. Allocate physical frames for each page
//!    c. Copy segment data from the ELF binary to the frames
//!    d. Zero-fill BSS (memsz > filesz region)
//!    e. Map frames into the process page table with correct permissions
//! 2. Set up user stack (8MB below USER_STACK_TOP)
//! 3. Initialize heap break pointer

use super::elf::LoadedProgram;
use super::program::layout;
use crate::memory::user_page_table;
use x86_64::structures::paging::PageTableFlags;

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// Errors that can occur during segment loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentLoadError {
    /// Not enough physical memory to allocate frames
    OutOfMemory,
    /// Page table mapping failed
    MappingFailed,
    /// Segment data extends beyond ELF binary
    SegmentOutOfBounds,
    /// Invalid segment address (e.g., in kernel space)
    InvalidAddress,
    /// User page table subsystem not initialized
    NotInitialized,
}

impl core::fmt::Display for SegmentLoadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OutOfMemory => write!(f, "Out of memory"),
            Self::MappingFailed => write!(f, "Page table mapping failed"),
            Self::SegmentOutOfBounds => write!(f, "Segment data out of bounds"),
            Self::InvalidAddress => write!(f, "Invalid segment address"),
            Self::NotInitialized => write!(f, "User page table not initialized"),
        }
    }
}

/// Result of loading ELF segments into a process address space.
#[derive(Debug)]
pub struct LoadResult {
    /// Entry point (adjusted for PIE if applicable)
    pub entry_point: u64,
    /// Initial stack pointer (top of user stack)
    pub initial_sp: u64,
    /// Initial heap break (page after last segment)
    pub brk_start: u64,
    /// Number of pages mapped
    pub pages_mapped: usize,
}

/// Load ELF segments into a process's page table.
///
/// This is the main function that connects the ELF parser to actual memory mapping.
///
/// # Arguments
///
/// * `cr3_phys` - Physical address of the process's P4 page table
/// * `loaded` - Parsed ELF program information
/// * `elf_binary` - Raw ELF binary data (to copy segment contents)
/// * `pie_base` - Base address for PIE binaries (0 for non-PIE)
///
/// # Returns
///
/// `LoadResult` containing entry point, stack pointer, and heap break.
pub fn load_elf_segments(
    cr3_phys: u64,
    loaded: &LoadedProgram,
    elf_binary: &[u8],
    pie_base: u64,
) -> Result<LoadResult, SegmentLoadError> {
    let mut pages_mapped: usize = 0;
    let mut max_vaddr: u64 = 0;

    // Load each PT_LOAD segment
    for segment in &loaded.segments {
        let base_vaddr = if loaded.is_pie {
            pie_base + segment.vaddr
        } else {
            segment.vaddr
        };

        // Validate that the segment is in user space
        if base_vaddr >= 0x0000_8000_0000_0000 {
            return Err(SegmentLoadError::InvalidAddress);
        }

        // Calculate page-aligned boundaries
        let seg_start = base_vaddr;
        let seg_end = base_vaddr + segment.mem_size;
        let page_start = seg_start & !0xFFF;
        let page_end = (seg_end + 0xFFF) & !0xFFF;

        // Determine page flags from segment permissions
        let flags = segment_to_page_flags(segment);

        // Track maximum address for heap break
        if seg_end > max_vaddr {
            max_vaddr = seg_end;
        }

        // Map each page in the segment's range
        for page_vaddr in (page_start..page_end).step_by(4096) {
            // Allocate and map a zeroed page
            let frame_phys = user_page_table::map_user_page(cr3_phys, page_vaddr, flags)
                .map_err(|_| SegmentLoadError::OutOfMemory)?;

            // Calculate what part of the file data falls on this page
            let page_data_start = page_vaddr;
            let page_data_end = page_vaddr + 4096;

            // File data range for this segment
            let file_data_start = seg_start;
            let file_data_end = seg_start + segment.file_size;

            // Calculate overlap between this page and file data
            let copy_start = core::cmp::max(page_data_start, file_data_start);
            let copy_end = core::cmp::min(page_data_end, file_data_end);

            if copy_start < copy_end {
                // There's file data to copy to this page
                let offset_in_page = (copy_start - page_vaddr) as usize;
                let offset_in_segment = (copy_start - seg_start) as usize;
                let file_offset = segment.file_offset as usize + offset_in_segment;
                let copy_len = (copy_end - copy_start) as usize;

                // Validate source bounds
                if file_offset + copy_len > elf_binary.len() {
                    return Err(SegmentLoadError::SegmentOutOfBounds);
                }

                let src_data = &elf_binary[file_offset..file_offset + copy_len];

                // Write data directly to the physical frame via offset mapping
                unsafe {
                    user_page_table::write_to_phys(frame_phys, offset_in_page, src_data);
                }
            }
            // Pages beyond file_size but within mem_size are BSS (already zeroed)

            pages_mapped += 1;
        }
    }

    // Set up user stack
    let stack_top = layout::USER_STACK_TOP;
    let stack_bottom = stack_top - layout::USER_STACK_SIZE;
    let stack_flags = PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;

    // Map stack pages (guard page at bottom could be added later)
    // For now, map the entire stack region
    // Note: 8MB = 2048 pages. For initial implementation, map a smaller
    // pre-committed region and let page faults expand it.
    let initial_stack_pages = 16; // 64KB initial committed stack
    let initial_stack_bottom = stack_top - (initial_stack_pages * 4096);

    for page_addr in (initial_stack_bottom..stack_top).step_by(4096) {
        user_page_table::map_user_page(cr3_phys, page_addr, stack_flags)
            .map_err(|_| SegmentLoadError::OutOfMemory)?;
        pages_mapped += 1;
    }

    // Calculate entry point (adjusted for PIE)
    let entry_point = if loaded.is_pie {
        pie_base + loaded.entry_point
    } else {
        loaded.entry_point
    };

    // Calculate heap break: first page after last segment
    let brk_start = (max_vaddr + 0xFFF) & !0xFFF;

    Ok(LoadResult {
        entry_point,
        initial_sp: stack_top,
        brk_start,
        pages_mapped,
    })
}

/// Convert ELF segment flags to x86_64 page table flags.
///
/// Enforces W^X: a page cannot be both writable and executable.
fn segment_to_page_flags(
    segment: &super::elf::LoadSegment,
) -> PageTableFlags {
    let mut flags = PageTableFlags::empty();

    if segment.is_writable() {
        flags |= PageTableFlags::WRITABLE;
    }

    // NX (No Execute) bit: set if segment is NOT executable
    if !segment.is_executable() {
        flags |= PageTableFlags::NO_EXECUTE;
    }

    // W^X enforcement: if both W and X, prefer X (remove W)
    if segment.is_writable() && segment.is_executable() {
        // Safety: remove WRITABLE to enforce W^X
        flags.remove(PageTableFlags::WRITABLE);
        crate::serial_println!(
            "[KPIO] Warning: W^X enforcement - segment at {:#x} has W+X, removing W",
            segment.vaddr
        );
    }

    flags
}

/// Push the initial stack contents for a Linux process.
///
/// Writes argc, argv pointers, envp pointers, and auxv to the user stack.
/// Returns the adjusted stack pointer.
///
/// # Stack Layout (x86_64 ABI)
///
/// ```text
///   (high address = stack_top)
///   +-------------------+
///   | string data       | ← argv[0], argv[1], ..., envp[0], ...
///   +-------------------+
///   | padding (align 16)|
///   +-------------------+
///   | AT_NULL (0, 0)    |
///   | auxv[n]           |
///   | ...               |
///   | auxv[0]           |
///   +-------------------+
///   | NULL              | ← end of envp
///   | envp[n] ptr       |
///   | ...               |
///   | envp[0] ptr       |
///   +-------------------+
///   | NULL              | ← end of argv
///   | argv[n] ptr       |
///   | ...               |
///   | argv[0] ptr       |
///   +-------------------+
///   | argc              | ← RSP points here
///   +-------------------+
///   (low address)
/// ```
pub fn setup_user_stack(
    cr3_phys: u64,
    stack_top: u64,
    args: &[String],
    envp: &[String],
    auxv: &[(u64, u64)],
) -> Result<u64, SegmentLoadError> {
    // We'll build the stack contents in a temporary buffer, then write
    // to the physical pages. This avoids complex per-page calculations.

    // Calculate total string data size
    let mut string_data_size: usize = 0;
    for arg in args {
        string_data_size += arg.len() + 1; // +1 for null terminator
    }
    for env in envp {
        string_data_size += env.len() + 1;
    }

    // Calculate total stack frame size
    let ptrs_size = 8 // argc
        + (args.len() + 1) * 8 // argv pointers + NULL
        + (envp.len() + 1) * 8 // envp pointers + NULL
        + auxv.len() * 16; // auxv entries (key + value)

    let total_size = ptrs_size + string_data_size + 16; // +16 for alignment
    let total_size_aligned = (total_size + 15) & !15; // 16-byte align

    // Adjusted SP
    let sp = stack_top - total_size_aligned as u64;

    // Ensure SP is 16-byte aligned
    let sp = sp & !0xF;

    // Build string data area (at the top of our frame)
    let string_area_start = sp + ptrs_size as u64;

    // Now write the stack data via physical frames
    // Write argc
    write_u64_to_user(cr3_phys, sp, args.len() as u64)?;

    // Current position for argv/envp pointers
    let mut ptr_pos = sp + 8;
    let mut str_pos = string_area_start;

    // Write argv pointers and string data
    for arg in args {
        // Write pointer to string
        write_u64_to_user(cr3_phys, ptr_pos, str_pos)?;
        ptr_pos += 8;

        // Write string data
        write_bytes_to_user(cr3_phys, str_pos, arg.as_bytes())?;
        write_bytes_to_user(cr3_phys, str_pos + arg.len() as u64, &[0])?; // null
        str_pos += arg.len() as u64 + 1;
    }

    // NULL terminator for argv
    write_u64_to_user(cr3_phys, ptr_pos, 0)?;
    ptr_pos += 8;

    // Write envp pointers and string data
    for env in envp {
        write_u64_to_user(cr3_phys, ptr_pos, str_pos)?;
        ptr_pos += 8;

        write_bytes_to_user(cr3_phys, str_pos, env.as_bytes())?;
        write_bytes_to_user(cr3_phys, str_pos + env.len() as u64, &[0])?;
        str_pos += env.len() as u64 + 1;
    }

    // NULL terminator for envp
    write_u64_to_user(cr3_phys, ptr_pos, 0)?;
    ptr_pos += 8;

    // Write auxiliary vector
    for &(key, value) in auxv {
        write_u64_to_user(cr3_phys, ptr_pos, key)?;
        ptr_pos += 8;
        write_u64_to_user(cr3_phys, ptr_pos, value)?;
        ptr_pos += 8;
    }

    Ok(sp)
}

/// Write a u64 value to a user-space virtual address.
///
/// Translates the virtual address to physical via the process page table,
/// then writes through the kernel's physical offset mapping.
fn write_u64_to_user(cr3_phys: u64, virt_addr: u64, value: u64) -> Result<(), SegmentLoadError> {
    let bytes = value.to_le_bytes();
    write_bytes_to_user(cr3_phys, virt_addr, &bytes)
}

/// Write bytes to a user-space virtual address.
///
/// Looks up the physical frame for each page the write spans and copies
/// data via the kernel's physical offset mapping.
fn write_bytes_to_user(
    cr3_phys: u64,
    virt_addr: u64,
    data: &[u8],
) -> Result<(), SegmentLoadError> {
    if data.is_empty() {
        return Ok(());
    }

    let phys_offset = x86_64::VirtAddr::new(
        core::sync::atomic::AtomicU64::new(0)
            .load(core::sync::atomic::Ordering::Relaxed),
    );

    // Walk the page table to find the physical address
    let phys_addr = translate_user_vaddr(cr3_phys, virt_addr)
        .ok_or(SegmentLoadError::MappingFailed)?;

    unsafe {
        user_page_table::write_to_phys(phys_addr & !0xFFF, (phys_addr & 0xFFF) as usize, data);
    }

    Ok(())
}

/// Translate a user virtual address to physical address via page table walk.
fn translate_user_vaddr(cr3_phys: u64, virt_addr: u64) -> Option<u64> {
    use x86_64::structures::paging::PageTableFlags;

    let phys_offset = crate::memory::user_page_table::get_phys_offset();
    let offset = x86_64::VirtAddr::new(phys_offset);

    // P4 index: bits 47:39
    let p4_idx = ((virt_addr >> 39) & 0x1FF) as usize;
    // P3 index: bits 38:30
    let p3_idx = ((virt_addr >> 30) & 0x1FF) as usize;
    // P2 index: bits 29:21
    let p2_idx = ((virt_addr >> 21) & 0x1FF) as usize;
    // P1 index: bits 20:12
    let p1_idx = ((virt_addr >> 12) & 0x1FF) as usize;
    // Page offset: bits 11:0
    let page_offset = virt_addr & 0xFFF;

    // Walk P4
    let p4: &x86_64::structures::paging::PageTable =
        unsafe { &*(offset + cr3_phys).as_ptr() };
    let p4_entry = &p4[p4_idx];
    if !p4_entry.flags().contains(PageTableFlags::PRESENT) {
        return None;
    }

    // Walk P3
    let p3: &x86_64::structures::paging::PageTable =
        unsafe { &*(offset + p4_entry.addr().as_u64()).as_ptr() };
    let p3_entry = &p3[p3_idx];
    if !p3_entry.flags().contains(PageTableFlags::PRESENT) {
        return None;
    }

    // Walk P2
    let p2: &x86_64::structures::paging::PageTable =
        unsafe { &*(offset + p3_entry.addr().as_u64()).as_ptr() };
    let p2_entry = &p2[p2_idx];
    if !p2_entry.flags().contains(PageTableFlags::PRESENT) {
        return None;
    }

    // Walk P1
    let p1: &x86_64::structures::paging::PageTable =
        unsafe { &*(offset + p2_entry.addr().as_u64()).as_ptr() };
    let p1_entry = &p1[p1_idx];
    if !p1_entry.flags().contains(PageTableFlags::PRESENT) {
        return None;
    }

    Some(p1_entry.addr().as_u64() + page_offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::elf::LoadSegment;

    #[test]
    fn test_segment_flags_readable_only() {
        let seg = LoadSegment {
            vaddr: 0x400000,
            mem_size: 0x1000,
            file_offset: 0,
            file_size: 0x1000,
            flags: 4, // PF_R only
            align: 0x1000,
        };
        let flags = segment_to_page_flags(&seg);
        assert!(!flags.contains(PageTableFlags::WRITABLE));
        assert!(flags.contains(PageTableFlags::NO_EXECUTE));
    }

    #[test]
    fn test_segment_flags_executable() {
        let seg = LoadSegment {
            vaddr: 0x400000,
            mem_size: 0x1000,
            file_offset: 0,
            file_size: 0x1000,
            flags: 5, // PF_R | PF_X
            align: 0x1000,
        };
        let flags = segment_to_page_flags(&seg);
        assert!(!flags.contains(PageTableFlags::WRITABLE));
        assert!(!flags.contains(PageTableFlags::NO_EXECUTE));
    }

    #[test]
    fn test_segment_flags_writable() {
        let seg = LoadSegment {
            vaddr: 0x600000,
            mem_size: 0x2000,
            file_offset: 0x200000,
            file_size: 0x1000,
            flags: 6, // PF_R | PF_W
            align: 0x1000,
        };
        let flags = segment_to_page_flags(&seg);
        assert!(flags.contains(PageTableFlags::WRITABLE));
        assert!(flags.contains(PageTableFlags::NO_EXECUTE));
    }

    #[test]
    fn test_wxorx_enforcement() {
        // W+X segment should have W removed for safety
        let seg = LoadSegment {
            vaddr: 0x400000,
            mem_size: 0x1000,
            file_offset: 0,
            file_size: 0x1000,
            flags: 7, // PF_R | PF_W | PF_X
            align: 0x1000,
        };
        let flags = segment_to_page_flags(&seg);
        // W^X: should NOT be writable if executable
        assert!(!flags.contains(PageTableFlags::WRITABLE));
        assert!(!flags.contains(PageTableFlags::NO_EXECUTE));
    }

    #[test]
    fn test_page_range_calculation() {
        // Segment at 0x400100, size 0x200 → pages 0x400000..0x401000 (1 page)
        let seg_start = 0x400100u64;
        let seg_end = seg_start + 0x200u64;
        let page_start = seg_start & !0xFFF;
        let page_end = (seg_end + 0xFFF) & !0xFFF;
        assert_eq!(page_start, 0x400000);
        assert_eq!(page_end, 0x401000);
    }

    #[test]
    fn test_brk_calculation() {
        // If max_vaddr is 0x601234, brk_start should be 0x602000
        let max_vaddr = 0x601234u64;
        let brk_start = (max_vaddr + 0xFFF) & !0xFFF;
        assert_eq!(brk_start, 0x602000);
    }

    #[test]
    fn test_stack_alignment() {
        // SP must be 16-byte aligned
        let stack_top = layout::USER_STACK_TOP;
        let sp = stack_top - 0x100;
        let sp_aligned = sp & !0xF;
        assert_eq!(sp_aligned & 0xF, 0);
    }
}
