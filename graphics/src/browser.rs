//! Browser Display List Renderer
//!
//! This module renders DisplayList commands from kpio-layout to GPU surfaces.
//! It bridges the browser layout engine with the graphics subsystem.
//!
//! ## Rendering Pipeline
//!
//! ```text
//! DisplayList (from kpio-layout)
//!     ↓
//! BrowserRenderer (this module)
//!     ↓
//! RenderBatch (optimized draw calls)
//!     ↓
//! GPU Surface (framebuffer)
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use kpio_layout::box_model::Rect;
use kpio_layout::paint::{
    BorderRadii, BorderStyle, BorderWidths, Color, DisplayCommand, DisplayList, TextStyle,
};

use crate::surface::SurfaceId;
use crate::GraphicsError;

/// Browser renderer for converting DisplayList to GPU commands.
pub struct BrowserRenderer {
    /// Target surface for rendering
    target_surface: Option<SurfaceId>,

    /// Viewport width
    viewport_width: u32,

    /// Viewport height
    viewport_height: u32,

    /// Background color
    background_color: RenderColor,

    /// Render batches for optimization
    batches: Vec<RenderBatch>,

    /// Clip stack for nested clips
    clip_stack: Vec<RenderRect>,

    /// Transform stack
    transform_stack: Vec<Transform2D>,

    /// Opacity stack
    opacity_stack: Vec<f32>,

    /// Current effective opacity
    current_opacity: f32,
}

impl BrowserRenderer {
    /// Create a new browser renderer.
    pub fn new(viewport_width: u32, viewport_height: u32) -> Self {
        Self {
            target_surface: None,
            viewport_width,
            viewport_height,
            background_color: RenderColor::white(),
            batches: Vec::new(),
            clip_stack: Vec::new(),
            transform_stack: Vec::new(),
            opacity_stack: Vec::new(),
            current_opacity: 1.0,
        }
    }

    /// Set the target surface for rendering.
    pub fn set_target(&mut self, surface: SurfaceId) {
        self.target_surface = Some(surface);
    }

    /// Set the viewport size.
    pub fn set_viewport(&mut self, width: u32, height: u32) {
        self.viewport_width = width;
        self.viewport_height = height;
    }

    /// Set the background color.
    pub fn set_background(&mut self, color: RenderColor) {
        self.background_color = color;
    }

    /// Render a display list to the target surface.
    pub fn render(&mut self, display_list: &DisplayList) -> Result<(), GraphicsError> {
        // Clear batches
        self.batches.clear();
        self.clip_stack.clear();
        self.transform_stack.clear();
        self.opacity_stack.clear();
        self.current_opacity = 1.0;

        // Add background clear
        self.batches.push(RenderBatch::Clear(self.background_color));

        // Process each command
        for command in display_list.commands() {
            self.process_command(command)?;
        }

        // Execute batches (in a real implementation, this would submit to GPU)
        self.execute_batches()?;

        Ok(())
    }

    /// Process a single display command.
    fn process_command(&mut self, command: &DisplayCommand) -> Result<(), GraphicsError> {
        match command {
            DisplayCommand::SolidRect { color, rect } => {
                self.draw_solid_rect(*color, *rect)?;
            }

            DisplayCommand::Border {
                color,
                rect,
                widths,
                style,
            } => {
                self.draw_border(*color, *rect, *widths, *style)?;
            }

            DisplayCommand::Text { text, rect, style } => {
                self.draw_text(text, *rect, style)?;
            }

            DisplayCommand::Image {
                image_id,
                source_rect,
                dest_rect,
            } => {
                self.draw_image(*image_id, source_rect.as_ref(), *dest_rect)?;
            }

            DisplayCommand::LinearGradient {
                start_color,
                end_color,
                rect,
                angle,
            } => {
                self.draw_gradient(*start_color, *end_color, *rect, *angle)?;
            }

            DisplayCommand::RoundedRect { color, rect, radii } => {
                self.draw_rounded_rect(*color, *rect, *radii)?;
            }

            DisplayCommand::BoxShadow {
                color,
                rect,
                blur_radius,
                spread_radius,
                offset_x,
                offset_y,
                inset,
            } => {
                self.draw_box_shadow(
                    *color,
                    *rect,
                    *blur_radius,
                    *spread_radius,
                    *offset_x,
                    *offset_y,
                    *inset,
                )?;
            }

            DisplayCommand::PushClip { rect } => {
                self.push_clip(*rect);
            }

            DisplayCommand::PopClip => {
                self.pop_clip();
            }

            DisplayCommand::PushTransform { matrix } => {
                self.push_transform(*matrix);
            }

            DisplayCommand::PopTransform => {
                self.pop_transform();
            }

            DisplayCommand::PushOpacity { opacity } => {
                self.push_opacity(*opacity);
            }

            DisplayCommand::PopOpacity => {
                self.pop_opacity();
            }
        }

        Ok(())
    }

