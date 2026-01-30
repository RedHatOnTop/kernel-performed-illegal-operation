//! Layout System
//!
//! Flexbox-inspired layout primitives for KPIO Browser UI.

use alloc::vec::Vec;
use super::tokens::spacing;

/// Flex container
#[derive(Debug, Clone)]
pub struct Flex {
    /// Flex direction
    pub direction: FlexDirection,
    /// Main axis alignment
    pub justify: JustifyContent,
    /// Cross axis alignment
    pub align: AlignItems,
    /// Wrap behavior
    pub wrap: FlexWrap,
    /// Gap between items
    pub gap: u32,
    /// Padding
    pub padding: EdgeInsets,
}

impl Flex {
    /// Create row flex
    pub fn row() -> Self {
        Self {
            direction: FlexDirection::Row,
            justify: JustifyContent::Start,
            align: AlignItems::Stretch,
            wrap: FlexWrap::NoWrap,
            gap: 0,
            padding: EdgeInsets::zero(),
        }
    }

    /// Create column flex
    pub fn column() -> Self {
        Self {
            direction: FlexDirection::Column,
            justify: JustifyContent::Start,
            align: AlignItems::Stretch,
            wrap: FlexWrap::NoWrap,
            gap: 0,
            padding: EdgeInsets::zero(),
        }
    }

    /// Set gap
    pub fn gap(mut self, gap: u32) -> Self {
        self.gap = gap;
        self
    }

    /// Set justify content
    pub fn justify(mut self, justify: JustifyContent) -> Self {
        self.justify = justify;
        self
    }

    /// Set align items
    pub fn align(mut self, align: AlignItems) -> Self {
        self.align = align;
        self
    }

    /// Center both axes
    pub fn center(mut self) -> Self {
        self.justify = JustifyContent::Center;
        self.align = AlignItems::Center;
        self
    }

    /// Set padding
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = padding;
        self
    }
}

/// Flex direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

/// Justify content (main axis)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Align items (cross axis)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    Start,
    End,
    Center,
    Baseline,
    #[default]
    Stretch,
}

/// Flex wrap
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

/// Edge insets (padding/margin)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EdgeInsets {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

impl EdgeInsets {
    /// Create new edge insets
    pub const fn new(top: u32, right: u32, bottom: u32, left: u32) -> Self {
        Self { top, right, bottom, left }
    }

    /// Zero insets
    pub const fn zero() -> Self {
        Self::new(0, 0, 0, 0)
    }

    /// All sides equal
    pub const fn all(value: u32) -> Self {
        Self::new(value, value, value, value)
    }

    /// Symmetric insets
    pub const fn symmetric(vertical: u32, horizontal: u32) -> Self {
        Self::new(vertical, horizontal, vertical, horizontal)
    }

    /// Only top
    pub const fn only_top(value: u32) -> Self {
        Self::new(value, 0, 0, 0)
    }

    /// Only bottom
    pub const fn only_bottom(value: u32) -> Self {
        Self::new(0, 0, value, 0)
    }

    /// Horizontal only
    pub const fn horizontal(value: u32) -> Self {
        Self::new(0, value, 0, value)
    }

    /// Vertical only
    pub const fn vertical(value: u32) -> Self {
        Self::new(value, 0, value, 0)
    }

    /// Get total horizontal
    pub const fn total_horizontal(&self) -> u32 {
        self.left + self.right
    }

    /// Get total vertical
    pub const fn total_vertical(&self) -> u32 {
        self.top + self.bottom
    }
}

/// Stack layout (overlapping items)
#[derive(Debug, Clone)]
pub struct Stack {
    /// Alignment
    pub alignment: Alignment,
    /// Clip children
    pub clip: bool,
}

impl Stack {
    /// Create new stack
    pub fn new() -> Self {
        Self {
            alignment: Alignment::TopLeft,
            clip: true,
        }
    }

    /// Set alignment
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}

/// Alignment for positioned elements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Alignment {
    /// Get horizontal factor (0.0 = left, 1.0 = right)
    pub fn horizontal_factor(&self) -> f32 {
        match self {
            Self::TopLeft | Self::CenterLeft | Self::BottomLeft => 0.0,
            Self::TopCenter | Self::Center | Self::BottomCenter => 0.5,
            Self::TopRight | Self::CenterRight | Self::BottomRight => 1.0,
        }
    }

    /// Get vertical factor (0.0 = top, 1.0 = bottom)
    pub fn vertical_factor(&self) -> f32 {
        match self {
            Self::TopLeft | Self::TopCenter | Self::TopRight => 0.0,
            Self::CenterLeft | Self::Center | Self::CenterRight => 0.5,
            Self::BottomLeft | Self::BottomCenter | Self::BottomRight => 1.0,
        }
    }
}

/// Grid layout
#[derive(Debug, Clone)]
pub struct Grid {
    /// Number of columns
    pub columns: usize,
    /// Column gap
    pub column_gap: u32,
    /// Row gap
    pub row_gap: u32,
    /// Padding
    pub padding: EdgeInsets,
}

