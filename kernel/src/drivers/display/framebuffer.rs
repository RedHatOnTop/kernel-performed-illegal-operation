//! Framebuffer Driver
//!
//! Generic framebuffer implementation for display output.

use super::{Display, DisplayInfo, DisplayMode, DisplayError, PixelFormat, DisplayConnection,
            CursorInfo, HardwareCursor, color::Color};
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

/// Framebuffer configuration
#[derive(Debug, Clone)]
pub struct FramebufferConfig {
    /// Base physical address
    pub base_address: u64,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Stride (bytes per row)
    pub stride: u32,
    /// Pixel format
    pub format: PixelFormat,
}

/// Generic framebuffer
pub struct Framebuffer {
    /// Configuration
    config: FramebufferConfig,
    /// Display name
    name: String,
    /// Current mode
    current_mode: DisplayMode,
    /// Is enabled
    enabled: bool,
    /// Brightness (0-100)
    brightness: u8,
    /// Back buffer for double buffering
    back_buffer: Vec<u32>,
    /// Use double buffering
    double_buffered: bool,
}

impl Framebuffer {
    /// Create a new framebuffer
    pub fn new(config: FramebufferConfig, name: &str) -> Self {
        let mode = DisplayMode {
            width: config.width,
            height: config.height,
            refresh_rate: 60,
            format: config.format,
        };

        let buffer_size = (config.width * config.height) as usize;
        
        Self {
            config,
            name: String::from(name),
            current_mode: mode,
            enabled: true,
            brightness: 100,
            back_buffer: vec![0u32; buffer_size],
            double_buffered: true,
        }
    }

    /// Get raw framebuffer pointer
    /// 
    /// # Safety
    /// Caller must ensure the address is valid and mapped
    pub unsafe fn raw_ptr(&self) -> *mut u32 {
        self.config.base_address as *mut u32
    }

    /// Get back buffer
    pub fn back_buffer(&mut self) -> &mut [u32] {
        &mut self.back_buffer
    }

    /// Clear framebuffer with color
    pub fn clear(&mut self, color: Color) {
        let value = color.to_argb();
        for pixel in &mut self.back_buffer {
            *pixel = value;
        }
        if !self.double_buffered {
            self.present();
        }
    }

