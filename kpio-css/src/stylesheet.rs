//! Stylesheet - CSS stylesheet representation

use alloc::string::String;
use alloc::vec::Vec;

use crate::properties::DeclarationBlock;
use crate::selector::SelectorList;

/// A CSS stylesheet containing rules.
#[derive(Debug, Clone, Default)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
    pub origin: StylesheetOrigin,
}

impl Stylesheet {
    /// Create a new empty stylesheet.
    pub fn new() -> Self {
        Stylesheet {
            rules: Vec::new(),
            origin: StylesheetOrigin::Author,
        }
    }

    /// Create a user-agent stylesheet.
    pub fn user_agent() -> Self {
        Stylesheet {
            rules: Vec::new(),
            origin: StylesheetOrigin::UserAgent,
        }
    }

    /// Add a rule to the stylesheet.
    pub fn push(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Get all style rules.
    pub fn style_rules(&self) -> impl Iterator<Item = &StyleRule> {
        self.rules.iter().filter_map(|r| {
            if let Rule::Style(s) = r {
                Some(s)
            } else {
                None
            }
        })
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Get the number of rules.
    pub fn len(&self) -> usize {
        self.rules.len()
    }
}

/// The origin of a stylesheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StylesheetOrigin {
    /// User-agent (browser) stylesheet
    UserAgent,
    /// User stylesheet
    User,
    /// Author (page) stylesheet
    #[default]
    Author,
}

impl StylesheetOrigin {
    /// Get the priority weight for cascade ordering.
    pub fn priority(&self) -> u8 {
        match self {
            StylesheetOrigin::UserAgent => 0,
            StylesheetOrigin::User => 1,
            StylesheetOrigin::Author => 2,
        }
    }
}

/// A CSS rule.
#[derive(Debug, Clone)]
pub enum Rule {
    /// A style rule (selector + declarations)
    Style(StyleRule),
    /// An at-rule (@media, @import, etc.)
    AtRule(AtRule),
}

/// A style rule: selector list + declaration block.
#[derive(Debug, Clone)]
pub struct StyleRule {
    pub selectors: SelectorList,
    pub declarations: DeclarationBlock,
}

impl StyleRule {
    /// Create a new style rule.
    pub fn new(selectors: SelectorList, declarations: DeclarationBlock) -> Self {
        StyleRule {
            selectors,
            declarations,
        }
    }

    /// Check if this rule has any declarations.
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }
}

/// An at-rule.
#[derive(Debug, Clone)]
pub struct AtRule {
    pub name: String,
    pub prelude: String,
    pub block: Option<String>,
}

impl AtRule {
    /// Create a new at-rule.
    pub fn new(name: String, prelude: String) -> Self {
        AtRule {
            name,
            prelude,
            block: None,
        }
    }

    /// Check if this is a @media rule.
    pub fn is_media(&self) -> bool {
        self.name == "media"
    }

    /// Check if this is an @import rule.
    pub fn is_import(&self) -> bool {
        self.name == "import"
    }

    /// Check if this is a @font-face rule.
    pub fn is_font_face(&self) -> bool {
        self.name == "font-face"
    }

    /// Check if this is a @keyframes rule.
    pub fn is_keyframes(&self) -> bool {
        self.name == "keyframes"
    }
}

/// Media query for @media rules.
#[derive(Debug, Clone)]
pub struct MediaQuery {
    pub media_type: MediaType,
    pub conditions: Vec<MediaCondition>,
}

impl MediaQuery {
    /// Check if this query matches.
    pub fn matches(&self, context: &MediaContext) -> bool {
        // Check media type
        let type_matches = match self.media_type {
            MediaType::All => true,
            MediaType::Screen => context.media_type == MediaType::Screen,
            MediaType::Print => context.media_type == MediaType::Print,
        };

        if !type_matches {
            return false;
        }

        // Check all conditions
        self.conditions.iter().all(|c| c.matches(context))
    }
}

/// Media type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MediaType {
    #[default]
    All,
    Screen,
    Print,
}

/// Media condition (feature query).
#[derive(Debug, Clone)]
pub enum MediaCondition {
    MinWidth(f32),
    MaxWidth(f32),
    MinHeight(f32),
    MaxHeight(f32),
    Orientation(Orientation),
    PrefersColorScheme(ColorScheme),
    PrefersReducedMotion(bool),
}

impl MediaCondition {
    /// Check if this condition matches.
    pub fn matches(&self, context: &MediaContext) -> bool {
        match self {
            MediaCondition::MinWidth(w) => context.viewport_width >= *w,
            MediaCondition::MaxWidth(w) => context.viewport_width <= *w,
            MediaCondition::MinHeight(h) => context.viewport_height >= *h,
            MediaCondition::MaxHeight(h) => context.viewport_height <= *h,
            MediaCondition::Orientation(o) => context.orientation == *o,
            MediaCondition::PrefersColorScheme(s) => context.color_scheme == *s,
            MediaCondition::PrefersReducedMotion(r) => context.reduced_motion == *r,
        }
    }
}

/// Screen orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    #[default]
    Landscape,
    Portrait,
}

/// Color scheme preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorScheme {
    #[default]
    Light,
    Dark,
}

/// Context for evaluating media queries.
#[derive(Debug, Clone)]
pub struct MediaContext {
    pub media_type: MediaType,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub orientation: Orientation,
    pub color_scheme: ColorScheme,
    pub reduced_motion: bool,
}

impl Default for MediaContext {
    fn default() -> Self {
        MediaContext {
            media_type: MediaType::Screen,
            viewport_width: 1920.0,
            viewport_height: 1080.0,
            orientation: Orientation::Landscape,
            color_scheme: ColorScheme::Light,
            reduced_motion: false,
        }
    }
}
