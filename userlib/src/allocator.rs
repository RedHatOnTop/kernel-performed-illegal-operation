//! Userspace memory allocator
//!
//! A simple bump allocator for userspace applications.
//! Uses mmap syscall to request memory from the kernel.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::mem::{map, mmap, munmap, prot};

/// Page size (4KB)
const PAGE_SIZE: usize = 4096;

/// Initial heap size (1MB)
const INITIAL_HEAP_SIZE: usize = 1024 * 1024;

/// Maximum heap size (256MB)
const MAX_HEAP_SIZE: usize = 256 * 1024 * 1024;

/// Userspace allocator using mmap
pub struct UserAllocator {
    /// Heap start address
    heap_start: AtomicUsize,
    /// Heap end address (exclusive)
    heap_end: AtomicUsize,
    /// Current allocation pointer
    next: AtomicUsize,
    /// Initialized flag
    initialized: AtomicUsize,
}

impl UserAllocator {
    /// Create a new uninitialized allocator
    pub const fn new() -> Self {
        Self {
            heap_start: AtomicUsize::new(0),
            heap_end: AtomicUsize::new(0),
            next: AtomicUsize::new(0),
            initialized: AtomicUsize::new(0),
        }
    }

    /// Initialize the allocator by mapping initial heap
    fn init(&self) {
        // Use compare_exchange to ensure only one thread initializes
        if self
            .initialized
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            // Already initialized or being initialized, spin until done
            while self.initialized.load(Ordering::SeqCst) != 2 {
                core::hint::spin_loop();
            }
            return;
        }

        // Map initial heap
        let flags = map::MAP_PRIVATE | map::MAP_ANONYMOUS;
        let protection = prot::PROT_READ | prot::PROT_WRITE;

        if let Ok(addr) = mmap(0, INITIAL_HEAP_SIZE, protection, flags) {
            self.heap_start.store(addr as usize, Ordering::SeqCst);
            self.heap_end
                .store(addr as usize + INITIAL_HEAP_SIZE, Ordering::SeqCst);
            self.next.store(addr as usize, Ordering::SeqCst);
        }

        // Mark as fully initialized
        self.initialized.store(2, Ordering::SeqCst);
    }

    /// Ensure the allocator is initialized
    #[inline]
    fn ensure_init(&self) {
        if self.initialized.load(Ordering::Acquire) != 2 {
            self.init();
        }
    }

    /// Expand the heap by mapping more memory
    fn expand(&self, min_size: usize) -> bool {
        let current_end = self.heap_end.load(Ordering::SeqCst);
        let current_start = self.heap_start.load(Ordering::SeqCst);
        let current_size = current_end - current_start;

        // Calculate new size (double or add min_size, whichever is larger)
        let expand_size = core::cmp::max(current_size, align_up(min_size, PAGE_SIZE));

        // Check if we'd exceed max
        if current_size + expand_size > MAX_HEAP_SIZE {
            return false;
        }

        // Try to map at the end of current heap
        let flags = map::MAP_PRIVATE | map::MAP_ANONYMOUS;
        let protection = prot::PROT_READ | prot::PROT_WRITE;

        if let Ok(addr) = mmap(current_end as u64, expand_size, protection, flags) {
            // Check if we got contiguous memory
            if addr as usize == current_end {
                self.heap_end
                    .store(current_end + expand_size, Ordering::SeqCst);
                return true;
            } else {
                // Got non-contiguous memory, unmap it and fail
                let _ = munmap(addr, expand_size);
            }
        }

        false
    }
}

unsafe impl GlobalAlloc for UserAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.ensure_init();

        let size = layout.size();
        let align = layout.align();

        loop {
            let current = self.next.load(Ordering::Relaxed);
            let aligned = align_up(current, align);
            let new_next = aligned + size;

            let heap_end = self.heap_end.load(Ordering::Relaxed);

            if new_next > heap_end {
                // Need more memory
                if !self.expand(new_next - heap_end) {
                    return ptr::null_mut();
                }
                continue;
            }

            // Try to bump the pointer
            if self
                .next
                .compare_exchange_weak(current, new_next, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                return aligned as *mut u8;
            }
            // Failed, retry
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator doesn't free individual allocations
        // Memory is reclaimed when the process exits
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // Simple realloc: allocate new, copy, don't free old
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        let new_ptr = self.alloc(new_layout);

        if !new_ptr.is_null() {
            let copy_size = core::cmp::min(layout.size(), new_size);
            ptr::copy_nonoverlapping(ptr, new_ptr, copy_size);
        }

        new_ptr
    }
}

/// Align address up to the given alignment
#[inline]
const fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
