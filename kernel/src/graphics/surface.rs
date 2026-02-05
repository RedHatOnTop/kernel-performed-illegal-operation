//! Surface - Renderable graphics layer
//!
//! A Surface represents a rectangular area that can be rendered to.
//! Surfaces can be composited together to form the final display output.

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

/// Unique surface identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub u64);

impl SurfaceId {
    /// Generate a new unique surface ID
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        SurfaceId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Surface creation flags
#[derive(Debug, Clone, Copy)]
pub struct SurfaceFlags {
    /// Surface supports alpha blending
    pub alpha: bool,
    /// Surface uses premultiplied alpha
    pub premultiplied: bool,
    /// Surface should be cached (for static content)
    pub cacheable: bool,
    /// Surface is double-buffered
    pub double_buffered: bool,
}

impl Default for SurfaceFlags {
    fn default() -> Self {
        Self {
            alpha: true,
            premultiplied: false,
            cacheable: false,
            double_buffered: true,
        }
    }
}

/// Pixel format for surfaces
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 32-bit BGRA (Blue, Green, Red, Alpha)
    BGRA8888,
    /// 32-bit RGBA (Red, Green, Blue, Alpha)
    RGBA8888,
    /// 24-bit BGR (Blue, Green, Red)
    BGR888,
    /// 24-bit RGB (Red, Green, Blue)
    RGB888,
    /// 16-bit RGB565
    RGB565,
    /// 8-bit grayscale
    Gray8,
    /// 8-bit with alpha
    GrayA8,
}

impl PixelFormat {
    /// Bytes per pixel for this format
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            PixelFormat::BGRA8888 | PixelFormat::RGBA8888 => 4,
            PixelFormat::BGR888 | PixelFormat::RGB888 => 3,
            PixelFormat::RGB565 | PixelFormat::GrayA8 => 2,
            PixelFormat::Gray8 => 1,
        }
    }

    /// Whether this format has alpha channel
    pub fn has_alpha(&self) -> bool {
        matches!(
            self,
            PixelFormat::BGRA8888 | PixelFormat::RGBA8888 | PixelFormat::GrayA8
        )
    }
}

/// A renderable graphics surface
#[derive(Debug)]
pub struct Surface {
    /// Unique identifier
    pub id: SurfaceId,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Pixel format
    pub format: PixelFormat,
    /// Pixel data buffer
    pub data: Vec<u8>,
    /// Stride (bytes per row)
    pub stride: usize,
    /// Position X (for compositing)
    pub x: i32,
    /// Position Y (for compositing)
    pub y: i32,
    /// Z-order (higher = on top)
    pub z_order: i32,
    /// Surface flags
    pub flags: SurfaceFlags,
    /// Opacity (0-255)
    pub opacity: u8,
    /// Whether surface has been modified
    pub dirty: bool,
    /// Generation counter (incremented on each modification)
    pub generation: u64,
}

impl Surface {
    /// Create a new surface
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        let bpp = format.bytes_per_pixel();
        let stride = width as usize * bpp;
        let data = alloc::vec![0u8; stride * height as usize];

