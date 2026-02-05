//! Renderer
//!
//! Low-level framebuffer rendering primitives with optional clipping support.

use super::font::Font8x8;
use crate::graphics::DamageRect;

/// Color in ARGB format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    
    /// Blend this color over another (alpha compositing)
    #[inline]
    pub fn blend_over(self, dst: Color) -> Color {
        if self.a == 255 {
            return self;
        }
        if self.a == 0 {
            return dst;
        }
        
        let src_a = self.a as u32;
        let dst_a = dst.a as u32;
        let inv_src_a = 255 - src_a;
        
        Color {
            r: ((self.r as u32 * src_a + dst.r as u32 * inv_src_a) / 255) as u8,
            g: ((self.g as u32 * src_a + dst.g as u32 * inv_src_a) / 255) as u8,
            b: ((self.b as u32 * src_a + dst.b as u32 * inv_src_a) / 255) as u8,
            a: ((src_a + dst_a * inv_src_a / 255).min(255)) as u8,
        }
    }
    
    /// Darken color by a factor (0-255)
    pub fn darken(self, amount: u8) -> Color {
        let factor = 255 - amount;
        Color {
            r: (self.r as u32 * factor as u32 / 255) as u8,
            g: (self.g as u32 * factor as u32 / 255) as u8,
            b: (self.b as u32 * factor as u32 / 255) as u8,
            a: self.a,
        }
    }
    
    /// Lighten color by a factor (0-255)
    pub fn lighten(self, amount: u8) -> Color {
        Color {
            r: self.r.saturating_add(((255 - self.r) as u32 * amount as u32 / 255) as u8),
            g: self.g.saturating_add(((255 - self.g) as u32 * amount as u32 / 255) as u8),
            b: self.b.saturating_add(((255 - self.b) as u32 * amount as u32 / 255) as u8),
            a: self.a,
        }
    }

    // Predefined colors
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const RED: Color = Color::rgb(255, 0, 0);
    pub const GREEN: Color = Color::rgb(0, 255, 0);
    pub const BLUE: Color = Color::rgb(0, 0, 255);
    pub const GRAY: Color = Color::rgb(128, 128, 128);
    pub const DARK_GRAY: Color = Color::rgb(64, 64, 64);
    pub const LIGHT_GRAY: Color = Color::rgb(192, 192, 192);
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);
    
    // Desktop colors
    pub const DESKTOP_BG: Color = Color::rgb(0, 30, 60);
    pub const TASKBAR_BG: Color = Color::rgb(30, 30, 30);
    pub const WINDOW_BG: Color = Color::rgb(240, 240, 240);
    pub const WINDOW_TITLE_ACTIVE: Color = Color::rgb(0, 120, 215);
    pub const WINDOW_TITLE_INACTIVE: Color = Color::rgb(100, 100, 100);
    pub const BUTTON_BG: Color = Color::rgb(225, 225, 225);
    pub const BUTTON_HOVER: Color = Color::rgb(200, 200, 200);
    pub const CLOSE_BUTTON_HOVER: Color = Color::rgb(232, 17, 35);
    
    // Additional UI colors
    pub const SELECTION: Color = Color::rgba(0, 120, 215, 180);
    pub const SHADOW: Color = Color::rgba(0, 0, 0, 64);
    pub const HIGHLIGHT: Color = Color::rgba(255, 255, 255, 128);
}

/// Clipping region for the renderer
#[derive(Debug, Clone, Copy)]
pub struct ClipRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl ClipRect {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }
    
    pub fn from_damage(rect: &DamageRect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    }
    
    #[inline]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.width as i32 &&
        y >= self.y && y < self.y + self.height as i32
    }
}

/// Framebuffer renderer
pub struct Renderer {
    buffer: *mut u8,
    width: u32,
    height: u32,
    bpp: usize,
    stride: usize,
}

impl Renderer {
    /// Create new renderer
    pub fn new(buffer: *mut u8, width: u32, height: u32, bpp: usize, stride: usize) -> Self {
        Self {
            buffer,
            width,
            height,
            bpp,
            stride,
        }
    }

