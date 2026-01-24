//! Computed Style - Final computed style values

use crate::properties::PropertyId;
use crate::values::{
    Color, Length, LengthContext, Display, Position, BoxSizing, Overflow, Visibility,
    FlexDirection, FlexWrap, JustifyContent, AlignItems, AlignContent, AlignSelf,
    TextAlign, FontWeight, FontStyle, WhiteSpace,
};
use crate::cascade::CascadedValues;

/// Computed style for an element.
///
/// This contains the final, resolved values for all CSS properties
/// after cascade, inheritance, and value computation.
#[derive(Debug, Clone)]
pub struct ComputedStyle {
    // Box model
    pub display: Display,
    pub position: Position,
    pub box_sizing: BoxSizing,
    
    // Dimensions
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub min_width: Option<Length>,
    pub min_height: Option<Length>,
    pub max_width: Option<Length>,
    pub max_height: Option<Length>,

    // Margin
    pub margin_top: Length,
    pub margin_right: Length,
    pub margin_bottom: Length,
    pub margin_left: Length,

    // Padding
    pub padding_top: Length,
    pub padding_right: Length,
    pub padding_bottom: Length,
    pub padding_left: Length,

    // Border
    pub border_top_width: Length,
    pub border_right_width: Length,
    pub border_bottom_width: Length,
    pub border_left_width: Length,
    pub border_color: Color,

    // Positioning
    pub top: Option<Length>,
    pub right: Option<Length>,
    pub bottom: Option<Length>,
    pub left: Option<Length>,
    pub z_index: Option<i32>,

    // Overflow
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub visibility: Visibility,

    // Flexbox
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_content: AlignContent,
    pub align_self: AlignSelf,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Option<Length>,
    pub order: i32,
    pub gap: Length,
    pub row_gap: Length,
    pub column_gap: Length,

    // Typography
    pub color: Color,
    pub font_size: Length,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub line_height: f32,
    pub text_align: TextAlign,
    pub white_space: WhiteSpace,
    pub letter_spacing: Length,
    pub word_spacing: Length,

    // Background
    pub background_color: Color,

    // Effects
    pub opacity: f32,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        ComputedStyle {
            // Box model
            display: Display::Block,
            position: Position::Static,
            box_sizing: BoxSizing::ContentBox,

            // Dimensions
            width: None,
            height: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,

            // Margin
            margin_top: Length::zero(),
            margin_right: Length::zero(),
            margin_bottom: Length::zero(),
            margin_left: Length::zero(),

            // Padding
            padding_top: Length::zero(),
            padding_right: Length::zero(),
            padding_bottom: Length::zero(),
            padding_left: Length::zero(),

            // Border
            border_top_width: Length::zero(),
            border_right_width: Length::zero(),
            border_bottom_width: Length::zero(),
            border_left_width: Length::zero(),
            border_color: Color::BLACK,

            // Positioning
            top: None,
            right: None,
            bottom: None,
            left: None,
            z_index: None,

            // Overflow
            overflow_x: Overflow::Visible,
            overflow_y: Overflow::Visible,
            visibility: Visibility::Visible,

            // Flexbox
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Nowrap,
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Stretch,
            align_content: AlignContent::Stretch,
            align_self: AlignSelf::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: None,
            order: 0,
            gap: Length::zero(),
            row_gap: Length::zero(),
            column_gap: Length::zero(),

            // Typography
            color: Color::BLACK,
            font_size: Length::px(16.0),
            font_weight: FontWeight::NORMAL,
            font_style: FontStyle::Normal,
            line_height: 1.2,
            text_align: TextAlign::Start,
            white_space: WhiteSpace::Normal,
            letter_spacing: Length::zero(),
            word_spacing: Length::zero(),

            // Background
            background_color: Color::TRANSPARENT,

            // Effects
            opacity: 1.0,
        }
    }
}

impl ComputedStyle {
    /// Create a new computed style with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute style from cascaded values and parent style.
    pub fn compute(
        cascaded: &CascadedValues,
        parent: Option<&ComputedStyle>,
        context: &LengthContext,
    ) -> Self {
        let mut style = Self::default();

        // Inherit from parent for inherited properties
        if let Some(parent) = parent {
            style.inherit_from(parent);
        }

        // Apply cascaded values
        style.apply_cascaded(cascaded, context);

        style
    }

