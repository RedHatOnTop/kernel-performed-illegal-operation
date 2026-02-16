//! Rendering Pipeline
//!
//! This module integrates all browser components into a complete rendering pipeline:
//!
//! ```text
//! HTML String
//!     ↓
//! kpio-html (Parser) → DOM Tree
//!     ↓
//! kpio-css (Stylesheets) → Style computation
//!     ↓
//! kpio-layout (LayoutBox) → BoxDimensions
//!     ↓
//! kpio-layout (DisplayList)
//!     ↓
//! Framebuffer output
//! ```

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use kpio_layout::paint::Color as LayoutColor;
use kpio_layout::paint::{BorderStyle, BorderWidths, TextStyle};
use kpio_layout::{BoxDimensions, DisplayCommand, DisplayList, EdgeSizes, Rect};
use libm::ceilf;

/// A color in RGBA format (pipeline-local type).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PipelineColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl PipelineColor {
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

    /// Convert to u32 (ARGB format).
    pub fn to_u32(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    /// Convert to kpio_layout Color.
    pub fn to_layout_color(&self) -> LayoutColor {
        LayoutColor::new(self.r, self.g, self.b, self.a)
    }
}

/// Rendering pipeline that processes HTML/CSS into display commands.
pub struct RenderPipeline {
    /// Viewport width in pixels.
    viewport_width: f32,
    /// Viewport height in pixels.
    viewport_height: f32,
    /// Default font size.
    default_font_size: f32,
    /// Background color.
    background_color: PipelineColor,
}

