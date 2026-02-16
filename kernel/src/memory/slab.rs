//! Slab allocator for fixed-size object caching.
//!
//! The slab allocator provides efficient allocation of frequently-used
//! fixed-size objects by maintaining pre-allocated pools.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Slab Allocator                           │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Size Classes: 8, 16, 32, 64, 128, 256, 512, 1024, 2048    │
//! ├─────────────────────────────────────────────────────────────┤
//! │                                                             │
//! │  ┌─────────┐  ┌─────────┐  ┌─────────┐                     │
//! │  │ Slab 8  │  │ Slab 16 │  │ Slab 32 │  ...                │
//! │  └────┬────┘  └────┬────┘  └────┬────┘                     │
//! │       │            │            │                           │
//! │       ▼            ▼            ▼                           │
//! │  ┌─────────┐  ┌─────────┐  ┌─────────┐                     │
//! │  │ Partial │  │  Full   │  │  Empty  │                     │
//! │  │  Slabs  │  │  Slabs  │  │  Slabs  │                     │
//! │  └─────────┘  └─────────┘  └─────────┘                     │
//! │                                                             │
//! │  Features:                                                  │
//! │  - O(1) allocation/deallocation                            │
//! │  - Per-CPU caches (future)                                 │
//! │  - Memory coloring (future)                                │
//! │  - Statistics tracking                                     │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;

/// Slab allocator statistics.
#[derive(Debug, Default)]
pub struct SlabStats {
    /// Total allocations.
    pub allocations: AtomicU64,
    /// Total deallocations.
    pub deallocations: AtomicU64,
    /// Current allocated objects.
    pub current_objects: AtomicUsize,
    /// Total slabs created.
    pub slabs_created: AtomicU64,
    /// Total slabs destroyed.
    pub slabs_destroyed: AtomicU64,
    /// Cache hits (allocation from partial slab).
    pub cache_hits: AtomicU64,
    /// Cache misses (new slab needed).
    pub cache_misses: AtomicU64,
}

impl SlabStats {
    pub const fn new() -> Self {
        Self {
            allocations: AtomicU64::new(0),
            deallocations: AtomicU64::new(0),
            current_objects: AtomicUsize::new(0),
            slabs_created: AtomicU64::new(0),
            slabs_destroyed: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
        }
    }

