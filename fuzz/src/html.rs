//! HTML Parser Fuzzing
//!
//! Fuzzing harness for HTML parsing.

use alloc::string::String;
use alloc::vec::Vec;
use crate::{FuzzTarget, FuzzResult, Mutator};

/// HTML Parser fuzzer
pub struct HtmlFuzzer {
    /// Maximum document size
    max_size: usize,
    /// Enable fragment parsing
    fragment_mode: bool,
    /// Track nesting depth
    max_depth: usize,
}

impl HtmlFuzzer {
    /// Create new HTML fuzzer
    pub fn new() -> Self {
        Self {
            max_size: 1024 * 1024,
            fragment_mode: false,
            max_depth: 512,
        }
    }

    /// Enable fragment parsing mode
    pub fn fragment_mode(mut self) -> Self {
        self.fragment_mode = true;
        self
    }

    /// Set maximum nesting depth
    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Parse HTML input
    fn parse(&self, input: &[u8]) -> Result<(), HtmlParseError> {
        // Check size limits
        if input.len() > self.max_size {
            return Err(HtmlParseError::TooLarge);
        }

        // Validate UTF-8 (with tolerance for fuzzing)
        let _text = match core::str::from_utf8(input) {
            Ok(s) => s,
            Err(_) => return Err(HtmlParseError::InvalidEncoding),
        };

        // Simulate parsing with depth tracking
        let mut depth = 0usize;
        let mut in_tag = false;
        let mut is_close_tag = false;

        for &byte in input {
            match byte {
                b'<' => {
                    in_tag = true;
                    is_close_tag = false;
                }
                b'/' if in_tag => {
                    is_close_tag = true;
                }
                b'>' => {
                    if in_tag {
                        if is_close_tag {
                            depth = depth.saturating_sub(1);
                        } else {
                            depth += 1;
                            if depth > self.max_depth {
                                return Err(HtmlParseError::TooDeep);
                            }
                        }
                    }
                    in_tag = false;
                }
                _ => {}
            }
        }

        Ok(())
    }
}

impl Default for HtmlFuzzer {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzTarget for HtmlFuzzer {
    fn name(&self) -> &str {
        "html_parser"
    }

    fn fuzz(&mut self, input: &[u8]) -> FuzzResult {
        match self.parse(input) {
            Ok(()) => FuzzResult::Ok,
            Err(e) => FuzzResult::ParseError(e.message()),
        }
    }