    /// Draw a solid rectangle.
    fn draw_solid_rect(&mut self, color: Color, rect: Rect) -> Result<(), GraphicsError> {
        let render_rect = self.transform_rect(rect);

        // Apply clipping
        if let Some(clipped) = self.clip_rect(render_rect) {
            let render_color = self.apply_opacity(RenderColor::from_layout(color));
            self.batches.push(RenderBatch::Rect {
                rect: clipped,
                color: render_color,
            });
        }

        Ok(())
    }

    /// Draw a border.
    fn draw_border(
        &mut self,
        color: Color,
        rect: Rect,
        widths: BorderWidths,
        _style: BorderStyle,
    ) -> Result<(), GraphicsError> {
        let render_rect = self.transform_rect(rect);
        let render_color = self.apply_opacity(RenderColor::from_layout(color));

        // Draw four border edges as separate rectangles
        // Top border
        if widths.top > 0.0 {
            let top_rect = RenderRect {
                x: render_rect.x,
                y: render_rect.y,
                width: render_rect.width,
                height: widths.top,
            };
            if let Some(clipped) = self.clip_rect(top_rect) {
                self.batches.push(RenderBatch::Rect {
                    rect: clipped,
                    color: render_color,
                });
            }
        }

        // Bottom border
        if widths.bottom > 0.0 {
            let bottom_rect = RenderRect {
                x: render_rect.x,
                y: render_rect.y + render_rect.height - widths.bottom,
                width: render_rect.width,
                height: widths.bottom,
            };
            if let Some(clipped) = self.clip_rect(bottom_rect) {
                self.batches.push(RenderBatch::Rect {
                    rect: clipped,
                    color: render_color,
                });
            }
        }

        // Left border
        if widths.left > 0.0 {
            let left_rect = RenderRect {
                x: render_rect.x,
                y: render_rect.y + widths.top,
                width: widths.left,
                height: render_rect.height - widths.top - widths.bottom,
            };
            if let Some(clipped) = self.clip_rect(left_rect) {
                self.batches.push(RenderBatch::Rect {
                    rect: clipped,
                    color: render_color,
                });
            }
        }

        // Right border
        if widths.right > 0.0 {
            let right_rect = RenderRect {
                x: render_rect.x + render_rect.width - widths.right,
                y: render_rect.y + widths.top,
                width: widths.right,
                height: render_rect.height - widths.top - widths.bottom,
            };
            if let Some(clipped) = self.clip_rect(right_rect) {
                self.batches.push(RenderBatch::Rect {
                    rect: clipped,
                    color: render_color,
                });
            }
        }

        Ok(())
    }

    /// Draw text.
    fn draw_text(
        &mut self,
        text: &str,
        rect: Rect,
        style: &TextStyle,
    ) -> Result<(), GraphicsError> {
        let render_rect = self.transform_rect(rect);
        let render_color = self.apply_opacity(RenderColor::from_layout(style.color));

        if let Some(clipped) = self.clip_rect(render_rect) {
            self.batches.push(RenderBatch::Text {
                text: String::from(text),
                rect: clipped,
                color: render_color,
                font_size: style.font_size,
                font_weight: style.font_weight,
            });
        }

        Ok(())
    }

    /// Draw an image.
    fn draw_image(
        &mut self,
        image_id: u64,
        _source_rect: Option<&Rect>,
        dest_rect: Rect,
    ) -> Result<(), GraphicsError> {
        let render_rect = self.transform_rect(dest_rect);

        if let Some(clipped) = self.clip_rect(render_rect) {
            self.batches.push(RenderBatch::Image {
                image_id,
                rect: clipped,
                opacity: self.current_opacity,
            });
        }

        Ok(())
    }

    /// Draw a linear gradient.
    fn draw_gradient(
        &mut self,
        start_color: Color,
        end_color: Color,
        rect: Rect,
        angle: f32,
    ) -> Result<(), GraphicsError> {
        let render_rect = self.transform_rect(rect);

        if let Some(clipped) = self.clip_rect(render_rect) {
            self.batches.push(RenderBatch::Gradient {
                rect: clipped,
                start_color: self.apply_opacity(RenderColor::from_layout(start_color)),
                end_color: self.apply_opacity(RenderColor::from_layout(end_color)),
                angle,
            });
        }

        Ok(())
    }

    /// Draw a rounded rectangle.
    fn draw_rounded_rect(
        &mut self,
        color: Color,
        rect: Rect,
        radii: BorderRadii,
    ) -> Result<(), GraphicsError> {
        let render_rect = self.transform_rect(rect);
        let render_color = self.apply_opacity(RenderColor::from_layout(color));

        if let Some(clipped) = self.clip_rect(render_rect) {
            self.batches.push(RenderBatch::RoundedRect {
                rect: clipped,
                color: render_color,
                radii: [
                    radii.top_left,
                    radii.top_right,
                    radii.bottom_right,
                    radii.bottom_left,
                ],
            });
        }

        Ok(())
    }

