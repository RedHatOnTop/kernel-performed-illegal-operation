//! CSS Values - Fundamental CSS value types

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// A CSS value that can be applied to a property.
#[derive(Debug, Clone, PartialEq)]
pub enum CssValue {
    /// A keyword value (e.g., `auto`, `none`, `inherit`)
    Keyword(String),
    /// A length value
    Length(Length),
    /// A percentage value
    Percentage(f32),
    /// A color value
    Color(Color),
    /// A number value
    Number(f32),
    /// An integer value
    Integer(i32),
    /// A string value
    String(String),
    /// A URL value
    Url(String),
    /// A time value (in seconds)
    Time(f32),
    /// An angle value (in degrees)
    Angle(f32),
    /// A resolution value (in dppx)
    Resolution(f32),
    /// A list of values
    List(Vec<CssValue>),
    /// A function call (name, arguments)
    Function(String, Vec<CssValue>),
    /// Initial value
    Initial,
    /// Inherit from parent
    Inherit,
    /// Unset (initial or inherit depending on property)
    Unset,
    /// Revert to user-agent style
    Revert,
}

impl Default for CssValue {
    fn default() -> Self {
        CssValue::Initial
    }
}

// ============================================================================
// Length
// ============================================================================

/// A CSS length value with unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Length {
    pub value: f32,
    pub unit: LengthUnit,
}

impl Length {
    /// Create a new length.
    pub fn new(value: f32, unit: LengthUnit) -> Self {
        Length { value, unit }
    }

    /// Zero length.
    pub fn zero() -> Self {
        Length {
            value: 0.0,
            unit: LengthUnit::Px,
        }
    }

    /// Create a pixel length.
    pub fn px(value: f32) -> Self {
        Length {
            value,
            unit: LengthUnit::Px,
        }
    }

    /// Create an em length.
    pub fn em(value: f32) -> Self {
        Length {
            value,
            unit: LengthUnit::Em,
        }
    }

    /// Create a rem length.
    pub fn rem(value: f32) -> Self {
        Length {
            value,
            unit: LengthUnit::Rem,
        }
    }

    /// Create a percentage-based length.
    pub fn percent(value: f32) -> Self {
        Length {
            value,
            unit: LengthUnit::Percent,
        }
    }

    /// Convert to pixels given a context.
    pub fn to_px(&self, context: &LengthContext) -> f32 {
        match self.unit {
            LengthUnit::Px => self.value,
            LengthUnit::Em => self.value * context.font_size,
            LengthUnit::Rem => self.value * context.root_font_size,
            LengthUnit::Ex => self.value * context.font_size * 0.5, // Approximation
            LengthUnit::Ch => self.value * context.font_size * 0.5, // Approximation
            LengthUnit::Vw => self.value * context.viewport_width / 100.0,
            LengthUnit::Vh => self.value * context.viewport_height / 100.0,
            LengthUnit::Vmin => {
                self.value * context.viewport_width.min(context.viewport_height) / 100.0
            }
            LengthUnit::Vmax => {
                self.value * context.viewport_width.max(context.viewport_height) / 100.0
            }
            LengthUnit::Percent => self.value * context.containing_block / 100.0,
            LengthUnit::Cm => self.value * 96.0 / 2.54,
            LengthUnit::Mm => self.value * 96.0 / 25.4,
            LengthUnit::In => self.value * 96.0,
            LengthUnit::Pt => self.value * 96.0 / 72.0,
            LengthUnit::Pc => self.value * 96.0 / 6.0,
            LengthUnit::Q => self.value * 96.0 / 101.6,
        }
    }

    /// Check if this is zero.
    pub fn is_zero(&self) -> bool {
        self.value == 0.0
    }
}

impl Default for Length {
    fn default() -> Self {
        Length::zero()
    }
}

impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.value, self.unit)
    }
}

/// Length units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LengthUnit {
    // Absolute units
    Px,
    Cm,
    Mm,
    In,
    Pt,
    Pc,
    Q,
    // Relative units
    Em,
    Rem,
    Ex,
    Ch,
    // Viewport units
    Vw,
    Vh,
    Vmin,
    Vmax,
    // Percentage
    Percent,
}

