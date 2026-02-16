//! CSS Box Model Implementation
//!
//! This module implements the CSS box model which defines how elements
//! are rendered as rectangular boxes with content, padding, border, and margin.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │                      Margin                          │
//! │  ┌───────────────────────────────────────────────┐  │
//! │  │                    Border                      │  │
//! │  │  ┌─────────────────────────────────────────┐  │  │
//! │  │  │                 Padding                  │  │  │
//! │  │  │  ┌───────────────────────────────────┐  │  │  │
//! │  │  │  │             Content               │  │  │  │
//! │  │  │  │                                   │  │  │  │
//! │  │  │  └───────────────────────────────────┘  │  │  │
//! │  │  │                                         │  │  │
//! │  │  └─────────────────────────────────────────┘  │  │
//! │  │                                               │  │
//! │  └───────────────────────────────────────────────┘  │
//! │                                                      │
//! └─────────────────────────────────────────────────────┘
//! ```

use core::ops::{Add, Sub};

/// A 2D point with x and y coordinates
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

impl Add for Point {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl Sub for Point {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

/// A 2D size with width and height
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub fn zero() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
        }
    }
}

/// A rectangle defined by position (top-left) and size
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn from_point_size(point: Point, size: Size) -> Self {
        Self {
            x: point.x,
            y: point.y,
            width: size.width,
            height: size.height,
        }
    }

    /// Get the position (top-left corner)
    pub fn position(&self) -> Point {
        Point::new(self.x, self.y)
    }

    /// Get the size
    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    /// Get the right edge (x + width)
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Get the bottom edge (y + height)
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Get the center point
    pub fn center(&self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Check if this rect contains a point
    pub fn contains_point(&self, point: Point) -> bool {
        point.x >= self.x && point.x < self.right() && point.y >= self.y && point.y < self.bottom()
    }

    /// Check if this rect intersects with another
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Expand this rect by the given amount on all sides
    pub fn expand(&self, amount: f32) -> Self {
        Self {
            x: self.x - amount,
            y: self.y - amount,
            width: self.width + amount * 2.0,
            height: self.height + amount * 2.0,
        }
    }

    /// Expand this rect by edge sizes
    pub fn expand_by(&self, edges: EdgeSizes) -> Self {
        Self {
            x: self.x - edges.left,
            y: self.y - edges.top,
            width: self.width + edges.left + edges.right,
            height: self.height + edges.top + edges.bottom,
        }
    }
}

/// Edge sizes for margin, padding, and border
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EdgeSizes {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl EdgeSizes {
    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub fn zero() -> Self {
        Self {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        }
    }

    pub fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Total horizontal size (left + right)
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// Total vertical size (top + bottom)
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

impl Add for EdgeSizes {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            top: self.top + other.top,
            right: self.right + other.right,
            bottom: self.bottom + other.bottom,
            left: self.left + other.left,
        }
    }
}

/// Complete CSS box dimensions
///
/// This represents all the dimensions of a CSS box:
/// - Content box: The actual content area
/// - Padding box: Content + padding
/// - Border box: Content + padding + border  
/// - Margin box: Content + padding + border + margin
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoxDimensions {
    /// The content area rectangle
    pub content: Rect,

    /// Padding around the content
    pub padding: EdgeSizes,

    /// Border around the padding
    pub border: EdgeSizes,

    /// Margin around the border
    pub margin: EdgeSizes,
}

impl BoxDimensions {
    /// Create new box dimensions with zero values
    pub fn zero() -> Self {
        Self::default()
    }

    /// Create box dimensions with just content size
    pub fn from_content(content: Rect) -> Self {
        Self {
            content,
            padding: EdgeSizes::zero(),
            border: EdgeSizes::zero(),
            margin: EdgeSizes::zero(),
        }
    }

    /// Get the padding box (content + padding)
    pub fn padding_box(&self) -> Rect {
        self.content.expand_by(self.padding)
    }

    /// Get the border box (content + padding + border)
    pub fn border_box(&self) -> Rect {
        self.padding_box().expand_by(self.border)
    }

    /// Get the margin box (content + padding + border + margin)
    pub fn margin_box(&self) -> Rect {
        self.border_box().expand_by(self.margin)
    }

    /// Total width including all edges
    pub fn total_width(&self) -> f32 {
        self.content.width
            + self.padding.horizontal()
            + self.border.horizontal()
            + self.margin.horizontal()
    }

    /// Total height including all edges
    pub fn total_height(&self) -> f32 {
        self.content.height
            + self.padding.vertical()
            + self.border.vertical()
            + self.margin.vertical()
    }

    /// Set content width
    pub fn set_content_width(&mut self, width: f32) {
        self.content.width = width;
    }

    /// Set content height
    pub fn set_content_height(&mut self, height: f32) {
        self.content.height = height;
    }

    /// Set content position
    pub fn set_content_position(&mut self, x: f32, y: f32) {
        self.content.x = x;
        self.content.y = y;
    }
}

/// Resolved length value in pixels
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResolvedLength {
    /// A definite length in pixels
    Px(f32),
    /// Auto - to be determined by layout algorithm
    Auto,
}

impl ResolvedLength {
    /// Get the pixel value, or 0.0 if auto
    pub fn to_px(&self) -> f32 {
        match self {
            ResolvedLength::Px(v) => *v,
            ResolvedLength::Auto => 0.0,
        }
    }

    /// Check if this is auto
    pub fn is_auto(&self) -> bool {
        matches!(self, ResolvedLength::Auto)
    }
}

impl Default for ResolvedLength {
    fn default() -> Self {
        ResolvedLength::Auto
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_contains() {
        let rect = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert!(rect.contains_point(Point::new(50.0, 40.0)));
        assert!(!rect.contains_point(Point::new(5.0, 40.0)));
    }

    #[test]
    fn test_box_dimensions() {
        let mut dims = BoxDimensions::zero();
        dims.content = Rect::new(0.0, 0.0, 100.0, 50.0);
        dims.padding = EdgeSizes::uniform(10.0);
        dims.border = EdgeSizes::uniform(1.0);
        dims.margin = EdgeSizes::uniform(5.0);

        // Content: 100x50
        // Padding box: 120x70 (content + 10*2 each side)
        // Border box: 122x72 (padding box + 1*2 each side)
        // Margin box: 132x82 (border box + 5*2 each side)

        assert_eq!(dims.padding_box().width, 120.0);
        assert_eq!(dims.border_box().width, 122.0);
        assert_eq!(dims.margin_box().width, 132.0);
    }
}