    /// Draw a box shadow.
    fn draw_box_shadow(
        &mut self,
        color: Color,
        rect: Rect,
        blur_radius: f32,
        spread_radius: f32,
        offset_x: f32,
        offset_y: f32,
        inset: bool,
    ) -> Result<(), GraphicsError> {
        let render_rect = self.transform_rect(rect);
        let render_color = self.apply_opacity(RenderColor::from_layout(color));

        self.batches.push(RenderBatch::Shadow {
            rect: render_rect,
            color: render_color,
            blur_radius,
            spread_radius,
            offset_x,
            offset_y,
            inset,
        });

        Ok(())
    }

    /// Push a clip rectangle.
    fn push_clip(&mut self, rect: Rect) {
        let render_rect = self.transform_rect(rect);
        self.clip_stack.push(render_rect);
    }

    /// Pop the clip rectangle.
    fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    /// Push a transform.
    fn push_transform(&mut self, matrix: [f32; 6]) {
        self.transform_stack.push(Transform2D::from_matrix(matrix));
    }

    /// Pop a transform.
    fn pop_transform(&mut self) {
        self.transform_stack.pop();
    }

    /// Push opacity.
    fn push_opacity(&mut self, opacity: f32) {
        self.opacity_stack.push(self.current_opacity);
        self.current_opacity *= opacity;
    }

    /// Pop opacity.
    fn pop_opacity(&mut self) {
        if let Some(prev) = self.opacity_stack.pop() {
            self.current_opacity = prev;
        }
    }

    /// Transform a rect using the current transform stack.
    fn transform_rect(&self, rect: Rect) -> RenderRect {
        let mut result = RenderRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        };

        for transform in &self.transform_stack {
            result = transform.apply_to_rect(result);
        }

        result
    }

    /// Clip a rect against the current clip stack.
    fn clip_rect(&self, rect: RenderRect) -> Option<RenderRect> {
        let mut result = rect;

        for clip in &self.clip_stack {
            result = result.intersect(clip)?;
        }

        // Clip against viewport
        let viewport = RenderRect {
            x: 0.0,
            y: 0.0,
            width: self.viewport_width as f32,
            height: self.viewport_height as f32,
        };

        result.intersect(&viewport)
    }

    /// Apply current opacity to a color.
    fn apply_opacity(&self, mut color: RenderColor) -> RenderColor {
        color.a = (color.a as f32 * self.current_opacity) as u8;
        color
    }

    /// Execute all render batches.
    fn execute_batches(&self) -> Result<(), GraphicsError> {
        // In a real implementation, this would:
        // 1. Sort batches by type for better GPU utilization
        // 2. Merge adjacent same-type batches
        // 3. Submit to GPU command buffer
        // 4. Present to surface

        // For now, we just count the batches (placeholder)
        log::debug!("Executing {} render batches", self.batches.len());

        for batch in &self.batches {
            match batch {
                RenderBatch::Clear(color) => {
                    log::trace!("Clear: {:?}", color);
                }
                RenderBatch::Rect { rect, color } => {
                    log::trace!("Rect: {:?} color={:?}", rect, color);
                }
                RenderBatch::Text { text, rect, .. } => {
                    log::trace!("Text: '{}' at {:?}", text, rect);
                }
                RenderBatch::Image { image_id, rect, .. } => {
                    log::trace!("Image: {} at {:?}", image_id, rect);
                }
                RenderBatch::Gradient { rect, .. } => {
                    log::trace!("Gradient at {:?}", rect);
                }
                RenderBatch::RoundedRect { rect, .. } => {
                    log::trace!("RoundedRect at {:?}", rect);
                }
                RenderBatch::Shadow { rect, .. } => {
                    log::trace!("Shadow at {:?}", rect);
                }
            }
        }

        Ok(())
    }

    /// Get statistics about the last render.
    pub fn get_stats(&self) -> RenderStats {
        let mut stats = RenderStats::default();

        for batch in &self.batches {
            match batch {
                RenderBatch::Clear(_) => stats.clear_count += 1,
                RenderBatch::Rect { .. } => stats.rect_count += 1,
                RenderBatch::Text { .. } => stats.text_count += 1,
                RenderBatch::Image { .. } => stats.image_count += 1,
                RenderBatch::Gradient { .. } => stats.gradient_count += 1,
                RenderBatch::RoundedRect { .. } => stats.rounded_rect_count += 1,
                RenderBatch::Shadow { .. } => stats.shadow_count += 1,
            }
        }

        stats.total_batches = self.batches.len();
        stats
    }
}