impl fmt::Display for LengthUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LengthUnit::Px => write!(f, "px"),
            LengthUnit::Cm => write!(f, "cm"),
            LengthUnit::Mm => write!(f, "mm"),
            LengthUnit::In => write!(f, "in"),
            LengthUnit::Pt => write!(f, "pt"),
            LengthUnit::Pc => write!(f, "pc"),
            LengthUnit::Q => write!(f, "Q"),
            LengthUnit::Em => write!(f, "em"),
            LengthUnit::Rem => write!(f, "rem"),
            LengthUnit::Ex => write!(f, "ex"),
            LengthUnit::Ch => write!(f, "ch"),
            LengthUnit::Vw => write!(f, "vw"),
            LengthUnit::Vh => write!(f, "vh"),
            LengthUnit::Vmin => write!(f, "vmin"),
            LengthUnit::Vmax => write!(f, "vmax"),
            LengthUnit::Percent => write!(f, "%"),
        }
    }
}

/// Context for resolving relative lengths.
#[derive(Debug, Clone, Copy)]
pub struct LengthContext {
    pub font_size: f32,
    pub root_font_size: f32,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub containing_block: f32,
}

impl Default for LengthContext {
    fn default() -> Self {
        LengthContext {
            font_size: 16.0,
            root_font_size: 16.0,
            viewport_width: 1920.0,
            viewport_height: 1080.0,
            containing_block: 0.0,
        }
    }
}

// ============================================================================
// Color
// ============================================================================

/// A CSS color value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Create a new color from RGBA values.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }

    /// Create a new color from RGB values (fully opaque).
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }

    /// Create a color from a hex value (e.g., 0xFF0000 for red).
    pub const fn from_hex(hex: u32) -> Self {
        Color {
            r: ((hex >> 16) & 0xFF) as u8,
            g: ((hex >> 8) & 0xFF) as u8,
            b: (hex & 0xFF) as u8,
            a: 255,
        }
    }

    /// Create a color from a hex value with alpha.
    pub const fn from_hex_alpha(hex: u32) -> Self {
        Color {
            r: ((hex >> 24) & 0xFF) as u8,
            g: ((hex >> 16) & 0xFF) as u8,
            b: ((hex >> 8) & 0xFF) as u8,
            a: (hex & 0xFF) as u8,
        }
    }

    /// Convert to a 32-bit RGBA value.
    pub const fn to_rgba32(&self) -> u32 {
        ((self.r as u32) << 24) | ((self.g as u32) << 16) | ((self.b as u32) << 8) | (self.a as u32)
    }

    /// Convert to a 32-bit ARGB value.
    pub const fn to_argb32(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    /// Get the alpha as a float (0.0 - 1.0).
    pub fn alpha_f32(&self) -> f32 {
        self.a as f32 / 255.0
    }

    /// Check if fully transparent.
    pub fn is_transparent(&self) -> bool {
        self.a == 0
    }

    /// Check if fully opaque.
    pub fn is_opaque(&self) -> bool {
        self.a == 255
    }

    // Named colors
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const RED: Color = Color::rgb(255, 0, 0);
    pub const GREEN: Color = Color::rgb(0, 128, 0);
    pub const BLUE: Color = Color::rgb(0, 0, 255);
    pub const YELLOW: Color = Color::rgb(255, 255, 0);
    pub const CYAN: Color = Color::rgb(0, 255, 255);
    pub const MAGENTA: Color = Color::rgb(255, 0, 255);
    pub const GRAY: Color = Color::rgb(128, 128, 128);
    pub const SILVER: Color = Color::rgb(192, 192, 192);
    pub const MAROON: Color = Color::rgb(128, 0, 0);
    pub const OLIVE: Color = Color::rgb(128, 128, 0);
    pub const LIME: Color = Color::rgb(0, 255, 0);
    pub const AQUA: Color = Color::rgb(0, 255, 255);
    pub const TEAL: Color = Color::rgb(0, 128, 128);
    pub const NAVY: Color = Color::rgb(0, 0, 128);
    pub const FUCHSIA: Color = Color::rgb(255, 0, 255);
    pub const PURPLE: Color = Color::rgb(128, 0, 128);
    pub const ORANGE: Color = Color::rgb(255, 165, 0);
    pub const PINK: Color = Color::rgb(255, 192, 203);
}

impl Default for Color {
    fn default() -> Self {
        Color::BLACK
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.a == 255 {
            write!(f, "rgb({}, {}, {})", self.r, self.g, self.b)
        } else {
            write!(
                f,
                "rgba({}, {}, {}, {})",
                self.r,
                self.g,
                self.b,
                self.alpha_f32()
            )
        }
    }
}

// ============================================================================
// Display
// ============================================================================

/// The `display` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Display {
    None,
    #[default]
    Block,
    Inline,
    InlineBlock,
    Flex,
    InlineFlex,
    Grid,
    InlineGrid,
    Table,
    TableRow,
    TableCell,
    TableColumn,
    TableRowGroup,
    TableColumnGroup,
    TableHeaderGroup,
    TableFooterGroup,
    TableCaption,
    ListItem,
    Contents,
    FlowRoot,
}

