//! Layout Box Tree
//!
//! This module defines the layout box tree structure which represents
//! the visual formatting of a document.
//!
//! Each layout box corresponds to a styled DOM node and contains:
//! - Box type (block, inline, anonymous)
//! - Computed dimensions (after layout)
//! - Child layout boxes

use alloc::string::String;
use alloc::vec::Vec;
use kpio_css::computed::ComputedStyle;
use kpio_css::values::{Display, Position};
use kpio_dom::NodeId;

use crate::box_model::{BoxDimensions, Rect, EdgeSizes, ResolvedLength};

/// Type of formatting context for a box
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxType {
    /// Block-level box - stacks vertically
    Block,
    /// Inline-level box - flows horizontally
    Inline,
    /// Anonymous block box (for inline-block mixed content)
    AnonymousBlock,
    /// Anonymous inline box (for text content)
    AnonymousInline,
    /// None - element is not rendered (display: none)
    None,
}

impl BoxType {
    /// Determine box type from display property
    pub fn from_display(display: Display) -> Self {
        match display {
            Display::Block => BoxType::Block,
            Display::Inline => BoxType::Inline,
            Display::InlineBlock => BoxType::Inline, // Treated as inline for flow
            Display::Flex => BoxType::Block, // Flex containers are block-level
            Display::InlineFlex => BoxType::Inline,
            Display::None => BoxType::None,
            Display::Grid => BoxType::Block,
            Display::InlineGrid => BoxType::Inline,
            Display::Table => BoxType::Block,
            Display::TableRow => BoxType::Block,
            Display::TableCell => BoxType::Block,
            Display::TableColumn => BoxType::Block,
            Display::TableRowGroup => BoxType::Block,
            Display::TableColumnGroup => BoxType::Block,
            Display::TableHeaderGroup => BoxType::Block,
            Display::TableFooterGroup => BoxType::Block,
            Display::TableCaption => BoxType::Block,
            Display::ListItem => BoxType::Block,
            Display::Contents => BoxType::None, // Contents don't generate a box
            Display::FlowRoot => BoxType::Block,
        }
    }
    
    pub fn is_block(&self) -> bool {
        matches!(self, BoxType::Block | BoxType::AnonymousBlock)
    }
    
    pub fn is_inline(&self) -> bool {
        matches!(self, BoxType::Inline | BoxType::AnonymousInline)
    }
}

/// Positioning scheme for a box
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Positioning {
    /// Normal flow
    Static,
    /// Relative to normal position
    Relative,
    /// Removed from flow, positioned relative to containing block
    Absolute,
    /// Removed from flow, positioned relative to viewport
    Fixed,
    /// Hybrid of relative and fixed
    Sticky,
}

impl Positioning {
    pub fn from_position(position: Position) -> Self {
        match position {
            Position::Static => Positioning::Static,
            Position::Relative => Positioning::Relative,
            Position::Absolute => Positioning::Absolute,
            Position::Fixed => Positioning::Fixed,
            Position::Sticky => Positioning::Sticky,
        }
    }
    
    /// Check if this positioning takes the element out of normal flow
    pub fn is_out_of_flow(&self) -> bool {
        matches!(self, Positioning::Absolute | Positioning::Fixed)
    }
}

/// Resolved style values needed for layout
#[derive(Debug, Clone, Default)]
pub struct LayoutStyle {
    /// Display type
    pub display: Display,
    
    /// Positioning scheme
    pub position: Position,
    
    /// Width
    pub width: ResolvedLength,
    pub min_width: ResolvedLength,
    pub max_width: ResolvedLength,
    
    /// Height
    pub height: ResolvedLength,
    pub min_height: ResolvedLength,
    pub max_height: ResolvedLength,
    
    /// Margins
    pub margin_top: ResolvedLength,
    pub margin_right: ResolvedLength,
    pub margin_bottom: ResolvedLength,
    pub margin_left: ResolvedLength,
    
    /// Padding
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,
    
    /// Border widths
    pub border_top_width: f32,
    pub border_right_width: f32,
    pub border_bottom_width: f32,
    pub border_left_width: f32,
    
    /// Position offsets (for positioned elements)
    pub top: ResolvedLength,
    pub right: ResolvedLength,
    pub bottom: ResolvedLength,
    pub left: ResolvedLength,
}

impl LayoutStyle {
    /// Create layout style from computed style
    pub fn from_computed(computed: &ComputedStyle) -> Self {
        let ctx = kpio_css::values::LengthContext::default();
        Self {
            display: computed.display,
            position: computed.position,
            width: resolve_optional_length(&computed.width, &ctx),
            min_width: resolve_optional_length(&computed.min_width, &ctx),
            max_width: resolve_optional_length(&computed.max_width, &ctx),
            height: resolve_optional_length(&computed.height, &ctx),
            min_height: resolve_optional_length(&computed.min_height, &ctx),
            max_height: resolve_optional_length(&computed.max_height, &ctx),
            margin_top: resolve_length(&computed.margin_top, &ctx),
            margin_right: resolve_length(&computed.margin_right, &ctx),
            margin_bottom: resolve_length(&computed.margin_bottom, &ctx),
            margin_left: resolve_length(&computed.margin_left, &ctx),
            padding_top: computed.padding_top.to_px(&ctx),
            padding_right: computed.padding_right.to_px(&ctx),
            padding_bottom: computed.padding_bottom.to_px(&ctx),
            padding_left: computed.padding_left.to_px(&ctx),
            border_top_width: computed.border_top_width.to_px(&ctx),
            border_right_width: computed.border_right_width.to_px(&ctx),
            border_bottom_width: computed.border_bottom_width.to_px(&ctx),
            border_left_width: computed.border_left_width.to_px(&ctx),
            top: resolve_optional_length(&computed.top, &ctx),
            right: resolve_optional_length(&computed.right, &ctx),
            bottom: resolve_optional_length(&computed.bottom, &ctx),
            left: resolve_optional_length(&computed.left, &ctx),
        }
    }
    
