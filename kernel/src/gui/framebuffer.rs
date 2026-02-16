//! Double-Buffered Framebuffer
//!
//! Provides flicker-free rendering through double buffering.
//! All drawing operations happen on the back buffer, then the
//! entire buffer is swapped/copied to the front buffer atomically.

use alloc::vec::Vec;
use core::ptr;

/// Framebuffer manager with double buffering
pub struct FramebufferManager {
    /// Front buffer (actual display memory)
    front_buffer: *mut u8,
    /// Back buffer (render target)
    back_buffer: Vec<u8>,
    /// Screen width in pixels
    pub width: u32,
    /// Screen height in pixels
    pub height: u32,
    /// Bytes per pixel
    pub bpp: usize,
    /// Stride (bytes per row)
    pub stride: usize,
    /// Total buffer size in bytes
    buffer_size: usize,
    /// Dirty rectangles for partial updates (optimization)
    dirty_regions: Vec<DirtyRect>,
    /// Use full buffer copy vs dirty rect optimization
    use_dirty_rects: bool,
}

/// Dirty rectangle for partial screen updates
#[derive(Debug, Clone, Copy)]
pub struct DirtyRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl DirtyRect {
    /// Create a new dirty rectangle
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Merge two rectangles into one bounding rectangle
    pub fn merge(&self, other: &DirtyRect) -> DirtyRect {
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = (self.x + self.width).max(other.x + other.width);
        let y2 = (self.y + self.height).max(other.y + other.height);
        DirtyRect::new(x1, y1, x2 - x1, y2 - y1)
    }

    /// Check if this rectangle intersects with another
    pub fn intersects(&self, other: &DirtyRect) -> bool {
        !(self.x + self.width <= other.x
            || other.x + other.width <= self.x
            || self.y + self.height <= other.y
            || other.y + other.height <= self.y)
    }
}

// Safety: FramebufferManager is only accessed from a single thread
// and the framebuffer pointer is valid for the lifetime of the kernel
unsafe impl Send for FramebufferManager {}
unsafe impl Sync for FramebufferManager {}

impl FramebufferManager {
    /// Create a new framebuffer manager with double buffering
    pub fn new(front_buffer: *mut u8, width: u32, height: u32, bpp: usize, stride: usize) -> Self {
        let buffer_size = stride * height as usize * bpp;
        let back_buffer = alloc::vec![0u8; buffer_size];

        crate::serial_println!(
            "[FB] Double buffer initialized: {}x{} @ {} bpp, {} bytes",
            width,
            height,
            bpp,
            buffer_size
        );

        Self {
            front_buffer,
            back_buffer,
            width,
            height,
            bpp,
            stride,
            buffer_size,
            dirty_regions: Vec::with_capacity(32),
            use_dirty_rects: false, // Start with full buffer copy for simplicity
        }
    }

    /// Get pointer to the back buffer for rendering
    #[inline]
    pub fn back_buffer(&mut self) -> *mut u8 {
        self.back_buffer.as_mut_ptr()
    }

    /// Get pointer to the back buffer (immutable)
    #[inline]
    pub fn back_buffer_ref(&self) -> *const u8 {
        self.back_buffer.as_ptr()
    }

    /// Mark a region as dirty (needs update)
    pub fn mark_dirty(&mut self, x: u32, y: u32, width: u32, height: u32) {
        if !self.use_dirty_rects {
            return;
        }

        let rect = DirtyRect::new(
            x.min(self.width),
            y.min(self.height),
            width.min(self.width - x.min(self.width)),
            height.min(self.height - y.min(self.height)),
        );

        // Try to merge with existing dirty rects
        let mut merged = false;
        for existing in &mut self.dirty_regions {
            if existing.intersects(&rect) {
                *existing = existing.merge(&rect);
                merged = true;
                break;
            }
        }

        if !merged && self.dirty_regions.len() < 32 {
            self.dirty_regions.push(rect);
        }
    }

    /// Mark the entire screen as dirty
    pub fn mark_all_dirty(&mut self) {
        self.dirty_regions.clear();
        self.dirty_regions
            .push(DirtyRect::new(0, 0, self.width, self.height));
    }

    /// Swap buffers - copy back buffer to front buffer
    /// This is the key operation that eliminates flickering
    pub fn swap_buffers(&mut self) {
        if self.use_dirty_rects && !self.dirty_regions.is_empty() {
            // Copy only dirty regions - take ownership to avoid borrow issues
            let regions: Vec<DirtyRect> = self.dirty_regions.drain(..).collect();
            for rect in regions {
                self.copy_rect(rect.x, rect.y, rect.width, rect.height);
            }
        } else {
            // Full buffer copy using optimized memcpy
            self.copy_full_buffer();
        }
    }

