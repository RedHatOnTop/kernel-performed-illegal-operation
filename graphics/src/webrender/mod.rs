//! WebRender-style Display List and GPU Compositor
//!
//! This module implements a WebRender-inspired GPU-accelerated rendering pipeline
//! for browser content. It uses tile-based rendering with Vulkan backend.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Display List                              │
//! │  (Serialized rendering commands from layout engine)         │
//! └─────────────────────────┬───────────────────────────────────┘
//!                           │
//!                           ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Frame Builder                             │
//! │  (Builds render passes, batches primitives)                 │
//! └─────────────────────────┬───────────────────────────────────┘
//!                           │
//!                           ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Tile Cache                                │
//! │  (Caches rendered tiles for incremental updates)            │
//! └─────────────────────────┬───────────────────────────────────┘
//!                           │
//!                           ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Vulkan Renderer                           │
//! │  (GPU command submission, texture management)               │
//! └─────────────────────────────────────────────────────────────┘
//! ```

pub mod batch;
pub mod compositor;
pub mod display_list;
pub mod primitives;
pub mod renderer;
pub mod tile_cache;

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use spin::RwLock;

pub use batch::{BatchKey, PrimitiveBatch};
pub use compositor::{CompositeMode, WebRenderCompositor};
pub use display_list::{DisplayItem, DisplayList, DisplayListBuilder};
pub use primitives::*;
pub use renderer::WebRenderRenderer;
pub use tile_cache::{CachedTile, TileCache, TileKey};

/// WebRender configuration.
#[derive(Debug, Clone)]
pub struct WebRenderConfig {
    /// Tile size in pixels.
    pub tile_size: u32,
    /// Maximum texture cache size in bytes.
    pub max_texture_cache: usize,
    /// Maximum glyph cache size in bytes.
    pub max_glyph_cache: usize,
    /// Enable picture caching for layers.
    pub enable_picture_caching: bool,
    /// Number of worker threads for rendering.
    pub worker_threads: usize,
    /// Enable low-end mode (reduced effects).
    pub low_end_mode: bool,
}

impl Default for WebRenderConfig {
    fn default() -> Self {
        Self {
            tile_size: 256,
            max_texture_cache: 256 * 1024 * 1024, // 256 MB
            max_glyph_cache: 64 * 1024 * 1024,    // 64 MB
            enable_picture_caching: true,
            worker_threads: 4,
            low_end_mode: false,
        }
    }
}

/// WebRender pipeline state.
pub struct WebRenderPipeline {
    /// Configuration.
    config: WebRenderConfig,
    /// Tile cache.
    tile_cache: Arc<RwLock<TileCache>>,
    /// Renderer.
    renderer: WebRenderRenderer,
    /// Current frame.
    current_frame: u64,
}

impl WebRenderPipeline {
    /// Create a new WebRender pipeline.
    pub fn new(config: WebRenderConfig) -> Self {
        Self {
            tile_cache: Arc::new(RwLock::new(TileCache::new(config.tile_size))),
            renderer: WebRenderRenderer::new(&config),
            config,
            current_frame: 0,
        }
    }

    /// Render a display list to a framebuffer.
    pub fn render(&mut self, display_list: &DisplayList, width: u32, height: u32) -> RenderResult {
        self.current_frame += 1;

        // Build frame from display list
        let frame = self.build_frame(display_list, width, height);

        // Update tile cache
        {
            let mut cache = self.tile_cache.write();
            cache.update(&frame);
        }

        // Render to GPU
        self.renderer.render(&frame, &self.tile_cache.read())
    }

