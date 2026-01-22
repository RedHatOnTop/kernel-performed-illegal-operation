//! Slab allocator for fixed-size object caching.
//!
//! The slab allocator provides efficient allocation of frequently-used
//! fixed-size objects by maintaining pre-allocated pools.

use alloc::vec::Vec;
use spin::Mutex;

/// A slab cache for objects of a specific size.
pub struct SlabCache {
    /// Object size in bytes.
    object_size: usize,
    
    /// Object alignment.
    alignment: usize,
    
    /// List of slabs.
    slabs: Vec<Slab>,
    
    /// Number of allocated objects.
    allocated: usize,
}

/// A single slab containing multiple objects.
struct Slab {
    /// Base address of the slab.
    base: *mut u8,
    
    /// Bitmap of free objects.
    free_bitmap: u64,
    
    /// Number of objects in this slab.
    object_count: usize,
}

/// Global slab allocator for common sizes.
static SLAB_8: Mutex<Option<SlabCache>> = Mutex::new(None);
static SLAB_16: Mutex<Option<SlabCache>> = Mutex::new(None);
static SLAB_32: Mutex<Option<SlabCache>> = Mutex::new(None);
static SLAB_64: Mutex<Option<SlabCache>> = Mutex::new(None);
static SLAB_128: Mutex<Option<SlabCache>> = Mutex::new(None);
static SLAB_256: Mutex<Option<SlabCache>> = Mutex::new(None);

/// Initialize the slab allocator subsystem.
pub fn init() {
    *SLAB_8.lock() = Some(SlabCache::new(8, 8));
    *SLAB_16.lock() = Some(SlabCache::new(16, 8));
    *SLAB_32.lock() = Some(SlabCache::new(32, 8));
    *SLAB_64.lock() = Some(SlabCache::new(64, 8));
    *SLAB_128.lock() = Some(SlabCache::new(128, 8));
    *SLAB_256.lock() = Some(SlabCache::new(256, 8));
}

impl SlabCache {
    /// Create a new slab cache.
    pub fn new(object_size: usize, alignment: usize) -> Self {
        Self {
            object_size: object_size.max(core::mem::size_of::<*mut u8>()),
            alignment,
            slabs: Vec::new(),
            allocated: 0,
        }
    }
    
    /// Allocate an object from the cache.
    pub fn allocate(&mut self) -> Option<*mut u8> {
        // Try to find a slab with free space
        for slab in &mut self.slabs {
            if let Some(ptr) = slab.allocate(self.object_size) {
                self.allocated += 1;
                return Some(ptr);
            }
        }
        
        // No free space, create a new slab
        let slab = Slab::new(self.object_size)?;
        self.slabs.push(slab);
        
        let slab = self.slabs.last_mut()?;
        let ptr = slab.allocate(self.object_size)?;
        self.allocated += 1;
        Some(ptr)
    }
    
    /// Free an object back to the cache.
    ///
    /// # Safety
    ///
    /// The pointer must have been allocated from this cache.
    pub unsafe fn free(&mut self, ptr: *mut u8) {
        for slab in &mut self.slabs {
            if slab.contains(ptr, self.object_size) {
                unsafe { slab.free(ptr, self.object_size) };
                self.allocated -= 1;
                return;
            }
        }
        
        // Pointer not from this cache - this is a bug
        panic!("Attempted to free pointer not from this slab cache");
    }
}

impl Slab {
    /// Create a new slab.
    fn new(object_size: usize) -> Option<Self> {
        // Allocate a page for the slab
        let base = crate::memory::allocate_frame()? as *mut u8;
        let object_count = (4096 / object_size).min(64);
        
        Some(Self {
            base,
            free_bitmap: (1u64 << object_count) - 1, // All objects free
            object_count,
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
    }
    
    /// Check if a pointer belongs to this slab.
    fn contains(&self, ptr: *mut u8, object_size: usize) -> bool {
        let base = self.base as usize;
        let end = base + self.object_count * object_size;
        let addr = ptr as usize;
        addr >= base && addr < end
    }
}

// SAFETY: Slab pointers are only accessed with proper synchronization
unsafe impl Send for Slab {}
