//! Display List and Paint Commands
//!
//! This module generates a display list from the layout tree.
//! A display list is a flat sequence of paint commands that the renderer
//! can execute to draw the page.
//!
//! ## Display List Benefits
//!
//! - Decouples layout from rendering
//! - Enables paint optimizations (culling, caching)
//! - Supports different renderers (Vulkan, software, etc.)

use crate::box_model::Rect;
use crate::layout_box::LayoutBox;
use alloc::string::String;
use alloc::vec::Vec;

/// A color in RGBA format
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn transparent() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }
    }

    pub const fn black() -> Self {
        Self::rgb(0, 0, 0)
    }

    pub const fn white() -> Self {
        Self::rgb(255, 255, 255)
    }

    /// Convert from CSS color value
    pub fn from_css(css_color: &kpio_css::values::Color) -> Self {
        Self::new(css_color.r, css_color.g, css_color.b, css_color.a)
    }

    /// Convert to f32 RGBA (0.0-1.0 range)
    pub fn to_f32_array(&self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }
}

/// Border style for painting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
}

impl Default for BorderStyle {
    fn default() -> Self {
        BorderStyle::None
    }
}

/// Text style for painting
#[derive(Debug, Clone)]
pub struct TextStyle {
    pub font_size: f32,
    pub color: Color,
    pub font_weight: u16,
    pub italic: bool,
    pub underline: bool,
    pub line_through: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_size: 16.0,
            color: Color::black(),
            font_weight: 400,
            italic: false,
            underline: false,
            line_through: false,
        }
    }
}

/// A paint command in the display list
#[derive(Debug, Clone)]
pub enum DisplayCommand {
    /// Fill a rectangle with a solid color
    SolidRect { color: Color, rect: Rect },

    /// Draw a border around a rectangle
    Border {
        color: Color,
        rect: Rect,
        widths: BorderWidths,
        style: BorderStyle,
    },

    /// Draw text
    Text {
        text: String,
        rect: Rect,
        style: TextStyle,
    },

    /// Draw an image
    Image {
        image_id: u64,
        source_rect: Option<Rect>,
        dest_rect: Rect,
    },

    /// Draw a gradient
    LinearGradient {
        start_color: Color,
        end_color: Color,
        rect: Rect,
        angle: f32,
    },

    /// Draw a rounded rectangle
    RoundedRect {
        color: Color,
        rect: Rect,
        radii: BorderRadii,
    },

    /// Draw a box shadow
    BoxShadow {
        color: Color,
        rect: Rect,
        blur_radius: f32,
        spread_radius: f32,
        offset_x: f32,
        offset_y: f32,
        inset: bool,
    },

    /// Push a clip rectangle (subsequent commands clipped to this rect)
    PushClip { rect: Rect },

    /// Pop the most recent clip rectangle
    PopClip,

    /// Push a transform
    PushTransform {
        matrix: [f32; 6], // 2D affine: [a, b, c, d, e, f]
    },

    /// Pop a transform
    PopTransform,

    /// Set opacity for subsequent commands
    PushOpacity { opacity: f32 },

    /// Restore previous opacity
    PopOpacity,
}

/// Border widths for all four sides
#[derive(Debug, Clone, Copy, Default)]
pub struct BorderWidths {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Border radii for rounded corners
#[derive(Debug, Clone, Copy, Default)]
pub struct BorderRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl BorderRadii {
    pub fn uniform(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }
}

/// A display list - ordered sequence of paint commands
#[derive(Debug, Clone)]
pub struct DisplayList {
    commands: Vec<DisplayCommand>,
}

impl DisplayList {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn push(&mut self, command: DisplayCommand) {
        self.commands.push(command);
    }

    pub fn extend(&mut self, commands: impl IntoIterator<Item = DisplayCommand>) {
        self.commands.extend(commands);
    }

    pub fn commands(&self) -> &[DisplayCommand] {
        &self.commands
    }

    pub fn into_commands(self) -> Vec<DisplayCommand> {
        self.commands
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }
}

impl Default for DisplayList {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a display list from a layout tree
pub fn build_display_list(layout_root: &LayoutBox) -> DisplayList {
    let mut display_list = DisplayList::new();
    paint_layout_box(&mut display_list, layout_root);
    display_list
}

/// Paint a single layout box and its children
fn paint_layout_box(display_list: &mut DisplayList, layout_box: &LayoutBox) {
    // Paint background
    paint_background(display_list, layout_box);

    // Paint borders
    paint_borders(display_list, layout_box);

    // Paint text content
    if let Some(ref text) = layout_box.text {
        paint_text(display_list, layout_box, text);
    }

    // Paint children (in document order for now)
    for child in &layout_box.children {
        paint_layout_box(display_list, child);
    }
}

/// Paint the background of a box
fn paint_background(display_list: &mut DisplayList, layout_box: &LayoutBox) {
    // Get background color (default to transparent)
    // In a full implementation, this would come from computed style
    let background_color = Color::transparent();

    // Only paint if not transparent
    if background_color.a > 0 {
        display_list.push(DisplayCommand::SolidRect {
            color: background_color,
            rect: layout_box.dimensions.border_box(),
        });
    }
}

/// Paint the borders of a box
fn paint_borders(display_list: &mut DisplayList, layout_box: &LayoutBox) {
    let border = &layout_box.dimensions.border;

    // Skip if no borders
    if border.top == 0.0 && border.right == 0.0 && border.bottom == 0.0 && border.left == 0.0 {
        return;
    }

    let border_box = layout_box.dimensions.border_box();

    // Paint border (simplified: single color for all sides)
    display_list.push(DisplayCommand::Border {
        color: Color::black(), // Would come from style
        rect: border_box,
        widths: BorderWidths {
            top: border.top,
            right: border.right,
            bottom: border.bottom,
            left: border.left,
        },
        style: BorderStyle::Solid,
    });
}

/// Paint text content
fn paint_text(display_list: &mut DisplayList, layout_box: &LayoutBox, text: &str) {
    if text.is_empty() {
        return;
    }

    let text_style = TextStyle::default(); // Would come from computed style

    display_list.push(DisplayCommand::Text {
        text: text.into(),
        rect: layout_box.dimensions.content,
        style: text_style,
    });
}

/// Paint a stacking context (for z-index, transforms, etc.)
pub fn paint_stacking_context(
    display_list: &mut DisplayList,
    layout_box: &LayoutBox,
    opacity: f32,
) {
    if (opacity - 1.0).abs() > f32::EPSILON {
        display_list.push(DisplayCommand::PushOpacity { opacity });
    }

    paint_layout_box(display_list, layout_box);

    if (opacity - 1.0).abs() > f32::EPSILON {
        display_list.push(DisplayCommand::PopOpacity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_box::BoxType;

    #[test]
    fn test_color_conversion() {
        let color = Color::rgb(255, 128, 0);
        let arr = color.to_f32_array();
        assert_eq!(arr[0], 1.0);
        assert!((arr[1] - 0.502).abs() < 0.01);
        assert_eq!(arr[2], 0.0);
        assert_eq!(arr[3], 1.0);
    }

    #[test]
    fn test_build_display_list() {
        let mut layout_box = LayoutBox::new(BoxType::Block);
        layout_box.dimensions.content = Rect::new(0.0, 0.0, 100.0, 50.0);

        let display_list = build_display_list(&layout_box);
        // Empty box with no visible content produces minimal commands
        assert!(display_list.len() < 10);
    }
}
