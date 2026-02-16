//! Shared Memory IPC
//!
//! This module implements shared memory regions for efficient
//! data transfer between processes without copying.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::{Mutex, RwLock};

use super::capability::{CapabilityId, CapabilityRights};

/// Shared memory region ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShmId(pub u64);

impl ShmId {
    /// Invalid shared memory ID.
    pub const INVALID: ShmId = ShmId(0);
}

/// Shared memory flags.
pub mod flags {
    /// Region can be read.
    pub const SHM_READ: u32 = 0x01;
    /// Region can be written.
    pub const SHM_WRITE: u32 = 0x02;
    /// Region can be executed.
    pub const SHM_EXEC: u32 = 0x04;
    /// Region is locked in memory.
    pub const SHM_LOCKED: u32 = 0x10;
    /// Region uses huge pages.
    pub const SHM_HUGE: u32 = 0x20;
}

/// Shared memory region state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShmState {
    /// Region is allocated but not mapped.
    Allocated,
    /// Region is mapped by at least one process.
    Mapped,
    /// Region is being destroyed.
    Destroying,
}

/// A mapping of shared memory into a process.
#[derive(Debug, Clone)]
pub struct ShmMapping {
    /// Process ID that has this mapping.
    pub pid: u64,
    /// Virtual address in the process.
    pub vaddr: u64,
    /// Access rights for this mapping.
    pub rights: u32,
}

/// Shared memory region descriptor.
pub struct SharedMemoryRegion {
    /// Unique ID.
    id: ShmId,

    /// Human-readable name (for debugging).
    name: String,

    /// Size in bytes.
    size: usize,

    /// Physical frames backing this region.
    /// Each entry is a physical address.
    frames: Vec<u64>,

    /// Current state.
    state: ShmState,

    /// Creation flags.
    flags: u32,

    /// Creator process ID.
    creator: u64,

    /// Reference count.
    ref_count: usize,

    /// All current mappings.
    mappings: Vec<ShmMapping>,

    /// Associated capability (for access control).
    capability: Option<CapabilityId>,
}

impl SharedMemoryRegion {
    /// Create a new shared memory region.
    pub fn new(id: ShmId, name: &str, size: usize, creator: u64, flags: u32) -> Self {
        // Calculate number of pages needed
        let page_size = 4096usize;
        let num_pages = (size + page_size - 1) / page_size;

        SharedMemoryRegion {
            id,
            name: String::from(name),
            size,
            frames: Vec::with_capacity(num_pages),
            state: ShmState::Allocated,
            flags,
            creator,
            ref_count: 1,
            mappings: Vec::new(),
            capability: None,
        }
    }

    /// Get the region ID.
    pub fn id(&self) -> ShmId {
        self.id
    }

    /// Get the region size.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get the number of pages.
    pub fn page_count(&self) -> usize {
        (self.size + 4095) / 4096
    }

    /// Check if the region is mapped.
    pub fn is_mapped(&self) -> bool {
        !self.mappings.is_empty()
    }

    /// Get reference count.
    pub fn ref_count(&self) -> usize {
        self.ref_count
    }

    /// Increment reference count.
    pub fn add_ref(&mut self) {
        self.ref_count += 1;
    }

    /// Decrement reference count.
    pub fn release(&mut self) -> bool {
        if self.ref_count > 0 {
            self.ref_count -= 1;
        }
        self.ref_count == 0
    }

    /// Add physical frames.
    pub fn add_frames(&mut self, frames: Vec<u64>) {
        self.frames = frames;
    }

    /// Get physical frames.
    pub fn frames(&self) -> &[u64] {
        &self.frames
    }

    /// Add a mapping.
    pub fn add_mapping(&mut self, mapping: ShmMapping) {
        self.mappings.push(mapping);
        self.state = ShmState::Mapped;
    }

