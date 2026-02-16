//! Buddy Allocator for Large Block Memory Management
//!
//! This module implements a buddy allocator for efficient management of
//! larger memory blocks (4KB to 4MB). It works alongside the slab allocator.
//!
//! # Algorithm
//!
//! The buddy allocator divides memory into power-of-two sized blocks.
//! When allocating, it finds the smallest block that fits the request.
//! When freeing, it merges adjacent "buddy" blocks to reduce fragmentation.
//!
//! ```text
//! Order 0: 4KB blocks (2^12)
//! Order 1: 8KB blocks (2^13)
//! Order 2: 16KB blocks (2^14)
//! ...
//! Order 10: 4MB blocks (2^22)
//! ```

use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

/// Maximum order (4MB = 2^22 bytes, order 10 = 2^(12+10) = 2^22)
pub const MAX_ORDER: usize = 10;

/// Minimum block size (4KB = 2^12)
pub const MIN_BLOCK_SIZE: usize = 4096;

/// Buddy allocator statistics.
#[derive(Debug)]
pub struct BuddyStats {
    /// Total allocations.
    pub allocations: AtomicU64,
    /// Total deallocations.
    pub deallocations: AtomicU64,
    /// Allocation failures.
    pub failures: AtomicU64,
    /// Successful merges.
    pub merges: AtomicU64,
    /// Splits performed.
    pub splits: AtomicU64,
}

impl BuddyStats {
    pub const fn new() -> Self {
        Self {
            allocations: AtomicU64::new(0),
            deallocations: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            merges: AtomicU64::new(0),
            splits: AtomicU64::new(0),
        }
    }
}

/// Global buddy allocator statistics.
pub static BUDDY_STATS: BuddyStats = BuddyStats::new();

/// A free block in the buddy system.
#[derive(Clone, Copy)]
struct FreeBlock {
    /// Address of the block.
    addr: usize,
    /// Next free block in the list.
    next: Option<usize>,
}

/// Buddy allocator for managing physical memory.
pub struct BuddyAllocator {
    /// Free lists for each order (0 to MAX_ORDER).
    free_lists: [Option<usize>; MAX_ORDER + 1],
    /// Block metadata storage.
    blocks: [Option<FreeBlock>; 4096],
    /// Next free slot for block metadata.
    next_slot: usize,
    /// Base address of managed memory.
    base: usize,
    /// Total size of managed memory.
    size: usize,
    /// Bitmap tracking allocated blocks.
    allocated_bitmap: [u64; 64],
}

impl BuddyAllocator {
    /// Create a new buddy allocator.
    ///
    /// # Arguments
    /// * `base` - Base address of the memory region
    /// * `size` - Size of the memory region (must be power of 2)
    pub const fn new() -> Self {
        Self {
            free_lists: [None; MAX_ORDER + 1],
            blocks: [None; 4096],
            next_slot: 0,
            base: 0,
            size: 0,
            allocated_bitmap: [0; 64],
        }
    }

    /// Initialize the allocator with a memory region.
    pub fn init(&mut self, base: usize, size: usize) {
        self.base = base;
        self.size = size;

        // Add the entire region as the largest possible block
        let order = Self::size_to_order(size).min(MAX_ORDER);
        self.add_to_free_list(order, base);
    }

    /// Allocate a block of the given size.
    pub fn allocate(&mut self, size: usize) -> Option<usize> {
        let size = size.max(MIN_BLOCK_SIZE);
        let order = Self::size_to_order(size);

        if order > MAX_ORDER {
            BUDDY_STATS.failures.fetch_add(1, Ordering::Relaxed);
            return None;
        }

        // Find a free block of sufficient size
        let block_order = self.find_free_block(order)?;

        // Remove from free list
        let addr = self.remove_from_free_list(block_order)?;

        // Split if necessary
        let mut current_order = block_order;
        while current_order > order {
            current_order -= 1;
            let buddy_addr = addr + Self::order_to_size(current_order);
            self.add_to_free_list(current_order, buddy_addr);
            BUDDY_STATS.splits.fetch_add(1, Ordering::Relaxed);
        }

        // Mark as allocated
        self.set_allocated(addr, order, true);

        BUDDY_STATS.allocations.fetch_add(1, Ordering::Relaxed);
        Some(addr)
    }

    /// Free a previously allocated block.
    pub fn deallocate(&mut self, addr: usize, size: usize) {
        let size = size.max(MIN_BLOCK_SIZE);
        let mut order = Self::size_to_order(size);
        let mut addr = addr;

        // Mark as free
        self.set_allocated(addr, order, false);

        // Try to merge with buddy
        while order < MAX_ORDER {
            let buddy_addr = self.buddy_address(addr, order);

            // Check if buddy is free
            if !self.is_free_at_order(buddy_addr, order) {
                break;
            }

            // Remove buddy from free list
            if !self.remove_specific_from_free_list(order, buddy_addr) {
                break;
            }

            // Merge
            addr = addr.min(buddy_addr);
            order += 1;

            BUDDY_STATS.merges.fetch_add(1, Ordering::Relaxed);
        }

        // Add merged block to free list
        self.add_to_free_list(order, addr);

        BUDDY_STATS.deallocations.fetch_add(1, Ordering::Relaxed);
    }

    /// Find the buddy address for a block.
    fn buddy_address(&self, addr: usize, order: usize) -> usize {
        let block_size = Self::order_to_size(order);
        let relative = addr - self.base;
        let buddy_relative = relative ^ block_size;
        self.base + buddy_relative
    }

