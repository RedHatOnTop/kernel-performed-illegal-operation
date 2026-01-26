//! Rendering pipeline.
//!
//! Converts layout tree to rendered pixels.

use alloc::vec::Vec;

use crate::document::{Document, DocumentNode, Color, DisplayValue};

/// Page renderer.
pub struct Renderer {
    /// Render cache.
    cache: RenderCache,
}

/// Render cache for incremental updates.
struct RenderCache {
    /// Cached layers.
    layers: Vec<RenderLayer>,
    /// Dirty flag.
    dirty: bool,
}

/// A render layer.
struct RenderLayer {
    /// Layer bounds.
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    /// Layer pixels.
    pixels: Vec<u32>,
}

/// Render command.
#[derive(Debug, Clone)]
pub enum RenderCommand {
    /// Fill a rectangle.
    FillRect { x: i32, y: i32, width: u32, height: u32, color: u32 },
    /// Draw text.
    DrawText { x: i32, y: i32, text: alloc::string::String, color: u32, font_size: f32 },
    /// Draw border.
    DrawBorder { x: i32, y: i32, width: u32, height: u32, color: u32, thickness: u32 },
    /// Draw image.
    DrawImage { x: i32, y: i32, width: u32, height: u32, data: Vec<u32> },
}

impl Renderer {
    /// Create a new renderer.
    pub fn new() -> Self {
        Self {
            cache: RenderCache {
                layers: Vec::new(),
                dirty: true,
            },
        }
    }
    
    /// Render a document to a framebuffer.
    pub fn render(&mut self, document: &Document, framebuffer: &mut [u32], width: u32, height: u32) {
        // Clear framebuffer with white
        for pixel in framebuffer.iter_mut() {
            *pixel = 0xFFFFFFFF;
        }
        
        // Generate render commands from layout
        let commands = self.generate_commands(document);
        
        // Execute commands
        self.execute_commands(&commands, framebuffer, width, height);
    }
    
    /// Generate render commands from document.
    fn generate_commands(&self, document: &Document) -> Vec<RenderCommand> {
        let mut commands = Vec::new();
        
        if let Some(root) = document.root() {
            self.generate_node_commands(&root.borrow(), &mut commands, 0, 0);
        }
        
        commands
    }
    
    fn generate_node_commands(
        &self,
        node: &DocumentNode,
        commands: &mut Vec<RenderCommand>,
        x: i32,
        y: i32,
    ) {
        let styles = &node.computed_styles;
        
        // Skip hidden elements
        if styles.display == DisplayValue::None {
            return;
        }
        
        // Calculate dimensions
        let width = styles.width.unwrap_or(100.0) as u32;
        let height = styles.height.unwrap_or(20.0) as u32;
        
        // Draw background
        if styles.background_color.a > 0 {
            commands.push(RenderCommand::FillRect {
                x,
                y,
                width,
                height,
                color: color_to_u32(&styles.background_color),
            });
        }
        
        // Draw text content
        if let Some(text) = &node.text_content {
            if !text.trim().is_empty() {
                commands.push(RenderCommand::DrawText {
                    x,
                    y,
                    text: text.clone(),
                    color: color_to_u32(&styles.color),
                    font_size: styles.font_size,
                });
            }
        }
        
        // Render children
        let mut child_y = y;
        for child in &node.children {
            let child_ref = child.borrow();
            self.generate_node_commands(&child_ref, commands, x, child_y);
            
            // Advance position for block elements
            if child_ref.computed_styles.display == DisplayValue::Block {
                child_y += child_ref.computed_styles.height.unwrap_or(20.0) as i32;
            }
        }
    }
    
    /// Execute render commands.
    fn execute_commands(
        &self,
        commands: &[RenderCommand],
        framebuffer: &mut [u32],
        width: u32,
        height: u32,
    ) {
        for cmd in commands {
            match cmd {
                RenderCommand::FillRect { x, y, width: w, height: h, color } => {
                    self.fill_rect(framebuffer, width, height, *x, *y, *w, *h, *color);
                }
                RenderCommand::DrawText { x, y, text, color, font_size } => {
                    self.draw_text(framebuffer, width, height, *x, *y, text, *color, *font_size);
                }
                RenderCommand::DrawBorder { x, y, width: w, height: h, color, thickness } => {
                    self.draw_border(framebuffer, width, height, *x, *y, *w, *h, *color, *thickness);
                }
                RenderCommand::DrawImage { x, y, width: w, height: h, data } => {
                    self.draw_image(framebuffer, width, height, *x, *y, *w, *h, data);
                }
            }
        }
    }
    
