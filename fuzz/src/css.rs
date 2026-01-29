//! CSS Parser Fuzzing
//!
//! Fuzzing harness for CSS parsing.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use crate::{FuzzTarget, FuzzResult};

/// CSS Parser fuzzer
pub struct CssFuzzer {
    /// Maximum stylesheet size
    max_size: usize,
    /// Maximum selector depth
    max_selector_depth: usize,
    /// Maximum properties per rule
    max_properties: usize,
}

impl CssFuzzer {
    /// Create new CSS fuzzer
    pub fn new() -> Self {
        Self {
            max_size: 1024 * 1024,
            max_selector_depth: 32,
            max_properties: 1000,
        }
    }

    /// Set maximum size
    pub fn max_size(mut self, size: usize) -> Self {
        self.max_size = size;
        self
    }

    /// Parse CSS input
    fn parse(&self, input: &[u8]) -> Result<(), CssParseError> {
        if input.len() > self.max_size {
            return Err(CssParseError::TooLarge);
        }

        let text = match core::str::from_utf8(input) {
            Ok(s) => s,
            Err(_) => return Err(CssParseError::InvalidEncoding),
        };

        // Check balanced braces
        let mut brace_depth = 0i32;
        let mut paren_depth = 0i32;
        let mut bracket_depth = 0i32;
        let mut in_string = false;
        let mut string_char = ' ';
        let mut last_char = ' ';

        for ch in text.chars() {
            if in_string {
                if ch == string_char && last_char != '\\' {
                    in_string = false;
                }
            } else {
                match ch {
                    '"' | '\'' => {
                        in_string = true;
                        string_char = ch;
                    }
                    '{' => {
                        brace_depth += 1;
                        if brace_depth > self.max_selector_depth as i32 {
                            return Err(CssParseError::TooDeep);
                        }
                    }
                    '}' => {
                        brace_depth -= 1;
                        if brace_depth < 0 {
                            return Err(CssParseError::UnbalancedBraces);
                        }
                    }
                    '(' => {
                        paren_depth += 1;
                    }
                    ')' => {
                        paren_depth -= 1;
                        if paren_depth < 0 {
                            return Err(CssParseError::UnbalancedParens);
                        }
                    }
                    '[' => {
                        bracket_depth += 1;
                    }
                    ']' => {
                        bracket_depth -= 1;
                        if bracket_depth < 0 {
                            return Err(CssParseError::UnbalancedBrackets);
                        }
                    }
                    _ => {}
                }
            }
            last_char = ch;
        }

        if brace_depth != 0 {
            return Err(CssParseError::UnbalancedBraces);
        }
        if paren_depth != 0 {
            return Err(CssParseError::UnbalancedParens);
        }
        if bracket_depth != 0 {
            return Err(CssParseError::UnbalancedBrackets);
        }

        Ok(())
    }
}

impl Default for CssFuzzer {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzTarget for CssFuzzer {
    fn name(&self) -> &str {
        "css_parser"
    }

    fn fuzz(&mut self, input: &[u8]) -> FuzzResult {
        match self.parse(input) {
            Ok(()) => FuzzResult::Ok,
            Err(e) => FuzzResult::ParseError(e.message()),
        }
    }

