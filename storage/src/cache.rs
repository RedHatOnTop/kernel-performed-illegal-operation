//! Block cache implementation.
//!
//! This module provides a block caching layer to improve I/O performance
//! by reducing disk accesses.

use alloc::boxed::Box;
use alloc::vec::Vec;
use spin::Mutex;

use crate::StorageError;

/// Maximum number of cached blocks.
const CACHE_SIZE: usize = 1024;

/// Block size for cache (must match filesystem block size).
const BLOCK_SIZE: usize = 4096;

/// Cache entry state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheState {
    /// Entry is free.
    Free,
    /// Entry contains valid data.
    Clean,
    /// Entry contains modified data.
    Dirty,
    /// Entry is locked for I/O.
    Locked,
}

/// Cache entry.
pub struct CacheEntry {
    /// Device ID.
    device_id: u32,
    /// Block number.
    block_num: u64,
    /// Entry state.
    state: CacheState,
    /// Reference count.
    ref_count: u32,
    /// Access count for LRU.
    access_count: u64,
    /// Block data.
    data: [u8; BLOCK_SIZE],
}

impl CacheEntry {
    /// Create a new free cache entry.
    pub const fn new() -> Self {
        CacheEntry {
            device_id: 0,
            block_num: 0,
            state: CacheState::Free,
            ref_count: 0,
            access_count: 0,
            data: [0; BLOCK_SIZE],
        }
    }

    /// Check if entry is free.
    pub fn is_free(&self) -> bool {
        self.state == CacheState::Free
    }

    /// Check if entry matches device and block.
    pub fn matches(&self, device_id: u32, block_num: u64) -> bool {
        self.device_id == device_id && self.block_num == block_num && self.state != CacheState::Free
    }

    /// Get data reference.
    pub fn data(&self) -> &[u8; BLOCK_SIZE] {
        &self.data
    }

    /// Get mutable data reference.
    pub fn data_mut(&mut self) -> &mut [u8; BLOCK_SIZE] {
        &mut self.data
    }

    /// Mark as dirty.
    pub fn mark_dirty(&mut self) {
        if self.state == CacheState::Clean {
            self.state = CacheState::Dirty;
        }
    }

    /// Mark as clean.
    pub fn mark_clean(&mut self) {
        if self.state == CacheState::Dirty {
            self.state = CacheState::Clean;
        }
    }
}

/// Block cache.
pub struct BlockCache {
    /// Cache entries.
    entries: [CacheEntry; CACHE_SIZE],
    /// Global access counter.
    access_counter: u64,
    /// Number of cache hits.
    hits: u64,
    /// Number of cache misses.
    misses: u64,
    /// Number of dirty entries.
    dirty_count: usize,
}

impl BlockCache {
    /// Create a new block cache.
    pub const fn new() -> Self {
        const ENTRY: CacheEntry = CacheEntry::new();
        BlockCache {
            entries: [ENTRY; CACHE_SIZE],
            access_counter: 0,
            hits: 0,
            misses: 0,
            dirty_count: 0,
        }
    }

