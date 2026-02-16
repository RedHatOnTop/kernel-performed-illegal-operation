//! CSS Properties - Property identifiers and declarations

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::values::{
    AlignContent, AlignItems, AlignSelf, BoxSizing, Color, CssValue, Display, FlexDirection,
    FlexWrap, FontStyle, FontWeight, JustifyContent, Length, Overflow, Position, TextAlign,
    TextDecorationLine, Visibility, WhiteSpace,
};

/// CSS property identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum PropertyId {
    // Box model
    Width,
    Height,
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,
    Margin,
    MarginTop,
    MarginRight,
    MarginBottom,
    MarginLeft,
    Padding,
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
    Border,
    BorderWidth,
    BorderTopWidth,
    BorderRightWidth,
    BorderBottomWidth,
    BorderLeftWidth,
    BorderStyle,
    BorderColor,
    BorderRadius,
    BoxSizing,

    // Layout
    Display,
    Position,
    Top,
    Right,
    Bottom,
    Left,
    Float,
    Clear,
    Overflow,
    OverflowX,
    OverflowY,
    Visibility,
    ZIndex,

    // Flexbox
    FlexDirection,
    FlexWrap,
    FlexFlow,
    JustifyContent,
    AlignItems,
    AlignContent,
    AlignSelf,
    Flex,
    FlexGrow,
    FlexShrink,
    FlexBasis,
    Order,
    Gap,
    RowGap,
    ColumnGap,

    // Grid
    GridTemplateColumns,
    GridTemplateRows,
    GridTemplateAreas,
    GridColumn,
    GridRow,
    GridArea,

    // Typography
    Color,
    FontFamily,
    FontSize,
    FontWeight,
    FontStyle,
    LineHeight,
    TextAlign,
    TextDecoration,
    TextDecorationLine,
    TextDecorationColor,
    TextDecorationStyle,
    TextTransform,
    TextIndent,
    LetterSpacing,
    WordSpacing,
    WhiteSpace,
    WordBreak,
    OverflowWrap,

    // Background
    Background,
    BackgroundColor,
    BackgroundImage,
    BackgroundPosition,
    BackgroundSize,
    BackgroundRepeat,
    BackgroundAttachment,
    BackgroundClip,
    BackgroundOrigin,

    // Effects
    Opacity,
    BoxShadow,
    TextShadow,
    Filter,
    Transform,
    TransformOrigin,
    Transition,
    Animation,

    // Lists
    ListStyle,
    ListStyleType,
    ListStylePosition,
    ListStyleImage,

    // Tables
    TableLayout,
    BorderCollapse,
    BorderSpacing,
    CaptionSide,
    EmptyCells,

    // Outline
    Outline,
    OutlineWidth,
    OutlineStyle,
    OutlineColor,
    OutlineOffset,

    // Cursor & interaction
    Cursor,
    PointerEvents,
    UserSelect,

    // Content
    Content,
    Quotes,
    CounterIncrement,
    CounterReset,

    // Print
    PageBreakBefore,
    PageBreakAfter,
    PageBreakInside,

    // Custom property
    Custom,
}

