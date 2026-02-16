//! GPU Memory Sharing Interface
//!
//! This module provides the interface for sharing GPU memory buffers
//! between the kernel and browser/renderer processes.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::{Mutex, RwLock};

use super::coordinator::TabId;
use crate::ipc::ShmId;

/// GPU buffer handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GpuBufferHandle(pub u64);

impl GpuBufferHandle {
    /// Invalid handle.
    pub const INVALID: GpuBufferHandle = GpuBufferHandle(0);
}

/// GPU buffer usage flags.
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum GpuBufferUsage {
    /// Vertex buffer.
    Vertex = 1 << 0,
    /// Index buffer.
    Index = 1 << 1,
    /// Uniform/constant buffer.
    Uniform = 1 << 2,
    /// Storage buffer.
    Storage = 1 << 3,
    /// Texture/image.
    Texture = 1 << 4,
    /// Render target/framebuffer.
    RenderTarget = 1 << 5,
    /// Depth/stencil buffer.
    DepthStencil = 1 << 6,
    /// Transfer source.
    TransferSrc = 1 << 7,
    /// Transfer destination.
    TransferDst = 1 << 8,
    /// CPU readable.
    CpuRead = 1 << 9,
    /// CPU writable.
    CpuWrite = 1 << 10,
}

/// GPU memory type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuMemoryType {
    /// Device-local (VRAM), fastest for GPU access.
    DeviceLocal,
    /// Host-visible, CPU can map it.
    HostVisible,
    /// Host-coherent, no explicit flush needed.
    HostCoherent,
    /// Host-cached, CPU caching enabled.
    HostCached,
}

/// GPU buffer descriptor.
#[derive(Debug, Clone)]
pub struct GpuBufferDesc {
    /// Buffer size in bytes.
    pub size: u64,
    /// Alignment requirement.
    pub alignment: u32,
    /// Usage flags.
    pub usage: u32,
    /// Memory type.
    pub memory_type: GpuMemoryType,
    /// Label for debugging.
    pub label: Option<&'static str>,
}

impl GpuBufferDesc {
    /// Create a vertex buffer descriptor.
    pub fn vertex(size: u64) -> Self {
        GpuBufferDesc {
            size,
            alignment: 4,
            usage: GpuBufferUsage::Vertex as u32,
            memory_type: GpuMemoryType::DeviceLocal,
            label: None,
        }
    }

    /// Create a texture descriptor.
    pub fn texture(size: u64) -> Self {
        GpuBufferDesc {
            size,
            alignment: 16,
            usage: GpuBufferUsage::Texture as u32,
            memory_type: GpuMemoryType::DeviceLocal,
            label: None,
        }
    }

    /// Create a staging buffer descriptor.
    pub fn staging(size: u64) -> Self {
        GpuBufferDesc {
            size,
            alignment: 4,
            usage: GpuBufferUsage::TransferSrc as u32 | GpuBufferUsage::CpuWrite as u32,
            memory_type: GpuMemoryType::HostVisible,
            label: None,
        }
    }

    /// Create a framebuffer descriptor.
    pub fn framebuffer(width: u32, height: u32) -> Self {
        let size = (width as u64) * (height as u64) * 4; // RGBA8
        GpuBufferDesc {
            size,
            alignment: 4096, // Page aligned
            usage: GpuBufferUsage::RenderTarget as u32,
            memory_type: GpuMemoryType::DeviceLocal,
            label: Some("framebuffer"),
        }
    }
}

/// GPU buffer allocation.
#[derive(Debug)]
pub struct GpuBuffer {
    /// Buffer handle.
    pub handle: GpuBufferHandle,
    /// Owning tab.
    pub owner: TabId,
    /// Buffer descriptor.
    pub desc: GpuBufferDesc,
    /// Shared memory ID (if mapped to CPU).
    pub shm_id: Option<ShmId>,
    /// Physical address (for DMA).
    pub paddr: u64,
    /// Virtual address (if CPU-mapped).
    pub vaddr: Option<u64>,
}