impl RenderPipeline {
    /// Create a new rendering pipeline.
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            viewport_width,
            viewport_height,
            default_font_size: 16.0,
            background_color: PipelineColor::white(),
        }
    }

    /// Set viewport size.
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.viewport_width = width;
        self.viewport_height = height;
    }

    /// Get viewport width.
    pub fn viewport_width(&self) -> f32 {
        self.viewport_width
    }

    /// Get viewport height.
    pub fn viewport_height(&self) -> f32 {
        self.viewport_height
    }

    /// Set background color.
    pub fn set_background(&mut self, color: PipelineColor) {
        self.background_color = color;
    }

    /// Process HTML and return a display list.
    ///
    /// This is the main entry point for the rendering pipeline.
    pub fn render(&self, html: &str) -> Result<DisplayList, PipelineError> {
        // Step 1: Parse HTML into layout tree
        let layout_tree = self.parse_and_layout(html)?;

        // Step 2: Generate display list
        let display_list = self.paint(&layout_tree);

        Ok(display_list)
    }

    /// Render to a framebuffer.
    pub fn render_to_framebuffer(
        &self,
        html: &str,
        framebuffer: &mut [u32],
        width: u32,
        height: u32,
    ) -> Result<(), PipelineError> {
        let display_list = self.render(html)?;

        // Clear framebuffer with background color
        let bg = self.background_color.to_u32();
        for pixel in framebuffer.iter_mut() {
            *pixel = bg;
        }

        // Execute display commands
        for command in display_list.commands() {
            self.execute_command(command, framebuffer, width, height);
        }

        Ok(())
    }

    /// Parse HTML and build layout tree.
    fn parse_and_layout(&self, html: &str) -> Result<LayoutTree, PipelineError> {
        let mut tree = LayoutTree::new();
        let mut parser = SimpleHtmlParser::new(html);

        // Parse and build tree
        self.parse_node(&mut parser, &mut tree, None);

        // Perform layout
        self.layout(&mut tree);

        Ok(tree)
    }

    /// Parse a node recursively.
    fn parse_node(
        &self,
        parser: &mut SimpleHtmlParser,
        tree: &mut LayoutTree,
        parent_id: Option<usize>,
    ) {
        while let Some(token) = parser.next_token() {
            match token {
                HtmlToken::OpenTag {
                    name,
                    attrs,
                    self_closing,
                } => {
                    // Determine box type from tag
                    let (box_type, style) = self.style_for_tag(&name, &attrs);

                    // Skip display:none elements
                    if matches!(box_type, LayoutBoxType::None) {
                        if !self_closing {
                            // Skip until closing tag
                            parser.skip_until_close(&name);
                        }
                        continue;
                    }

                    let node = LayoutNode {
                        box_type,
                        style,
                        dimensions: BoxDimensions::default(),
                        text: None,
                        children: Vec::new(),
                    };

                    let node_id = tree.add(node, parent_id);

                    if !self_closing {
                        // Parse children
                        self.parse_node(parser, tree, Some(node_id));
                    }
                }
                HtmlToken::CloseTag { name } => {
                    // Return to parent
                    return;
                }
                HtmlToken::Text(text) => {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        let node = LayoutNode {
                            box_type: LayoutBoxType::Inline,
                            style: NodeStyle::default(),
                            dimensions: BoxDimensions::default(),
                            text: Some(trimmed.to_string()),
                            children: Vec::new(),
                        };
                        tree.add(node, parent_id);
                    }
                }
                HtmlToken::Eof => break,
            }
        }
    }

    /// Get style for an HTML tag.
    fn style_for_tag(&self, tag: &str, attrs: &[(String, String)]) -> (LayoutBoxType, NodeStyle) {
        let tag_lower = tag.to_ascii_lowercase();
        let mut style = NodeStyle::default();

        // Determine box type
        let box_type = match tag_lower.as_str() {
            // Block elements
            "html" | "body" | "div" | "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul"
            | "ol" | "li" | "article" | "section" | "header" | "footer" | "main" | "nav"
            | "aside" | "pre" | "blockquote" | "form" | "table" | "hr" => LayoutBoxType::Block,
            // Inline elements
            "span" | "a" | "strong" | "em" | "b" | "i" | "u" | "code" | "small" | "br" => {
                LayoutBoxType::Inline
            }
            // Hidden elements
            "head" | "script" | "style" | "meta" | "link" | "title" | "noscript" => {
                LayoutBoxType::None
            }
            // Default to inline
            _ => LayoutBoxType::Inline,
        };

        // Apply default styles
        match tag_lower.as_str() {
            "body" => {
                style.margin = EdgeSizes::uniform(8.0);
            }
            "h1" => {
                style.font_size = 32.0;
                style.font_weight = 700;
                style.margin = EdgeSizes::new(21.0, 0.0, 21.0, 0.0);
            }
            "h2" => {
                style.font_size = 24.0;
                style.font_weight = 700;
                style.margin = EdgeSizes::new(19.0, 0.0, 19.0, 0.0);
            }
            "h3" => {
                style.font_size = 18.0;
                style.font_weight = 700;
                style.margin = EdgeSizes::new(18.0, 0.0, 18.0, 0.0);
            }
            "p" => {
                style.margin = EdgeSizes::new(16.0, 0.0, 16.0, 0.0);
            }
            "strong" | "b" => {
                style.font_weight = 700;
            }
            "em" | "i" => {
                style.italic = true;
            }
            "a" => {
                style.color = PipelineColor::rgb(0, 0, 238); // Blue
                style.underline = true;
            }
            "code" | "pre" => {
                style.background_color = Some(PipelineColor::rgb(240, 240, 240));
            }
            "hr" => {
                style.height = Some(1.0);
                style.background_color = Some(PipelineColor::rgb(128, 128, 128));
                style.margin = EdgeSizes::new(8.0, 0.0, 8.0, 0.0);
            }
            _ => {}
        }

        // Parse inline style attribute
        for (name, value) in attrs {
            if name == "style" {
                self.parse_inline_style(value, &mut style);
            }
        }

        (box_type, style)
    }

    /// Parse inline style attribute.
    fn parse_inline_style(&self, style_str: &str, style: &mut NodeStyle) {
        for decl in style_str.split(';') {
            let decl = decl.trim();
            if decl.is_empty() {
                continue;
            }

            if let Some((prop, val)) = decl.split_once(':') {
                let prop = prop.trim().to_ascii_lowercase();
                let val = val.trim();

                match prop.as_str() {
                    "color" => {
                        if let Some(c) = parse_color(val) {
                            style.color = c;
                        }
                    }
                    "background-color" | "background" => {
                        if let Some(c) = parse_color(val) {
                            style.background_color = Some(c);
                        }
                    }
                    "font-size" => {
                        if let Some(size) = parse_length(val) {
                            style.font_size = size;
                        }
                    }
                    "font-weight" => {
                        if let Some(weight) = parse_font_weight(val) {
                            style.font_weight = weight;
                        }
                    }
                    "width" => {
                        if let Some(w) = parse_length(val) {
                            style.width = Some(w);
                        }
                    }
                    "height" => {
                        if let Some(h) = parse_length(val) {
                            style.height = Some(h);
                        }
                    }
                    "margin" => {
                        if let Some(m) = parse_length(val) {
                            style.margin = EdgeSizes::uniform(m);
                        }
                    }
                    "padding" => {
                        if let Some(p) = parse_length(val) {
                            style.padding = EdgeSizes::uniform(p);
                        }
                    }
                    "display" => {
                        // Note: handled by parse phase
                    }
                    _ => {}
                }
            }
        }
    }

    /// Perform layout on the tree.
    fn layout(&self, tree: &mut LayoutTree) {
        let containing_block = Rect {
            x: 0.0,
            y: 0.0,
            width: self.viewport_width,
            height: self.viewport_height,
        };

        if let Some(root_id) = tree.root_id() {
            self.layout_node(tree, root_id, &containing_block, 0.0);
        }
    }

    /// Layout a single node.
    fn layout_node(
        &self,
        tree: &mut LayoutTree,
        node_id: usize,
        containing_block: &Rect,
        y_offset: f32,
    ) -> f32 {
        let (box_type, style, text, children) = {
            let node = tree.get(node_id).unwrap();
            (
                node.box_type,
                node.style.clone(),
                node.text.clone(),
                node.children.clone(),
            )
        };

        match box_type {
            LayoutBoxType::Block => {
                self.layout_block(tree, node_id, containing_block, y_offset, &style, &children)
            }
            LayoutBoxType::Inline => {
                self.layout_inline(tree, node_id, containing_block, y_offset, &style, &text)
            }
            LayoutBoxType::None => y_offset,
        }
    }

    /// Layout a block element.
    fn layout_block(
        &self,
        tree: &mut LayoutTree,
        node_id: usize,
        containing_block: &Rect,
        y_offset: f32,
        style: &NodeStyle,
        children: &[usize],
    ) -> f32 {
        let node = tree.get_mut(node_id).unwrap();

        // Calculate width
        let width = style.width.unwrap_or(
            containing_block.width
                - style.margin.left
                - style.margin.right
                - style.padding.left
                - style.padding.right
                - style.border.left
                - style.border.right,
        );

        // Set position
        node.dimensions.content.x =
            containing_block.x + style.margin.left + style.border.left + style.padding.left;
        node.dimensions.content.y =
            y_offset + style.margin.top + style.border.top + style.padding.top;
        node.dimensions.content.width = width;

        node.dimensions.margin = style.margin;
        node.dimensions.padding = style.padding;
        node.dimensions.border = style.border;

        // Layout children
        let content_x = node.dimensions.content.x;
        let mut child_y = node.dimensions.content.y;

        for &child_id in children {
            let child_block = Rect {
                x: content_x,
                y: child_y,
                width,
                height: 0.0,
            };
            child_y = self.layout_node(tree, child_id, &child_block, child_y);
        }

        // Calculate height
        let node = tree.get_mut(node_id).unwrap();
        let content_height = style.height.unwrap_or_else(|| {
            if children.is_empty() {
                0.0
            } else {
                child_y - node.dimensions.content.y
            }
        });
        node.dimensions.content.height = content_height;

        // Return the bottom of this box
        node.dimensions.margin_box().y + node.dimensions.margin_box().height
    }

    /// Layout an inline element.
    fn layout_inline(
        &self,
        tree: &mut LayoutTree,
        node_id: usize,
        containing_block: &Rect,
        y_offset: f32,
        style: &NodeStyle,
        text: &Option<String>,
    ) -> f32 {
        let node = tree.get_mut(node_id).unwrap();

        // Calculate text dimensions
        let (width, height) = if let Some(ref t) = text {
            let char_width = style.font_size * 0.6;
            let text_width = (t.len() as f32) * char_width;
            let available_width = containing_block.width;

            let lines = ceilf(text_width / available_width).max(1.0);
            let line_height = style.font_size * 1.4;

            (text_width.min(available_width), lines * line_height)
        } else {
            (0.0, style.font_size * 1.4)
        };

        node.dimensions.content.x = containing_block.x;
        node.dimensions.content.y = y_offset;
        node.dimensions.content.width = width;
        node.dimensions.content.height = height;

        y_offset + height
    }

    /// Generate display list from layout tree.
    fn paint(&self, tree: &LayoutTree) -> DisplayList {
        let mut list = DisplayList::new();

        // Paint background
        list.push(DisplayCommand::SolidRect {
            color: self.background_color.to_layout_color(),
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: self.viewport_width,
                height: self.viewport_height,
            },
        });

        // Paint nodes
        if let Some(root_id) = tree.root_id() {
            self.paint_node(tree, root_id, &mut list);
        }

        list
    }

    /// Paint a single node.
    fn paint_node(&self, tree: &LayoutTree, node_id: usize, list: &mut DisplayList) {
        let node = match tree.get(node_id) {
            Some(n) => n,
            None => return,
        };

        let rect = node.dimensions.border_box();

        // Paint background
        if let Some(bg) = node.style.background_color {
            list.push(DisplayCommand::SolidRect {
                color: bg.to_layout_color(),
                rect,
            });
        }

        // Paint border
        let border = &node.dimensions.border;
        if border.top > 0.0 || border.right > 0.0 || border.bottom > 0.0 || border.left > 0.0 {
            list.push(DisplayCommand::Border {
                color: node
                    .style
                    .border_color
                    .unwrap_or(PipelineColor::black())
                    .to_layout_color(),
                rect,
                widths: BorderWidths {
                    top: border.top,
                    right: border.right,
                    bottom: border.bottom,
                    left: border.left,
                },
                style: BorderStyle::Solid,
            });
        }

        // Paint text
        if let Some(ref text) = node.text {
            list.push(DisplayCommand::Text {
                text: text.clone(),
                rect: node.dimensions.content.clone(),
                style: TextStyle {
                    font_size: node.style.font_size,
                    color: node.style.color.to_layout_color(),
                    font_weight: node.style.font_weight,
                    italic: node.style.italic,
                    underline: node.style.underline,
                    line_through: false,
                },
            });
        }

        // Paint children
        for &child_id in &node.children {
            self.paint_node(tree, child_id, list);
        }
    }

    /// Execute a display command.
    fn execute_command(
        &self,
        command: &DisplayCommand,
        framebuffer: &mut [u32],
        width: u32,
        height: u32,
    ) {
        match command {
            DisplayCommand::SolidRect { color, rect } => {
                self.fill_rect(framebuffer, width, height, rect, *color);
            }
            DisplayCommand::Border {
                color,
                rect,
                widths,
                ..
            } => {
                // Top border
                if widths.top > 0.0 {
                    let top_rect = Rect {
                        x: rect.x,
                        y: rect.y,
                        width: rect.width,
                        height: widths.top,
                    };
                    self.fill_rect(framebuffer, width, height, &top_rect, *color);
                }
                // Bottom border
                if widths.bottom > 0.0 {
                    let bottom_rect = Rect {
                        x: rect.x,
                        y: rect.y + rect.height - widths.bottom,
                        width: rect.width,
                        height: widths.bottom,
                    };
                    self.fill_rect(framebuffer, width, height, &bottom_rect, *color);
                }
                // Left border
                if widths.left > 0.0 {
                    let left_rect = Rect {
                        x: rect.x,
                        y: rect.y,
                        width: widths.left,
                        height: rect.height,
                    };
                    self.fill_rect(framebuffer, width, height, &left_rect, *color);
                }
                // Right border
                if widths.right > 0.0 {
                    let right_rect = Rect {
                        x: rect.x + rect.width - widths.right,
                        y: rect.y,
                        width: widths.right,
                        height: rect.height,
                    };
                    self.fill_rect(framebuffer, width, height, &right_rect, *color);
                }
            }
            DisplayCommand::Text { text, rect, style } => {
                // Simple text rendering (placeholder)
                // Real implementation would use font rasterization
                self.draw_text(framebuffer, width, height, text, rect, style);
            }
            _ => {}
        }
    }

    /// Fill a rectangle in the framebuffer.
    fn fill_rect(
        &self,
        framebuffer: &mut [u32],
        fb_width: u32,
        fb_height: u32,
        rect: &Rect,
        color: LayoutColor,
    ) {
        let x0 = (rect.x as i32).max(0) as u32;
        let y0 = (rect.y as i32).max(0) as u32;
        let x1 = ((rect.x + rect.width) as u32).min(fb_width);
        let y1 = ((rect.y + rect.height) as u32).min(fb_height);

        let pixel = color_to_u32(&color);

        for y in y0..y1 {
            for x in x0..x1 {
                let idx = (y * fb_width + x) as usize;
                if idx < framebuffer.len() {
                    if color.a == 255 {
                        framebuffer[idx] = pixel;
                    } else {
                        // Alpha blending
                        framebuffer[idx] = blend_pixel(framebuffer[idx], pixel, color.a);
                    }
                }
            }
        }
    }

    /// Draw text (simplified).
    fn draw_text(
        &self,
        framebuffer: &mut [u32],
        fb_width: u32,
        fb_height: u32,
        text: &str,
        rect: &Rect,
        style: &TextStyle,
    ) {
        // Simplified text rendering - just draw colored rectangles for each character
        // A real implementation would use a font rasterizer
        let char_width = style.font_size * 0.6;
        let char_height = style.font_size * 0.8;
        let mut x = rect.x;
        let y = rect.y + (rect.height - char_height) / 2.0;

        for ch in text.chars() {
            if ch.is_whitespace() {
                x += char_width;
                continue;
            }

            // Draw a simple rectangle for each character
            let char_rect = Rect {
                x,
                y,
                width: char_width * 0.8,
                height: char_height,
            };
            self.fill_rect(framebuffer, fb_width, fb_height, &char_rect, style.color);

            x += char_width;
            if x > rect.x + rect.width {
                break;
            }
        }

        // Draw underline
        if style.underline {
            let underline_rect = Rect {
                x: rect.x,
                y: rect.y + rect.height - 2.0,
                width: (text.len() as f32) * char_width,
                height: 1.0,
            };
            self.fill_rect(
                framebuffer,
                fb_width,
                fb_height,
                &underline_rect,
                style.color,
            );
        }
    }
}

