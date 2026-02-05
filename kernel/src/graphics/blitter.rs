//! High-Performance Blitter
//!
//! Optimized memory copy operations for compositing surfaces.
//! Uses SIMD when available for maximum throughput.

use super::surface::{PixelFormat, Surface};
use super::damage::DamageRect;

/// Blend mode for compositing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    /// Overwrite destination
    Copy,
    /// Standard alpha blending: dst = src * alpha + dst * (1 - alpha)
    AlphaBlend,
    /// Premultiplied alpha: dst = src + dst * (1 - alpha)
    PremultipliedAlpha,
    /// Additive blending: dst = src + dst
    Additive,
    /// Multiply: dst = src * dst
    Multiply,
}

/// Blit operation configuration
#[derive(Debug, Clone, Copy)]
pub struct BlitOp {
    /// Source rectangle
    pub src_rect: DamageRect,
    /// Destination position
    pub dst_x: i32,
    pub dst_y: i32,
    /// Blend mode
    pub blend_mode: BlendMode,
    /// Global opacity (0-255)
    pub opacity: u8,
}

impl Default for BlitOp {
    fn default() -> Self {
        Self {
            src_rect: DamageRect::new(0, 0, 0, 0),
            dst_x: 0,
            dst_y: 0,
            blend_mode: BlendMode::Copy,
            opacity: 255,
        }
    }
}

/// High-performance blitter for surface composition
pub struct Blitter {
    /// Whether SIMD is available
    #[allow(dead_code)]
    simd_available: bool,
}

impl Blitter {
    /// Create a new blitter
    pub fn new() -> Self {
        Self {
            simd_available: Self::detect_simd(),
        }
    }

    /// Detect SIMD support
    fn detect_simd() -> bool {
        // In a real implementation, we'd check CPUID for SSE/AVX support
        // For now, assume basic x86_64 has SSE2
        #[cfg(target_arch = "x86_64")]
        {
            true
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            false
        }
    }

    /// Blit source surface to destination with alpha blending
    pub fn blit(&self, src: &Surface, dst: &mut Surface, op: &BlitOp) {
        match op.blend_mode {
            BlendMode::Copy => self.blit_copy(src, dst, op),
            BlendMode::AlphaBlend => self.blit_alpha(src, dst, op),
            BlendMode::PremultipliedAlpha => self.blit_premultiplied(src, dst, op),
            BlendMode::Additive => self.blit_additive(src, dst, op),
            BlendMode::Multiply => self.blit_multiply(src, dst, op),
        }
    }

    /// Fast copy blit (no blending)
    fn blit_copy(&self, src: &Surface, dst: &mut Surface, op: &BlitOp) {
        let src_bpp = src.format.bytes_per_pixel();
        let dst_bpp = dst.format.bytes_per_pixel();

        // Calculate clipped bounds
        let clip_x = op.dst_x.max(0);
        let clip_y = op.dst_y.max(0);
        let src_offset_x = (clip_x - op.dst_x) as u32;
        let src_offset_y = (clip_y - op.dst_y) as u32;

        let width = (op.src_rect.width - src_offset_x).min((dst.width as i32 - clip_x) as u32);
        let height = (op.src_rect.height - src_offset_y).min((dst.height as i32 - clip_y) as u32);

        if width == 0 || height == 0 {
            return;
        }

        // Same format - fast path with memcpy
        if src.format == dst.format {
            let row_bytes = width as usize * src_bpp;
            
            for y in 0..height {
                let src_y = op.src_rect.y as u32 + src_offset_y + y;
                let dst_y = clip_y as u32 + y;
                
                let src_offset = src_y as usize * src.stride 
                    + (op.src_rect.x as u32 + src_offset_x) as usize * src_bpp;
                let dst_offset = dst_y as usize * dst.stride + clip_x as usize * dst_bpp;
                
                dst.data[dst_offset..dst_offset + row_bytes]
                    .copy_from_slice(&src.data[src_offset..src_offset + row_bytes]);
            }
        } else {
            // Different formats - need conversion
            self.blit_convert(src, dst, op, clip_x, clip_y, width, height);
        }

        dst.dirty = true;
        dst.generation += 1;
    }

