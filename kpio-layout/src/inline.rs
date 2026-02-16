//! Inline Layout Algorithm
//!
//! This module implements the CSS inline formatting context layout algorithm.
//! Inline boxes flow horizontally and wrap to new lines when they reach the edge.
//!
//! ## Inline Layout Concepts
//!
//! - **Line Box**: A horizontal box containing inline elements
//! - **Inline Box**: An element that flows with text (span, a, etc.)
//! - **Text Run**: A continuous run of text within an inline box
//! - **Line Breaking**: Wrapping content to new lines

use crate::box_model::{EdgeSizes, Rect};
use crate::layout_box::{BoxType, ContainingBlock, LayoutBox, LayoutContext};
use alloc::string::String;
use alloc::vec::Vec;

/// A line box containing inline content
#[derive(Debug)]
pub struct LineBox {
    /// Y position of this line
    pub y: f32,
    /// Height of this line
    pub height: f32,
    /// Baseline position relative to line top
    pub baseline: f32,
    /// Fragments on this line
    pub fragments: Vec<LineFragment>,
}

impl LineBox {
    pub fn new(y: f32) -> Self {
        Self {
            y,
            height: 0.0,
            baseline: 0.0,
            fragments: Vec::new(),
        }
    }

    /// Current width used on this line
    pub fn width(&self) -> f32 {
        self.fragments.iter().map(|f| f.width).sum()
    }

    /// Current X position for next fragment
    pub fn current_x(&self, start_x: f32) -> f32 {
        start_x + self.width()
    }

    /// Add a fragment to this line
    pub fn add_fragment(&mut self, fragment: LineFragment) {
        // Update line height if needed
        if fragment.height > self.height {
            self.height = fragment.height;
        }
        self.fragments.push(fragment);
    }
}

/// A fragment of inline content on a line
#[derive(Debug)]
pub struct LineFragment {
    /// X position within the line
    pub x: f32,
    /// Y position (same as line Y for now)
    pub y: f32,
    /// Width of this fragment
    pub width: f32,
    /// Height of this fragment
    pub height: f32,
    /// The type of content
    pub content: FragmentContent,
}

/// Content of a line fragment
#[derive(Debug)]
pub enum FragmentContent {
    /// Text content
    Text {
        text: String,
        /// Index into the original text (for selection/editing)
        start_index: usize,
        end_index: usize,
    },
    /// Start of an inline box (opening tag)
    InlineStart { box_index: usize },
    /// End of an inline box (closing tag)
    InlineEnd { box_index: usize },
    /// Atomic inline (image, inline-block, etc.)
    Atomic { box_index: usize },
}

/// Font metrics for text layout
#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    /// Font size in pixels
    pub size: f32,
    /// Line height in pixels
    pub line_height: f32,
    /// Ascender (baseline to top)
    pub ascender: f32,
    /// Descender (baseline to bottom, positive value)
    pub descender: f32,
    /// Average character width (for approximation)
    pub avg_char_width: f32,
}

impl Default for FontMetrics {
    fn default() -> Self {
        Self {
            size: 16.0,
            line_height: 20.0,
            ascender: 14.0,
            descender: 4.0,
            avg_char_width: 8.0,
        }
    }
}

/// Inline formatting context
pub struct InlineFormattingContext {
    /// The containing block
    pub containing_block: ContainingBlock,
    /// Current Y position
    pub current_y: f32,
    /// Line boxes
    pub lines: Vec<LineBox>,
    /// Default font metrics
    pub font_metrics: FontMetrics,
}

impl InlineFormattingContext {
    pub fn new(containing_block: ContainingBlock) -> Self {
        Self {
            containing_block,
            current_y: containing_block.y,
            lines: Vec::new(),
            font_metrics: FontMetrics::default(),
        }
    }

    /// Start a new line
    pub fn new_line(&mut self) {
        // Finalize current line if it exists
        if let Some(current_line) = self.lines.last() {
            self.current_y += current_line.height;
        }

        self.lines.push(LineBox::new(self.current_y));
    }

    /// Get current line, creating one if needed
    pub fn current_line(&mut self) -> &mut LineBox {
        if self.lines.is_empty() {
            self.new_line();
        }
        self.lines.last_mut().unwrap()
    }

    /// Get remaining width on current line
    pub fn remaining_width(&self) -> f32 {
        let used = self.lines.last().map(|l| l.width()).unwrap_or(0.0);
        (self.containing_block.width - used).max(0.0)
    }