    /// Find a cache entry.
    pub fn find(&mut self, device_id: u32, block_num: u64) -> Option<usize> {
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.matches(device_id, block_num) {
                return Some(i);
            }
        }
        None
    }

    /// Get a block from cache.
    pub fn get(&mut self, device_id: u32, block_num: u64) -> Option<&[u8; BLOCK_SIZE]> {
        if let Some(idx) = self.find(device_id, block_num) {
            self.access_counter += 1;
            self.entries[idx].access_count = self.access_counter;
            self.entries[idx].ref_count += 1;
            self.hits += 1;
            Some(&self.entries[idx].data)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Get a mutable block from cache.
    pub fn get_mut(&mut self, device_id: u32, block_num: u64) -> Option<&mut [u8; BLOCK_SIZE]> {
        if let Some(idx) = self.find(device_id, block_num) {
            self.access_counter += 1;
            self.entries[idx].access_count = self.access_counter;
            self.entries[idx].ref_count += 1;
            self.entries[idx].mark_dirty();
            self.hits += 1;
            Some(&mut self.entries[idx].data)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Find a free or evictable slot.
    fn find_slot(&self) -> Option<usize> {
        // First, look for a free entry
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.is_free() {
                return Some(i);
            }
        }

        // Find LRU clean entry
        let mut min_access = u64::MAX;
        let mut min_idx = None;

        for (i, entry) in self.entries.iter().enumerate() {
            if entry.state == CacheState::Clean && entry.ref_count == 0 {
                if entry.access_count < min_access {
                    min_access = entry.access_count;
                    min_idx = Some(i);
                }
            }
        }

        min_idx
    }

    /// Insert a block into cache.
    pub fn insert(
        &mut self,
        device_id: u32,
        block_num: u64,
        data: &[u8],
    ) -> Result<usize, StorageError> {
        // Check if already cached
        if let Some(idx) = self.find(device_id, block_num) {
            // Update existing entry
            self.entries[idx].data[..data.len()].copy_from_slice(data);
            self.access_counter += 1;
            self.entries[idx].access_count = self.access_counter;
            return Ok(idx);
        }

        // Find a slot
        let idx = self.find_slot().ok_or(StorageError::NoSpace)?;

        // Initialize entry
        self.entries[idx].device_id = device_id;
        self.entries[idx].block_num = block_num;
        self.entries[idx].state = CacheState::Clean;
        self.entries[idx].ref_count = 0;
        self.access_counter += 1;
        self.entries[idx].access_count = self.access_counter;
        self.entries[idx].data[..data.len()].copy_from_slice(data);

        Ok(idx)
    }

    /// Release a reference to a cache entry.
    pub fn release(&mut self, device_id: u32, block_num: u64) {
        if let Some(idx) = self.find(device_id, block_num) {
            if self.entries[idx].ref_count > 0 {
                self.entries[idx].ref_count -= 1;
            }
        }
    }

    /// Invalidate a cache entry.
    pub fn invalidate(&mut self, device_id: u32, block_num: u64) {
        if let Some(idx) = self.find(device_id, block_num) {
            if self.entries[idx].state == CacheState::Dirty {
                self.dirty_count -= 1;
            }
            self.entries[idx].state = CacheState::Free;
            self.entries[idx].ref_count = 0;
        }
    }

    /// Invalidate all entries for a device.
    pub fn invalidate_device(&mut self, device_id: u32) {
        for entry in &mut self.entries {
            if entry.device_id == device_id && entry.state != CacheState::Free {
                if entry.state == CacheState::Dirty {
                    self.dirty_count -= 1;
                }
                entry.state = CacheState::Free;
                entry.ref_count = 0;
            }
        }
    }

    /// Get all dirty entries for a device.
    pub fn get_dirty_blocks(&self, device_id: u32) -> Vec<(u64, &[u8; BLOCK_SIZE])> {
        let mut result = Vec::new();

        for entry in &self.entries {
            if entry.device_id == device_id && entry.state == CacheState::Dirty {
                result.push((entry.block_num, &entry.data));
            }
        }

        result
    }

    /// Mark a block as clean.
    pub fn mark_clean(&mut self, device_id: u32, block_num: u64) {
        if let Some(idx) = self.find(device_id, block_num) {
            if self.entries[idx].state == CacheState::Dirty {
                self.entries[idx].state = CacheState::Clean;
                self.dirty_count -= 1;
            }
        }
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            size: CACHE_SIZE,
            used: self.entries.iter().filter(|e| !e.is_free()).count(),
            dirty: self.dirty_count,
            hits: self.hits,
            misses: self.misses,
        }
    }

    /// Flush all dirty entries.
    pub fn flush_all(&mut self) {
        // Note: Actual write-back would require device access
        for entry in &mut self.entries {
            if entry.state == CacheState::Dirty {
                entry.state = CacheState::Clean;
            }
        }
        self.dirty_count = 0;
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            entry.state = CacheState::Free;
            entry.ref_count = 0;
        }
        self.dirty_count = 0;
    }
}

/// Cache statistics.
#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    /// Total cache size (entries).
    pub size: usize,
    /// Number of used entries.
    pub used: usize,
    /// Number of dirty entries.
    pub dirty: usize,
    /// Number of cache hits.
    pub hits: u64,
    /// Number of cache misses.
    pub misses: u64,
}

impl CacheStats {
    /// Get hit ratio.
    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// Global block cache.
static CACHE: Mutex<BlockCache> = Mutex::new(BlockCache::new());

/// Initialize the block cache.
pub fn init() -> Result<(), StorageError> {
    let mut cache = CACHE.lock();
    cache.clear();
    Ok(())
}

/// Get a block from cache.
pub fn get_block(device_id: u32, block_num: u64) -> Option<[u8; BLOCK_SIZE]> {
    let mut cache = CACHE.lock();
    cache.get(device_id, block_num).copied()
}

/// Insert a block into cache.
pub fn cache_block(device_id: u32, block_num: u64, data: &[u8]) -> Result<(), StorageError> {
    let mut cache = CACHE.lock();
    cache.insert(device_id, block_num, data)?;
    Ok(())
}

/// Invalidate a cached block.
pub fn invalidate_block(device_id: u32, block_num: u64) {
    let mut cache = CACHE.lock();
    cache.invalidate(device_id, block_num);
}

/// Get cache statistics.
pub fn get_stats() -> CacheStats {
    let cache = CACHE.lock();
    cache.stats()
}

/// Flush all dirty blocks.
pub fn flush() {
    let mut cache = CACHE.lock();
    cache.flush_all();
}
