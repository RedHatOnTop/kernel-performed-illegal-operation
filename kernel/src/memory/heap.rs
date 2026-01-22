//! Kernel heap allocator.
//!
//! Provides dynamic memory allocation for the kernel using a linked list
//! allocator backed by the physical frame allocator.

use crate::config::{KERNEL_HEAP_BASE, KERNEL_HEAP_SIZE};
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use linked_list_allocator::Heap;
use spin::Mutex;

/// Global allocator instance.
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// Thread-safe wrapper around the heap allocator.
pub struct LockedHeap(Mutex<Heap>);

impl LockedHeap {
    /// Create an empty heap.
    pub const fn empty() -> Self {
        Self(Mutex::new(Heap::empty()))
    }
    
    /// Initialize the heap with a memory region.
    ///
    /// # Safety
    ///
    /// The memory region must be valid and not used for anything else.
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) {
        unsafe {
            self.0.lock().init(heap_start as *mut u8, heap_size);
        }
    }
}

unsafe impl GlobalAlloc for LockedHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0
            .lock()
            .allocate_first_fit(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr())
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Some(ptr) = NonNull::new(ptr) {
            unsafe {
                self.0.lock().deallocate(ptr, layout);
            }
        }
    }
}

/// Initialize the kernel heap.
pub fn init_heap() {
    // Map heap pages
    // TODO: Actually map the pages using the page table
    
    // Initialize the allocator
    // SAFETY: We're using a dedicated region for the heap
    unsafe {
        ALLOCATOR.init(KERNEL_HEAP_BASE as usize, KERNEL_HEAP_SIZE);
    }
}

/// Get heap statistics.
pub fn stats() -> (usize, usize) {
    let heap = ALLOCATOR.0.lock();
    (heap.used(), heap.size())
}

/// Handle allocation failures.
#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("Allocation error: {:?}", layout)
}