    /// Set pixel in back buffer
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        if x < self.config.width && y < self.config.height {
            let offset = (y * self.config.width + x) as usize;
            if offset < self.back_buffer.len() {
                self.back_buffer[offset] = color.to_argb();
            }
        }
    }

    /// Get pixel from back buffer
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Color> {
        if x < self.config.width && y < self.config.height {
            let offset = (y * self.config.width + x) as usize;
            self.back_buffer.get(offset).map(|&v| Color::from_argb(v))
        } else {
            None
        }
    }

    /// Draw horizontal line
    pub fn draw_hline(&mut self, x: u32, y: u32, width: u32, color: Color) {
        let value = color.to_argb();
        if y >= self.config.height {
            return;
        }
        
        let start_x = x.min(self.config.width);
        let end_x = (x + width).min(self.config.width);
        let y_offset = (y * self.config.width) as usize;

        for x in start_x..end_x {
            self.back_buffer[y_offset + x as usize] = value;
        }
    }

    /// Draw vertical line
    pub fn draw_vline(&mut self, x: u32, y: u32, height: u32, color: Color) {
        let value = color.to_argb();
        if x >= self.config.width {
            return;
        }

        let start_y = y.min(self.config.height);
        let end_y = (y + height).min(self.config.height);

        for y in start_y..end_y {
            self.back_buffer[(y * self.config.width + x) as usize] = value;
        }
    }

    /// Draw rectangle
    pub fn draw_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: Color) {
        self.draw_hline(x, y, width, color);
        self.draw_hline(x, y + height - 1, width, color);
        self.draw_vline(x, y, height, color);
        self.draw_vline(x + width - 1, y, height, color);
    }

    /// Fill rectangle
    pub fn fill_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: Color) {
        for row in y..(y + height).min(self.config.height) {
            self.draw_hline(x, row, width, color);
        }
    }

    /// Copy rectangle from source buffer
    pub fn blit(&mut self, x: u32, y: u32, width: u32, height: u32, src: &[u32]) {
        for row in 0..height {
            let dst_y = y + row;
            if dst_y >= self.config.height {
                break;
            }
            
            for col in 0..width {
                let dst_x = x + col;
                if dst_x >= self.config.width {
                    break;
                }
                
                let src_offset = (row * width + col) as usize;
                let dst_offset = (dst_y * self.config.width + dst_x) as usize;
                
                if src_offset < src.len() && dst_offset < self.back_buffer.len() {
                    self.back_buffer[dst_offset] = src[src_offset];
                }
            }
        }
    }

    /// Copy rectangle with alpha blending
    pub fn blit_alpha(&mut self, x: u32, y: u32, width: u32, height: u32, src: &[u32]) {
        for row in 0..height {
            let dst_y = y + row;
            if dst_y >= self.config.height {
                break;
            }
            
            for col in 0..width {
                let dst_x = x + col;
                if dst_x >= self.config.width {
                    break;
                }
                
                let src_offset = (row * width + col) as usize;
                let dst_offset = (dst_y * self.config.width + dst_x) as usize;
                
                if src_offset < src.len() && dst_offset < self.back_buffer.len() {
                    let src_pixel = src[src_offset];
                    let dst_pixel = self.back_buffer[dst_offset];
                    self.back_buffer[dst_offset] = blend_pixel(src_pixel, dst_pixel);
                }
            }
        }
    }

    /// Present back buffer to front buffer
    pub fn present(&mut self) {
        if self.double_buffered {
            // Copy back buffer to framebuffer
            unsafe {
                let fb_ptr = self.raw_ptr();
                let stride_pixels = self.config.stride / 4;
                
                for y in 0..self.config.height {
                    let src_offset = (y * self.config.width) as usize;
                    let dst_offset = (y * stride_pixels) as isize;
                    
                    for x in 0..self.config.width {
                        let src_idx = src_offset + x as usize;
                        if src_idx < self.back_buffer.len() {
                            fb_ptr.offset(dst_offset + x as isize).write_volatile(self.back_buffer[src_idx]);
                        }
                    }
                }
            }
        }
    }

    /// Scroll display up by lines
    pub fn scroll_up(&mut self, lines: u32) {
        let lines = lines.min(self.config.height);
        let width = self.config.width as usize;
        let height = self.config.height as usize;
        let scroll_offset = (lines as usize) * width;
        
        // Move pixels up
        for i in 0..(height - lines as usize) * width {
            self.back_buffer[i] = self.back_buffer[i + scroll_offset];
        }
        
        // Clear bottom lines
        for i in (height - lines as usize) * width..height * width {
            self.back_buffer[i] = 0;
        }
    }
}

/// Alpha blend two pixels
fn blend_pixel(src: u32, dst: u32) -> u32 {
    let src_a = ((src >> 24) & 0xFF) as u32;
    let src_r = ((src >> 16) & 0xFF) as u32;
    let src_g = ((src >> 8) & 0xFF) as u32;
    let src_b = (src & 0xFF) as u32;

    let dst_r = ((dst >> 16) & 0xFF) as u32;
    let dst_g = ((dst >> 8) & 0xFF) as u32;
    let dst_b = (dst & 0xFF) as u32;

    let inv_a = 255 - src_a;
    
    let r = (src_r * src_a + dst_r * inv_a) / 255;
    let g = (src_g * src_a + dst_g * inv_a) / 255;
    let b = (src_b * src_a + dst_b * inv_a) / 255;

    0xFF000000 | (r << 16) | (g << 8) | b
}

impl Display for Framebuffer {
    fn info(&self) -> DisplayInfo {
        DisplayInfo {
            name: self.name.clone(),
            manufacturer: String::from("Generic"),
            physical_width_mm: 0,
            physical_height_mm: 0,
            current_mode: self.current_mode,
            available_modes: vec![self.current_mode],
            is_primary: true,
            connection: DisplayConnection::Internal,
        }
    }

    fn current_mode(&self) -> DisplayMode {
        self.current_mode
    }

    fn set_mode(&mut self, mode: DisplayMode) -> Result<(), DisplayError> {
        // For generic framebuffer, mode is fixed
        if mode == self.current_mode {
            Ok(())
        } else {
            Err(DisplayError::ModeNotSupported)
        }
    }