/// Convert LayoutColor to u32.
fn color_to_u32(color: &LayoutColor) -> u32 {
    ((color.a as u32) << 24) | ((color.r as u32) << 16) | ((color.g as u32) << 8) | (color.b as u32)
}

/// Alpha blend two pixels.
fn blend_pixel(dst: u32, src: u32, alpha: u8) -> u32 {
    let a = alpha as u32;
    let inv_a = 255 - a;

    let dst_r = (dst >> 16) & 0xFF;
    let dst_g = (dst >> 8) & 0xFF;
    let dst_b = dst & 0xFF;

    let src_r = (src >> 16) & 0xFF;
    let src_g = (src >> 8) & 0xFF;
    let src_b = src & 0xFF;

    let r = (src_r * a + dst_r * inv_a) / 255;
    let g = (src_g * a + dst_g * inv_a) / 255;
    let b = (src_b * a + dst_b * inv_a) / 255;

    0xFF000000 | (r << 16) | (g << 8) | b
}

/// Parse a CSS color value.
fn parse_color(s: &str) -> Option<PipelineColor> {
    let s = s.trim().to_ascii_lowercase();

    // Named colors
    match s.as_str() {
        "black" => return Some(PipelineColor::rgb(0, 0, 0)),
        "white" => return Some(PipelineColor::rgb(255, 255, 255)),
        "red" => return Some(PipelineColor::rgb(255, 0, 0)),
        "green" => return Some(PipelineColor::rgb(0, 128, 0)),
        "blue" => return Some(PipelineColor::rgb(0, 0, 255)),
        "yellow" => return Some(PipelineColor::rgb(255, 255, 0)),
        "cyan" => return Some(PipelineColor::rgb(0, 255, 255)),
        "magenta" => return Some(PipelineColor::rgb(255, 0, 255)),
        "gray" | "grey" => return Some(PipelineColor::rgb(128, 128, 128)),
        "orange" => return Some(PipelineColor::rgb(255, 165, 0)),
        "purple" => return Some(PipelineColor::rgb(128, 0, 128)),
        "transparent" => return Some(PipelineColor::transparent()),
        _ => {}
    }

    // Hex colors
    if s.starts_with('#') {
        let hex = &s[1..];
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(PipelineColor::rgb(r, g, b));
        } else if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            return Some(PipelineColor::rgb(r, g, b));
        }
    }

    // rgb() / rgba()
    if s.starts_with("rgb") {
        // Parse rgb(r, g, b) or rgba(r, g, b, a)
        let start = s.find('(')?;
        let end = s.find(')')?;
        let values: Vec<&str> = s[start + 1..end].split(',').collect();

        if values.len() >= 3 {
            let r: u8 = values[0].trim().parse().ok()?;
            let g: u8 = values[1].trim().parse().ok()?;
            let b: u8 = values[2].trim().parse().ok()?;
            let a: u8 = if values.len() >= 4 {
                let a_f: f32 = values[3].trim().parse().ok()?;
                (a_f * 255.0) as u8
            } else {
                255
            };
            return Some(PipelineColor::new(r, g, b, a));
        }
    }

    None
}

