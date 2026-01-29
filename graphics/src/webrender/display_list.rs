//! Display List Builder
//!
//! This module provides a display list structure that serializes rendering
//! commands from the layout engine for processing by the WebRender compositor.

use alloc::vec;
use alloc::vec::Vec;
use alloc::string::String;
use super::primitives::*;

/// A serialized list of rendering commands.
#[derive(Debug, Clone)]
pub struct DisplayList {
    /// Display items.
    items: Vec<DisplayItem>,
    /// Total bounds of all items.
    bounds: Rect,
    /// Epoch (version number).
    epoch: Epoch,
}

impl DisplayList {
    /// Create a new empty display list.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            bounds: Rect::ZERO,
            epoch: Epoch(0),
        }
    }
    
    /// Get display items.
    pub fn items(&self) -> &[DisplayItem] {
        &self.items
    }
    
    /// Get the bounds.
    pub fn bounds(&self) -> Rect {
        self.bounds
    }
    
    /// Get the epoch.
    pub fn epoch(&self) -> Epoch {
        self.epoch
    }
    
    /// Check if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    
    /// Get the number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

/// Epoch identifier for display list versioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Epoch(pub u32);

impl Epoch {
    pub fn next(&self) -> Epoch {
        Epoch(self.0.wrapping_add(1))
    }
}

/// Builder for constructing display lists.
pub struct DisplayListBuilder {
    /// Current items.
    items: Vec<DisplayItem>,
    /// Current bounds.
    bounds: Rect,
    /// Next epoch.
    epoch: Epoch,
    /// Current spatial node (for transforms).
    current_spatial: SpatialId,
    /// Current clip node.
    current_clip: ClipId,
    /// Spatial node stack.
    spatial_stack: Vec<SpatialId>,
    /// Clip node stack.
    clip_stack: Vec<ClipId>,
    /// Next spatial ID.
    next_spatial_id: u32,
    /// Next clip ID.
    next_clip_id: u32,
}

impl DisplayListBuilder {
    /// Create a new display list builder.
    pub fn new(viewport: Rect) -> Self {
        Self {
            items: Vec::new(),
            bounds: viewport,
            epoch: Epoch(0),
            current_spatial: SpatialId::ROOT,
            current_clip: ClipId::ROOT,
            spatial_stack: vec![SpatialId::ROOT],
            clip_stack: vec![ClipId::ROOT],
            next_spatial_id: 1,
            next_clip_id: 1,
        }
    }
    
    /// Build the display list.
    pub fn build(self) -> DisplayList {
        DisplayList {
            items: self.items,
            bounds: self.bounds,
            epoch: self.epoch.next(),
        }
    }
    
    /// Push a rectangle.
    pub fn push_rect(&mut self, rect: Rect, color: Color) {
        self.items.push(DisplayItem::Rectangle { rect, color });
        self.extend_bounds(rect);
    }
    
    /// Push a rounded rectangle.
    pub fn push_rounded_rect(&mut self, rect: Rect, color: Color, radii: BorderRadius) {
        self.items.push(DisplayItem::RoundedRectangle { rect, color, radii });
        self.extend_bounds(rect);
    }
    
    /// Push text.
    pub fn push_text(&mut self, rect: Rect, glyphs: Vec<GlyphInstance>, color: Color, font_size: f32) {
        self.items.push(DisplayItem::Text { rect, glyphs, color, font_size });
        self.extend_bounds(rect);
    }
    
    /// Push an image.
    pub fn push_image(&mut self, rect: Rect, image_key: ImageKey, image_size: Size) {
        self.items.push(DisplayItem::Image { rect, image_key, image_size });
        self.extend_bounds(rect);
    }
    
    /// Push a border.
    pub fn push_border(&mut self, rect: Rect, widths: SideOffsets, colors: BorderColors, styles: BorderStyles) {
        self.items.push(DisplayItem::Border { rect, widths, colors, styles });
        self.extend_bounds(rect);
    }
    
