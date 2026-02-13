//! Heap allocator initialization
//!
//! This module initializes the kernel heap and sets up the global allocator.
//! It uses linked_list_allocator to support dynamic memory allocation.

use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

/// Heap start virtual address.
///
/// Located in the upper kernel space to avoid conflicts with other mappings.
pub const HEAP_START: usize = 0x_4444_4444_0000;

/// Heap size (16 MiB).
pub const HEAP_SIZE: usize = 16 * 1024 * 1024;

/// Global heap allocator.
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// Initialize the heap.
///
/// This function allocates and maps pages for the heap area,
/// then initializes the global allocator.
///
/// # Arguments
///
/// * `mapper` - Page table mapper
/// * `frame_allocator` - Physical frame allocator
///
/// # Errors
///
/// Returns `MapToError` on page mapping failure.
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    // Calculate page range for the heap area
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    // Allocate and map physical frames for each page
    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    // Initialize the allocator
    unsafe {
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
    }

    Ok(())
}

/// Allocation error handler.
///
/// Called when memory allocation fails.
#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

/// Heap usage statistics.
#[derive(Debug, Clone, Copy)]
pub struct HeapStats {
    /// Total heap size in bytes.
    pub total: usize,
    /// Used heap size in bytes.
    pub used: usize,
    /// Free heap size in bytes.
    pub free: usize,
}

/// Get current heap usage statistics.
pub fn heap_stats() -> HeapStats {
    let allocator = ALLOCATOR.lock();
    let free = allocator.free();
    let used = allocator.used();
    HeapStats {
        total: HEAP_SIZE,
        used,
        free,
    }
}