    fn available_modes(&self) -> Vec<DisplayMode> {
        vec![self.current_mode]
    }

    fn framebuffer_address(&self) -> u64 {
        self.config.base_address
    }

    fn framebuffer_size(&self) -> usize {
        (self.config.stride * self.config.height) as usize
    }

    fn stride(&self) -> u32 {
        self.config.stride
    }

    fn enable(&mut self) -> Result<(), DisplayError> {
        self.enabled = true;
        Ok(())
    }

    fn disable(&mut self) -> Result<(), DisplayError> {
        self.enabled = false;
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn wait_vsync(&self) {
        // No vsync for generic framebuffer
    }

    fn set_brightness(&mut self, brightness: u8) -> Result<(), DisplayError> {
        self.brightness = brightness.min(100);
        Ok(())
    }

    fn brightness(&self) -> u8 {
        self.brightness
    }
}

impl HardwareCursor for Framebuffer {
    fn hardware_cursor_supported(&self) -> bool {
        false
    }

    fn set_cursor_image(&mut self, _cursor: &CursorInfo) -> Result<(), DisplayError> {
        Err(DisplayError::ModeNotSupported)
    }

    fn show_cursor(&mut self) {}
    fn hide_cursor(&mut self) {}
    fn move_cursor(&mut self, _x: u32, _y: u32) {}
}

/// Software cursor implementation
pub struct SoftwareCursor {
    /// Cursor image
    image: CursorInfo,
    /// Current X position
    x: u32,
    /// Current Y position
    y: u32,
    /// Visible
    visible: bool,
    /// Saved background under cursor
    saved_background: Vec<u32>,
}

impl SoftwareCursor {
    /// Create a new software cursor
    pub fn new() -> Self {
        Self {
            image: CursorInfo {
                width: 0,
                height: 0,
                hotspot_x: 0,
                hotspot_y: 0,
                data: Vec::new(),
            },
            x: 0,
            y: 0,
            visible: false,
            saved_background: Vec::new(),
        }
    }

    /// Set cursor image
    pub fn set_image(&mut self, cursor: CursorInfo) {
        let size = (cursor.width * cursor.height) as usize;
        self.saved_background.resize(size, 0);
        self.image = cursor;
    }

    /// Draw cursor to framebuffer
    pub fn draw(&mut self, fb: &mut Framebuffer) {
        if !self.visible || self.image.data.is_empty() {
            return;
        }

        let cursor_x = self.x.saturating_sub(self.image.hotspot_x);
        let cursor_y = self.y.saturating_sub(self.image.hotspot_y);

        // Save background
        for row in 0..self.image.height {
            for col in 0..self.image.width {
                let screen_x = cursor_x + col;
                let screen_y = cursor_y + row;
                let idx = (row * self.image.width + col) as usize;
                
                if let Some(color) = fb.get_pixel(screen_x, screen_y) {
                    if idx < self.saved_background.len() {
                        self.saved_background[idx] = color.to_argb();
                    }
                }
            }
        }

        // Draw cursor with alpha blending
        fb.blit_alpha(cursor_x, cursor_y, self.image.width, self.image.height, &self.image.data);
    }

    /// Erase cursor from framebuffer
    pub fn erase(&mut self, fb: &mut Framebuffer) {
        if !self.visible || self.image.data.is_empty() {
            return;
        }

        let cursor_x = self.x.saturating_sub(self.image.hotspot_x);
        let cursor_y = self.y.saturating_sub(self.image.hotspot_y);

        // Restore background
        fb.blit(cursor_x, cursor_y, self.image.width, self.image.height, &self.saved_background);
    }

    /// Move cursor
    pub fn move_to(&mut self, x: u32, y: u32, fb: &mut Framebuffer) {
        self.erase(fb);
        self.x = x;
        self.y = y;
        self.draw(fb);
    }

    /// Show cursor
    pub fn show(&mut self, fb: &mut Framebuffer) {
        self.visible = true;
        self.draw(fb);
    }

    /// Hide cursor
    pub fn hide(&mut self, fb: &mut Framebuffer) {
        self.erase(fb);
        self.visible = false;
    }
}

impl Default for SoftwareCursor {
    fn default() -> Self {
        Self::new()
    }
}