impl Display {
    /// Check if this creates a block formatting context.
    pub fn is_block_level(&self) -> bool {
        matches!(
            self,
            Display::Block
                | Display::Flex
                | Display::Grid
                | Display::Table
                | Display::ListItem
                | Display::FlowRoot
        )
    }

    /// Check if this is inline-level.
    pub fn is_inline_level(&self) -> bool {
        matches!(
            self,
            Display::Inline | Display::InlineBlock | Display::InlineFlex | Display::InlineGrid
        )
    }

    /// Check if this is a flex container.
    pub fn is_flex(&self) -> bool {
        matches!(self, Display::Flex | Display::InlineFlex)
    }

    /// Check if this is a grid container.
    pub fn is_grid(&self) -> bool {
        matches!(self, Display::Grid | Display::InlineGrid)
    }

    /// Check if this hides the element.
    pub fn is_none(&self) -> bool {
        matches!(self, Display::None)
    }
}

// ============================================================================
// Position
// ============================================================================

/// The `position` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Position {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

impl Position {
    /// Check if this creates a new stacking context.
    pub fn creates_stacking_context(&self) -> bool {
        !matches!(self, Position::Static)
    }

    /// Check if this element is positioned.
    pub fn is_positioned(&self) -> bool {
        !matches!(self, Position::Static)
    }

    /// Check if this is out of normal flow.
    pub fn is_out_of_flow(&self) -> bool {
        matches!(self, Position::Absolute | Position::Fixed)
    }
}

// ============================================================================
// Box model values
// ============================================================================

/// The `box-sizing` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BoxSizing {
    #[default]
    ContentBox,
    BorderBox,
}

/// The `overflow` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Scroll,
    Auto,
    Clip,
}

/// The `visibility` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Visibility {
    #[default]
    Visible,
    Hidden,
    Collapse,
}

// ============================================================================
// Flexbox values
// ============================================================================

/// The `flex-direction` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

/// The `flex-wrap` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FlexWrap {
    #[default]
    Nowrap,
    Wrap,
    WrapReverse,
}

/// The `justify-content` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum JustifyContent {
    #[default]
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    Start,
    End,
}

/// The `align-items` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AlignItems {
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
    #[default]
    Stretch,
    Start,
    End,
}

/// The `align-content` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AlignContent {
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    #[default]
    Stretch,
}

/// The `align-self` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AlignSelf {
    #[default]
    Auto,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
    Stretch,
}

// ============================================================================
// Text values
// ============================================================================

/// The `text-align` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextAlign {
    #[default]
    Start,
    End,
    Left,
    Right,
    Center,
    Justify,
}

/// The `text-decoration-line` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextDecorationLine {
    #[default]
    None,
    Underline,
    Overline,
    LineThrough,
}

/// The `font-weight` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontWeight(pub u16);

impl FontWeight {
    pub const THIN: FontWeight = FontWeight(100);
    pub const EXTRA_LIGHT: FontWeight = FontWeight(200);
    pub const LIGHT: FontWeight = FontWeight(300);
    pub const NORMAL: FontWeight = FontWeight(400);
    pub const MEDIUM: FontWeight = FontWeight(500);
    pub const SEMI_BOLD: FontWeight = FontWeight(600);
    pub const BOLD: FontWeight = FontWeight(700);
    pub const EXTRA_BOLD: FontWeight = FontWeight(800);
    pub const BLACK: FontWeight = FontWeight(900);

    pub fn is_bold(&self) -> bool {
        self.0 >= 700
    }
}

impl Default for FontWeight {
    fn default() -> Self {
        FontWeight::NORMAL
    }
}

/// The `font-style` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

/// The `white-space` property value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WhiteSpace {
    #[default]
    Normal,
    Nowrap,
    Pre,
    PreWrap,
    PreLine,
    BreakSpaces,
}

impl WhiteSpace {
    pub fn preserves_newlines(&self) -> bool {
        matches!(
            self,
            WhiteSpace::Pre | WhiteSpace::PreWrap | WhiteSpace::PreLine | WhiteSpace::BreakSpaces
        )
    }

    pub fn preserves_spaces(&self) -> bool {
        matches!(
            self,
            WhiteSpace::Pre | WhiteSpace::PreWrap | WhiteSpace::BreakSpaces
        )
    }

    pub fn allows_wrap(&self) -> bool {
        !matches!(self, WhiteSpace::Pre | WhiteSpace::Nowrap)
    }
}