/// GPU fence for synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GpuFence(pub u64);

impl GpuFence {
    /// Invalid fence.
    pub const INVALID: GpuFence = GpuFence(0);
}

/// Fence state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenceState {
    /// Fence not yet signaled.
    Unsignaled,
    /// Fence signaled (GPU work complete).
    Signaled,
}

/// GPU command buffer.
#[derive(Debug)]
pub struct GpuCommandBuffer {
    /// Command buffer handle.
    pub handle: GpuBufferHandle,
    /// Associated tab.
    pub tab: TabId,
    /// Commands (simplified - in reality would be GPU-specific).
    pub commands: Vec<GpuCommand>,
}

/// GPU commands (simplified).
#[derive(Debug, Clone)]
pub enum GpuCommand {
    /// Copy buffer to buffer.
    CopyBuffer {
        src: GpuBufferHandle,
        dst: GpuBufferHandle,
        size: u64,
    },
    /// Clear render target.
    Clear {
        target: GpuBufferHandle,
        color: [f32; 4],
    },
    /// Draw primitives.
    Draw {
        vertex_buffer: GpuBufferHandle,
        vertex_count: u32,
    },
    /// Present to screen.
    Present { target: GpuBufferHandle },
}

/// GPU memory manager.
pub struct GpuMemoryManager {
    /// All allocated buffers.
    buffers: BTreeMap<GpuBufferHandle, Arc<Mutex<GpuBuffer>>>,

    /// Per-tab allocations.
    tab_allocations: BTreeMap<TabId, Vec<GpuBufferHandle>>,

    /// Fences.
    fences: BTreeMap<GpuFence, FenceState>,

    /// Next handle.
    next_handle: AtomicU64,

    /// Next fence ID.
    next_fence: AtomicU64,

    /// Total allocated memory.
    total_allocated: AtomicU64,

    /// Memory limit.
    memory_limit: u64,
}

impl GpuMemoryManager {
    /// Create a new manager.
    pub fn new(memory_limit: u64) -> Self {
        GpuMemoryManager {
            buffers: BTreeMap::new(),
            tab_allocations: BTreeMap::new(),
            fences: BTreeMap::new(),
            next_handle: AtomicU64::new(1),
            next_fence: AtomicU64::new(1),
            total_allocated: AtomicU64::new(0),
            memory_limit,
        }
    }

    /// Allocate a GPU buffer.
    pub fn alloc(&mut self, tab: TabId, desc: GpuBufferDesc) -> Result<GpuBufferHandle, GpuError> {
        // Check memory limit
        let current = self.total_allocated.load(Ordering::Relaxed);
        if current + desc.size > self.memory_limit {
            return Err(GpuError::OutOfMemory);
        }

        // Generate handle
        let handle = GpuBufferHandle(self.next_handle.fetch_add(1, Ordering::Relaxed));

        // Create buffer
        let buffer = GpuBuffer {
            handle,
            owner: tab,
            desc: desc.clone(),
            shm_id: None,
            paddr: 0, // TODO: Actual allocation
            vaddr: None,
        };

        // Track allocation
        self.total_allocated.fetch_add(desc.size, Ordering::Relaxed);
        self.buffers.insert(handle, Arc::new(Mutex::new(buffer)));

        self.tab_allocations
            .entry(tab)
            .or_insert_with(Vec::new)
            .push(handle);

        crate::serial_println!(
            "[GPU] Allocated buffer {:?} for tab {}: {} bytes",
            handle,
            tab.0,
            desc.size
        );

        Ok(handle)
    }

    /// Free a GPU buffer.
    pub fn free(&mut self, handle: GpuBufferHandle) -> Result<(), GpuError> {
        let buffer = self
            .buffers
            .remove(&handle)
            .ok_or(GpuError::InvalidHandle)?;
        let buffer = buffer.lock();

        // Update tracking
        self.total_allocated
            .fetch_sub(buffer.desc.size, Ordering::Relaxed);

        if let Some(handles) = self.tab_allocations.get_mut(&buffer.owner) {
            handles.retain(|h| *h != handle);
        }

        crate::serial_println!("[GPU] Freed buffer {:?}", handle);

        Ok(())
    }