impl Grid {
    /// Create new grid
    pub fn new(columns: usize) -> Self {
        Self {
            columns: columns.max(1),
            column_gap: spacing::MD,
            row_gap: spacing::MD,
            padding: EdgeInsets::zero(),
        }
    }

    /// Set gap (both)
    pub fn gap(mut self, gap: u32) -> Self {
        self.column_gap = gap;
        self.row_gap = gap;
        self
    }

    /// Set padding
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = padding;
        self
    }
}

/// Constraint box
#[derive(Debug, Clone, Copy)]
pub struct Constraints {
    /// Minimum width
    pub min_width: u32,
    /// Maximum width
    pub max_width: u32,
    /// Minimum height
    pub min_height: u32,
    /// Maximum height
    pub max_height: u32,
}

impl Constraints {
    /// Unconstrained
    pub const fn unbounded() -> Self {
        Self {
            min_width: 0,
            max_width: u32::MAX,
            min_height: 0,
            max_height: u32::MAX,
        }
    }

    /// Tight constraints (exact size)
    pub const fn tight(width: u32, height: u32) -> Self {
        Self {
            min_width: width,
            max_width: width,
            min_height: height,
            max_height: height,
        }
    }

    /// Expand to fill
    pub const fn expand() -> Self {
        Self {
            min_width: u32::MAX,
            max_width: u32::MAX,
            min_height: u32::MAX,
            max_height: u32::MAX,
        }
    }

    /// Constrain a size
    pub fn constrain(&self, width: u32, height: u32) -> (u32, u32) {
        (
            width.max(self.min_width).min(self.max_width),
            height.max(self.min_height).min(self.max_height),
        )
    }
}

impl Default for Constraints {
    fn default() -> Self {
        Self::unbounded()
    }
}

/// Positioned element
#[derive(Debug, Clone, Copy)]
pub struct Positioned {
    /// Left offset
    pub left: Option<i32>,
    /// Top offset
    pub top: Option<i32>,
    /// Right offset
    pub right: Option<i32>,
    /// Bottom offset
    pub bottom: Option<i32>,
    /// Width
    pub width: Option<u32>,
    /// Height
    pub height: Option<u32>,
}

impl Positioned {
    /// Create from top-left
    pub fn from_top_left(left: i32, top: i32) -> Self {
        Self {
            left: Some(left),
            top: Some(top),
            right: None,
            bottom: None,
            width: None,
            height: None,
        }
    }

    /// Fill parent
    pub fn fill() -> Self {
        Self {
            left: Some(0),
            top: Some(0),
            right: Some(0),
            bottom: Some(0),
            width: None,
            height: None,
        }
    }
}

/// Scrollable container
#[derive(Debug, Clone)]
pub struct Scrollable {
    /// Horizontal scroll enabled
    pub horizontal: bool,
    /// Vertical scroll enabled
    pub vertical: bool,
    /// Always show scrollbar
    pub always_show: bool,
    /// Scroll position X
    pub scroll_x: f32,
    /// Scroll position Y
    pub scroll_y: f32,
}

impl Scrollable {
    /// Vertical scroll only
    pub fn vertical() -> Self {
        Self {
            horizontal: false,
            vertical: true,
            always_show: false,
            scroll_x: 0.0,
            scroll_y: 0.0,
        }
    }

    /// Horizontal scroll only
    pub fn horizontal() -> Self {
        Self {
            horizontal: true,
            vertical: false,
            always_show: false,
            scroll_x: 0.0,
            scroll_y: 0.0,
        }
    }

    /// Both directions
    pub fn both() -> Self {
        Self {
            horizontal: true,
            vertical: true,
            always_show: false,
            scroll_x: 0.0,
            scroll_y: 0.0,
        }
    }
}

/// Divider
#[derive(Debug, Clone, Copy)]
pub struct Divider {
    /// Thickness
    pub thickness: u32,
    /// Orientation
    pub vertical: bool,
    /// Margin
    pub margin: u32,
}

impl Divider {
    /// Horizontal divider
    pub fn horizontal() -> Self {
        Self {
            thickness: 1,
            vertical: false,
            margin: spacing::SM,
        }
    }

    /// Vertical divider
    pub fn vertical() -> Self {
        Self {
            thickness: 1,
            vertical: true,
            margin: spacing::SM,
        }
    }
}

/// Spacer (flexible space)
#[derive(Debug, Clone, Copy)]
pub struct Spacer {
    /// Flex value
    pub flex: u32,
}

impl Spacer {
    /// Create new spacer
    pub fn new() -> Self {
        Self { flex: 1 }
    }

    /// Create spacer with flex
    pub fn flex(flex: u32) -> Self {
        Self { flex }
    }
}

impl Default for Spacer {
    fn default() -> Self {
        Self::new()
    }
}