impl PropertyId {
    /// Get the property name as a string.
    pub fn name(&self) -> &'static str {
        match self {
            PropertyId::Width => "width",
            PropertyId::Height => "height",
            PropertyId::MinWidth => "min-width",
            PropertyId::MinHeight => "min-height",
            PropertyId::MaxWidth => "max-width",
            PropertyId::MaxHeight => "max-height",
            PropertyId::Margin => "margin",
            PropertyId::MarginTop => "margin-top",
            PropertyId::MarginRight => "margin-right",
            PropertyId::MarginBottom => "margin-bottom",
            PropertyId::MarginLeft => "margin-left",
            PropertyId::Padding => "padding",
            PropertyId::PaddingTop => "padding-top",
            PropertyId::PaddingRight => "padding-right",
            PropertyId::PaddingBottom => "padding-bottom",
            PropertyId::PaddingLeft => "padding-left",
            PropertyId::Border => "border",
            PropertyId::BorderWidth => "border-width",
            PropertyId::BorderTopWidth => "border-top-width",
            PropertyId::BorderRightWidth => "border-right-width",
            PropertyId::BorderBottomWidth => "border-bottom-width",
            PropertyId::BorderLeftWidth => "border-left-width",
            PropertyId::BorderStyle => "border-style",
            PropertyId::BorderColor => "border-color",
            PropertyId::BorderRadius => "border-radius",
            PropertyId::BoxSizing => "box-sizing",
            PropertyId::Display => "display",
            PropertyId::Position => "position",
            PropertyId::Top => "top",
            PropertyId::Right => "right",
            PropertyId::Bottom => "bottom",
            PropertyId::Left => "left",
            PropertyId::Float => "float",
            PropertyId::Clear => "clear",
            PropertyId::Overflow => "overflow",
            PropertyId::OverflowX => "overflow-x",
            PropertyId::OverflowY => "overflow-y",
            PropertyId::Visibility => "visibility",
            PropertyId::ZIndex => "z-index",
            PropertyId::FlexDirection => "flex-direction",
            PropertyId::FlexWrap => "flex-wrap",
            PropertyId::FlexFlow => "flex-flow",
            PropertyId::JustifyContent => "justify-content",
            PropertyId::AlignItems => "align-items",
            PropertyId::AlignContent => "align-content",
            PropertyId::AlignSelf => "align-self",
            PropertyId::Flex => "flex",
            PropertyId::FlexGrow => "flex-grow",
            PropertyId::FlexShrink => "flex-shrink",
            PropertyId::FlexBasis => "flex-basis",
            PropertyId::Order => "order",
            PropertyId::Gap => "gap",
            PropertyId::RowGap => "row-gap",
            PropertyId::ColumnGap => "column-gap",
            PropertyId::GridTemplateColumns => "grid-template-columns",
            PropertyId::GridTemplateRows => "grid-template-rows",
            PropertyId::GridTemplateAreas => "grid-template-areas",
            PropertyId::GridColumn => "grid-column",
            PropertyId::GridRow => "grid-row",
            PropertyId::GridArea => "grid-area",
            PropertyId::Color => "color",
            PropertyId::FontFamily => "font-family",
            PropertyId::FontSize => "font-size",
            PropertyId::FontWeight => "font-weight",
            PropertyId::FontStyle => "font-style",
            PropertyId::LineHeight => "line-height",
            PropertyId::TextAlign => "text-align",
            PropertyId::TextDecoration => "text-decoration",
            PropertyId::TextDecorationLine => "text-decoration-line",
            PropertyId::TextDecorationColor => "text-decoration-color",
            PropertyId::TextDecorationStyle => "text-decoration-style",
            PropertyId::TextTransform => "text-transform",
            PropertyId::TextIndent => "text-indent",
            PropertyId::LetterSpacing => "letter-spacing",
            PropertyId::WordSpacing => "word-spacing",
            PropertyId::WhiteSpace => "white-space",
            PropertyId::WordBreak => "word-break",
            PropertyId::OverflowWrap => "overflow-wrap",
            PropertyId::Background => "background",
            PropertyId::BackgroundColor => "background-color",
            PropertyId::BackgroundImage => "background-image",
            PropertyId::BackgroundPosition => "background-position",
            PropertyId::BackgroundSize => "background-size",
            PropertyId::BackgroundRepeat => "background-repeat",
            PropertyId::BackgroundAttachment => "background-attachment",
            PropertyId::BackgroundClip => "background-clip",
            PropertyId::BackgroundOrigin => "background-origin",
            PropertyId::Opacity => "opacity",
            PropertyId::BoxShadow => "box-shadow",
            PropertyId::TextShadow => "text-shadow",
            PropertyId::Filter => "filter",
            PropertyId::Transform => "transform",
            PropertyId::TransformOrigin => "transform-origin",
            PropertyId::Transition => "transition",
            PropertyId::Animation => "animation",
            PropertyId::ListStyle => "list-style",
            PropertyId::ListStyleType => "list-style-type",
            PropertyId::ListStylePosition => "list-style-position",
            PropertyId::ListStyleImage => "list-style-image",
            PropertyId::TableLayout => "table-layout",
            PropertyId::BorderCollapse => "border-collapse",
            PropertyId::BorderSpacing => "border-spacing",
            PropertyId::CaptionSide => "caption-side",
            PropertyId::EmptyCells => "empty-cells",
            PropertyId::Outline => "outline",
            PropertyId::OutlineWidth => "outline-width",
            PropertyId::OutlineStyle => "outline-style",
            PropertyId::OutlineColor => "outline-color",
            PropertyId::OutlineOffset => "outline-offset",
            PropertyId::Cursor => "cursor",
            PropertyId::PointerEvents => "pointer-events",
            PropertyId::UserSelect => "user-select",
            PropertyId::Content => "content",
            PropertyId::Quotes => "quotes",
            PropertyId::CounterIncrement => "counter-increment",
            PropertyId::CounterReset => "counter-reset",
            PropertyId::PageBreakBefore => "page-break-before",
            PropertyId::PageBreakAfter => "page-break-after",
            PropertyId::PageBreakInside => "page-break-inside",
            PropertyId::Custom => "custom",
        }
    }

    /// Check if this property is inherited by default.
    pub fn is_inherited(&self) -> bool {
        matches!(
            self,
            PropertyId::Color
                | PropertyId::FontFamily
                | PropertyId::FontSize
                | PropertyId::FontWeight
                | PropertyId::FontStyle
                | PropertyId::LineHeight
                | PropertyId::TextAlign
                | PropertyId::TextIndent
                | PropertyId::LetterSpacing
                | PropertyId::WordSpacing
                | PropertyId::WhiteSpace
                | PropertyId::WordBreak
                | PropertyId::OverflowWrap
                | PropertyId::Visibility
                | PropertyId::Cursor
                | PropertyId::ListStyle
                | PropertyId::ListStyleType
                | PropertyId::ListStylePosition
                | PropertyId::ListStyleImage
                | PropertyId::Quotes
                | PropertyId::BorderCollapse
                | PropertyId::BorderSpacing
                | PropertyId::CaptionSide
                | PropertyId::EmptyCells
        )
    }

    /// Parse a property name to PropertyId.
    pub fn from_name(name: &str) -> Option<PropertyId> {
        match name {
            "width" => Some(PropertyId::Width),
            "height" => Some(PropertyId::Height),
            "min-width" => Some(PropertyId::MinWidth),
            "min-height" => Some(PropertyId::MinHeight),
            "max-width" => Some(PropertyId::MaxWidth),
            "max-height" => Some(PropertyId::MaxHeight),
            "margin" => Some(PropertyId::Margin),
            "margin-top" => Some(PropertyId::MarginTop),
            "margin-right" => Some(PropertyId::MarginRight),
            "margin-bottom" => Some(PropertyId::MarginBottom),
            "margin-left" => Some(PropertyId::MarginLeft),
            "padding" => Some(PropertyId::Padding),
            "padding-top" => Some(PropertyId::PaddingTop),
            "padding-right" => Some(PropertyId::PaddingRight),
            "padding-bottom" => Some(PropertyId::PaddingBottom),
            "padding-left" => Some(PropertyId::PaddingLeft),
            "border" => Some(PropertyId::Border),
            "border-width" => Some(PropertyId::BorderWidth),
            "border-top-width" => Some(PropertyId::BorderTopWidth),
            "border-right-width" => Some(PropertyId::BorderRightWidth),
            "border-bottom-width" => Some(PropertyId::BorderBottomWidth),
            "border-left-width" => Some(PropertyId::BorderLeftWidth),
            "border-style" => Some(PropertyId::BorderStyle),
            "border-color" => Some(PropertyId::BorderColor),
            "border-radius" => Some(PropertyId::BorderRadius),
            "box-sizing" => Some(PropertyId::BoxSizing),
            "display" => Some(PropertyId::Display),
            "position" => Some(PropertyId::Position),
            "top" => Some(PropertyId::Top),
            "right" => Some(PropertyId::Right),
            "bottom" => Some(PropertyId::Bottom),
            "left" => Some(PropertyId::Left),
            "float" => Some(PropertyId::Float),
            "clear" => Some(PropertyId::Clear),
            "overflow" => Some(PropertyId::Overflow),
            "overflow-x" => Some(PropertyId::OverflowX),
            "overflow-y" => Some(PropertyId::OverflowY),
            "visibility" => Some(PropertyId::Visibility),
            "z-index" => Some(PropertyId::ZIndex),
            "flex-direction" => Some(PropertyId::FlexDirection),
            "flex-wrap" => Some(PropertyId::FlexWrap),
            "flex-flow" => Some(PropertyId::FlexFlow),
            "justify-content" => Some(PropertyId::JustifyContent),
            "align-items" => Some(PropertyId::AlignItems),
            "align-content" => Some(PropertyId::AlignContent),
            "align-self" => Some(PropertyId::AlignSelf),
            "flex" => Some(PropertyId::Flex),
            "flex-grow" => Some(PropertyId::FlexGrow),
            "flex-shrink" => Some(PropertyId::FlexShrink),
            "flex-basis" => Some(PropertyId::FlexBasis),
            "order" => Some(PropertyId::Order),
            "gap" => Some(PropertyId::Gap),
            "row-gap" => Some(PropertyId::RowGap),
            "column-gap" => Some(PropertyId::ColumnGap),
            "color" => Some(PropertyId::Color),
            "font-family" => Some(PropertyId::FontFamily),
            "font-size" => Some(PropertyId::FontSize),
            "font-weight" => Some(PropertyId::FontWeight),
            "font-style" => Some(PropertyId::FontStyle),
            "line-height" => Some(PropertyId::LineHeight),
            "text-align" => Some(PropertyId::TextAlign),
            "text-decoration" => Some(PropertyId::TextDecoration),
            "text-decoration-line" => Some(PropertyId::TextDecorationLine),
            "white-space" => Some(PropertyId::WhiteSpace),
            "background" => Some(PropertyId::Background),
            "background-color" => Some(PropertyId::BackgroundColor),
            "background-image" => Some(PropertyId::BackgroundImage),
            "opacity" => Some(PropertyId::Opacity),
            "transform" => Some(PropertyId::Transform),
            "transition" => Some(PropertyId::Transition),
            "animation" => Some(PropertyId::Animation),
            "cursor" => Some(PropertyId::Cursor),
            "pointer-events" => Some(PropertyId::PointerEvents),
            _ => None,
        }
    }
}