/// Render color in RGBA format.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RenderColor {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn white() -> Self {
        Self::rgb(255, 255, 255)
    }

    pub const fn black() -> Self {
        Self::rgb(0, 0, 0)
    }

    pub fn from_layout(color: Color) -> Self {
        Self::new(color.r, color.g, color.b, color.a)
    }

    /// Convert to f32 array (0.0-1.0).
    pub fn to_f32_array(&self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }
}

/// Render rectangle.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl RenderRect {
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Intersect with another rect.
    pub fn intersect(&self, other: &RenderRect) -> Option<RenderRect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());

        if right > x && bottom > y {
            Some(RenderRect {
                x,
                y,
                width: right - x,
                height: bottom - y,
            })
        } else {
            None
        }
    }
}

/// 2D transform.
#[derive(Debug, Clone, Copy)]
pub struct Transform2D {
    /// Affine transform matrix [a, b, c, d, e, f]
    /// x' = a*x + c*y + e
    /// y' = b*x + d*y + f
    matrix: [f32; 6],
}

impl Transform2D {
    pub fn identity() -> Self {
        Self {
            matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        }
    }

    pub fn from_matrix(matrix: [f32; 6]) -> Self {
        Self { matrix }
    }

    pub fn translate(x: f32, y: f32) -> Self {
        Self {
            matrix: [1.0, 0.0, 0.0, 1.0, x, y],
        }
    }

    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            matrix: [sx, 0.0, 0.0, sy, 0.0, 0.0],
        }
    }

    /// Apply transform to a rect.
    pub fn apply_to_rect(&self, rect: RenderRect) -> RenderRect {
        let [a, b, c, d, e, f] = self.matrix;

        // Transform top-left corner
        let x = a * rect.x + c * rect.y + e;
        let y = b * rect.x + d * rect.y + f;

        // Scale dimensions (simplified, assumes no rotation)
        let width = rect.width * a.abs();
        let height = rect.height * d.abs();

        RenderRect {
            x,
            y,
            width,
            height,
        }
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::identity()
    }
}

/// Render batch for GPU submission.
#[derive(Debug)]
pub enum RenderBatch {
    /// Clear the surface.
    Clear(RenderColor),

    /// Draw a rectangle.
    Rect {
        rect: RenderRect,
        color: RenderColor,
    },

    /// Draw text.
    Text {
        text: String,
        rect: RenderRect,
        color: RenderColor,
        font_size: f32,
        font_weight: u16,
    },

    /// Draw an image.
    Image {
        image_id: u64,
        rect: RenderRect,
        opacity: f32,
    },

    /// Draw a gradient.
    Gradient {
        rect: RenderRect,
        start_color: RenderColor,
        end_color: RenderColor,
        angle: f32,
    },

    /// Draw a rounded rectangle.
    RoundedRect {
        rect: RenderRect,
        color: RenderColor,
        radii: [f32; 4], // top-left, top-right, bottom-right, bottom-left
    },

    /// Draw a box shadow.
    Shadow {
        rect: RenderRect,
        color: RenderColor,
        blur_radius: f32,
        spread_radius: f32,
        offset_x: f32,
        offset_y: f32,
        inset: bool,
    },
}

/// Render statistics.
#[derive(Debug, Default)]
pub struct RenderStats {
    pub total_batches: usize,
    pub clear_count: usize,
    pub rect_count: usize,
    pub text_count: usize,
    pub image_count: usize,
    pub gradient_count: usize,
    pub rounded_rect_count: usize,
    pub shadow_count: usize,
}

impl RenderStats {
    pub fn total_draw_calls(&self) -> usize {
        self.rect_count
            + self.text_count
            + self.image_count
            + self.gradient_count
            + self.rounded_rect_count
            + self.shadow_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_intersect() {
        let a = RenderRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        let b = RenderRect {
            x: 50.0,
            y: 50.0,
            width: 100.0,
            height: 100.0,
        };

        let result = a.intersect(&b).unwrap();
        assert_eq!(result.x, 50.0);
        assert_eq!(result.y, 50.0);
        assert_eq!(result.width, 50.0);
        assert_eq!(result.height, 50.0);
    }

    #[test]
    fn test_render_color() {
        let color = RenderColor::rgb(255, 128, 0);
        let arr = color.to_f32_array();
        assert_eq!(arr[0], 1.0);
        assert!((arr[1] - 0.502).abs() < 0.01);
        assert_eq!(arr[2], 0.0);
        assert_eq!(arr[3], 1.0);
    }

    #[test]
    fn test_browser_renderer_creation() {
        let renderer = BrowserRenderer::new(800, 600);
        assert_eq!(renderer.viewport_width, 800);
        assert_eq!(renderer.viewport_height, 600);
    }
}
