//! Physical frame allocator using buddy system.
//!
//! The buddy allocator efficiently manages physical memory by organizing
//! free blocks into power-of-two sized groups. This allows for O(log n)
//! allocation and deallocation while minimizing fragmentation.

use crate::config::PAGE_SIZE;
use core::cmp;

/// Maximum order for the buddy allocator (2^MAX_ORDER pages).
const MAX_ORDER: usize = 11; // Up to 8 MB blocks (2^11 * 4KB)

/// Buddy allocator for physical memory.
pub struct FrameAllocator {
    /// Free lists for each order.
    /// Order 0 = single page, Order 1 = 2 pages, etc.
    free_lists: [FreeList; MAX_ORDER + 1],
    
    /// Total memory managed by this allocator.
    total_memory: usize,
    
    /// Currently free memory.
    free_memory: usize,
}

/// A linked list of free blocks.
struct FreeList {
    head: Option<*mut FreeBlock>,
    count: usize,
}

/// A free block in the free list.
#[repr(C)]
struct FreeBlock {
    next: Option<*mut FreeBlock>,
}

impl FrameAllocator {
    /// Create a new empty frame allocator.
    pub const fn new() -> Self {
        Self {
            free_lists: [FreeList::new(); MAX_ORDER + 1],
            total_memory: 0,
            free_memory: 0,
        }
    }
    
    /// Add a memory region to the allocator.
    ///
    /// # Arguments
    ///
    /// * `start` - Physical start address (must be page-aligned)
    /// * `size` - Size in bytes (will be rounded down to page size)
    ///
    /// # Safety
    ///
    /// The caller must ensure the memory region is valid and not in use.
    pub unsafe fn add_region(&mut self, start: u64, size: usize) {
        // Align start up to page boundary
        let aligned_start = (start + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1);
        let adjustment = (aligned_start - start) as usize;
        
        if adjustment >= size {
            return; // Region too small
        }
        
        let adjusted_size = (size - adjustment) & !(PAGE_SIZE - 1);
        if adjusted_size == 0 {
            return;
        }
        
        self.total_memory += adjusted_size;
        self.free_memory += adjusted_size;
        
        // Add blocks to free lists
        let mut addr = aligned_start;
        let mut remaining = adjusted_size;
        
        while remaining > 0 {
            // Find the largest order that fits
            let order = self.size_to_order(remaining);
            let block_size = self.order_to_size(order);
            
            // Check alignment - block must be aligned to its size
            let aligned_order = self.alignment_order(addr);
            let actual_order = cmp::min(order, aligned_order);
            let actual_size = self.order_to_size(actual_order);
            
            // Add to free list
            unsafe { self.add_to_free_list(addr, actual_order) };
            
            addr += actual_size as u64;
            remaining -= actual_size;
        }
    }
    
    /// Allocate memory of the given size.
    ///
    /// # Arguments
    ///
    /// * `size` - Size in bytes (will be rounded up to a power of two pages)
    ///
    /// # Returns
    ///
    /// Physical address of the allocated memory, or `None` if out of memory.
    pub fn allocate(&mut self, size: usize) -> Option<u64> {
        let order = self.size_to_order(size);
        
        // Find a free block of sufficient size
        for current_order in order..=MAX_ORDER {
            if let Some(block) = self.remove_from_free_list(current_order) {
                // Split larger blocks if necessary
                let mut split_order = current_order;
                while split_order > order {
                    split_order -= 1;
                    let buddy = block + self.order_to_size(split_order) as u64;
                    // SAFETY: We're adding back a valid split block
                    unsafe { self.add_to_free_list(buddy, split_order) };
                }
                
                self.free_memory -= self.order_to_size(order);
                return Some(block);
            }
        }
        
        None
    }
    
    /// Free previously allocated memory.
    ///
    /// # Arguments
    ///
    /// * `addr` - Physical address to free
    /// * `size` - Size that was allocated
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - The address was previously allocated with the same size
    /// - The memory is no longer in use
    pub unsafe fn free(&mut self, addr: u64, size: usize) {
        let mut order = self.size_to_order(size);
        let mut current_addr = addr;
        
        // Try to coalesce with buddies
        while order < MAX_ORDER {
            let buddy_addr = self.buddy_of(current_addr, order);
            
            // Try to remove buddy from free list
            if self.remove_block_from_free_list(buddy_addr, order) {
                // Merge with buddy
                current_addr = cmp::min(current_addr, buddy_addr);
                order += 1;
            } else {
                break;
            }
        }
        
        // Add merged block to free list
        unsafe { self.add_to_free_list(current_addr, order) };
        self.free_memory += size;
    }
    
    /// Get total memory managed by this allocator.
    pub fn total_memory(&self) -> usize {
        self.total_memory
    }
    
    /// Get free memory available for allocation.
    pub fn free_memory(&self) -> usize {
        self.free_memory
    }
    
    /// Convert size to order (smallest order that can hold size).
    fn size_to_order(&self, size: usize) -> usize {
        let pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;
        let order = (usize::BITS - pages.saturating_sub(1).leading_zeros()) as usize;
        cmp::min(order, MAX_ORDER)
    }
    
    /// Convert order to size in bytes.
    fn order_to_size(&self, order: usize) -> usize {
        PAGE_SIZE << order
    }
    
    /// Get the buddy address for a block.
    fn buddy_of(&self, addr: u64, order: usize) -> u64 {
        addr ^ (self.order_to_size(order) as u64)
    }
    
    /// Get the maximum order this address can be aligned to.
    fn alignment_order(&self, addr: u64) -> usize {
        let trailing_zeros = addr.trailing_zeros() as usize;
        let page_bits = PAGE_SIZE.trailing_zeros() as usize;
        cmp::min(trailing_zeros.saturating_sub(page_bits), MAX_ORDER)
    }
    
    /// Add a block to the free list.
    ///
    /// # Safety
    ///
    /// The caller must ensure the block is valid and not already in the list.
    unsafe fn add_to_free_list(&mut self, addr: u64, order: usize) {
        let block = addr as *mut FreeBlock;
        unsafe {
            (*block).next = self.free_lists[order].head;
        }
        self.free_lists[order].head = Some(block);
        self.free_lists[order].count += 1;
    }
    
    /// Remove a block from the free list.
    fn remove_from_free_list(&mut self, order: usize) -> Option<u64> {
        let head = self.free_lists[order].head?;
        // SAFETY: head is a valid pointer from our free list
        self.free_lists[order].head = unsafe { (*head).next };
        self.free_lists[order].count -= 1;
        Some(head as u64)
    }
    
    /// Remove a specific block from the free list.
    fn remove_block_from_free_list(&mut self, addr: u64, order: usize) -> bool {
        let target = addr as *mut FreeBlock;
        let mut current = &mut self.free_lists[order].head;
        
        while let Some(block) = *current {
            if block == target {
                // SAFETY: block is a valid pointer from our free list
                *current = unsafe { (*block).next };
                self.free_lists[order].count -= 1;
                return true;
            }
            // SAFETY: block is a valid pointer from our free list
            current = unsafe { &mut (*block).next };
        }
        
        false
    }
}

impl FreeList {
    const fn new() -> Self {
        Self {
            head: None,
            count: 0,
        }
    }
}

// SAFETY: FreeList pointers are only accessed with proper synchronization
unsafe impl Send for FreeList {}