    /// Push a box shadow.
    pub fn push_box_shadow(
        &mut self,
        rect: Rect,
        offset: Point,
        color: Color,
        blur_radius: f32,
        spread_radius: f32,
        inset: bool,
    ) {
        self.items.push(DisplayItem::BoxShadow {
            rect,
            offset,
            color,
            blur_radius,
            spread_radius,
            inset,
        });
        // Box shadows extend beyond the rect
        let shadow_rect = if inset {
            rect
        } else {
            Rect {
                x: rect.x + offset.x - blur_radius - spread_radius,
                y: rect.y + offset.y - blur_radius - spread_radius,
                w: rect.w + 2.0 * (blur_radius + spread_radius),
                h: rect.h + 2.0 * (blur_radius + spread_radius),
            }
        };
        self.extend_bounds(shadow_rect);
    }
    
    /// Push a linear gradient.
    pub fn push_linear_gradient(&mut self, rect: Rect, gradient: LinearGradient) {
        self.items.push(DisplayItem::LinearGradient { rect, gradient });
        self.extend_bounds(rect);
    }
    
    /// Push a radial gradient.
    pub fn push_radial_gradient(&mut self, rect: Rect, gradient: RadialGradient) {
        self.items.push(DisplayItem::RadialGradient { rect, gradient });
        self.extend_bounds(rect);
    }
    
    /// Push a clip rect.
    pub fn push_clip(&mut self, rect: Rect) {
        self.items.push(DisplayItem::PushClip { rect });
        let clip_id = ClipId(self.next_clip_id);
        self.next_clip_id += 1;
        self.current_clip = clip_id;
        self.clip_stack.push(clip_id);
    }
    
    /// Pop the current clip.
    pub fn pop_clip(&mut self) {
        self.items.push(DisplayItem::PopClip);
        self.clip_stack.pop();
        self.current_clip = *self.clip_stack.last().unwrap_or(&ClipId::ROOT);
    }
    
    /// Push a stacking context.
    pub fn push_stacking_context(&mut self, transform: Transform, opacity: f32) {
        self.items.push(DisplayItem::PushStackingContext { transform, opacity });
        let spatial_id = SpatialId(self.next_spatial_id);
        self.next_spatial_id += 1;
        self.current_spatial = spatial_id;
        self.spatial_stack.push(spatial_id);
    }
    
    /// Pop the current stacking context.
    pub fn pop_stacking_context(&mut self) {
        self.items.push(DisplayItem::PopStackingContext);
        self.spatial_stack.pop();
        self.current_spatial = *self.spatial_stack.last().unwrap_or(&SpatialId::ROOT);
    }
    
    /// Push a reference frame with transform.
    pub fn push_reference_frame(&mut self, origin: Point, transform: Transform) {
        self.push_stacking_context(Transform::translate(origin.x, origin.y).then(&transform), 1.0);
    }
    
    /// Pop reference frame.
    pub fn pop_reference_frame(&mut self) {
        self.pop_stacking_context();
    }
    
    /// Push a scroll frame.
    pub fn push_scroll_frame(
        &mut self,
        content_rect: Rect,
        clip_rect: Rect,
        scroll_offset: Point,
    ) {
        // Clip to the visible area
        self.push_clip(clip_rect);
        // Transform by scroll offset
        self.push_stacking_context(Transform::translate(-scroll_offset.x, -scroll_offset.y), 1.0);
    }
    
    /// Pop scroll frame.
    pub fn pop_scroll_frame(&mut self) {
        self.pop_stacking_context();
        self.pop_clip();
    }
    
    /// Define a hit test region.
    pub fn push_hit_test(&mut self, rect: Rect, tag: HitTestTag) {
        self.items.push(DisplayItem::HitTest { rect, tag });
    }
    
    fn extend_bounds(&mut self, rect: Rect) {
        self.bounds = self.bounds.union(&rect);
    }
}