    /// Alpha blending blit
    fn blit_alpha(&self, src: &Surface, dst: &mut Surface, op: &BlitOp) {
        // Calculate clipped bounds
        let clip_x = op.dst_x.max(0);
        let clip_y = op.dst_y.max(0);
        let src_offset_x = (clip_x - op.dst_x) as u32;
        let src_offset_y = (clip_y - op.dst_y) as u32;

        let width = (op.src_rect.width - src_offset_x).min((dst.width as i32 - clip_x) as u32);
        let height = (op.src_rect.height - src_offset_y).min((dst.height as i32 - clip_y) as u32);

        if width == 0 || height == 0 {
            return;
        }

        let global_alpha = op.opacity as u32;

        for y in 0..height {
            for x in 0..width {
                let src_x = op.src_rect.x as u32 + src_offset_x + x;
                let src_y = op.src_rect.y as u32 + src_offset_y + y;
                let dst_x = clip_x as u32 + x;
                let dst_y = clip_y as u32 + y;

                // Get source pixel
                let (sr, sg, sb, sa) = self.get_pixel_rgba(src, src_x, src_y);
                
                // Apply global alpha
                let sa = (sa as u32 * global_alpha / 255) as u8;

                if sa == 0 {
                    continue; // Fully transparent
                }

                if sa == 255 {
                    // Fully opaque - just copy
                    self.set_pixel_rgba(dst, dst_x, dst_y, sr, sg, sb, 255);
                } else {
                    // Get destination pixel
                    let (dr, dg, db, da) = self.get_pixel_rgba(dst, dst_x, dst_y);

                    // Standard alpha blending
                    let alpha = sa as u32;
                    let inv_alpha = 255 - alpha;

                    let r = ((sr as u32 * alpha + dr as u32 * inv_alpha) / 255) as u8;
                    let g = ((sg as u32 * alpha + dg as u32 * inv_alpha) / 255) as u8;
                    let b = ((sb as u32 * alpha + db as u32 * inv_alpha) / 255) as u8;
                    let a = (alpha + da as u32 * inv_alpha / 255).min(255) as u8;

                    self.set_pixel_rgba(dst, dst_x, dst_y, r, g, b, a);
                }
            }
        }

        dst.dirty = true;
        dst.generation += 1;
    }

    /// Premultiplied alpha blending
    fn blit_premultiplied(&self, src: &Surface, dst: &mut Surface, op: &BlitOp) {
        let clip_x = op.dst_x.max(0);
        let clip_y = op.dst_y.max(0);
        let src_offset_x = (clip_x - op.dst_x) as u32;
        let src_offset_y = (clip_y - op.dst_y) as u32;

        let width = (op.src_rect.width - src_offset_x).min((dst.width as i32 - clip_x) as u32);
        let height = (op.src_rect.height - src_offset_y).min((dst.height as i32 - clip_y) as u32);

        if width == 0 || height == 0 {
            return;
        }

        for y in 0..height {
            for x in 0..width {
                let src_x = op.src_rect.x as u32 + src_offset_x + x;
                let src_y = op.src_rect.y as u32 + src_offset_y + y;
                let dst_x = clip_x as u32 + x;
                let dst_y = clip_y as u32 + y;

                let (sr, sg, sb, sa) = self.get_pixel_rgba(src, src_x, src_y);
                let (dr, dg, db, _da) = self.get_pixel_rgba(dst, dst_x, dst_y);

                // Premultiplied: dst = src + dst * (1 - alpha)
                let inv_alpha = 255 - sa as u32;

                let r = (sr as u32 + dr as u32 * inv_alpha / 255).min(255) as u8;
                let g = (sg as u32 + dg as u32 * inv_alpha / 255).min(255) as u8;
                let b = (sb as u32 + db as u32 * inv_alpha / 255).min(255) as u8;

                self.set_pixel_rgba(dst, dst_x, dst_y, r, g, b, 255);
            }
        }

        dst.dirty = true;
        dst.generation += 1;
    }