/// Parse a CSS length value.
fn parse_length(s: &str) -> Option<f32> {
    let s = s.trim().to_ascii_lowercase();

    if s.ends_with("px") {
        s[..s.len() - 2].trim().parse().ok()
    } else if s.ends_with("em") {
        // Convert em to px (assuming 16px base)
        s[..s.len() - 2]
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| v * 16.0)
    } else if s.ends_with("rem") {
        s[..s.len() - 3]
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| v * 16.0)
    } else if s.ends_with('%') {
        // Percentage - can't resolve without context
        None
    } else {
        // Try parsing as plain number (treated as px)
        s.parse().ok()
    }
}

/// Parse font-weight.
fn parse_font_weight(s: &str) -> Option<u16> {
    match s.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(400),
        "bold" => Some(700),
        "lighter" => Some(300),
        "bolder" => Some(800),
        _ => s.trim().parse().ok(),
    }
}

/// Pipeline error types.
#[derive(Debug, Clone)]
pub enum PipelineError {
    /// HTML parsing failed.
    ParseError(String),
    /// Layout error.
    LayoutError(String),
    /// Rendering error.
    RenderError(String),
}

impl core::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PipelineError::ParseError(s) => write!(f, "Parse error: {}", s),
            PipelineError::LayoutError(s) => write!(f, "Layout error: {}", s),
            PipelineError::RenderError(s) => write!(f, "Render error: {}", s),
        }
    }
}