    /// Build a frame from a display list.
    fn build_frame(&self, display_list: &DisplayList, width: u32, height: u32) -> Frame {
        let mut frame = Frame::new(width, height, self.current_frame);

        // Process display items
        for item in display_list.items() {
            match item {
                DisplayItem::Rectangle { rect, color } => {
                    frame.add_rect(*rect, *color);
                }
                DisplayItem::RoundedRectangle { rect, color, radii } => {
                    frame.add_rounded_rect(*rect, *color, *radii);
                }
                DisplayItem::Text {
                    rect,
                    glyphs,
                    color,
                    font_size,
                } => {
                    frame.add_text(*rect, glyphs, *color, *font_size);
                }
                DisplayItem::Image {
                    rect, image_key, ..
                } => {
                    frame.add_image(*rect, *image_key);
                }
                DisplayItem::Border {
                    rect,
                    widths,
                    colors,
                    styles,
                } => {
                    frame.add_border(*rect, *widths, *colors, *styles);
                }
                DisplayItem::BoxShadow {
                    rect,
                    offset,
                    color,
                    blur_radius,
                    spread_radius,
                    inset,
                } => {
                    frame.add_box_shadow(
                        *rect,
                        *offset,
                        *color,
                        *blur_radius,
                        *spread_radius,
                        *inset,
                    );
                }
                DisplayItem::PushClip { rect } => {
                    frame.push_clip(*rect);
                }
                DisplayItem::PopClip => {
                    frame.pop_clip();
                }
                DisplayItem::PushStackingContext { transform, opacity } => {
                    frame.push_stacking_context(*transform, *opacity);
                }
                DisplayItem::PopStackingContext => {
                    frame.pop_stacking_context();
                }
                DisplayItem::LinearGradient { rect, gradient } => {
                    frame.add_linear_gradient(*rect, gradient);
                }
                DisplayItem::RadialGradient { rect, gradient } => {
                    frame.add_radial_gradient(*rect, gradient);
                }
                DisplayItem::HitTest { .. } => {
                    // Hit test regions don't produce visual output
                }
            }
        }

        // Batch primitives
        frame.finalize();

        frame
    }

    /// Invalidate a region for repainting.
    pub fn invalidate(&mut self, rect: Rect) {
        let mut cache = self.tile_cache.write();
        cache.invalidate(rect);
    }

    /// Clear all caches.
    pub fn clear_caches(&mut self) {
        self.tile_cache.write().clear();
        self.renderer.clear_caches();
    }

    /// Get statistics.
    pub fn stats(&self) -> RenderStats {
        RenderStats {
            frame_number: self.current_frame,
            cached_tiles: self.tile_cache.read().len(),
            tile_cache_size: self.tile_cache.read().memory_usage(),
            ..Default::default()
        }
    }
}

/// Render result.
#[derive(Debug)]
pub struct RenderResult {
    /// Whether rendering succeeded.
    pub success: bool,
    /// Render time in microseconds.
    pub render_time_us: u64,
    /// Number of draw calls.
    pub draw_calls: u32,
    /// Number of triangles rendered.
    pub triangles: u32,
}

/// Frame being built for rendering.
pub struct Frame {
    /// Frame dimensions.
    pub width: u32,
    pub height: u32,
    /// Frame number.
    pub frame_number: u64,
    /// Primitive batches.
    pub batches: Vec<PrimitiveBatch>,
    /// Clip stack.
    clip_stack: Vec<Rect>,
    /// Stacking context stack.
    context_stack: Vec<StackingContext>,
    /// Current primitives.
    primitives: Vec<Primitive>,
}

impl Frame {
    fn new(width: u32, height: u32, frame_number: u64) -> Self {
        Self {
            width,
            height,
            frame_number,
            batches: Vec::new(),
            clip_stack: vec![Rect {
                x: 0.0,
                y: 0.0,
                w: width as f32,
                h: height as f32,
            }],
            context_stack: vec![StackingContext::default()],
            primitives: Vec::new(),
        }
    }

    fn current_clip(&self) -> Rect {
        self.clip_stack.last().copied().unwrap_or(Rect::ZERO)
    }

    fn current_context(&self) -> &StackingContext {
        self.context_stack.last().unwrap()
    }

    fn add_rect(&mut self, rect: Rect, color: Color) {
        self.primitives.push(Primitive::Rect { rect, color });
    }

    fn add_rounded_rect(&mut self, rect: Rect, color: Color, radii: BorderRadius) {
        self.primitives
            .push(Primitive::RoundedRect { rect, color, radii });
    }

    fn add_text(&mut self, rect: Rect, glyphs: &[GlyphInstance], color: Color, font_size: f32) {
        self.primitives.push(Primitive::Text {
            rect,
            glyphs: glyphs.to_vec(),
            color,
            font_size,
        });
    }