    fn fill_rect(
        &self,
        framebuffer: &mut [u32],
        fb_width: u32,
        fb_height: u32,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        color: u32,
    ) {
        let x_start = x.max(0) as u32;
        let y_start = y.max(0) as u32;
        let x_end = ((x + width as i32) as u32).min(fb_width);
        let y_end = ((y + height as i32) as u32).min(fb_height);
        
        for py in y_start..y_end {
            for px in x_start..x_end {
                let idx = (py * fb_width + px) as usize;
                if idx < framebuffer.len() {
                    framebuffer[idx] = alpha_blend(framebuffer[idx], color);
                }
            }
        }
    }
    
    fn draw_text(
        &self,
        framebuffer: &mut [u32],
        fb_width: u32,
        fb_height: u32,
        x: i32,
        y: i32,
        text: &str,
        color: u32,
        font_size: f32,
    ) {
        // Simplified text rendering - use built-in font
        let char_width = (font_size * 0.6) as i32;
        let char_height = font_size as i32;
        
        let mut cx = x;
        for ch in text.chars() {
            if ch == ' ' {
                cx += char_width / 2;
                continue;
            }
            if ch == '\n' {
                // Skip newlines for now
                continue;
            }
            
            // Draw a simple rectangle for each character
            // In a real implementation, we'd use a font rasterizer
            self.fill_rect(
                framebuffer,
                fb_width,
                fb_height,
                cx,
                y,
                char_width as u32,
                char_height as u32,
                color,
            );
            
            cx += char_width;
        }
    }
    
    fn draw_border(
        &self,
        framebuffer: &mut [u32],
        fb_width: u32,
        fb_height: u32,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        color: u32,
        thickness: u32,
    ) {
        // Top border
        self.fill_rect(framebuffer, fb_width, fb_height, x, y, width, thickness, color);
        // Bottom border
        self.fill_rect(framebuffer, fb_width, fb_height, x, y + height as i32 - thickness as i32, width, thickness, color);
        // Left border
        self.fill_rect(framebuffer, fb_width, fb_height, x, y, thickness, height, color);
        // Right border
        self.fill_rect(framebuffer, fb_width, fb_height, x + width as i32 - thickness as i32, y, thickness, height, color);
    }
    
    fn draw_image(
        &self,
        framebuffer: &mut [u32],
        fb_width: u32,
        fb_height: u32,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        data: &[u32],
    ) {
        for dy in 0..height {
            for dx in 0..width {
                let src_idx = (dy * width + dx) as usize;
                if src_idx >= data.len() {
                    continue;
                }
                
                let px = x + dx as i32;
                let py = y + dy as i32;
                
                if px >= 0 && py >= 0 && px < fb_width as i32 && py < fb_height as i32 {
                    let dst_idx = (py as u32 * fb_width + px as u32) as usize;
                    if dst_idx < framebuffer.len() {
                        framebuffer[dst_idx] = alpha_blend(framebuffer[dst_idx], data[src_idx]);
                    }
                }
            }
        }
    }
    
    /// Mark cache as dirty.
    pub fn invalidate(&mut self) {
        self.cache.dirty = true;
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert Color to u32 (ARGB).
fn color_to_u32(color: &Color) -> u32 {
    ((color.a as u32) << 24)
        | ((color.r as u32) << 16)
        | ((color.g as u32) << 8)
        | (color.b as u32)
}

/// Alpha blend two colors.
fn alpha_blend(dst: u32, src: u32) -> u32 {
    let src_a = ((src >> 24) & 0xFF) as u32;
    
    if src_a == 0 {
        return dst;
    }
    if src_a == 255 {
        return src;
    }
    
    let dst_a = ((dst >> 24) & 0xFF) as u32;
    let dst_r = ((dst >> 16) & 0xFF) as u32;
    let dst_g = ((dst >> 8) & 0xFF) as u32;
    let dst_b = (dst & 0xFF) as u32;
    
    let src_r = ((src >> 16) & 0xFF) as u32;
    let src_g = ((src >> 8) & 0xFF) as u32;
    let src_b = (src & 0xFF) as u32;
    
    let inv_src_a = 255 - src_a;
    
    let out_a = src_a + (dst_a * inv_src_a) / 255;
    let out_r = (src_r * src_a + dst_r * inv_src_a) / 255;
    let out_g = (src_g * src_a + dst_g * inv_src_a) / 255;
    let out_b = (src_b * src_a + dst_b * inv_src_a) / 255;
    
    (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b
}
