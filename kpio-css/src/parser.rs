//! CSS Parser - Tokenization and parsing of CSS

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::iter::Peekable;
use core::str::Chars;

use crate::properties::{DeclarationBlock, PropertyDeclaration, PropertyId};
use crate::selector::{
    AttributeOperator, CaseSensitivity, Combinator, NthExpr, PseudoClass, PseudoElement, Selector,
    SelectorComponent, SelectorList,
};
use crate::stylesheet::{AtRule, Rule, StyleRule, Stylesheet};
use crate::values::{Color, CssValue, Length, LengthUnit};

use servo_types::LocalName;

/// CSS parse error.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    /// Unexpected end of input
    UnexpectedEof,
    /// Unexpected token
    UnexpectedToken(String),
    /// Invalid value for property
    InvalidValue(String),
    /// Unknown property
    UnknownProperty(String),
    /// Invalid selector
    InvalidSelector(String),
    /// Invalid color
    InvalidColor(String),
    /// Invalid number
    InvalidNumber(String),
    /// Unclosed block
    UnclosedBlock,
    /// Unclosed string
    UnclosedString,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ParseError::UnexpectedEof => write!(f, "Unexpected end of input"),
            ParseError::UnexpectedToken(t) => write!(f, "Unexpected token: {}", t),
            ParseError::InvalidValue(v) => write!(f, "Invalid value: {}", v),
            ParseError::UnknownProperty(p) => write!(f, "Unknown property: {}", p),
            ParseError::InvalidSelector(s) => write!(f, "Invalid selector: {}", s),
            ParseError::InvalidColor(c) => write!(f, "Invalid color: {}", c),
            ParseError::InvalidNumber(n) => write!(f, "Invalid number: {}", n),
            ParseError::UnclosedBlock => write!(f, "Unclosed block"),
            ParseError::UnclosedString => write!(f, "Unclosed string"),
        }
    }
}