    /// Get padding as EdgeSizes
    pub fn padding(&self) -> EdgeSizes {
        EdgeSizes::new(
            self.padding_top,
            self.padding_right,
            self.padding_bottom,
            self.padding_left,
        )
    }
    
    /// Get border as EdgeSizes
    pub fn border(&self) -> EdgeSizes {
        EdgeSizes::new(
            self.border_top_width,
            self.border_right_width,
            self.border_bottom_width,
            self.border_left_width,
        )
    }
}

/// Helper to resolve an optional Length to ResolvedLength
fn resolve_optional_length(
    value: &Option<kpio_css::values::Length>,
    ctx: &kpio_css::values::LengthContext,
) -> ResolvedLength {
    match value {
        Some(len) => ResolvedLength::Px(len.to_px(ctx)),
        None => ResolvedLength::Auto,
    }
}

/// Helper to resolve a Length to ResolvedLength
fn resolve_length(
    value: &kpio_css::values::Length,
    ctx: &kpio_css::values::LengthContext,
) -> ResolvedLength {
    // Check if it's an auto-equivalent length (e.g., 0 with no unit could be auto in some contexts)
    // For now, always treat as a pixel value
    ResolvedLength::Px(value.to_px(ctx))
}

/// A layout box in the layout tree
#[derive(Debug)]
pub struct LayoutBox {
    /// The type of this box
    pub box_type: BoxType,
    
    /// Computed dimensions after layout
    pub dimensions: BoxDimensions,
    
    /// Style values needed for layout
    pub style: LayoutStyle,
    
    /// Reference to the original DOM node (if any)
    pub node_id: Option<NodeId>,
    
    /// Text content (for text nodes)
    pub text: Option<String>,
    
    /// Child layout boxes
    pub children: Vec<LayoutBox>,
    
    /// Whether this box establishes a new stacking context
    pub creates_stacking_context: bool,
}

impl LayoutBox {
    /// Create a new layout box
    pub fn new(box_type: BoxType) -> Self {
        Self {
            box_type,
            dimensions: BoxDimensions::zero(),
            style: LayoutStyle::default(),
            node_id: None,
            text: None,
            children: Vec::new(),
            creates_stacking_context: false,
        }
    }
    
    /// Create a block box
    pub fn block() -> Self {
        Self::new(BoxType::Block)
    }
    
    /// Create an inline box
    pub fn inline() -> Self {
        Self::new(BoxType::Inline)
    }
    
    /// Create an anonymous block box
    pub fn anonymous_block() -> Self {
        Self::new(BoxType::AnonymousBlock)
    }
    
    /// Create an anonymous inline box for text
    pub fn anonymous_inline(text: String) -> Self {
        let mut layout_box = Self::new(BoxType::AnonymousInline);
        layout_box.text = Some(text);
        layout_box
    }
    
    /// Create from a styled node
    pub fn from_style(style: &ComputedStyle, node_id: NodeId) -> Self {
        let box_type = BoxType::from_display(style.display);
        let mut layout_box = Self::new(box_type);
        layout_box.style = LayoutStyle::from_computed(style);
        layout_box.node_id = Some(node_id);
        layout_box
    }
    
    /// Add a child box
    pub fn add_child(&mut self, child: LayoutBox) {
        self.children.push(child);
    }
    
    /// Get or create an anonymous block box for inline children
    /// (Used when mixing block and inline children)
    pub fn get_inline_container(&mut self) -> &mut LayoutBox {
        // If last child is an anonymous block, use it
        if let Some(last) = self.children.last() {
            if last.box_type == BoxType::AnonymousBlock {
                return self.children.last_mut().unwrap();
            }
        }
        
        // Otherwise, create a new anonymous block
        self.children.push(LayoutBox::anonymous_block());
        self.children.last_mut().unwrap()
    }
    
    /// Check if this box has block children
    pub fn has_block_children(&self) -> bool {
        self.children.iter().any(|c| c.box_type.is_block())
    }
    
    /// Check if this box has inline children
    pub fn has_inline_children(&self) -> bool {
        self.children.iter().any(|c| c.box_type.is_inline())
    }
    
    /// Get the content rect
    pub fn content_rect(&self) -> Rect {
        self.dimensions.content
    }
    
    /// Get the border box rect
    pub fn border_box(&self) -> Rect {
        self.dimensions.border_box()
    }
    
    /// Get the margin box rect
    pub fn margin_box(&self) -> Rect {
        self.dimensions.margin_box()
    }
}

/// Context for layout calculations
#[derive(Debug, Clone)]
pub struct LayoutContext {
    /// Viewport width
    pub viewport_width: f32,
    /// Viewport height
    pub viewport_height: f32,
    /// Default font size in pixels
    pub default_font_size: f32,
    /// Root font size (for rem units)
    pub root_font_size: f32,
}

impl LayoutContext {
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            viewport_width,
            viewport_height,
            default_font_size: 16.0,
            root_font_size: 16.0,
        }
    }
}

impl Default for LayoutContext {
    fn default() -> Self {
        Self::new(800.0, 600.0)
    }
}

/// Containing block information for layout
#[derive(Debug, Clone, Copy)]
pub struct ContainingBlock {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
}

impl ContainingBlock {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height, x: 0.0, y: 0.0 }
    }
    
    pub fn from_rect(rect: Rect) -> Self {
        Self {
            width: rect.width,
            height: rect.height,
            x: rect.x,
            y: rect.y,
        }
    }
}