// ============================================================================
// Layout Tree
// ============================================================================

/// Layout box type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutBoxType {
    Block,
    Inline,
    None,
}

/// Node style.
#[derive(Debug, Clone)]
pub struct NodeStyle {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub margin: EdgeSizes,
    pub padding: EdgeSizes,
    pub border: EdgeSizes,
    pub color: PipelineColor,
    pub background_color: Option<PipelineColor>,
    pub border_color: Option<PipelineColor>,
    pub font_size: f32,
    pub font_weight: u16,
    pub italic: bool,
    pub underline: bool,
}

impl Default for NodeStyle {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            margin: EdgeSizes::zero(),
            padding: EdgeSizes::zero(),
            border: EdgeSizes::zero(),
            color: PipelineColor::black(),
            background_color: None,
            border_color: None,
            font_size: 16.0,
            font_weight: 400,
            italic: false,
            underline: false,
        }
    }
}

/// Layout node.
#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub box_type: LayoutBoxType,
    pub style: NodeStyle,
    pub dimensions: BoxDimensions,
    pub text: Option<String>,
    pub children: Vec<usize>,
}

/// Layout tree.
pub struct LayoutTree {
    nodes: Vec<LayoutNode>,
    root: Option<usize>,
}

impl LayoutTree {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            root: None,
        }
    }

    pub fn add(&mut self, mut node: LayoutNode, parent: Option<usize>) -> usize {
        let id = self.nodes.len();
        self.nodes.push(node);

        if self.root.is_none() {
            self.root = Some(id);
        }

        if let Some(pid) = parent {
            if let Some(p) = self.nodes.get_mut(pid) {
                p.children.push(id);
            }
        }

        id
    }

    pub fn root_id(&self) -> Option<usize> {
        self.root
    }

    pub fn get(&self, id: usize) -> Option<&LayoutNode> {
        self.nodes.get(id)
    }

    pub fn get_mut(&mut self, id: usize) -> Option<&mut LayoutNode> {
        self.nodes.get_mut(id)
    }
}