/// CSS parser.
pub struct CssParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> CssParser<'a> {
    /// Create a new parser for the given input.
    pub fn new(input: &'a str) -> Self {
        CssParser { input, pos: 0 }
    }

    /// Parse a complete stylesheet.
    pub fn parse_stylesheet(&mut self) -> Result<Stylesheet, ParseError> {
        let mut stylesheet = Stylesheet::new();

        while !self.is_eof() {
            self.skip_whitespace_and_comments();
            if self.is_eof() {
                break;
            }

            if self.peek_char() == Some('@') {
                // At-rule
                if let Ok(at_rule) = self.parse_at_rule() {
                    stylesheet.rules.push(Rule::AtRule(at_rule));
                }
            } else {
                // Style rule
                if let Ok(style_rule) = self.parse_style_rule() {
                    stylesheet.rules.push(Rule::Style(style_rule));
                }
            }
        }

        Ok(stylesheet)
    }

    /// Parse a style rule (selector + declaration block).
    pub fn parse_style_rule(&mut self) -> Result<StyleRule, ParseError> {
        let selectors = self.parse_selector_list()?;
        self.skip_whitespace();

        if self.consume_char() != Some('{') {
            return Err(ParseError::UnexpectedToken("expected '{'".into()));
        }

        let declarations = self.parse_declaration_block()?;

        if self.consume_char() != Some('}') {
            return Err(ParseError::UnclosedBlock);
        }

        Ok(StyleRule {
            selectors,
            declarations,
        })
    }

    /// Parse a selector list (comma-separated).
    pub fn parse_selector_list(&mut self) -> Result<SelectorList, ParseError> {
        let mut list = SelectorList::new();

        loop {
            self.skip_whitespace();
            let selector = self.parse_selector()?;
            list.push(selector);

            self.skip_whitespace();
            if self.peek_char() == Some(',') {
                self.consume_char();
            } else {
                break;
            }
        }

        Ok(list)
    }

    /// Parse a single selector.
    pub fn parse_selector(&mut self) -> Result<Selector, ParseError> {
        let mut selector = Selector::new();

        loop {
            self.skip_whitespace();

            match self.peek_char() {
                Some('{') | Some(',') | None => break,
                Some('>') => {
                    self.consume_char();
                    selector
                        .components
                        .push(SelectorComponent::Combinator(Combinator::Child));
                }
                Some('+') => {
                    self.consume_char();
                    selector
                        .components
                        .push(SelectorComponent::Combinator(Combinator::NextSibling));
                }
                Some('~') => {
                    self.consume_char();
                    selector
                        .components
                        .push(SelectorComponent::Combinator(Combinator::SubsequentSibling));
                }
                Some('*') => {
                    self.consume_char();
                    selector.components.push(SelectorComponent::Universal);
                }
                Some('.') => {
                    self.consume_char();
                    let name = self.parse_ident()?;
                    selector.components.push(SelectorComponent::Class(name));
                }
                Some('#') => {
                    self.consume_char();
                    let name = self.parse_ident()?;
                    selector.components.push(SelectorComponent::Id(name));
                }
                Some('[') => {
                    let attr = self.parse_attribute_selector()?;
                    selector.components.push(attr);
                }
                Some(':') => {
                    self.consume_char();
                    if self.peek_char() == Some(':') {
                        self.consume_char();
                        let pseudo_element = self.parse_pseudo_element()?;
                        selector
                            .components
                            .push(SelectorComponent::PseudoElement(pseudo_element));
                    } else {
                        let pseudo_class = self.parse_pseudo_class()?;
                        selector
                            .components
                            .push(SelectorComponent::PseudoClass(pseudo_class));
                    }
                }
                Some(c) if is_ident_start(c) => {
                    let name = self.parse_ident()?;
                    selector
                        .components
                        .push(SelectorComponent::Type(LocalName::new(&name)));
                }
                _ => break,
            }
        }

        if selector.is_empty() {
            return Err(ParseError::InvalidSelector("empty selector".into()));
        }

        Ok(selector)
    }

    /// Parse an attribute selector.
    fn parse_attribute_selector(&mut self) -> Result<SelectorComponent, ParseError> {
        self.expect_char('[')?;
        self.skip_whitespace();

        let name = self.parse_ident()?;
        self.skip_whitespace();

        let (operator, value) = if self.peek_char() == Some(']') {
            (AttributeOperator::Exists, None)
        } else {
            let op = self.parse_attribute_operator()?;
            self.skip_whitespace();
            let value = self.parse_string_or_ident()?;
            (op, Some(value))
        };

        self.skip_whitespace();

        // Check for case sensitivity flag
        let case_sensitivity = if self.peek_char() == Some('i') || self.peek_char() == Some('I') {
            self.consume_char();
            CaseSensitivity::AsciiCaseInsensitive
        } else {
            CaseSensitivity::CaseSensitive
        };

        self.skip_whitespace();
        self.expect_char(']')?;

        Ok(SelectorComponent::Attribute {
            name: LocalName::new(&name),
            namespace: None,
            operator,
            value,
            case_sensitivity,
        })
    }

    /// Parse an attribute operator.
    fn parse_attribute_operator(&mut self) -> Result<AttributeOperator, ParseError> {
        match self.peek_char() {
            Some('=') => {
                self.consume_char();
                Ok(AttributeOperator::Equals)
            }
            Some('~') => {
                self.consume_char();
                self.expect_char('=')?;
                Ok(AttributeOperator::Includes)
            }
            Some('|') => {
                self.consume_char();
                self.expect_char('=')?;
                Ok(AttributeOperator::DashMatch)
            }
            Some('^') => {
                self.consume_char();
                self.expect_char('=')?;
                Ok(AttributeOperator::Prefix)
            }
            Some('$') => {
                self.consume_char();
                self.expect_char('=')?;
                Ok(AttributeOperator::Suffix)
            }
            Some('*') => {
                self.consume_char();
                self.expect_char('=')?;
                Ok(AttributeOperator::Substring)
            }
            _ => Err(ParseError::UnexpectedToken(
                "expected attribute operator".into(),
            )),
        }
    }

    /// Parse a pseudo-class.
    fn parse_pseudo_class(&mut self) -> Result<PseudoClass, ParseError> {
        let name = self.parse_ident()?;

        match name.as_str() {
            "hover" => Ok(PseudoClass::Hover),
            "active" => Ok(PseudoClass::Active),
            "focus" => Ok(PseudoClass::Focus),
            "focus-visible" => Ok(PseudoClass::FocusVisible),
            "focus-within" => Ok(PseudoClass::FocusWithin),
            "visited" => Ok(PseudoClass::Visited),
            "link" => Ok(PseudoClass::Link),
            "any-link" => Ok(PseudoClass::AnyLink),
            "enabled" => Ok(PseudoClass::Enabled),
            "disabled" => Ok(PseudoClass::Disabled),
            "checked" => Ok(PseudoClass::Checked),
            "required" => Ok(PseudoClass::Required),
            "optional" => Ok(PseudoClass::Optional),
            "valid" => Ok(PseudoClass::Valid),
            "invalid" => Ok(PseudoClass::Invalid),
            "read-only" => Ok(PseudoClass::ReadOnly),
            "read-write" => Ok(PseudoClass::ReadWrite),
            "root" => Ok(PseudoClass::Root),
            "empty" => Ok(PseudoClass::Empty),
            "first-child" => Ok(PseudoClass::FirstChild),
            "last-child" => Ok(PseudoClass::LastChild),
            "only-child" => Ok(PseudoClass::OnlyChild),
            "first-of-type" => Ok(PseudoClass::FirstOfType),
            "last-of-type" => Ok(PseudoClass::LastOfType),
            "only-of-type" => Ok(PseudoClass::OnlyOfType),
            "target" => Ok(PseudoClass::Target),
            "nth-child" | "nth-last-child" | "nth-of-type" | "nth-last-of-type" => {
                self.expect_char('(')?;
                let expr = self.parse_nth_expr()?;
                self.expect_char(')')?;
                match name.as_str() {
                    "nth-child" => Ok(PseudoClass::NthChild(expr)),
                    "nth-last-child" => Ok(PseudoClass::NthLastChild(expr)),
                    "nth-of-type" => Ok(PseudoClass::NthOfType(expr)),
                    "nth-last-of-type" => Ok(PseudoClass::NthLastOfType(expr)),
                    _ => unreachable!(),
                }
            }
            _ => Err(ParseError::InvalidSelector(alloc::format!(
                "unknown pseudo-class: {}",
                name
            ))),
        }
    }

    /// Parse an An+B expression.
    fn parse_nth_expr(&mut self) -> Result<NthExpr, ParseError> {
        self.skip_whitespace();

        let s = self.consume_until(')');
        let s = s.trim();

        match s {
            "odd" => Ok(NthExpr::ODD),
            "even" => Ok(NthExpr::EVEN),
            _ => {
                // Simplified parsing: just handle basic cases
                if let Ok(n) = s.parse::<i32>() {
                    Ok(NthExpr::new(0, n))
                } else if s == "n" {
                    Ok(NthExpr::new(1, 0))
                } else {
                    // Try to parse An+B
                    Ok(NthExpr::new(1, 0)) // Fallback
                }
            }
        }
    }

    /// Parse a pseudo-element.
    fn parse_pseudo_element(&mut self) -> Result<PseudoElement, ParseError> {
        let name = self.parse_ident()?;

        match name.as_str() {
            "before" => Ok(PseudoElement::Before),
            "after" => Ok(PseudoElement::After),
            "first-line" => Ok(PseudoElement::FirstLine),
            "first-letter" => Ok(PseudoElement::FirstLetter),
            "selection" => Ok(PseudoElement::Selection),
            "placeholder" => Ok(PseudoElement::Placeholder),
            "marker" => Ok(PseudoElement::Marker),
            "backdrop" => Ok(PseudoElement::Backdrop),
            _ => Err(ParseError::InvalidSelector(alloc::format!(
                "unknown pseudo-element: {}",
                name
            ))),
        }
    }

    /// Parse a declaration block.
    pub fn parse_declaration_block(&mut self) -> Result<DeclarationBlock, ParseError> {
        let mut block = DeclarationBlock::new();

        loop {
            self.skip_whitespace_and_comments();

            if self.peek_char() == Some('}') || self.is_eof() {
                break;
            }

            if let Ok(decl) = self.parse_declaration() {
                block.push(decl);
            }

            self.skip_whitespace();
            if self.peek_char() == Some(';') {
                self.consume_char();
            }
        }

        Ok(block)
    }

    /// Parse a single declaration.
    pub fn parse_declaration(&mut self) -> Result<PropertyDeclaration, ParseError> {
        self.skip_whitespace();
        let property_name = self.parse_ident()?;
        self.skip_whitespace();
        self.expect_char(':')?;
        self.skip_whitespace();

        let value_str = self.consume_until_declaration_end();
        let value_str = value_str.trim();

        let (value_str, important) = if value_str.ends_with("!important") {
            (value_str.trim_end_matches("!important").trim(), true)
        } else {
            (value_str, false)
        };

        let property = PropertyId::from_name(&property_name)
            .ok_or_else(|| ParseError::UnknownProperty(property_name.clone()))?;

        let value = self.parse_value(value_str, property)?;

        let mut decl = PropertyDeclaration::new(property, value);
        decl.important = important;

        Ok(decl)
    }

    /// Parse a CSS value.
    fn parse_value(&self, value_str: &str, property: PropertyId) -> Result<CssValue, ParseError> {
        let value_str = value_str.trim();

        // Global keywords
        match value_str {
            "initial" => return Ok(CssValue::Initial),
            "inherit" => return Ok(CssValue::Inherit),
            "unset" => return Ok(CssValue::Unset),
            "revert" => return Ok(CssValue::Revert),
            _ => {}
        }

        // Property-specific parsing
        match property {
            PropertyId::Color | PropertyId::BackgroundColor | PropertyId::BorderColor => {
                self.parse_color_value(value_str)
            }
            PropertyId::Width
            | PropertyId::Height
            | PropertyId::MinWidth
            | PropertyId::MinHeight
            | PropertyId::MaxWidth
            | PropertyId::MaxHeight
            | PropertyId::MarginTop
            | PropertyId::MarginRight
            | PropertyId::MarginBottom
            | PropertyId::MarginLeft
            | PropertyId::PaddingTop
            | PropertyId::PaddingRight
            | PropertyId::PaddingBottom
            | PropertyId::PaddingLeft
            | PropertyId::Top
            | PropertyId::Right
            | PropertyId::Bottom
            | PropertyId::Left
            | PropertyId::FontSize
            | PropertyId::LineHeight
            | PropertyId::Gap
            | PropertyId::RowGap
            | PropertyId::ColumnGap => self.parse_length_value(value_str),
            PropertyId::Display => Ok(CssValue::Keyword(value_str.to_string())),
            PropertyId::Position => Ok(CssValue::Keyword(value_str.to_string())),
            PropertyId::FlexGrow
            | PropertyId::FlexShrink
            | PropertyId::Order
            | PropertyId::ZIndex => self.parse_number_value(value_str),
            PropertyId::Opacity => self.parse_number_value(value_str),
            _ => {
                // Default: try to parse as length, number, or keyword
                if let Ok(length) = self.parse_length_value(value_str) {
                    Ok(length)
                } else if let Ok(num) = self.parse_number_value(value_str) {
                    Ok(num)
                } else {
                    Ok(CssValue::Keyword(value_str.to_string()))
                }
            }
        }
    }

    /// Parse a color value.
    fn parse_color_value(&self, s: &str) -> Result<CssValue, ParseError> {
        // Named colors
        let color = match s.to_ascii_lowercase().as_str() {
            "transparent" => Color::TRANSPARENT,
            "black" => Color::BLACK,
            "white" => Color::WHITE,
            "red" => Color::RED,
            "green" => Color::GREEN,
            "blue" => Color::BLUE,
            "yellow" => Color::YELLOW,
            "cyan" | "aqua" => Color::CYAN,
            "magenta" | "fuchsia" => Color::MAGENTA,
            "gray" | "grey" => Color::GRAY,
            "silver" => Color::SILVER,
            "maroon" => Color::MAROON,
            "olive" => Color::OLIVE,
            "lime" => Color::LIME,
            "teal" => Color::TEAL,
            "navy" => Color::NAVY,
            "purple" => Color::PURPLE,
            "orange" => Color::ORANGE,
            "pink" => Color::PINK,
            _ if s.starts_with('#') => self.parse_hex_color(&s[1..])?,
            _ if s.starts_with("rgb(") || s.starts_with("rgba(") => self.parse_rgb_function(s)?,
            _ => return Err(ParseError::InvalidColor(s.to_string())),
        };

        Ok(CssValue::Color(color))
    }

    /// Parse a hex color.
    fn parse_hex_color(&self, s: &str) -> Result<Color, ParseError> {
        let s = s.trim();
        match s.len() {
            3 => {
                // #RGB
                let r = u8::from_str_radix(&s[0..1], 16)
                    .map_err(|_| ParseError::InvalidColor(s.to_string()))?;
                let g = u8::from_str_radix(&s[1..2], 16)
                    .map_err(|_| ParseError::InvalidColor(s.to_string()))?;
                let b = u8::from_str_radix(&s[2..3], 16)
                    .map_err(|_| ParseError::InvalidColor(s.to_string()))?;
                Ok(Color::rgb(r * 17, g * 17, b * 17))
            }
            6 => {
                // #RRGGBB
                let r = u8::from_str_radix(&s[0..2], 16)
                    .map_err(|_| ParseError::InvalidColor(s.to_string()))?;
                let g = u8::from_str_radix(&s[2..4], 16)
                    .map_err(|_| ParseError::InvalidColor(s.to_string()))?;
                let b = u8::from_str_radix(&s[4..6], 16)
                    .map_err(|_| ParseError::InvalidColor(s.to_string()))?;
                Ok(Color::rgb(r, g, b))
            }
            8 => {
                // #RRGGBBAA
                let r = u8::from_str_radix(&s[0..2], 16)
                    .map_err(|_| ParseError::InvalidColor(s.to_string()))?;
                let g = u8::from_str_radix(&s[2..4], 16)
                    .map_err(|_| ParseError::InvalidColor(s.to_string()))?;
                let b = u8::from_str_radix(&s[4..6], 16)
                    .map_err(|_| ParseError::InvalidColor(s.to_string()))?;
                let a = u8::from_str_radix(&s[6..8], 16)
                    .map_err(|_| ParseError::InvalidColor(s.to_string()))?;
                Ok(Color::rgba(r, g, b, a))
            }
            _ => Err(ParseError::InvalidColor(s.to_string())),
        }
    }

    /// Parse rgb() or rgba() function.
    fn parse_rgb_function(&self, s: &str) -> Result<Color, ParseError> {
        // Simplified: extract numbers
        let start = s
            .find('(')
            .ok_or_else(|| ParseError::InvalidColor(s.to_string()))?
            + 1;
        let end = s
            .find(')')
            .ok_or_else(|| ParseError::InvalidColor(s.to_string()))?;
        let inner = &s[start..end];

        let parts: Vec<&str> = inner
            .split(|c| c == ',' || c == ' ')
            .filter(|s| !s.is_empty())
            .collect();

        if parts.len() < 3 {
            return Err(ParseError::InvalidColor(s.to_string()));
        }

        let r = parts[0]
            .trim()
            .parse::<u8>()
            .map_err(|_| ParseError::InvalidColor(s.to_string()))?;
        let g = parts[1]
            .trim()
            .parse::<u8>()
            .map_err(|_| ParseError::InvalidColor(s.to_string()))?;
        let b = parts[2]
            .trim()
            .parse::<u8>()
            .map_err(|_| ParseError::InvalidColor(s.to_string()))?;

        let a = if parts.len() >= 4 {
            let a_str = parts[3].trim();
            if a_str.contains('.') {
                (a_str
                    .parse::<f32>()
                    .map_err(|_| ParseError::InvalidColor(s.to_string()))?
                    * 255.0) as u8
            } else {
                a_str
                    .parse::<u8>()
                    .map_err(|_| ParseError::InvalidColor(s.to_string()))?
            }
        } else {
            255
        };

        Ok(Color::rgba(r, g, b, a))
    }

    /// Parse a length value.
    fn parse_length_value(&self, s: &str) -> Result<CssValue, ParseError> {
        if s == "auto" {
            return Ok(CssValue::Keyword("auto".to_string()));
        }
        if s == "0" {
            return Ok(CssValue::Length(Length::zero()));
        }

        // Find where the number ends and unit begins
        let num_end = s
            .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
            .unwrap_or(s.len());
        let num_str = &s[..num_end];
        let unit_str = &s[num_end..];

        let value: f32 = num_str
            .parse()
            .map_err(|_| ParseError::InvalidNumber(s.to_string()))?;

        let unit = match unit_str {
            "px" | "" => LengthUnit::Px,
            "em" => LengthUnit::Em,
            "rem" => LengthUnit::Rem,
            "%" => LengthUnit::Percent,
            "vw" => LengthUnit::Vw,
            "vh" => LengthUnit::Vh,
            "vmin" => LengthUnit::Vmin,
            "vmax" => LengthUnit::Vmax,
            "ex" => LengthUnit::Ex,
            "ch" => LengthUnit::Ch,
            "cm" => LengthUnit::Cm,
            "mm" => LengthUnit::Mm,
            "in" => LengthUnit::In,
            "pt" => LengthUnit::Pt,
            "pc" => LengthUnit::Pc,
            _ => return Err(ParseError::InvalidValue(s.to_string())),
        };

        Ok(CssValue::Length(Length::new(value, unit)))
    }

    /// Parse a number value.
    fn parse_number_value(&self, s: &str) -> Result<CssValue, ParseError> {
        if let Ok(i) = s.parse::<i32>() {
            Ok(CssValue::Integer(i))
        } else if let Ok(f) = s.parse::<f32>() {
            Ok(CssValue::Number(f))
        } else {
            Err(ParseError::InvalidNumber(s.to_string()))
        }
    }

    /// Parse an at-rule.
    fn parse_at_rule(&mut self) -> Result<AtRule, ParseError> {
        self.expect_char('@')?;
        let name = self.parse_ident()?;
        self.skip_whitespace();

        // Consume until block or semicolon
        let prelude = self.consume_until_block_start();

        let block = if self.peek_char() == Some('{') {
            self.consume_char();
            let content = self.consume_block_content();
            self.consume_char(); // '}'
            Some(content)
        } else {
            if self.peek_char() == Some(';') {
                self.consume_char();
            }
            None
        };

        Ok(AtRule {
            name,
            prelude: prelude.trim().to_string(),
            block,
        })
    }

    // ========================================================================
    // Helper methods
    // ========================================================================

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn consume_char(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn expect_char(&mut self, expected: char) -> Result<(), ParseError> {
        if self.consume_char() == Some(expected) {
            Ok(())
        } else {
            Err(ParseError::UnexpectedToken(alloc::format!(
                "expected '{}'",
                expected
            )))
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.consume_char();
            } else {
                break;
            }
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            self.skip_whitespace();
            if self.input[self.pos..].starts_with("/*") {
                self.pos += 2;
                while !self.is_eof() && !self.input[self.pos..].starts_with("*/") {
                    self.pos += 1;
                }
                if !self.is_eof() {
                    self.pos += 2;
                }
            } else {
                break;
            }
        }
    }

    fn parse_ident(&mut self) -> Result<String, ParseError> {
        let mut result = String::new();

        while let Some(c) = self.peek_char() {
            if is_ident_char(c) {
                result.push(c);
                self.consume_char();
            } else {
                break;
            }
        }

        if result.is_empty() {
            Err(ParseError::UnexpectedToken("expected identifier".into()))
        } else {
            Ok(result)
        }
    }

    fn parse_string_or_ident(&mut self) -> Result<String, ParseError> {
        match self.peek_char() {
            Some('"') | Some('\'') => self.parse_string(),
            _ => self.parse_ident(),
        }
    }

    fn parse_string(&mut self) -> Result<String, ParseError> {
        let quote = self.consume_char().ok_or(ParseError::UnexpectedEof)?;
        let mut result = String::new();

        while let Some(c) = self.consume_char() {
            if c == quote {
                return Ok(result);
            } else if c == '\\' {
                if let Some(escaped) = self.consume_char() {
                    result.push(escaped);
                }
            } else {
                result.push(c);
            }
        }

        Err(ParseError::UnclosedString)
    }

    fn consume_until(&mut self, end: char) -> String {
        let mut result = String::new();
        while let Some(c) = self.peek_char() {
            if c == end {
                break;
            }
            result.push(c);
            self.consume_char();
        }
        result
    }

    fn consume_until_declaration_end(&mut self) -> String {
        let mut result = String::new();
        let mut depth = 0;

        while let Some(c) = self.peek_char() {
            match c {
                '(' | '[' => depth += 1,
                ')' | ']' => depth -= 1,
                ';' if depth == 0 => break,
                '}' if depth == 0 => break,
                _ => {}
            }
            result.push(c);
            self.consume_char();
        }

        result
    }

    fn consume_until_block_start(&mut self) -> String {
        let mut result = String::new();
        while let Some(c) = self.peek_char() {
            if c == '{' || c == ';' {
                break;
            }
            result.push(c);
            self.consume_char();
        }
        result
    }

    fn consume_block_content(&mut self) -> String {
        let mut result = String::new();
        let mut depth = 1;

        while let Some(c) = self.peek_char() {
            match c {
                '{' => {
                    depth += 1;
                    result.push(c);
                    self.consume_char();
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    result.push(c);
                    self.consume_char();
                }
                _ => {
                    result.push(c);
                    self.consume_char();
                }
            }
        }

        result
    }
}

/// Check if a character can start an identifier.
fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '-'
}

/// Check if a character can be part of an identifier.
fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_selector() {
        let mut parser = CssParser::new("div.class#id");
        let selector = parser.parse_selector().unwrap();
        assert_eq!(selector.components.len(), 3);
    }

    #[test]
    fn test_parse_declaration() {
        let mut parser = CssParser::new("color: red");
        let decl = parser.parse_declaration().unwrap();
        assert_eq!(decl.property, PropertyId::Color);
    }
}