    fn reset(&mut self) {
        // No state
    }
}

/// CSS parsing error
#[derive(Debug)]
enum CssParseError {
    /// Too large
    TooLarge,
    /// Invalid encoding
    InvalidEncoding,
    /// Nesting too deep
    TooDeep,
    /// Unbalanced braces
    UnbalancedBraces,
    /// Unbalanced parentheses
    UnbalancedParens,
    /// Unbalanced brackets
    UnbalancedBrackets,
    /// Invalid property
    InvalidProperty,
    /// Invalid value
    InvalidValue,
}

impl CssParseError {
    fn message(&self) -> String {
        match self {
            Self::TooLarge => String::from("Stylesheet too large"),
            Self::InvalidEncoding => String::from("Invalid encoding"),
            Self::TooDeep => String::from("Nesting too deep"),
            Self::UnbalancedBraces => String::from("Unbalanced braces"),
            Self::UnbalancedParens => String::from("Unbalanced parentheses"),
            Self::UnbalancedBrackets => String::from("Unbalanced brackets"),
            Self::InvalidProperty => String::from("Invalid property"),
            Self::InvalidValue => String::from("Invalid value"),
        }
    }
}

/// Create CSS dictionary for fuzzing
pub fn css_dictionary() -> Vec<Vec<u8>> {
    let entries: &[&[u8]] = &[
        // Selectors
        b"*",
        b"body",
        b"div",
        b".class",
        b"#id",
        b"[attr]",
        b"[attr=value]",
        b":hover",
        b":focus",
        b"::before",
        b"::after",
        b":not()",
        b":nth-child()",
        b":nth-of-type()",
        b":has()",
        b":where()",
        b":is()",
        // Combinators
        b" ",
        b">",
        b"+",
        b"~",
        // At-rules
        b"@media",
        b"@keyframes",
        b"@import",
        b"@font-face",
        b"@supports",
        b"@charset",
        b"@namespace",
        b"@page",
        b"@layer",
        b"@container",
        // Common properties
        b"display:",
        b"position:",
        b"width:",
        b"height:",
        b"margin:",
        b"padding:",
        b"border:",
        b"background:",
        b"color:",
        b"font-size:",
        b"transform:",
        b"transition:",
        b"animation:",
        b"flex:",
        b"grid:",
        // Values
        b"none",
        b"block",
        b"inline",
        b"flex",
        b"grid",
        b"absolute",
        b"relative",
        b"fixed",
        b"sticky",
        b"100%",
        b"100vh",
        b"100vw",
        b"auto",
        b"inherit",
        b"initial",
        b"unset",
        b"revert",
        // Functions
        b"url()",
        b"rgb()",
        b"rgba()",
        b"hsl()",
        b"hsla()",
        b"calc()",
        b"var()",
        b"min()",
        b"max()",
        b"clamp()",
        b"linear-gradient()",
        b"radial-gradient()",
        // Units
        b"px",
        b"em",
        b"rem",
        b"vh",
        b"vw",
        b"%",
        b"deg",
        b"rad",
        b"s",
        b"ms",
        // Edge cases
        b"\\",
        b"/*",
        b"*/",
        b"\\0",
        b"\\9",
        b"expression(",
        b"behavior:",
    ];

    entries.iter().map(|e| e.to_vec()).collect()
}

/// Generate interesting CSS inputs
pub fn generate_css_corpus() -> Vec<Vec<u8>> {
    let mut corpus = Vec::new();

    // Simple rule
    corpus.push(b"body { color: red; }".to_vec());

    // Empty
    corpus.push(Vec::new());

    // Media query
    corpus.push(b"@media (max-width: 800px) { .class { display: none; } }".to_vec());

    // Keyframes
    corpus.push(b"@keyframes anim { 0% { opacity: 0; } 100% { opacity: 1; } }".to_vec());

    // Complex selectors
    corpus.push(b"div > p + span ~ a:hover::before[data-foo=\"bar\"] { }".to_vec());

    // Nested (CSS nesting)
    corpus.push(b".parent { .child { color: blue; } }".to_vec());

    // Variables
    corpus.push(b":root { --color: blue; } body { color: var(--color); }".to_vec());

    // calc()
    corpus.push(b".box { width: calc(100% - 20px); height: calc(100vh / 2); }".to_vec());

    // Deep nesting
    let mut deep = Vec::new();
    for _ in 0..20 {
        deep.extend_from_slice(b"@media all { ");
    }
    deep.extend_from_slice(b"body { color: red; }");
    for _ in 0..20 {
        deep.extend_from_slice(b" }");
    }
    corpus.push(deep);

    // Many selectors
    let mut many = Vec::new();
    for i in 0..100 {
        many.extend_from_slice(format!(".c{}, ", i).as_bytes());
    }
    many.extend_from_slice(b".last { color: red; }");
    corpus.push(many);

    // Malformed
    corpus.push(b"body { color: red".to_vec());
    corpus.push(b"body color: red }".to_vec());
    corpus.push(b"{{{{}}}}".to_vec());

    // Comments
    corpus.push(b"/* comment */ body { color: red; }".to_vec());
    corpus.push(b"body { /* nested */ color: red; }".to_vec());

    // Strings
    corpus.push(b"body { content: \"hello\"; }".to_vec());
    corpus.push(b"body { content: 'hello'; }".to_vec());
    corpus.push(b"body { content: \"hello\\\"world\"; }".to_vec());

    // URLs
    corpus.push(b"body { background: url(image.png); }".to_vec());
    corpus.push(b"body { background: url(\"image.png\"); }".to_vec());
    corpus.push(b"body { background: url('data:image/png;base64,AAAA'); }".to_vec());

    // Unicode
    corpus.push("body { content: \"日本語\"; }".as_bytes().to_vec());
    corpus.push("body::before { content: \"\\2764\"; }".as_bytes().to_vec());

    // IE hacks (historical edge cases)
    corpus.push(b"body { *color: red; _color: blue; }".to_vec());

    corpus
}

/// CSS selector fuzzer
pub struct CssSelectorFuzzer {
    /// Maximum complexity
    max_complexity: usize,
}

impl CssSelectorFuzzer {
    /// Create new selector fuzzer
    pub fn new() -> Self {
        Self { max_complexity: 100 }
    }
}

impl Default for CssSelectorFuzzer {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzTarget for CssSelectorFuzzer {
    fn name(&self) -> &str {
        "css_selector"
    }