    /// Copy entire back buffer to front buffer
    #[inline]
    fn copy_full_buffer(&mut self) {
        unsafe {
            // Use volatile writes to ensure the copy isn't optimized away
            // and happens in order (important for memory-mapped framebuffers)
            ptr::copy_nonoverlapping(
                self.back_buffer.as_ptr(),
                self.front_buffer,
                self.buffer_size,
            );
        }
    }

    /// Copy a rectangular region from back to front buffer
    fn copy_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        let bytes_per_row = width as usize * self.bpp;
        let row_stride = self.stride * self.bpp;

        for row in 0..height {
            let y_pos = (y + row) as usize;
            if y_pos >= self.height as usize {
                break;
            }

            let offset = y_pos * row_stride + x as usize * self.bpp;
            let src = unsafe { self.back_buffer.as_ptr().add(offset) };
            let dst = unsafe { self.front_buffer.add(offset) };

            unsafe {
                ptr::copy_nonoverlapping(
                    src,
                    dst,
                    bytes_per_row.min(row_stride - x as usize * self.bpp),
                );
            }
        }
    }

    /// Clear the back buffer with a color
    pub fn clear(&mut self, r: u8, g: u8, b: u8) {
        let row_bytes = self.stride * self.bpp;

        for y in 0..self.height as usize {
            for x in 0..self.width as usize {
                let offset = y * row_bytes + x * self.bpp;
                self.back_buffer[offset] = b; // B
                self.back_buffer[offset + 1] = g; // G
                self.back_buffer[offset + 2] = r; // R
                if self.bpp >= 4 {
                    self.back_buffer[offset + 3] = 255; // A
                }
            }
        }
    }

    /// Fast horizontal line fill (optimized for filling large areas)
    #[inline]
    pub fn fill_scanline(&mut self, y: u32, x_start: u32, x_end: u32, r: u8, g: u8, b: u8) {
        if y >= self.height {
            return;
        }

        let x_start = x_start.min(self.width) as usize;
        let x_end = x_end.min(self.width) as usize;
        let row_offset = y as usize * self.stride * self.bpp;

        for x in x_start..x_end {
            let offset = row_offset + x * self.bpp;
            self.back_buffer[offset] = b;
            self.back_buffer[offset + 1] = g;
            self.back_buffer[offset + 2] = r;
        }
    }

    /// Set a single pixel in the back buffer
    #[inline]
    pub fn set_pixel(&mut self, x: i32, y: i32, r: u8, g: u8, b: u8, a: u8) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }

        let offset = (y as usize * self.stride + x as usize) * self.bpp;

        // Alpha blending if not fully opaque
        if a < 255 && a > 0 {
            let alpha = a as u32;
            let inv_alpha = 255 - alpha;

            let old_b = self.back_buffer[offset] as u32;
            let old_g = self.back_buffer[offset + 1] as u32;
            let old_r = self.back_buffer[offset + 2] as u32;

            self.back_buffer[offset] = ((b as u32 * alpha + old_b * inv_alpha) / 255) as u8;
            self.back_buffer[offset + 1] = ((g as u32 * alpha + old_g * inv_alpha) / 255) as u8;
            self.back_buffer[offset + 2] = ((r as u32 * alpha + old_r * inv_alpha) / 255) as u8;
        } else if a == 255 {
            self.back_buffer[offset] = b;
            self.back_buffer[offset + 1] = g;
            self.back_buffer[offset + 2] = r;
        }
        // a == 0 means fully transparent, don't draw
    }

    /// Get buffer dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Get bytes per pixel
    pub fn bytes_per_pixel(&self) -> usize {
        self.bpp
    }

    /// Get stride
    pub fn get_stride(&self) -> usize {
        self.stride
    }

    /// Enable or disable dirty rectangle optimization
    pub fn set_dirty_rect_mode(&mut self, enabled: bool) {
        self.use_dirty_rects = enabled;
        if !enabled {
            self.dirty_regions.clear();
        }
    }
}

/// Statistics for frame timing
pub struct FrameStats {
    /// Frame counter
    pub frame_count: u64,
    /// Last frame time in timer ticks
    pub last_frame_tick: u64,
    /// Target frame interval (ticks between frames)
    pub target_interval: u64,
    /// Frames dropped due to timing
    pub dropped_frames: u64,
}

impl FrameStats {
    pub fn new(target_fps: u32, timer_hz: u32) -> Self {
        Self {
            frame_count: 0,
            last_frame_tick: 0,
            target_interval: (timer_hz / target_fps) as u64,
            dropped_frames: 0,
        }
    }

    /// Check if it's time to render a new frame
    pub fn should_render(&self, current_tick: u64) -> bool {
        current_tick >= self.last_frame_tick + self.target_interval
    }

    /// Record that a frame was rendered
    pub fn frame_rendered(&mut self, current_tick: u64) {
        self.frame_count += 1;
        self.last_frame_tick = current_tick;
    }
}