// ============================================================================
// Simple HTML Parser
// ============================================================================

/// HTML token.
#[derive(Debug)]
enum HtmlToken {
    OpenTag {
        name: String,
        attrs: Vec<(String, String)>,
        self_closing: bool,
    },
    CloseTag {
        name: String,
    },
    Text(String),
    Eof,
}

/// Simple HTML parser.
struct SimpleHtmlParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> SimpleHtmlParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn next_token(&mut self) -> Option<HtmlToken> {
        self.skip_whitespace();

        if self.pos >= self.input.len() {
            return Some(HtmlToken::Eof);
        }

        if self.starts_with("<!--") {
            // Skip comment
            if let Some(end) = self.input[self.pos..].find("-->") {
                self.pos += end + 3;
                return self.next_token();
            }
        }

        if self.starts_with("<!") {
            // Skip doctype
            if let Some(end) = self.input[self.pos..].find('>') {
                self.pos += end + 1;
                return self.next_token();
            }
        }

        if self.starts_with("</") {
            // Close tag
            self.pos += 2;
            let name = self.read_tag_name();
            self.skip_until('>');
            self.pos += 1;
            return Some(HtmlToken::CloseTag { name });
        }

        if self.starts_with("<") {
            // Open tag
            self.pos += 1;
            let name = self.read_tag_name();
            let attrs = self.read_attributes();
            let self_closing = self.consume_if("/");
            self.skip_until('>');
            self.pos += 1;

            // Void elements
            let void_elements = [
                "br", "hr", "img", "input", "meta", "link", "area", "base", "col", "embed",
                "param", "source", "track", "wbr",
            ];
            let is_void = void_elements.contains(&name.to_ascii_lowercase().as_str());

            return Some(HtmlToken::OpenTag {
                name,
                attrs,
                self_closing: self_closing || is_void,
            });
        }