    /// Get buffer info.
    pub fn get_buffer(&self, handle: GpuBufferHandle) -> Option<Arc<Mutex<GpuBuffer>>> {
        self.buffers.get(&handle).cloned()
    }

    /// Free all buffers for a tab.
    pub fn free_tab_buffers(&mut self, tab: TabId) {
        if let Some(handles) = self.tab_allocations.remove(&tab) {
            for handle in handles {
                if let Some(buffer) = self.buffers.remove(&handle) {
                    let size = buffer.lock().desc.size;
                    self.total_allocated.fetch_sub(size, Ordering::Relaxed);
                }
            }
        }
    }

    /// Create a fence.
    pub fn create_fence(&mut self) -> GpuFence {
        let fence = GpuFence(self.next_fence.fetch_add(1, Ordering::Relaxed));
        self.fences.insert(fence, FenceState::Unsignaled);
        fence
    }

    /// Signal a fence.
    pub fn signal_fence(&mut self, fence: GpuFence) {
        if let Some(state) = self.fences.get_mut(&fence) {
            *state = FenceState::Signaled;
        }
    }

    /// Check fence state.
    pub fn fence_state(&self, fence: GpuFence) -> FenceState {
        self.fences
            .get(&fence)
            .copied()
            .unwrap_or(FenceState::Signaled)
    }

    /// Get total allocated memory.
    pub fn allocated(&self) -> u64 {
        self.total_allocated.load(Ordering::Relaxed)
    }

    /// Get tab's allocated memory.
    pub fn tab_allocated(&self, tab: TabId) -> u64 {
        self.tab_allocations
            .get(&tab)
            .map(|handles| {
                handles
                    .iter()
                    .filter_map(|h| self.buffers.get(h))
                    .map(|b| b.lock().desc.size)
                    .sum()
            })
            .unwrap_or(0)
    }
}

/// GPU errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuError {
    /// Out of GPU memory.
    OutOfMemory,
    /// Invalid buffer handle.
    InvalidHandle,
    /// Invalid fence.
    InvalidFence,
    /// Access denied.
    AccessDenied,
    /// Device error.
    DeviceError,
}

/// Global GPU memory manager.
static GPU_MANAGER: RwLock<Option<GpuMemoryManager>> = RwLock::new(None);

/// Initialize GPU memory manager.
pub fn init(memory_limit: u64) {
    let mut mgr = GPU_MANAGER.write();
    *mgr = Some(GpuMemoryManager::new(memory_limit));
    crate::serial_println!(
        "[GPU] Memory manager initialized: {} MB limit",
        memory_limit / 1024 / 1024
    );
}

/// Allocate GPU buffer.
pub fn alloc(tab: TabId, desc: GpuBufferDesc) -> Result<GpuBufferHandle, GpuError> {
    GPU_MANAGER
        .write()
        .as_mut()
        .ok_or(GpuError::DeviceError)?
        .alloc(tab, desc)
}

/// Free GPU buffer.
pub fn free(handle: GpuBufferHandle) -> Result<(), GpuError> {
    GPU_MANAGER
        .write()
        .as_mut()
        .ok_or(GpuError::DeviceError)?
        .free(handle)
}

/// Get buffer info.
pub fn get_buffer(handle: GpuBufferHandle) -> Option<Arc<Mutex<GpuBuffer>>> {
    GPU_MANAGER.read().as_ref()?.get_buffer(handle)
}

/// Create fence.
pub fn create_fence() -> Option<GpuFence> {
    Some(GPU_MANAGER.write().as_mut()?.create_fence())
}

/// Signal fence.
pub fn signal_fence(fence: GpuFence) {
    if let Some(mgr) = GPU_MANAGER.write().as_mut() {
        mgr.signal_fence(fence);
    }
}