    fn fuzz(&mut self, input: &[u8]) -> FuzzResult {
        let text = match core::str::from_utf8(input) {
            Ok(s) => s,
            Err(_) => return FuzzResult::ParseError(String::from("invalid utf8")),
        };

        // Check selector complexity
        let complexity = text.matches(|c| matches!(c, ' ' | '>' | '+' | '~' | ':' | '['))
            .count();

        if complexity > self.max_complexity {
            return FuzzResult::Interesting(String::from("very complex selector"));
        }

        FuzzResult::Ok
    }

    fn reset(&mut self) {}
}

/// CSS value fuzzer
pub struct CssValueFuzzer {
    /// Property name
    property: String,
}

impl CssValueFuzzer {
    /// Create fuzzer for specific property
    pub fn new(property: &str) -> Self {
        Self {
            property: String::from(property),
        }
    }
}

impl FuzzTarget for CssValueFuzzer {
    fn name(&self) -> &str {
        "css_value"
    }

    fn fuzz(&mut self, input: &[u8]) -> FuzzResult {
        let _text = match core::str::from_utf8(input) {
            Ok(s) => s,
            Err(_) => return FuzzResult::ParseError(String::from("invalid utf8")),
        };

        // Property-specific validation would go here
        FuzzResult::Ok
    }

    fn reset(&mut self) {}
}

/// CSS animation fuzzer
pub struct CssAnimationFuzzer;

impl FuzzTarget for CssAnimationFuzzer {
    fn name(&self) -> &str {
        "css_animation"
    }

    fn fuzz(&mut self, input: &[u8]) -> FuzzResult {
        let text = match core::str::from_utf8(input) {
            Ok(s) => s,
            Err(_) => return FuzzResult::ParseError(String::from("invalid utf8")),
        };

        // Check for animation-related issues
        if text.contains("infinite") && text.contains("steps(0") {
            return FuzzResult::Interesting(String::from("infinite animation with zero steps"));
        }

        if text.matches("keyframes").count() > 100 {
            return FuzzResult::Interesting(String::from("many keyframe definitions"));
        }

        FuzzResult::Ok
    }

    fn reset(&mut self) {}
}