    /// Remove a mapping by process ID.
    pub fn remove_mapping(&mut self, pid: u64) -> Option<ShmMapping> {
        if let Some(pos) = self.mappings.iter().position(|m| m.pid == pid) {
            let mapping = self.mappings.remove(pos);
            if self.mappings.is_empty() {
                self.state = ShmState::Allocated;
            }
            Some(mapping)
        } else {
            None
        }
    }

    /// Get all mappings.
    pub fn mappings(&self) -> &[ShmMapping] {
        &self.mappings
    }

    /// Find mapping for a process.
    pub fn find_mapping(&self, pid: u64) -> Option<&ShmMapping> {
        self.mappings.iter().find(|m| m.pid == pid)
    }

    /// Set capability.
    pub fn set_capability(&mut self, cap: CapabilityId) {
        self.capability = Some(cap);
    }

    /// Get capability.
    pub fn capability(&self) -> Option<CapabilityId> {
        self.capability
    }
}

/// Error type for shared memory operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShmError {
    /// Shared memory not found.
    NotFound,
    /// Out of memory.
    OutOfMemory,
    /// Invalid size.
    InvalidSize,
    /// Permission denied.
    PermissionDenied,
    /// Already mapped.
    AlreadyMapped,
    /// Not mapped.
    NotMapped,
    /// Invalid address.
    InvalidAddress,
    /// Region limit reached.
    LimitReached,
}

/// Global shared memory manager.
pub struct SharedMemoryManager {
    /// All shared memory regions.
    regions: BTreeMap<ShmId, Arc<Mutex<SharedMemoryRegion>>>,

    /// Next region ID.
    next_id: AtomicU64,

    /// Maximum number of regions.
    max_regions: usize,

    /// Total allocated size.
    total_size: AtomicU64,

    /// Maximum total size.
    max_total_size: usize,
}

impl SharedMemoryManager {
    /// Create a new shared memory manager.
    pub fn new() -> Self {
        SharedMemoryManager {
            regions: BTreeMap::new(),
            next_id: AtomicU64::new(1),
            max_regions: 4096,
            total_size: AtomicU64::new(0),
            max_total_size: 1024 * 1024 * 1024, // 1GB max
        }
    }

    /// Create a new shared memory region.
    pub fn create(
        &mut self,
        name: &str,
        size: usize,
        creator: u64,
        flags: u32,
    ) -> Result<ShmId, ShmError> {
        // Validate size
        if size == 0 || size > self.max_total_size {
            return Err(ShmError::InvalidSize);
        }

        // Check limits
        if self.regions.len() >= self.max_regions {
            return Err(ShmError::LimitReached);
        }

        let current_total = self.total_size.load(Ordering::Relaxed) as usize;
        if current_total + size > self.max_total_size {
            return Err(ShmError::OutOfMemory);
        }

        // Generate ID
        let id = ShmId(self.next_id.fetch_add(1, Ordering::Relaxed));

        // Create region
        let region = SharedMemoryRegion::new(id, name, size, creator, flags);

        // TODO: Allocate physical frames
        // This would integrate with the memory subsystem

        self.regions.insert(id, Arc::new(Mutex::new(region)));
        self.total_size.fetch_add(size as u64, Ordering::Relaxed);

        Ok(id)
    }

    /// Get a shared memory region.
    pub fn get(&self, id: ShmId) -> Option<Arc<Mutex<SharedMemoryRegion>>> {
        self.regions.get(&id).cloned()
    }

    /// Map shared memory into a process.
    pub fn map(&self, id: ShmId, pid: u64, vaddr: u64, rights: u32) -> Result<u64, ShmError> {
        let region = self.regions.get(&id).ok_or(ShmError::NotFound)?;
        let mut region = region.lock();

        // Check if already mapped in this process
        if region.find_mapping(pid).is_some() {
            return Err(ShmError::AlreadyMapped);
        }

        // Add mapping
        region.add_mapping(ShmMapping { pid, vaddr, rights });
        region.add_ref();

        // TODO: Actually map pages into process page table

        Ok(vaddr)
    }