    fn reset(&mut self) {
        // No state to reset
    }
}

/// HTML parsing error
#[derive(Debug)]
enum HtmlParseError {
    /// Document too large
    TooLarge,
    /// Invalid encoding
    InvalidEncoding,
    /// Nesting too deep
    TooDeep,
    /// Unclosed tag
    UnclosedTag,
    /// Invalid tag name
    InvalidTagName,
}

impl HtmlParseError {
    fn message(&self) -> String {
        match self {
            Self::TooLarge => String::from("Document too large"),
            Self::InvalidEncoding => String::from("Invalid encoding"),
            Self::TooDeep => String::from("Nesting too deep"),
            Self::UnclosedTag => String::from("Unclosed tag"),
            Self::InvalidTagName => String::from("Invalid tag name"),
        }
    }
}

/// Create HTML dictionary for fuzzing
pub fn html_dictionary() -> Vec<Vec<u8>> {
    let entries: &[&[u8]] = &[
        // Doctype
        b"<!DOCTYPE html>",
        b"<!DOCTYPE HTML PUBLIC",
        // Common tags
        b"<html>",
        b"</html>",
        b"<head>",
        b"</head>",
        b"<body>",
        b"</body>",
        b"<div>",
        b"</div>",
        b"<span>",
        b"</span>",
        b"<p>",
        b"</p>",
        b"<a href=\"\">",
        b"</a>",
        b"<img src=\"\">",
        b"<script>",
        b"</script>",
        b"<style>",
        b"</style>",
        b"<link rel=\"stylesheet\">",
        b"<meta charset=\"utf-8\">",
        // Void elements
        b"<br>",
        b"<hr>",
        b"<input>",
        b"<meta>",
        b"<link>",
        // Attributes
        b" class=\"\"",
        b" id=\"\"",
        b" style=\"\"",
        b" onclick=\"\"",
        b" onerror=\"\"",
        // Edge cases
        b"<script/xss>",
        b"<img/src=x onerror=alert(1)>",
        b"<svg onload=alert(1)>",
        b"<!--",
        b"-->",
        b"<![CDATA[",
        b"]]>",
        // Encoding edge cases
        b"\xef\xbb\xbf", // BOM
        b"\x00",         // NULL
        b"\r\n",
        b"\r",
        // Template syntax
        b"{{",
        b"}}",
        b"{%",
        b"%}",
    ];

    entries.iter().map(|e| e.to_vec()).collect()
}

/// Generate interesting HTML inputs
pub fn generate_html_corpus() -> Vec<Vec<u8>> {
    let mut corpus = Vec::new();

    // Minimal valid HTML
    corpus.push(b"<!DOCTYPE html><html><head></head><body></body></html>".to_vec());

    // Empty document
    corpus.push(Vec::new());

    // Just text
    corpus.push(b"Hello, World!".to_vec());

    // Deeply nested
    let mut deep = Vec::new();
    for _ in 0..100 {
        deep.extend_from_slice(b"<div>");
    }
    for _ in 0..100 {
        deep.extend_from_slice(b"</div>");
    }
    corpus.push(deep);

    // Many siblings
    let mut siblings = Vec::new();
    for _ in 0..1000 {
        siblings.extend_from_slice(b"<span></span>");
    }
    corpus.push(siblings);

    // Malformed
    corpus.push(b"<div><span></div></span>".to_vec());
    corpus.push(b"<<<<<>>>>".to_vec());
    corpus.push(b"</not-opened>".to_vec());
    corpus.push(b"<never-closed".to_vec());

    // Scripts
    corpus.push(b"<script>alert('xss')</script>".to_vec());
    corpus.push(b"<script>/*".to_vec());
    corpus.push(b"<script><script></script>".to_vec());

    // Comments
    corpus.push(b"<!-- comment -->".to_vec());
    corpus.push(b"<!---->".to_vec());
    corpus.push(b"<!-- -- -->".to_vec());
    corpus.push(b"<!------>".to_vec());

    // CDATA
    corpus.push(b"<![CDATA[content]]>".to_vec());

    // SVG/MathML (foreign content)
    corpus.push(b"<svg><foreignObject><div></div></foreignObject></svg>".to_vec());
    corpus.push(b"<math><mtext><div></div></mtext></math>".to_vec());

    // Table quirks
    corpus.push(b"<table><tr><td><table><tr><td></td></tr></table></td></tr></table>".to_vec());
    corpus.push(b"<table><div></div></table>".to_vec());

    // Forms
    corpus.push(b"<form><input><button>Submit</button></form>".to_vec());

    // Templates
    corpus.push(b"<template><div></div></template>".to_vec());

    corpus
}

/// HTML attribute fuzzer
pub struct HtmlAttributeFuzzer {
    /// Attribute name to fuzz
    attr_name: String,
}

impl HtmlAttributeFuzzer {
    /// Create fuzzer for specific attribute
    pub fn new(attr_name: &str) -> Self {
        Self {
            attr_name: String::from(attr_name),
        }
    }

    /// Generate test input
    pub fn generate_input(&self, mutator: &mut Mutator, value: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend_from_slice(b"<div ");
        result.extend_from_slice(self.attr_name.as_bytes());
        result.extend_from_slice(b"=\"");
        
        let mut value = value.to_vec();
        mutator.mutate(&mut value);
        result.extend(value);
        
        result.extend_from_slice(b"\"></div>");
        result
    }
}

impl FuzzTarget for HtmlAttributeFuzzer {
    fn name(&self) -> &str {
        "html_attribute"
    }

    fn fuzz(&mut self, input: &[u8]) -> FuzzResult {
        // Attribute-specific parsing
        // Check for script injection, encoding issues, etc.
        if input.iter().any(|&b| b == 0) {
            return FuzzResult::Interesting(String::from("null byte in attribute"));
        }

        FuzzResult::Ok
    }

    fn reset(&mut self) {
        // No state
    }
}

/// Entity decoding fuzzer
pub struct HtmlEntityFuzzer;

impl FuzzTarget for HtmlEntityFuzzer {
    fn name(&self) -> &str {
        "html_entity"
    }

    fn fuzz(&mut self, input: &[u8]) -> FuzzResult {
        // Parse entities in input
        let text = match core::str::from_utf8(input) {
            Ok(s) => s,
            Err(_) => return FuzzResult::ParseError(String::from("invalid utf8")),
        };

        // Look for entity patterns
        let mut in_entity = false;
        let mut entity_len = 0;

        for ch in text.chars() {
            if ch == '&' {
                in_entity = true;
                entity_len = 0;
            } else if in_entity {
                entity_len += 1;
                if ch == ';' {
                    in_entity = false;
                    if entity_len > 32 {
                        return FuzzResult::Interesting(String::from("very long entity"));
                    }
                } else if entity_len > 64 {
                    return FuzzResult::Interesting(String::from("unterminated long entity"));
                }
            }
        }

        FuzzResult::Ok
    }

    fn reset(&mut self) {}
}
