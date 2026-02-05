//! Renderer
//!
//! Low-level framebuffer rendering primitives.

use super::font::Font8x8;

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

    // Predefined colors
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const RED: Color = Color::rgb(255, 0, 0);
    pub const GREEN: Color = Color::rgb(0, 255, 0);
    pub const BLUE: Color = Color::rgb(0, 0, 255);
    pub const GRAY: Color = Color::rgb(128, 128, 128);
    pub const DARK_GRAY: Color = Color::rgb(64, 64, 64);
    pub const LIGHT_GRAY: Color = Color::rgb(192, 192, 192);
    
    // Desktop colors
    pub const DESKTOP_BG: Color = Color::rgb(0, 30, 60);
    pub const TASKBAR_BG: Color = Color::rgb(30, 30, 30);
    pub const WINDOW_BG: Color = Color::rgb(240, 240, 240);
    pub const WINDOW_TITLE_ACTIVE: Color = Color::rgb(0, 120, 215);
    pub const WINDOW_TITLE_INACTIVE: Color = Color::rgb(100, 100, 100);
    pub const BUTTON_BG: Color = Color::rgb(225, 225, 225);
    pub const BUTTON_HOVER: Color = Color::rgb(200, 200, 200);
    pub const CLOSE_BUTTON_HOVER: Color = Color::rgb(232, 17, 35);
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
}