    /// Unmap shared memory from a process.
    pub fn unmap(&self, id: ShmId, pid: u64) -> Result<(), ShmError> {
        let region = self.regions.get(&id).ok_or(ShmError::NotFound)?;
        let mut region = region.lock();

        region.remove_mapping(pid).ok_or(ShmError::NotMapped)?;

        // TODO: Actually unmap pages from process page table

        Ok(())
    }

    /// Destroy a shared memory region.
    pub fn destroy(&mut self, id: ShmId, pid: u64) -> Result<(), ShmError> {
        let region = self.regions.get(&id).ok_or(ShmError::NotFound)?;

        {
            let mut region = region.lock();

            // Only creator can destroy
            if region.creator != pid {
                return Err(ShmError::PermissionDenied);
            }

            // Release reference
            if !region.release() {
                // Still has references, just mark for destruction
                region.state = ShmState::Destroying;
                return Ok(());
            }

            let size = region.size;
            self.total_size.fetch_sub(size as u64, Ordering::Relaxed);
        }

        // Remove from registry
        self.regions.remove(&id);

        // TODO: Free physical frames

        Ok(())
    }

    /// Get statistics.
    pub fn stats(&self) -> (usize, usize) {
        (
            self.regions.len(),
            self.total_size.load(Ordering::Relaxed) as usize,
        )
    }
}

/// Global shared memory manager instance.
static SHM_MANAGER: RwLock<Option<SharedMemoryManager>> = RwLock::new(None);

/// Initialize shared memory subsystem.
pub fn init() {
    let mut manager = SHM_MANAGER.write();
    *manager = Some(SharedMemoryManager::new());
}

/// Create shared memory region.
pub fn create(name: &str, size: usize, creator: u64, flags: u32) -> Result<ShmId, ShmError> {
    SHM_MANAGER
        .write()
        .as_mut()
        .ok_or(ShmError::NotFound)?
        .create(name, size, creator, flags)
}

/// Get shared memory region.
pub fn get(id: ShmId) -> Option<Arc<Mutex<SharedMemoryRegion>>> {
    SHM_MANAGER.read().as_ref()?.get(id)
}

/// Map shared memory.
pub fn map(id: ShmId, pid: u64, vaddr: u64, rights: u32) -> Result<u64, ShmError> {
    SHM_MANAGER
        .read()
        .as_ref()
        .ok_or(ShmError::NotFound)?
        .map(id, pid, vaddr, rights)
}

/// Unmap shared memory.
pub fn unmap(id: ShmId, pid: u64) -> Result<(), ShmError> {
    SHM_MANAGER
        .read()
        .as_ref()
        .ok_or(ShmError::NotFound)?
        .unmap(id, pid)
}

/// Destroy shared memory.
pub fn destroy(id: ShmId, pid: u64) -> Result<(), ShmError> {
    SHM_MANAGER
        .write()
        .as_mut()
        .ok_or(ShmError::NotFound)?
        .destroy(id, pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shm_create() {
        let mut manager = SharedMemoryManager::new();
        let id = manager.create("test", 4096, 1, flags::SHM_READ | flags::SHM_WRITE);
        assert!(id.is_ok());

        let id = id.unwrap();
        assert!(manager.get(id).is_some());
    }

    #[test]
    fn test_shm_map_unmap() {
        let mut manager = SharedMemoryManager::new();
        let id = manager.create("test", 4096, 1, flags::SHM_READ).unwrap();

        // Map
        let result = manager.map(id, 2, 0x1000_0000, flags::SHM_READ);
        assert!(result.is_ok());

        // Double map should fail
        let result = manager.map(id, 2, 0x2000_0000, flags::SHM_READ);
        assert_eq!(result, Err(ShmError::AlreadyMapped));

        // Unmap
        let result = manager.unmap(id, 2);
        assert!(result.is_ok());
    }
}
