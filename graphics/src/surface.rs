//! Surface management for applications.
//!
//! This module provides the surface abstraction that applications
//! use to render their content.

use alloc::vec::Vec;

use crate::{GraphicsError, PixelFormat};

/// Unique surface identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SurfaceId(pub u64);

/// A rendering surface.
pub struct Surface {
    /// Surface ID.
    id: SurfaceId,
    /// Width in pixels.
    width: u32,
    /// Height in pixels.
    height: u32,
    /// Pixel format.
    format: PixelFormat,
    /// Associated buffers.
    buffers: Vec<SurfaceBuffer>,
    /// Current buffer index.
    current_buffer: usize,
}

impl Surface {
    /// Create a new surface.
    pub fn new(id: SurfaceId, width: u32, height: u32, format: PixelFormat) -> Result<Self, GraphicsError> {
        let mut surface = Surface {
            id,
            width,
            height,
            format,
            buffers: Vec::new(),
            current_buffer: 0,
        };
        
        // Create double buffering
        surface.allocate_buffers(2)?;
        
        Ok(surface)
    }
    
    /// Allocate buffers for the surface.
    fn allocate_buffers(&mut self, count: usize) -> Result<(), GraphicsError> {
        self.buffers.clear();
        
        for i in 0..count {
            let buffer = SurfaceBuffer::new(
                self.width,
                self.height,
                self.format,
            )?;
            self.buffers.push(buffer);
        }
        
        Ok(())
    }
    
    /// Get the surface ID.
    pub fn id(&self) -> SurfaceId {
        self.id
    }
    
    /// Get the width.
    pub fn width(&self) -> u32 {
        self.width
    }
    
    /// Get the height.
    pub fn height(&self) -> u32 {
        self.height
    }
    
    /// Get the pixel format.
    pub fn format(&self) -> PixelFormat {
        self.format
    }
    
    /// Get the current buffer for rendering.
    pub fn current_buffer(&mut self) -> &mut SurfaceBuffer {
        &mut self.buffers[self.current_buffer]
    }
    
    /// Swap buffers (for double buffering).
    pub fn swap_buffers(&mut self) {
        self.current_buffer = (self.current_buffer + 1) % self.buffers.len();
    }
    
    /// Resize the surface.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), GraphicsError> {
        self.width = width;
        self.height = height;
        self.allocate_buffers(self.buffers.len())?;
        Ok(())
    }
}

/// A buffer attached to a surface.
pub struct SurfaceBuffer {
    /// Buffer handle.
    handle: u64,
    /// Width in pixels.
    width: u32,
    /// Height in pixels.
    height: u32,
    /// Pixel format.
    format: PixelFormat,
    /// Row stride in bytes.
    stride: u32,
    /// Buffer state.
    state: BufferState,
}

impl SurfaceBuffer {
    /// Create a new surface buffer.
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Result<Self, GraphicsError> {
        let stride = width * format.bytes_per_pixel();
        
        Ok(SurfaceBuffer {
            handle: 0, // Would be allocated from GPU
            width,
            height,
            format,
            stride,
            state: BufferState::Ready,
        })
    }
    
    /// Get the buffer handle.
    pub fn handle(&self) -> u64 {
        self.handle
    }
    
    /// Get the width.
    pub fn width(&self) -> u32 {
        self.width
    }
    
    /// Get the height.
    pub fn height(&self) -> u32 {
        self.height
    }
    
    /// Get the stride.
    pub fn stride(&self) -> u32 {
        self.stride
    }
    
    /// Get the buffer size in bytes.
    pub fn size(&self) -> usize {
        (self.stride * self.height) as usize
    }
    
    /// Get the buffer state.
    pub fn state(&self) -> BufferState {
        self.state
    }
    
    /// Set the buffer state.
    pub fn set_state(&mut self, state: BufferState) {
        self.state = state;
    }
}

/// Buffer state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferState {
    /// Buffer is ready for rendering.
    Ready,
    /// Buffer is being rendered to.
    Rendering,
    /// Buffer is pending presentation.
    Pending,
    /// Buffer is being displayed.
    Displayed,
}