impl Default for DisplayListBuilder {
    fn default() -> Self {
        Self::new(Rect::ZERO)
    }
}

/// Spatial node ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpatialId(pub u32);

impl SpatialId {
    pub const ROOT: SpatialId = SpatialId(0);
}

/// Clip node ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClipId(pub u32);

impl ClipId {
    pub const ROOT: ClipId = ClipId(0);
}

/// Display item types.
#[derive(Debug, Clone)]
pub enum DisplayItem {
    /// A solid rectangle.
    Rectangle {
        rect: Rect,
        color: Color,
    },
    /// A rounded rectangle.
    RoundedRectangle {
        rect: Rect,
        color: Color,
        radii: BorderRadius,
    },
    /// Text glyphs.
    Text {
        rect: Rect,
        glyphs: Vec<GlyphInstance>,
        color: Color,
        font_size: f32,
    },
    /// An image.
    Image {
        rect: Rect,
        image_key: ImageKey,
        image_size: Size,
    },
    /// A border.
    Border {
        rect: Rect,
        widths: SideOffsets,
        colors: BorderColors,
        styles: BorderStyles,
    },
    /// A box shadow.
    BoxShadow {
        rect: Rect,
        offset: Point,
        color: Color,
        blur_radius: f32,
        spread_radius: f32,
        inset: bool,
    },
    /// A linear gradient.
    LinearGradient {
        rect: Rect,
        gradient: LinearGradient,
    },
    /// A radial gradient.
    RadialGradient {
        rect: Rect,
        gradient: RadialGradient,
    },
    /// Push a clip rect.
    PushClip {
        rect: Rect,
    },
    /// Pop clip.
    PopClip,
    /// Push a stacking context.
    PushStackingContext {
        transform: Transform,
        opacity: f32,
    },
    /// Pop stacking context.
    PopStackingContext,
    /// Hit test region.
    HitTest {
        rect: Rect,
        tag: HitTestTag,
    },
}

/// Hit test tag for event handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HitTestTag {
    pub pipeline_id: u64,
    pub node_id: u64,
}

/// Linear gradient.
#[derive(Debug, Clone)]
pub struct LinearGradient {
    pub start: Point,
    pub end: Point,
    pub stops: Vec<GradientStop>,
    pub extend_mode: ExtendMode,
}

/// Radial gradient.
#[derive(Debug, Clone)]
pub struct RadialGradient {
    pub center: Point,
    pub radius: Size,
    pub stops: Vec<GradientStop>,
    pub extend_mode: ExtendMode,
}

/// Gradient stop.
#[derive(Debug, Clone, Copy)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

/// Gradient extend mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtendMode {
    Clamp,
    Repeat,
    Reflect,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_display_list_builder() {
        let mut builder = DisplayListBuilder::new(Rect { x: 0.0, y: 0.0, w: 800.0, h: 600.0 });
        
        builder.push_rect(Rect { x: 10.0, y: 10.0, w: 100.0, h: 100.0 }, Color::RED);
        builder.push_clip(Rect { x: 0.0, y: 0.0, w: 50.0, h: 50.0 });
        builder.push_rect(Rect { x: 20.0, y: 20.0, w: 30.0, h: 30.0 }, Color::BLUE);
        builder.pop_clip();
        
        let display_list = builder.build();
        
        assert_eq!(display_list.len(), 4);
        assert!(!display_list.is_empty());
    }
    
    #[test]
    fn test_stacking_context() {
        let mut builder = DisplayListBuilder::new(Rect { x: 0.0, y: 0.0, w: 800.0, h: 600.0 });
        
        builder.push_stacking_context(Transform::identity(), 0.5);
        builder.push_rect(Rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 }, Color::WHITE);
        builder.pop_stacking_context();
        
        let display_list = builder.build();
        assert_eq!(display_list.len(), 3);
    }
}
