//! GPU buffer management.
//!
//! This module provides GPU buffer allocation and management.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use spin::Mutex;

use crate::GraphicsError;

/// Global buffer allocator.
static BUFFER_ALLOCATOR: Mutex<Option<BufferAllocator>> = Mutex::new(None);

/// Buffer usage flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferUsage(u32);

impl BufferUsage {
    /// Vertex buffer usage.
    pub const VERTEX: BufferUsage = BufferUsage(1 << 0);
    /// Index buffer usage.
    pub const INDEX: BufferUsage = BufferUsage(1 << 1);
    /// Uniform buffer usage.
    pub const UNIFORM: BufferUsage = BufferUsage(1 << 2);
    /// Storage buffer usage.
    pub const STORAGE: BufferUsage = BufferUsage(1 << 3);
    /// Indirect buffer usage.
    pub const INDIRECT: BufferUsage = BufferUsage(1 << 4);
    /// Transfer source usage.
    pub const TRANSFER_SRC: BufferUsage = BufferUsage(1 << 5);
    /// Transfer destination usage.
    pub const TRANSFER_DST: BufferUsage = BufferUsage(1 << 6);
    
    /// Combine usages.
    pub fn or(self, other: BufferUsage) -> BufferUsage {
        BufferUsage(self.0 | other.0)
    }
    
    /// Check if contains usage.
    pub fn contains(self, other: BufferUsage) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// Buffer memory location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryLocation {
    /// GPU-only memory (fastest for GPU access).
    GpuOnly,
    /// CPU-visible memory (for uploads).
    CpuToGpu,
    /// GPU-to-CPU memory (for readback).
    GpuToCpu,
}

/// A GPU buffer.
pub struct Buffer {
    /// Buffer handle.
    handle: BufferHandle,
    /// Buffer size in bytes.
    size: u64,
    /// Buffer usage.
    usage: BufferUsage,
    /// Memory location.
    location: MemoryLocation,
    /// Mapped pointer (if mapped).
    mapped: Option<*mut u8>,
}

/// Buffer handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferHandle(pub u64);

impl Buffer {
    /// Create a new buffer.
    pub fn new(size: u64, usage: BufferUsage, location: MemoryLocation) -> Result<Self, GraphicsError> {
        let handle = allocate_buffer(size, usage, location)?;
        
        Ok(Buffer {
            handle,
            size,
            usage,
            location,
            mapped: None,
        })
    }
    
    /// Get the buffer handle.
    pub fn handle(&self) -> BufferHandle {
        self.handle
    }
    
    /// Get the buffer size.
    pub fn size(&self) -> u64 {
        self.size
    }
    
    /// Get the usage flags.
    pub fn usage(&self) -> BufferUsage {
        self.usage
    }
    
    /// Map the buffer for CPU access.
    pub fn map(&mut self) -> Result<&mut [u8], GraphicsError> {
        if self.location == MemoryLocation::GpuOnly {
            return Err(GraphicsError::InvalidOperation(
                "Cannot map GPU-only buffer".into()
            ));
        }
        
        if self.mapped.is_some() {
            return Err(GraphicsError::InvalidOperation(
                "Buffer already mapped".into()
            ));
        }
        
        // Map the buffer
        let ptr = map_buffer(self.handle)?;
        self.mapped = Some(ptr);
        
        unsafe {
            Ok(core::slice::from_raw_parts_mut(ptr, self.size as usize))
        }
    }
    
    /// Unmap the buffer.
    pub fn unmap(&mut self) -> Result<(), GraphicsError> {
        if self.mapped.is_none() {
            return Err(GraphicsError::InvalidOperation(
                "Buffer not mapped".into()
            ));
        }
        
        unmap_buffer(self.handle)?;
        self.mapped = None;
        
        Ok(())
    }
    
    /// Write data to the buffer.
    pub fn write(&mut self, offset: u64, data: &[u8]) -> Result<(), GraphicsError> {
        if offset + data.len() as u64 > self.size {
            return Err(GraphicsError::InvalidOperation(
                "Write exceeds buffer size".into()
            ));
        }
        
        let slice = self.map()?;
        slice[offset as usize..offset as usize + data.len()].copy_from_slice(data);
        self.unmap()?;
        
        Ok(())
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if self.mapped.is_some() {
            let _ = self.unmap();
        }
        let _ = free_buffer(self.handle);
    }
}

/// Buffer allocator.
struct BufferAllocator {
    /// Allocated buffers.
    buffers: BTreeMap<BufferHandle, BufferAllocation>,
    /// Next buffer handle.
    next_handle: u64,
    /// Total allocated memory.
    total_allocated: u64,
}

struct BufferAllocation {
    size: u64,
    usage: BufferUsage,
    location: MemoryLocation,
}

impl BufferAllocator {
    fn new() -> Self {
        BufferAllocator {
            buffers: BTreeMap::new(),
            next_handle: 1,
            total_allocated: 0,
        }
    }
    
    fn allocate(&mut self, size: u64, usage: BufferUsage, location: MemoryLocation) -> BufferHandle {
        let handle = BufferHandle(self.next_handle);
        self.next_handle += 1;
        
        self.buffers.insert(handle, BufferAllocation {
            size,
            usage,
            location,
        });
        
        self.total_allocated += size;
        
        handle
    }
    
    fn free(&mut self, handle: BufferHandle) -> Result<(), GraphicsError> {
        if let Some(alloc) = self.buffers.remove(&handle) {
            self.total_allocated -= alloc.size;
            Ok(())
        } else {
            Err(GraphicsError::InvalidOperation("Invalid buffer handle".into()))
        }
    }
}

/// Allocate a buffer.
fn allocate_buffer(size: u64, usage: BufferUsage, location: MemoryLocation) -> Result<BufferHandle, GraphicsError> {
    let mut allocator = BUFFER_ALLOCATOR.lock();
    let allocator = allocator.get_or_insert_with(BufferAllocator::new);
    Ok(allocator.allocate(size, usage, location))
}

/// Free a buffer.
fn free_buffer(handle: BufferHandle) -> Result<(), GraphicsError> {
    if let Some(ref mut allocator) = *BUFFER_ALLOCATOR.lock() {
        allocator.free(handle)
    } else {
        Err(GraphicsError::InvalidOperation("Buffer allocator not initialized".into()))
    }
}

/// Map a buffer.
fn map_buffer(_handle: BufferHandle) -> Result<*mut u8, GraphicsError> {
    // Placeholder - actual implementation would use Vulkan
    Err(GraphicsError::InvalidOperation("Not implemented".into()))
}

/// Unmap a buffer.
fn unmap_buffer(_handle: BufferHandle) -> Result<(), GraphicsError> {
    // Placeholder - actual implementation would use Vulkan
    Ok(())
}