    /// Inherit inherited properties from parent.
    fn inherit_from(&mut self, parent: &ComputedStyle) {
        // Inherited properties
        self.color = parent.color;
        self.font_size = parent.font_size;
        self.font_weight = parent.font_weight;
        self.font_style = parent.font_style;
        self.line_height = parent.line_height;
        self.text_align = parent.text_align;
        self.white_space = parent.white_space;
        self.letter_spacing = parent.letter_spacing;
        self.word_spacing = parent.word_spacing;
        self.visibility = parent.visibility;
    }

    /// Apply cascaded values to the computed style.
    fn apply_cascaded(&mut self, cascaded: &CascadedValues, context: &LengthContext) {
        use crate::values::CssValue;

        for decl in cascaded.iter() {
            match decl.property {
                PropertyId::Display => {
                    if let CssValue::Keyword(ref k) = decl.value {
                        self.display = match k.as_str() {
                            "none" => Display::None,
                            "block" => Display::Block,
                            "inline" => Display::Inline,
                            "inline-block" => Display::InlineBlock,
                            "flex" => Display::Flex,
                            "inline-flex" => Display::InlineFlex,
                            "grid" => Display::Grid,
                            "inline-grid" => Display::InlineGrid,
                            _ => Display::Block,
                        };
                    }
                }
                PropertyId::Position => {
                    if let CssValue::Keyword(ref k) = decl.value {
                        self.position = match k.as_str() {
                            "static" => Position::Static,
                            "relative" => Position::Relative,
                            "absolute" => Position::Absolute,
                            "fixed" => Position::Fixed,
                            "sticky" => Position::Sticky,
                            _ => Position::Static,
                        };
                    }
                }
                PropertyId::Color => {
                    if let CssValue::Color(c) = decl.value {
                        self.color = c;
                    }
                }
                PropertyId::BackgroundColor => {
                    if let CssValue::Color(c) = decl.value {
                        self.background_color = c;
                    }
                }
                PropertyId::Width => {
                    if let CssValue::Length(l) = decl.value {
                        self.width = Some(l);
                    }
                }
                PropertyId::Height => {
                    if let CssValue::Length(l) = decl.value {
                        self.height = Some(l);
                    }
                }
                PropertyId::MarginTop => {
                    if let CssValue::Length(l) = decl.value {
                        self.margin_top = l;
                    }
                }
                PropertyId::MarginRight => {
                    if let CssValue::Length(l) = decl.value {
                        self.margin_right = l;
                    }
                }
                PropertyId::MarginBottom => {
                    if let CssValue::Length(l) = decl.value {
                        self.margin_bottom = l;
                    }
                }
                PropertyId::MarginLeft => {
                    if let CssValue::Length(l) = decl.value {
                        self.margin_left = l;
                    }
                }
                PropertyId::PaddingTop => {
                    if let CssValue::Length(l) = decl.value {
                        self.padding_top = l;
                    }
                }
                PropertyId::PaddingRight => {
                    if let CssValue::Length(l) = decl.value {
                        self.padding_right = l;
                    }
                }
                PropertyId::PaddingBottom => {
                    if let CssValue::Length(l) = decl.value {
                        self.padding_bottom = l;
                    }
                }
                PropertyId::PaddingLeft => {
                    if let CssValue::Length(l) = decl.value {
                        self.padding_left = l;
                    }
                }
                PropertyId::FontSize => {
                    if let CssValue::Length(l) = decl.value {
                        self.font_size = l;
                    }
                }
                PropertyId::FontWeight => {
                    if let CssValue::Integer(w) = decl.value {
                        self.font_weight = FontWeight(w as u16);
                    } else if let CssValue::Keyword(ref k) = decl.value {
                        self.font_weight = match k.as_str() {
                            "normal" => FontWeight::NORMAL,
                            "bold" => FontWeight::BOLD,
                            _ => FontWeight::NORMAL,
                        };
                    }
                }
                PropertyId::LineHeight => {
                    if let CssValue::Number(n) = decl.value {
                        self.line_height = n;
                    }
                }
                PropertyId::Opacity => {
                    if let CssValue::Number(n) = decl.value {
                        self.opacity = n.clamp(0.0, 1.0);
                    }
                }
                PropertyId::FlexDirection => {
                    if let CssValue::Keyword(ref k) = decl.value {
                        self.flex_direction = match k.as_str() {
                            "row" => FlexDirection::Row,
                            "row-reverse" => FlexDirection::RowReverse,
                            "column" => FlexDirection::Column,
                            "column-reverse" => FlexDirection::ColumnReverse,
                            _ => FlexDirection::Row,
                        };
                    }
                }
                PropertyId::FlexWrap => {
                    if let CssValue::Keyword(ref k) = decl.value {
                        self.flex_wrap = match k.as_str() {
                            "nowrap" => FlexWrap::Nowrap,
                            "wrap" => FlexWrap::Wrap,
                            "wrap-reverse" => FlexWrap::WrapReverse,
                            _ => FlexWrap::Nowrap,
                        };
                    }
                }
                PropertyId::JustifyContent => {
                    if let CssValue::Keyword(ref k) = decl.value {
                        self.justify_content = match k.as_str() {
                            "flex-start" | "start" => JustifyContent::FlexStart,
                            "flex-end" | "end" => JustifyContent::FlexEnd,
                            "center" => JustifyContent::Center,
                            "space-between" => JustifyContent::SpaceBetween,
                            "space-around" => JustifyContent::SpaceAround,
                            "space-evenly" => JustifyContent::SpaceEvenly,
                            _ => JustifyContent::FlexStart,
                        };
                    }
                }
                PropertyId::AlignItems => {
                    if let CssValue::Keyword(ref k) = decl.value {
                        self.align_items = match k.as_str() {
                            "flex-start" | "start" => AlignItems::FlexStart,
                            "flex-end" | "end" => AlignItems::FlexEnd,
                            "center" => AlignItems::Center,
                            "baseline" => AlignItems::Baseline,
                            "stretch" => AlignItems::Stretch,
                            _ => AlignItems::Stretch,
                        };
                    }
                }
                PropertyId::FlexGrow => {
                    if let CssValue::Number(n) = decl.value {
                        self.flex_grow = n;
                    } else if let CssValue::Integer(i) = decl.value {
                        self.flex_grow = i as f32;
                    }
                }
                PropertyId::FlexShrink => {
                    if let CssValue::Number(n) = decl.value {
                        self.flex_shrink = n;
                    } else if let CssValue::Integer(i) = decl.value {
                        self.flex_shrink = i as f32;
                    }
                }
                PropertyId::Gap | PropertyId::RowGap => {
                    if let CssValue::Length(l) = decl.value {
                        self.gap = l;
                        self.row_gap = l;
                    }
                }
                PropertyId::ColumnGap => {
                    if let CssValue::Length(l) = decl.value {
                        self.column_gap = l;
                    }
                }
                PropertyId::ZIndex => {
                    if let CssValue::Integer(z) = decl.value {
                        self.z_index = Some(z);
                    }
                }
                _ => {
                    // Other properties not yet handled
                }
            }
        }
    }