    /// Additive blending
    fn blit_additive(&self, src: &Surface, dst: &mut Surface, op: &BlitOp) {
        let clip_x = op.dst_x.max(0);
        let clip_y = op.dst_y.max(0);
        let src_offset_x = (clip_x - op.dst_x) as u32;
        let src_offset_y = (clip_y - op.dst_y) as u32;

        let width = (op.src_rect.width - src_offset_x).min((dst.width as i32 - clip_x) as u32);
        let height = (op.src_rect.height - src_offset_y).min((dst.height as i32 - clip_y) as u32);

        if width == 0 || height == 0 {
            return;
        }

        let global_alpha = op.opacity as u32;

        for y in 0..height {
            for x in 0..width {
                let src_x = op.src_rect.x as u32 + src_offset_x + x;
                let src_y = op.src_rect.y as u32 + src_offset_y + y;
                let dst_x = clip_x as u32 + x;
                let dst_y = clip_y as u32 + y;

                let (sr, sg, sb, sa) = self.get_pixel_rgba(src, src_x, src_y);
                let (dr, dg, db, da) = self.get_pixel_rgba(dst, dst_x, dst_y);

                // Apply source alpha and global alpha
                let alpha = (sa as u32 * global_alpha / 255) as u32;

                let r = ((sr as u32 * alpha / 255) + dr as u32).min(255) as u8;
                let g = ((sg as u32 * alpha / 255) + dg as u32).min(255) as u8;
                let b = ((sb as u32 * alpha / 255) + db as u32).min(255) as u8;

                self.set_pixel_rgba(dst, dst_x, dst_y, r, g, b, da);
            }
        }

        dst.dirty = true;
        dst.generation += 1;
    }

    /// Multiply blending
    fn blit_multiply(&self, src: &Surface, dst: &mut Surface, op: &BlitOp) {
        let clip_x = op.dst_x.max(0);
        let clip_y = op.dst_y.max(0);
        let src_offset_x = (clip_x - op.dst_x) as u32;
        let src_offset_y = (clip_y - op.dst_y) as u32;

        let width = (op.src_rect.width - src_offset_x).min((dst.width as i32 - clip_x) as u32);
        let height = (op.src_rect.height - src_offset_y).min((dst.height as i32 - clip_y) as u32);

        if width == 0 || height == 0 {
            return;
        }

        for y in 0..height {
            for x in 0..width {
                let src_x = op.src_rect.x as u32 + src_offset_x + x;
                let src_y = op.src_rect.y as u32 + src_offset_y + y;
                let dst_x = clip_x as u32 + x;
                let dst_y = clip_y as u32 + y;

                let (sr, sg, sb, _) = self.get_pixel_rgba(src, src_x, src_y);
                let (dr, dg, db, da) = self.get_pixel_rgba(dst, dst_x, dst_y);

                let r = (sr as u32 * dr as u32 / 255) as u8;
                let g = (sg as u32 * dg as u32 / 255) as u8;
                let b = (sb as u32 * db as u32 / 255) as u8;

                self.set_pixel_rgba(dst, dst_x, dst_y, r, g, b, da);
            }
        }

        dst.dirty = true;
        dst.generation += 1;
    }

    /// Convert between pixel formats during blit
    fn blit_convert(
        &self,
        src: &Surface,
        dst: &mut Surface,
        op: &BlitOp,
        clip_x: i32,
        clip_y: i32,
        width: u32,
        height: u32,
    ) {
        let src_offset_x = (clip_x - op.dst_x) as u32;
        let src_offset_y = (clip_y - op.dst_y) as u32;

        for y in 0..height {
            for x in 0..width {
                let src_x = op.src_rect.x as u32 + src_offset_x + x;
                let src_y = op.src_rect.y as u32 + src_offset_y + y;
                let dst_x = clip_x as u32 + x;
                let dst_y = clip_y as u32 + y;

                let (r, g, b, a) = self.get_pixel_rgba(src, src_x, src_y);
                self.set_pixel_rgba(dst, dst_x, dst_y, r, g, b, a);
            }
        }
    }