    /// Convert size to order.
    fn size_to_order(size: usize) -> usize {
        let mut order = 0;
        let mut block_size = MIN_BLOCK_SIZE;
        while block_size < size {
            block_size *= 2;
            order += 1;
        }
        order
    }

    /// Convert order to size.
    fn order_to_size(order: usize) -> usize {
        MIN_BLOCK_SIZE << order
    }

    /// Find a free block of at least the given order.
    fn find_free_block(&self, min_order: usize) -> Option<usize> {
        for order in min_order..=MAX_ORDER {
            if self.free_lists[order].is_some() {
                return Some(order);
            }
        }
        None
    }

    /// Add a block to the free list.
    fn add_to_free_list(&mut self, order: usize, addr: usize) {
        if self.next_slot >= self.blocks.len() {
            return; // Out of metadata slots
        }

        let slot = self.next_slot;
        self.next_slot += 1;

        self.blocks[slot] = Some(FreeBlock {
            addr,
            next: self.free_lists[order],
        });
        self.free_lists[order] = Some(slot);
    }

    /// Remove a block from the free list.
    fn remove_from_free_list(&mut self, order: usize) -> Option<usize> {
        let slot = self.free_lists[order]?;
        let block = self.blocks[slot]?;
        self.free_lists[order] = block.next;
        self.blocks[slot] = None;
        Some(block.addr)
    }

    /// Remove a specific block from the free list.
    fn remove_specific_from_free_list(&mut self, order: usize, addr: usize) -> bool {
        let mut prev: Option<usize> = None;
        let mut current = self.free_lists[order];

        while let Some(slot) = current {
            if let Some(block) = self.blocks[slot] {
                if block.addr == addr {
                    // Found it
                    if let Some(prev_slot) = prev {
                        if let Some(ref mut prev_block) = self.blocks[prev_slot] {
                            prev_block.next = block.next;
                        }
                    } else {
                        self.free_lists[order] = block.next;
                    }
                    self.blocks[slot] = None;
                    return true;
                }
                prev = current;
                current = block.next;
            } else {
                break;
            }
        }
        false
    }

    /// Check if a block at a given order is free.
    fn is_free_at_order(&self, addr: usize, order: usize) -> bool {
        let mut current = self.free_lists[order];
        while let Some(slot) = current {
            if let Some(block) = self.blocks[slot] {
                if block.addr == addr {
                    return true;
                }
                current = block.next;
            } else {
                break;
            }
        }
        false
    }

    /// Mark a block as allocated or free in the bitmap.
    fn set_allocated(&mut self, addr: usize, _order: usize, allocated: bool) {
        let relative = (addr - self.base) / MIN_BLOCK_SIZE;
        let word = relative / 64;
        let bit = relative % 64;

        if word < self.allocated_bitmap.len() {
            if allocated {
                self.allocated_bitmap[word] |= 1 << bit;
            } else {
                self.allocated_bitmap[word] &= !(1 << bit);
            }
        }
    }

    /// Get statistics about memory usage.
    pub fn stats(&self) -> BuddyAllocatorStats {
        let mut free_blocks = [0usize; MAX_ORDER + 1];

        for order in 0..=MAX_ORDER {
            let mut count = 0;
            let mut current = self.free_lists[order];
            while let Some(slot) = current {
                if let Some(block) = self.blocks[slot] {
                    count += 1;
                    current = block.next;
                } else {
                    break;
                }
            }
            free_blocks[order] = count;
        }

        let mut total_free = 0;
        for order in 0..=MAX_ORDER {
            total_free += free_blocks[order] * Self::order_to_size(order);
        }

        BuddyAllocatorStats {
            free_blocks,
            total_size: self.size,
            free_size: total_free,
        }
    }
}

/// Statistics about the buddy allocator state.
#[derive(Debug)]
pub struct BuddyAllocatorStats {
    /// Number of free blocks at each order.
    pub free_blocks: [usize; MAX_ORDER + 1],
    /// Total managed memory size.
    pub total_size: usize,
    /// Total free memory.
    pub free_size: usize,
}

/// Global buddy allocator.
static BUDDY_ALLOCATOR: Mutex<BuddyAllocator> = Mutex::new(BuddyAllocator::new());

/// Initialize the buddy allocator with a memory region.
pub fn init(base: usize, size: usize) {
    BUDDY_ALLOCATOR.lock().init(base, size);
}

/// Allocate memory from the buddy allocator.
pub fn alloc(size: usize) -> Option<usize> {
    BUDDY_ALLOCATOR.lock().allocate(size)
}

/// Free memory back to the buddy allocator.
pub fn free(addr: usize, size: usize) {
    BUDDY_ALLOCATOR.lock().deallocate(addr, size);
}

/// Get buddy allocator statistics.
pub fn stats() -> BuddyAllocatorStats {
    BUDDY_ALLOCATOR.lock().stats()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_to_order() {
        assert_eq!(BuddyAllocator::size_to_order(4096), 0);
        assert_eq!(BuddyAllocator::size_to_order(8192), 1);
        assert_eq!(BuddyAllocator::size_to_order(4097), 1);
        assert_eq!(BuddyAllocator::size_to_order(16384), 2);
    }

    #[test]
    fn test_order_to_size() {
        assert_eq!(BuddyAllocator::order_to_size(0), 4096);
        assert_eq!(BuddyAllocator::order_to_size(1), 8192);
        assert_eq!(BuddyAllocator::order_to_size(10), 4 * 1024 * 1024);
    }
}