    pub fn record_alloc(&self, is_cache_hit: bool) {
        self.allocations.fetch_add(1, Ordering::Relaxed);
        self.current_objects.fetch_add(1, Ordering::Relaxed);
        if is_cache_hit {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.cache_misses.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_dealloc(&self) {
        self.deallocations.fetch_add(1, Ordering::Relaxed);
        self.current_objects.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn record_slab_created(&self) {
        self.slabs_created.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_slab_destroyed(&self) {
        self.slabs_destroyed.fetch_add(1, Ordering::Relaxed);
    }
}

/// Global slab statistics.
pub static GLOBAL_STATS: SlabStats = SlabStats::new();

/// A slab cache for objects of a specific size.
pub struct SlabCache {
    /// Object size in bytes.
    object_size: usize,

    /// Object alignment.
    alignment: usize,

    /// Partial slabs (have free space).
    partial_slabs: Vec<Slab>,

    /// Full slabs (no free space).
    full_slabs: Vec<Slab>,

    /// Empty slabs (all free, kept for reuse).
    empty_slabs: Vec<Slab>,

    /// Maximum empty slabs to keep.
    max_empty_slabs: usize,

    /// Number of allocated objects.
    allocated: usize,

    /// Local statistics for this cache.
    stats: SlabStats,
}

/// A single slab containing multiple objects.
struct Slab {
    /// Base address of the slab.
    base: *mut u8,

    /// Bitmap of free objects (1 = free, 0 = allocated).
    free_bitmap: u64,

    /// Number of objects in this slab.
    object_count: usize,

    /// Number of free objects.
    free_count: usize,

    /// Slab size in bytes.
    slab_size: usize,
}

/// Size classes for slab allocator.
pub const SIZE_CLASSES: [usize; 9] = [8, 16, 32, 64, 128, 256, 512, 1024, 2048];

/// Global slab allocators for common sizes.
static SLAB_8: Mutex<Option<SlabCache>> = Mutex::new(None);
static SLAB_16: Mutex<Option<SlabCache>> = Mutex::new(None);
static SLAB_32: Mutex<Option<SlabCache>> = Mutex::new(None);
static SLAB_64: Mutex<Option<SlabCache>> = Mutex::new(None);
static SLAB_128: Mutex<Option<SlabCache>> = Mutex::new(None);
static SLAB_256: Mutex<Option<SlabCache>> = Mutex::new(None);
static SLAB_512: Mutex<Option<SlabCache>> = Mutex::new(None);
static SLAB_1024: Mutex<Option<SlabCache>> = Mutex::new(None);
static SLAB_2048: Mutex<Option<SlabCache>> = Mutex::new(None);

/// Initialize the slab allocator subsystem.
pub fn init() {
    *SLAB_8.lock() = Some(SlabCache::new(8, 8));
    *SLAB_16.lock() = Some(SlabCache::new(16, 8));
    *SLAB_32.lock() = Some(SlabCache::new(32, 8));
    *SLAB_64.lock() = Some(SlabCache::new(64, 8));
    *SLAB_128.lock() = Some(SlabCache::new(128, 8));
    *SLAB_256.lock() = Some(SlabCache::new(256, 8));
    *SLAB_512.lock() = Some(SlabCache::new(512, 8));
    *SLAB_1024.lock() = Some(SlabCache::new(1024, 8));
    *SLAB_2048.lock() = Some(SlabCache::new(2048, 8));
}

/// Get the appropriate size class for a given size.
pub fn get_size_class(size: usize) -> Option<usize> {
    for &class in &SIZE_CLASSES {
        if size <= class {
            return Some(class);
        }
    }
    None
}

/// Allocate memory from the slab allocator.
pub fn slab_alloc(size: usize) -> Option<*mut u8> {
    match get_size_class(size)? {
        8 => SLAB_8.lock().as_mut()?.allocate(),
        16 => SLAB_16.lock().as_mut()?.allocate(),
        32 => SLAB_32.lock().as_mut()?.allocate(),
        64 => SLAB_64.lock().as_mut()?.allocate(),
        128 => SLAB_128.lock().as_mut()?.allocate(),
        256 => SLAB_256.lock().as_mut()?.allocate(),
        512 => SLAB_512.lock().as_mut()?.allocate(),
        1024 => SLAB_1024.lock().as_mut()?.allocate(),
        2048 => SLAB_2048.lock().as_mut()?.allocate(),
        _ => None,
    }
}

/// Free memory back to the slab allocator.
///
/// # Safety
///
/// The pointer must have been allocated from the slab allocator.
pub unsafe fn slab_free(ptr: *mut u8, size: usize) {
    match get_size_class(size) {
        Some(8) => {
            if let Some(cache) = SLAB_8.lock().as_mut() {
                unsafe { cache.free(ptr) };
            }
        }
        Some(16) => {
            if let Some(cache) = SLAB_16.lock().as_mut() {
                unsafe { cache.free(ptr) };
            }
        }
        Some(32) => {
            if let Some(cache) = SLAB_32.lock().as_mut() {
                unsafe { cache.free(ptr) };
            }
        }
        Some(64) => {
            if let Some(cache) = SLAB_64.lock().as_mut() {
                unsafe { cache.free(ptr) };
            }
        }
        Some(128) => {
            if let Some(cache) = SLAB_128.lock().as_mut() {
                unsafe { cache.free(ptr) };
            }
        }
        Some(256) => {
            if let Some(cache) = SLAB_256.lock().as_mut() {
                unsafe { cache.free(ptr) };
            }
        }
        Some(512) => {
            if let Some(cache) = SLAB_512.lock().as_mut() {
                unsafe { cache.free(ptr) };
            }
        }
        Some(1024) => {
            if let Some(cache) = SLAB_1024.lock().as_mut() {
                unsafe { cache.free(ptr) };
            }
        }
        Some(2048) => {
            if let Some(cache) = SLAB_2048.lock().as_mut() {
                unsafe { cache.free(ptr) };
            }
        }
        _ => panic!("Invalid size class for slab_free"),
    }
}

/// Get global slab allocator statistics.
pub fn get_stats() -> &'static SlabStats {
    &GLOBAL_STATS
}

impl SlabCache {
    /// Create a new slab cache.
    pub fn new(object_size: usize, alignment: usize) -> Self {
        Self {
            object_size: object_size.max(core::mem::size_of::<*mut u8>()),
            alignment,
            partial_slabs: Vec::new(),
            full_slabs: Vec::new(),
            empty_slabs: Vec::new(),
            max_empty_slabs: 2, // Keep up to 2 empty slabs for quick reuse
            allocated: 0,
            stats: SlabStats::new(),
        }
    }

    /// Allocate an object from the cache.
    pub fn allocate(&mut self) -> Option<*mut u8> {
        // 1. Try partial slabs first (most likely to succeed)
        if let Some(slab) = self.partial_slabs.last_mut() {
            if let Some(ptr) = slab.allocate(self.object_size) {
                self.allocated += 1;
                self.stats.record_alloc(true);
                GLOBAL_STATS.record_alloc(true);

                // Move to full if no more space
                if slab.is_full() {
                    let slab = self.partial_slabs.pop().unwrap();
                    self.full_slabs.push(slab);
                }
                return Some(ptr);
            }
        }

        // 2. Try to reuse an empty slab
        if let Some(mut slab) = self.empty_slabs.pop() {
            let ptr = slab.allocate(self.object_size)?;
            self.allocated += 1;
            self.stats.record_alloc(true);
            GLOBAL_STATS.record_alloc(true);
            self.partial_slabs.push(slab);
            return Some(ptr);
        }

        // 3. Create a new slab
        let mut slab = Slab::new(self.object_size)?;
        GLOBAL_STATS.record_slab_created();
        self.stats.slabs_created.fetch_add(1, Ordering::Relaxed);

        let ptr = slab.allocate(self.object_size)?;
        self.allocated += 1;
        self.stats.record_alloc(false);
        GLOBAL_STATS.record_alloc(false);

        if slab.is_full() {
            self.full_slabs.push(slab);
        } else {
            self.partial_slabs.push(slab);
        }

        Some(ptr)
    }

    /// Free an object back to the cache.
    ///
    /// # Safety
    ///
    /// The pointer must have been allocated from this cache.
    pub unsafe fn free(&mut self, ptr: *mut u8) {
        // Check partial slabs
        for i in 0..self.partial_slabs.len() {
            if self.partial_slabs[i].contains(ptr, self.object_size) {
                // SAFETY: ptr is verified to belong to this slab
                unsafe { self.partial_slabs[i].free(ptr, self.object_size) };
                self.allocated -= 1;
                self.stats.record_dealloc();
                GLOBAL_STATS.record_dealloc();

                // Check if slab is now empty
                if self.partial_slabs[i].is_empty() {
                    let slab = self.partial_slabs.remove(i);
                    self.handle_empty_slab(slab);
                }
                return;
            }
        }

        // Check full slabs
        for i in 0..self.full_slabs.len() {
            if self.full_slabs[i].contains(ptr, self.object_size) {
                // SAFETY: ptr is verified to belong to this slab
                unsafe { self.full_slabs[i].free(ptr, self.object_size) };
                self.allocated -= 1;
                self.stats.record_dealloc();
                GLOBAL_STATS.record_dealloc();

                // Move to partial
                let slab = self.full_slabs.remove(i);
                if slab.is_empty() {
                    self.handle_empty_slab(slab);
                } else {
                    self.partial_slabs.push(slab);
                }
                return;
            }
        }

        panic!("Attempted to free pointer not from this slab cache");
    }

    /// Handle an empty slab (keep or destroy).
    fn handle_empty_slab(&mut self, slab: Slab) {
        if self.empty_slabs.len() < self.max_empty_slabs {
            self.empty_slabs.push(slab);
        } else {
            // Destroy the slab
            GLOBAL_STATS.record_slab_destroyed();
            self.stats.slabs_destroyed.fetch_add(1, Ordering::Relaxed);
            drop(slab);
        }
    }

    /// Get the number of allocated objects.
    pub fn allocated_count(&self) -> usize {
        self.allocated
    }

    /// Get cache statistics.
    pub fn stats(&self) -> &SlabStats {
        &self.stats
    }

    /// Shrink the cache by releasing empty slabs.
    pub fn shrink(&mut self) {
        while let Some(slab) = self.empty_slabs.pop() {
            GLOBAL_STATS.record_slab_destroyed();
            self.stats.slabs_destroyed.fetch_add(1, Ordering::Relaxed);
            drop(slab);
        }
    }

    /// Get total memory used by this cache.
    pub fn memory_usage(&self) -> usize {
        let slab_count = self.partial_slabs.len() + self.full_slabs.len() + self.empty_slabs.len();
        slab_count * 4096 // Each slab is one page
    }
}

impl Slab {
    /// Create a new slab.
    fn new(object_size: usize) -> Option<Self> {
        // Allocate a page for the slab
        let base = crate::memory::allocate_frame()? as *mut u8;
        let slab_size = 4096;
        let object_count = (slab_size / object_size).min(64);

        Some(Self {
            base,
            free_bitmap: (1u64 << object_count) - 1, // All objects free
            object_count,
            free_count: object_count,
            slab_size,
        })
    }

    /// Allocate an object from this slab.
    fn allocate(&mut self, object_size: usize) -> Option<*mut u8> {
        if self.free_bitmap == 0 {
            return None;
        }

        // Find first free bit
        let index = self.free_bitmap.trailing_zeros() as usize;
        self.free_bitmap &= !(1u64 << index);
        self.free_count -= 1;

        Some(unsafe { self.base.add(index * object_size) })
    }

    /// Free an object back to this slab.
    ///
    /// # Safety
    ///
    /// The pointer must have been allocated from this slab.
    unsafe fn free(&mut self, ptr: *mut u8, object_size: usize) {
        let offset = ptr as usize - self.base as usize;
        let index = offset / object_size;
        self.free_bitmap |= 1u64 << index;
        self.free_count += 1;
    }

    /// Check if a pointer belongs to this slab.
    fn contains(&self, ptr: *mut u8, object_size: usize) -> bool {
        let base = self.base as usize;
        let end = base + self.object_count * object_size;
        let addr = ptr as usize;
        addr >= base && addr < end
    }

    /// Check if the slab is full (no free objects).
    fn is_full(&self) -> bool {
        self.free_count == 0
    }

    /// Check if the slab is empty (all objects free).
    fn is_empty(&self) -> bool {
        self.free_count == self.object_count
    }
}

// SAFETY: Slab pointers are only accessed with proper synchronization
unsafe impl Send for Slab {}

impl Drop for Slab {
    fn drop(&mut self) {
        // Free the underlying page
        if !self.base.is_null() {
            unsafe {
                crate::memory::free_frame(self.base as usize);
            }
        }
    }
}