    /// Get total margin box width addition.
    pub fn horizontal_margin(&self, context: &LengthContext) -> f32 {
        self.margin_left.to_px(context) + self.margin_right.to_px(context)
    }

    /// Get total margin box height addition.
    pub fn vertical_margin(&self, context: &LengthContext) -> f32 {
        self.margin_top.to_px(context) + self.margin_bottom.to_px(context)
    }

    /// Get total padding box width addition.
    pub fn horizontal_padding(&self, context: &LengthContext) -> f32 {
        self.padding_left.to_px(context) + self.padding_right.to_px(context)
    }

    /// Get total padding box height addition.
    pub fn vertical_padding(&self, context: &LengthContext) -> f32 {
        self.padding_top.to_px(context) + self.padding_bottom.to_px(context)
    }

    /// Get total border box width addition.
    pub fn horizontal_border(&self, context: &LengthContext) -> f32 {
        self.border_left_width.to_px(context) + self.border_right_width.to_px(context)
    }

    /// Get total border box height addition.
    pub fn vertical_border(&self, context: &LengthContext) -> f32 {
        self.border_top_width.to_px(context) + self.border_bottom_width.to_px(context)
    }

    /// Check if this element creates a stacking context.
    pub fn creates_stacking_context(&self) -> bool {
        self.position.creates_stacking_context()
            || self.opacity < 1.0
            || self.z_index.is_some()
    }

    /// Check if this element is positioned.
    pub fn is_positioned(&self) -> bool {
        self.position.is_positioned()
    }

    /// Check if this element is out of normal flow.
    pub fn is_out_of_flow(&self) -> bool {
        self.position.is_out_of_flow() || self.display.is_none()
    }
}