    fn add_image(&mut self, rect: Rect, image_key: ImageKey) {
        self.primitives.push(Primitive::Image { rect, image_key });
    }

    fn add_border(
        &mut self,
        rect: Rect,
        widths: SideOffsets,
        colors: BorderColors,
        styles: BorderStyles,
    ) {
        self.primitives.push(Primitive::Border {
            rect,
            widths,
            colors,
            styles,
        });
    }

    fn add_box_shadow(
        &mut self,
        rect: Rect,
        offset: Point,
        color: Color,
        blur_radius: f32,
        spread_radius: f32,
        inset: bool,
    ) {
        self.primitives.push(Primitive::BoxShadow {
            rect,
            offset,
            color,
            blur_radius,
            spread_radius,
            inset,
        });
    }

    fn add_linear_gradient(&mut self, rect: Rect, gradient: &display_list::LinearGradient) {
        self.primitives.push(Primitive::LinearGradient {
            rect,
            start: gradient.start,
            end: gradient.end,
            stops: gradient.stops.iter().map(|s| (s.offset, s.color)).collect(),
        });
    }

    fn add_radial_gradient(&mut self, rect: Rect, gradient: &display_list::RadialGradient) {
        self.primitives.push(Primitive::RadialGradient {
            rect,
            center: gradient.center,
            radius: gradient.radius,
            stops: gradient.stops.iter().map(|s| (s.offset, s.color)).collect(),
        });
    }

    fn push_clip(&mut self, rect: Rect) {
        let current = self.current_clip();
        let clipped = rect.intersect(&current);
        self.clip_stack.push(clipped);
    }

    fn pop_clip(&mut self) {
        if self.clip_stack.len() > 1 {
            self.clip_stack.pop();
        }
    }

    fn push_stacking_context(&mut self, transform: Transform, opacity: f32) {
        let mut ctx = self.current_context().clone();
        ctx.transform = ctx.transform.then(&transform);
        ctx.opacity *= opacity;
        self.context_stack.push(ctx);
    }

    fn pop_stacking_context(&mut self) {
        if self.context_stack.len() > 1 {
            self.context_stack.pop();
        }
    }

    fn finalize(&mut self) {
        // Sort and batch primitives
        self.batch_primitives();
    }

    fn batch_primitives(&mut self) {
        // Group primitives by type for efficient GPU batching
        let mut rect_primitives = Vec::new();
        let mut text_primitives = Vec::new();
        let mut image_primitives = Vec::new();

        for prim in self.primitives.drain(..) {
            match prim {
                Primitive::Rect { .. } | Primitive::RoundedRect { .. } => {
                    rect_primitives.push(prim);
                }
                Primitive::Text { .. } => {
                    text_primitives.push(prim);
                }
                Primitive::Image { .. } => {
                    image_primitives.push(prim);
                }
                _ => {
                    // Other primitives go into their own batches
                    self.batches.push(PrimitiveBatch::single(prim));
                }
            }
        }

        // Create batches
        if !rect_primitives.is_empty() {
            self.batches.push(PrimitiveBatch::from_primitives(
                BatchKey::Rects,
                rect_primitives,
            ));
        }
        if !text_primitives.is_empty() {
            self.batches.push(PrimitiveBatch::from_primitives(
                BatchKey::Text,
                text_primitives,
            ));
        }
        if !image_primitives.is_empty() {
            self.batches.push(PrimitiveBatch::from_primitives(
                BatchKey::Images,
                image_primitives,
            ));
        }
    }
}

/// Stacking context.
#[derive(Debug, Clone)]
pub struct StackingContext {
    pub transform: Transform,
    pub opacity: f32,
    pub blend_mode: BlendMode,
}

impl Default for StackingContext {
    fn default() -> Self {
        Self {
            transform: Transform::identity(),
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
        }
    }
}

/// Blend mode for compositing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
}

/// Render statistics.
#[derive(Debug, Default)]
pub struct RenderStats {
    pub frame_number: u64,
    pub render_time_us: u64,
    pub draw_calls: u32,
    pub triangles: u32,
    pub cached_tiles: usize,
    pub tile_cache_size: usize,
    pub texture_uploads: u32,
    pub texture_upload_bytes: usize,
}