        // Text content
        let start = self.pos;
        while self.pos < self.input.len() && !self.starts_with("<") {
            self.pos += 1;
        }

        if self.pos > start {
            let text = self.input[start..self.pos].to_string();
            Some(HtmlToken::Text(text))
        } else {
            Some(HtmlToken::Eof)
        }
    }

    fn skip_until_close(&mut self, tag: &str) {
        let close_tag = alloc::format!("</{}", tag.to_ascii_lowercase());
        while self.pos < self.input.len() {
            if self.input[self.pos..]
                .to_ascii_lowercase()
                .starts_with(&close_tag)
            {
                // Consume the closing tag
                self.pos += close_tag.len();
                self.skip_until('>');
                self.pos += 1;
                return;
            }
            self.pos += 1;
        }
    }

    fn starts_with(&self, s: &str) -> bool {
        self.input[self.pos..].starts_with(s)
    }

    fn consume_if(&mut self, s: &str) -> bool {
        if self.starts_with(s) {
            self.pos += s.len();
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            let ch = self.input[self.pos..].chars().next().unwrap();
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn skip_until(&mut self, ch: char) {
        while self.pos < self.input.len() {
            if self.input[self.pos..].starts_with(ch) {
                return;
            }
            self.pos += 1;
        }
    }

    fn read_tag_name(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.input.len() {
            let ch = self.input[self.pos..].chars().next().unwrap();
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        self.input[start..self.pos].to_string()
    }

    fn read_attributes(&mut self) -> Vec<(String, String)> {
        let mut attrs = Vec::new();

        loop {
            self.skip_whitespace();

            if self.pos >= self.input.len() || self.starts_with(">") || self.starts_with("/>") {
                break;
            }

            // Read attribute name
            let name = self.read_attr_name();
            if name.is_empty() {
                break;
            }

            self.skip_whitespace();

            // Check for =
            if self.consume_if("=") {
                self.skip_whitespace();
                let value = self.read_attr_value();
                attrs.push((name, value));
            } else {
                // Boolean attribute
                attrs.push((name, String::new()));
            }
        }

        attrs
    }

    fn read_attr_name(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.input.len() {
            let ch = self.input[self.pos..].chars().next().unwrap();
            if ch.is_alphanumeric() || ch == '-' || ch == '_' || ch == ':' {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        self.input[start..self.pos].to_string()
    }

    fn read_attr_value(&mut self) -> String {
        if self.starts_with("\"") {
            self.pos += 1;
            let start = self.pos;
            while self.pos < self.input.len() && !self.starts_with("\"") {
                self.pos += 1;
            }
            let value = self.input[start..self.pos].to_string();
            if self.starts_with("\"") {
                self.pos += 1;
            }
            value
        } else if self.starts_with("'") {
            self.pos += 1;
            let start = self.pos;
            while self.pos < self.input.len() && !self.starts_with("'") {
                self.pos += 1;
            }
            let value = self.input[start..self.pos].to_string();
            if self.starts_with("'") {
                self.pos += 1;
            }
            value
        } else {
            // Unquoted value
            let start = self.pos;
            while self.pos < self.input.len() {
                let ch = self.input[self.pos..].chars().next().unwrap();
                if ch.is_whitespace() || ch == '>' || ch == '/' {
                    break;
                }
                self.pos += ch.len_utf8();
            }
            self.input[start..self.pos].to_string()
        }
    }
}