    /// Set a pixel
    #[inline]
    pub fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }

        let offset = (y as usize * self.stride + x as usize) * self.bpp;
        
        unsafe {
            // BGR format (most common)
            *self.buffer.add(offset) = color.b;
            *self.buffer.add(offset + 1) = color.g;
            *self.buffer.add(offset + 2) = color.r;
            if self.bpp == 4 {
                *self.buffer.add(offset + 3) = color.a;
            }
        }
    }

    /// Fill rectangle
    pub fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
        let x_start = x.max(0) as u32;
        let y_start = y.max(0) as u32;
        let x_end = ((x + w as i32) as u32).min(self.width);
        let y_end = ((y + h as i32) as u32).min(self.height);

        for py in y_start..y_end {
            for px in x_start..x_end {
                self.set_pixel(px as i32, py as i32, color);
            }
        }
    }

    /// Draw rectangle outline
    pub fn draw_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
        self.draw_hline(x, y, w, color);
        self.draw_hline(x, y + h as i32 - 1, w, color);
        self.draw_vline(x, y, h, color);
        self.draw_vline(x + w as i32 - 1, y, h, color);
    }

    /// Draw horizontal line
    pub fn draw_hline(&mut self, x: i32, y: i32, w: u32, color: Color) {
        for px in 0..w as i32 {
            self.set_pixel(x + px, y, color);
        }
    }

    /// Draw vertical line
    pub fn draw_vline(&mut self, x: i32, y: i32, h: u32, color: Color) {
        for py in 0..h as i32 {
            self.set_pixel(x, y + py, color);
        }
    }

    /// Draw text using 8x8 font
    pub fn draw_text(&mut self, x: i32, y: i32, text: &str, color: Color) {
        let mut cx = x;
        for ch in text.chars() {
            self.draw_char(cx, y, ch, color);
            cx += 8;
        }
    }

    /// Draw a single character
    pub fn draw_char(&mut self, x: i32, y: i32, ch: char, color: Color) {
        let glyph = Font8x8::get_glyph(ch);
        
        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if (bits >> (7 - col)) & 1 == 1 {
                    self.set_pixel(x + col, y + row as i32, color);
                }
            }
        }
    }

    /// Draw text with scale
    pub fn draw_text_scaled(&mut self, x: i32, y: i32, text: &str, color: Color, scale: u32) {
        let mut cx = x;
        for ch in text.chars() {
            self.draw_char_scaled(cx, y, ch, color, scale);
            cx += 8 * scale as i32;
        }
    }

    /// Draw a scaled character
    pub fn draw_char_scaled(&mut self, x: i32, y: i32, ch: char, color: Color, scale: u32) {
        let glyph = Font8x8::get_glyph(ch);
        
        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if (bits >> (7 - col)) & 1 == 1 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            self.set_pixel(
                                x + col * scale as i32 + sx as i32,
                                y + row as i32 * scale as i32 + sy as i32,
                                color,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Clear screen with color
    pub fn clear(&mut self, color: Color) {
        self.fill_rect(0, 0, self.width, self.height, color);
    }

    /// Draw a simple icon (16x16)
    pub fn draw_icon(&mut self, x: i32, y: i32, icon: &[u16; 16], color: Color) {
        for (row, &bits) in icon.iter().enumerate() {
            for col in 0..16 {
                if (bits >> (15 - col)) & 1 == 1 {
                    self.set_pixel(x + col, y + row as i32, color);
                }
            }
        }
    }
    
    /// Fill rectangle with alpha blending
    pub fn fill_rect_alpha(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
        if color.a == 255 {
            self.fill_rect(x, y, w, h, color);
            return;
        }
        if color.a == 0 {
            return;
        }
        
        let x_start = x.max(0) as u32;
        let y_start = y.max(0) as u32;
        let x_end = ((x + w as i32) as u32).min(self.width);
        let y_end = ((y + h as i32) as u32).min(self.height);

        for py in y_start..y_end {
            for px in x_start..x_end {
                let dst = self.get_pixel(px as i32, py as i32);
                let blended = color.blend_over(dst);
                self.set_pixel(px as i32, py as i32, blended);
            }
        }
    }
    
    /// Get pixel color at position
    #[inline]
    pub fn get_pixel(&self, x: i32, y: i32) -> Color {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return Color::BLACK;
        }

        let offset = (y as usize * self.stride + x as usize) * self.bpp;
        
        unsafe {
            Color {
                b: *self.buffer.add(offset),
                g: *self.buffer.add(offset + 1),
                r: *self.buffer.add(offset + 2),
                a: if self.bpp == 4 { *self.buffer.add(offset + 3) } else { 255 },
            }
        }
    }
    
    /// Draw a rounded rectangle
    pub fn draw_rounded_rect(&mut self, x: i32, y: i32, w: u32, h: u32, radius: u32, color: Color) {
        let r = radius as i32;
        
        // Top and bottom horizontal lines
        self.draw_hline(x + r, y, w - 2 * radius, color);
        self.draw_hline(x + r, y + h as i32 - 1, w - 2 * radius, color);
        
        // Left and right vertical lines
        self.draw_vline(x, y + r, h - 2 * radius, color);
        self.draw_vline(x + w as i32 - 1, y + r, h - 2 * radius, color);
        
        // Corners (simple approximation without anti-aliasing)
        self.draw_corner(x + r, y + r, r, 0, color);           // Top-left
        self.draw_corner(x + w as i32 - r - 1, y + r, r, 1, color);    // Top-right
        self.draw_corner(x + r, y + h as i32 - r - 1, r, 2, color);    // Bottom-left
        self.draw_corner(x + w as i32 - r - 1, y + h as i32 - r - 1, r, 3, color); // Bottom-right
    }
    
    /// Draw corner arc (quarter circle)
    fn draw_corner(&mut self, cx: i32, cy: i32, r: i32, quadrant: u8, color: Color) {
        // Simple midpoint circle algorithm for one quadrant
        let mut x = r;
        let mut y = 0;
        let mut err = 1 - r;
        
        while x >= y {
            let points: [(i32, i32); 2] = match quadrant {
                0 => [(-x, -y), (-y, -x)], // Top-left
                1 => [(x, -y), (y, -x)],   // Top-right
                2 => [(-x, y), (-y, x)],   // Bottom-left
                _ => [(x, y), (y, x)],     // Bottom-right
            };
            
            for (dx, dy) in points {
                self.set_pixel(cx + dx, cy + dy, color);
            }
            
            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x + 1);
            }
        }
    }
    
    /// Fill a rounded rectangle
    pub fn fill_rounded_rect(&mut self, x: i32, y: i32, w: u32, h: u32, radius: u32, color: Color) {
        let r = radius.min(w / 2).min(h / 2) as i32;
        
        // Fill center rectangle
        self.fill_rect(x, y + r, w, h - 2 * radius, color);
        
        // Fill top and bottom strips
        self.fill_rect(x + r, y, w - 2 * radius, radius, color);
        self.fill_rect(x + r, y + h as i32 - r, w - 2 * radius, radius, color);
        
        // Fill corners
        self.fill_corner(x + r, y + r, r, 0, color);
        self.fill_corner(x + w as i32 - r - 1, y + r, r, 1, color);
        self.fill_corner(x + r, y + h as i32 - r - 1, r, 2, color);
        self.fill_corner(x + w as i32 - r - 1, y + h as i32 - r - 1, r, 3, color);
    }
    
    /// Fill corner (quarter disc)
    fn fill_corner(&mut self, cx: i32, cy: i32, r: i32, quadrant: u8, color: Color) {
        for dy in 0..=r {
            for dx in 0..=r {
                if dx * dx + dy * dy <= r * r {
                    let (px, py) = match quadrant {
                        0 => (cx - dx, cy - dy),
                        1 => (cx + dx, cy - dy),
                        2 => (cx - dx, cy + dy),
                        _ => (cx + dx, cy + dy),
                    };
                    self.set_pixel(px, py, color);
                }
            }
        }
    }
    
    /// Draw a gradient rectangle (vertical)
    pub fn fill_gradient_v(&mut self, x: i32, y: i32, w: u32, h: u32, top: Color, bottom: Color) {
        for py in 0..h as i32 {
            let t = py as f32 / (h - 1).max(1) as f32;
            let color = Color {
                r: ((1.0 - t) * top.r as f32 + t * bottom.r as f32) as u8,
                g: ((1.0 - t) * top.g as f32 + t * bottom.g as f32) as u8,
                b: ((1.0 - t) * top.b as f32 + t * bottom.b as f32) as u8,
                a: ((1.0 - t) * top.a as f32 + t * bottom.a as f32) as u8,
            };
            self.draw_hline(x, y + py, w, color);
        }
    }
    
    /// Draw a shadow under a rectangle
    pub fn draw_shadow(&mut self, x: i32, y: i32, w: u32, h: u32, offset: i32, blur: u32) {
        let shadow = Color::SHADOW;
        // Simple shadow: just offset filled rect with alpha
        self.fill_rect_alpha(x + offset, y + offset, w, h, shadow);
    }
}