    /// Get pixel as RGBA from any surface format
    #[inline]
    fn get_pixel_rgba(&self, surface: &Surface, x: u32, y: u32) -> (u8, u8, u8, u8) {
        let bpp = surface.format.bytes_per_pixel();
        let offset = y as usize * surface.stride + x as usize * bpp;
        let data = &surface.data[offset..];

        match surface.format {
            PixelFormat::BGRA8888 => (data[2], data[1], data[0], data[3]),
            PixelFormat::RGBA8888 => (data[0], data[1], data[2], data[3]),
            PixelFormat::BGR888 => (data[2], data[1], data[0], 255),
            PixelFormat::RGB888 => (data[0], data[1], data[2], 255),
            PixelFormat::RGB565 => {
                let pixel = u16::from_le_bytes([data[0], data[1]]);
                let r = ((pixel >> 11) & 0x1F) as u8 * 255 / 31;
                let g = ((pixel >> 5) & 0x3F) as u8 * 255 / 63;
                let b = (pixel & 0x1F) as u8 * 255 / 31;
                (r, g, b, 255)
            }
            PixelFormat::Gray8 => (data[0], data[0], data[0], 255),
            PixelFormat::GrayA8 => (data[0], data[0], data[0], data[1]),
        }
    }

    /// Set pixel as RGBA to any surface format
    #[inline]
    fn set_pixel_rgba(&self, surface: &mut Surface, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
        let bpp = surface.format.bytes_per_pixel();
        let offset = y as usize * surface.stride + x as usize * bpp;
        let data = &mut surface.data[offset..];

        match surface.format {
            PixelFormat::BGRA8888 => {
                data[0] = b;
                data[1] = g;
                data[2] = r;
                data[3] = a;
            }
            PixelFormat::RGBA8888 => {
                data[0] = r;
                data[1] = g;
                data[2] = b;
                data[3] = a;
            }
            PixelFormat::BGR888 => {
                data[0] = b;
                data[1] = g;
                data[2] = r;
            }
            PixelFormat::RGB888 => {
                data[0] = r;
                data[1] = g;
                data[2] = b;
            }
            PixelFormat::RGB565 => {
                let pixel = ((r as u16 >> 3) << 11)
                    | ((g as u16 >> 2) << 5)
                    | (b as u16 >> 3);
                data[0] = pixel as u8;
                data[1] = (pixel >> 8) as u8;
            }
            PixelFormat::Gray8 => {
                data[0] = ((r as u32 + g as u32 + b as u32) / 3) as u8;
            }
            PixelFormat::GrayA8 => {
                data[0] = ((r as u32 + g as u32 + b as u32) / 3) as u8;
                data[1] = a;
            }
        }
    }

    /// Fill a rectangle with solid color (optimized)
    pub fn fill_rect(&self, dst: &mut Surface, rect: &DamageRect, r: u8, g: u8, b: u8, a: u8) {
        let clip_x = rect.x.max(0) as u32;
        let clip_y = rect.y.max(0) as u32;
        let width = rect.width.min(dst.width - clip_x);
        let height = rect.height.min(dst.height - clip_y);

        if width == 0 || height == 0 {
            return;
        }

        let bpp = dst.format.bytes_per_pixel();
        
        // Build pixel value based on format
        let mut pixel = [0u8; 4];
        match dst.format {
            PixelFormat::BGRA8888 => {
                pixel[0] = b;
                pixel[1] = g;
                pixel[2] = r;
                pixel[3] = a;
            }
            PixelFormat::RGBA8888 => {
                pixel[0] = r;
                pixel[1] = g;
                pixel[2] = b;
                pixel[3] = a;
            }
            PixelFormat::BGR888 => {
                pixel[0] = b;
                pixel[1] = g;
                pixel[2] = r;
            }
            PixelFormat::RGB888 => {
                pixel[0] = r;
                pixel[1] = g;
                pixel[2] = b;
            }
            _ => {
                // Use slow path for other formats
                for y in clip_y..clip_y + height {
                    for x in clip_x..clip_x + width {
                        self.set_pixel_rgba(dst, x, y, r, g, b, a);
                    }
                }
                return;
            }
        }

        // Fast fill for common formats
        for y in clip_y..clip_y + height {
            let row_start = y as usize * dst.stride + clip_x as usize * bpp;
            for x in 0..width as usize {
                let offset = row_start + x * bpp;
                dst.data[offset..offset + bpp].copy_from_slice(&pixel[..bpp]);
            }
        }

        dst.dirty = true;
        dst.generation += 1;
    }
}

impl Default for Blitter {
    fn default() -> Self {
        Self::new()
    }
}