        Self {
            id: SurfaceId::new(),
            width,
            height,
            format,
            data,
            stride,
            x: 0,
            y: 0,
            z_order: 0,
            flags: SurfaceFlags::default(),
            opacity: 255,
            dirty: true,
            generation: 0,
        }
    }

    /// Create a surface with specific flags
    pub fn with_flags(width: u32, height: u32, format: PixelFormat, flags: SurfaceFlags) -> Self {
        let mut surface = Self::new(width, height, format);
        surface.flags = flags;
        surface
    }

    /// Get pixel at coordinates (unchecked for performance)
    #[inline]
    pub unsafe fn get_pixel_unchecked(&self, x: u32, y: u32) -> &[u8] {
        let bpp = self.format.bytes_per_pixel();
        let offset = y as usize * self.stride + x as usize * bpp;
        &self.data[offset..offset + bpp]
    }

    /// Set pixel at coordinates (unchecked for performance)
    #[inline]
    pub unsafe fn set_pixel_unchecked(&mut self, x: u32, y: u32, pixel: &[u8]) {
        let bpp = self.format.bytes_per_pixel();
        let offset = y as usize * self.stride + x as usize * bpp;
        self.data[offset..offset + bpp].copy_from_slice(pixel);
    }

    /// Get pixel at coordinates (checked)
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<&[u8]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let bpp = self.format.bytes_per_pixel();
        let offset = y as usize * self.stride + x as usize * bpp;
        Some(&self.data[offset..offset + bpp])
    }

    /// Set pixel at coordinates (checked)
    pub fn set_pixel(&mut self, x: u32, y: u32, pixel: &[u8]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let bpp = self.format.bytes_per_pixel();
        let offset = y as usize * self.stride + x as usize * bpp;
        let len = bpp.min(pixel.len());
        self.data[offset..offset + len].copy_from_slice(&pixel[..len]);
        self.dirty = true;
    }

    /// Fill entire surface with a color
    pub fn fill(&mut self, color: &[u8]) {
        let bpp = self.format.bytes_per_pixel();
        for y in 0..self.height {
            for x in 0..self.width {
                let offset = y as usize * self.stride + x as usize * bpp;
                let len = bpp.min(color.len());
                self.data[offset..offset + len].copy_from_slice(&color[..len]);
            }
        }
        self.dirty = true;
        self.generation += 1;
    }

    /// Fill a rectangle with a color
    pub fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: &[u8]) {
        let bpp = self.format.bytes_per_pixel();
        let x_start = x.max(0) as u32;
        let y_start = y.max(0) as u32;
        let x_end = ((x + w as i32) as u32).min(self.width);
        let y_end = ((y + h as i32) as u32).min(self.height);

        for py in y_start..y_end {
            for px in x_start..x_end {
                let offset = py as usize * self.stride + px as usize * bpp;
                let len = bpp.min(color.len());
                self.data[offset..offset + len].copy_from_slice(&color[..len]);
            }
        }
        self.dirty = true;
        self.generation += 1;
    }

    /// Clear surface to transparent (if alpha) or black
    pub fn clear(&mut self) {
        self.data.fill(0);
        self.dirty = true;
        self.generation += 1;
    }

    /// Get raw data pointer for direct manipulation
    #[inline]
    pub fn data_ptr(&mut self) -> *mut u8 {
        self.data.as_mut_ptr()
    }

    /// Get raw data slice
    #[inline]
    pub fn data_slice(&self) -> &[u8] {
        &self.data
    }

    /// Get mutable data slice
    #[inline]
    pub fn data_slice_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Mark surface as clean (after composition)
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Mark surface as dirty (needs recomposition)
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Get bounds as a rectangle
    pub fn bounds(&self) -> (i32, i32, u32, u32) {
        (self.x, self.y, self.width, self.height)
    }

    /// Check if a point is inside the surface
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x
            && px < self.x + self.width as i32
            && py >= self.y
            && py < self.y + self.height as i32
    }

    /// Get row data for a specific scanline
    #[inline]
    pub fn row(&self, y: u32) -> &[u8] {
        let start = y as usize * self.stride;
        let end = start + self.width as usize * self.format.bytes_per_pixel();
        &self.data[start..end]
    }

    /// Get mutable row data
    #[inline]
    pub fn row_mut(&mut self, y: u32) -> &mut [u8] {
        let start = y as usize * self.stride;
        let end = start + self.width as usize * self.format.bytes_per_pixel();
        &mut self.data[start..end]
    }
}

/// Surface transform operations
#[derive(Debug, Clone, Copy, Default)]
pub struct Transform {
    /// X translation
    pub translate_x: f32,
    /// Y translation
    pub translate_y: f32,
    /// Rotation in radians
    pub rotation: f32,
    /// X scale factor
    pub scale_x: f32,
    /// Y scale factor
    pub scale_y: f32,
}

impl Transform {
    /// Identity transform (no change)
    pub const IDENTITY: Transform = Transform {
        translate_x: 0.0,
        translate_y: 0.0,
        rotation: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
    };

    /// Create a translation transform
    pub fn translate(x: f32, y: f32) -> Self {
        Self {
            translate_x: x,
            translate_y: y,
            ..Self::IDENTITY
        }
    }

    /// Create a scale transform
    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            scale_x: sx,
            scale_y: sy,
            ..Self::IDENTITY
        }
    }

    /// Create a rotation transform
    pub fn rotate(radians: f32) -> Self {
        Self {
            rotation: radians,
            ..Self::IDENTITY
        }
    }

    /// Check if this is an identity transform
    pub fn is_identity(&self) -> bool {
        self.translate_x == 0.0
            && self.translate_y == 0.0
            && self.rotation == 0.0
            && self.scale_x == 1.0
            && self.scale_y == 1.0
    }
}