impl fmt::Display for PropertyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A CSS property declaration (property-value pair).
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyDeclaration {
    pub property: PropertyId,
    pub value: CssValue,
    pub important: bool,
}

impl PropertyDeclaration {
    /// Create a new property declaration.
    pub fn new(property: PropertyId, value: CssValue) -> Self {
        PropertyDeclaration {
            property,
            value,
            important: false,
        }
    }

    /// Create an important declaration.
    pub fn important(property: PropertyId, value: CssValue) -> Self {
        PropertyDeclaration {
            property,
            value,
            important: true,
        }
    }

    /// Get the property name.
    pub fn name(&self) -> &'static str {
        self.property.name()
    }
}

impl fmt::Display for PropertyDeclaration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.property.name(), self.value)?;
        if self.important {
            write!(f, " !important")?;
        }
        Ok(())
    }
}

/// A block of property declarations.
#[derive(Debug, Clone, Default)]
pub struct DeclarationBlock {
    pub declarations: Vec<PropertyDeclaration>,
}

impl DeclarationBlock {
    /// Create a new empty declaration block.
    pub fn new() -> Self {
        DeclarationBlock {
            declarations: Vec::new(),
        }
    }

    /// Add a declaration.
    pub fn push(&mut self, decl: PropertyDeclaration) {
        // Remove existing declaration for the same property
        self.declarations.retain(|d| d.property != decl.property);
        self.declarations.push(decl);
    }

    /// Get a declaration by property.
    pub fn get(&self, property: PropertyId) -> Option<&PropertyDeclaration> {
        self.declarations.iter().find(|d| d.property == property)
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    /// Get the number of declarations.
    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    /// Iterate over declarations.
    pub fn iter(&self) -> impl Iterator<Item = &PropertyDeclaration> {
        self.declarations.iter()
    }
}