    /// Layout text content
    pub fn layout_text(&mut self, text: &str, start_x: f32) {
        if text.is_empty() {
            return;
        }

        let line_height = self.font_metrics.line_height;
        let char_width = self.font_metrics.avg_char_width;

        let mut remaining_text = text;
        let mut text_index = 0;

        while !remaining_text.is_empty() {
            let line = self.current_line();
            let current_x = line.current_x(start_x);
            let available_width =
                self.containing_block.width - (current_x - self.containing_block.x);

            if available_width <= 0.0 {
                // No space, start new line
                self.new_line();
                continue;
            }

            // Calculate how many characters fit
            let max_chars = (available_width / char_width) as usize;
            if max_chars == 0 {
                self.new_line();
                continue;
            }

            // Find break point (word boundary or forced)
            let (break_index, forced_break) = find_break_point(remaining_text, max_chars);

            if break_index == 0 {
                // Can't fit anything, force new line
                self.new_line();
                continue;
            }

            // Create fragment for this portion
            let fragment_text: String = remaining_text[..break_index].into();
            let fragment_width = fragment_text.chars().count() as f32 * char_width;

            let line = self.current_line();
            let fragment = LineFragment {
                x: line.current_x(start_x),
                y: line.y,
                width: fragment_width,
                height: line_height,
                content: FragmentContent::Text {
                    text: fragment_text,
                    start_index: text_index,
                    end_index: text_index + break_index,
                },
            };
            line.add_fragment(fragment);

            // Advance
            text_index += break_index;
            remaining_text = &remaining_text[break_index..];

            // Skip leading whitespace on new line
            if forced_break {
                remaining_text = remaining_text.trim_start();
            }

            // If we filled the line, start a new one
            if self.remaining_width() <= char_width && !remaining_text.is_empty() {
                self.new_line();
            }
        }
    }

    /// Get total height of all lines
    pub fn total_height(&self) -> f32 {
        self.lines.iter().map(|l| l.height).sum()
    }

    /// Finalize the context and return final Y position
    pub fn finalize(&mut self) -> f32 {
        let total = self.lines.iter().map(|l| l.height).sum::<f32>();
        self.containing_block.y + total
    }
}

/// Find a good break point in text
fn find_break_point(text: &str, max_chars: usize) -> (usize, bool) {
    let chars: Vec<char> = text.chars().collect();

    if chars.len() <= max_chars {
        return (text.len(), false);
    }

    // Look for whitespace break point
    let mut last_space_char_index = None;
    for (i, &c) in chars.iter().enumerate().take(max_chars + 1) {
        if c.is_whitespace() {
            last_space_char_index = Some(i);
        }
    }

    if let Some(space_idx) = last_space_char_index {
        // Break at whitespace
        let byte_index = chars[..=space_idx].iter().map(|c| c.len_utf8()).sum();
        return (byte_index, true);
    }

    // No whitespace found, force break at max_chars
    let byte_index = chars[..max_chars].iter().map(|c| c.len_utf8()).sum();
    (byte_index, false)
}

/// Layout inline children of a block box
pub fn layout_inline_children(
    layout_box: &mut LayoutBox,
    containing_block: ContainingBlock,
    _context: &LayoutContext,
) {
    let mut ifc = InlineFormattingContext::new(containing_block);

    for child in &mut layout_box.children {
        match child.box_type {
            BoxType::Inline | BoxType::AnonymousInline => {
                if let Some(ref text) = child.text {
                    ifc.layout_text(text, containing_block.x);
                }

                // Set child dimensions based on its content
                let line = ifc.current_line();
                child.dimensions.content.x = containing_block.x;
                child.dimensions.content.y = line.y;
                child.dimensions.content.height = ifc.font_metrics.line_height;
            }
            _ => {
                // Skip block-level children in inline context
            }
        }
    }

    // Set parent height based on lines
    let total_height = ifc.total_height();
    if layout_box.dimensions.content.height < total_height {
        layout_box.dimensions.content.height = total_height;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_break_point() {
        let text = "Hello World";
        let (idx, forced) = find_break_point(text, 7);
        assert_eq!(&text[..idx], "Hello ");
        assert!(forced);
    }

    #[test]
    fn test_inline_context_text() {
        let cb = ContainingBlock::new(100.0, 100.0);
        let mut ifc = InlineFormattingContext::new(cb);
        ifc.layout_text("Hello World", 0.0);

        assert!(!ifc.lines.is_empty());
        assert!(ifc.total_height() > 0.0);
    }
}
