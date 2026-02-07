//! Lightweight HTML Renderer for the Browser Window
//!
//! Parses simple HTML into styled render commands that the GUI
//! `Renderer` can execute.  This is a kernel-side mini-pipeline
//! intentionally kept small — for full rendering kpio-browser
//! provides a complete pipeline (outside the kernel crate).

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

// ── Render primitives ───────────────────────────────────────

/// A single render command produced by the HTML pipeline.
#[derive(Debug, Clone)]
pub enum RenderCmd {
    /// Fill a rectangle.
    FillRect { x: i32, y: i32, w: u32, h: u32, color: u32 },
    /// Draw text.
    Text { x: i32, y: i32, text: String, color: u32, bold: bool },
    /// Horizontal rule.
    HRule { x: i32, y: i32, w: u32, color: u32 },
}

/// A fully laid-out page ready to be drawn.
#[derive(Debug, Clone)]
pub struct RenderedPage {
    pub commands: Vec<RenderCmd>,
    pub total_height: u32,
    pub bg_color: u32,
    /// Plain-text title extracted from <title>.
    pub title: String,
}

// ── Mini HTML tokeniser ─────────────────────────────────────

#[derive(Debug)]
enum Token {
    OpenTag(String, Vec<(String, String)>),
    CloseTag(String),
    Text(String),
    SelfClose(String, Vec<(String, String)>),
}

fn tokenise(html: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = html.chars().peekable();
    let mut text_buf = String::new();

    while let Some(&ch) = chars.peek() {
        if ch == '<' {
            // Flush text
            if !text_buf.is_empty() {
                tokens.push(Token::Text(core::mem::take(&mut text_buf)));
            }
            chars.next(); // consume '<'

            // Close tag?
            let is_close = chars.peek() == Some(&'/');
            if is_close { chars.next(); }

            // Tag name
            let mut name = String::new();
            while let Some(&c) = chars.peek() {
                if c == '>' || c == ' ' || c == '/' { break; }
                name.push(c);
                chars.next();
            }

            // Attributes (simplified)
            let mut attrs = Vec::new();
            let mut is_self_close = false;
            loop {
                // skip whitespace
                while chars.peek() == Some(&' ') { chars.next(); }
                match chars.peek() {
                    Some(&'>') => { chars.next(); break; }
                    Some(&'/') => {
                        chars.next(); // consume /
                        if chars.peek() == Some(&'>') { chars.next(); }
                        is_self_close = true;
                        break;
                    }
                    Some(_) => {
                        // attr name
                        let mut aname = String::new();
                        while let Some(&c) = chars.peek() {
                            if c == '=' || c == '>' || c == ' ' || c == '/' { break; }
                            aname.push(c);
                            chars.next();
                        }
                        let mut aval = String::new();
                        if chars.peek() == Some(&'=') {
                            chars.next();
                            let quote = chars.peek().copied();
                            if quote == Some('"') || quote == Some('\'') {
                                chars.next();
                                while let Some(&c) = chars.peek() {
                                    if c == quote.unwrap() { chars.next(); break; }
                                    aval.push(c);
                                    chars.next();
                                }
                            } else {
                                while let Some(&c) = chars.peek() {
                                    if c == ' ' || c == '>' { break; }
                                    aval.push(c);
                                    chars.next();
                                }
                            }
                        }
                        attrs.push((aname.to_ascii_lowercase(), aval));
                    }
                    None => break,
                }
            }

            if !name.is_empty() {
                let lower = name.to_ascii_lowercase();
                if is_self_close {
                    tokens.push(Token::SelfClose(lower, attrs));
                } else if is_close {
                    tokens.push(Token::CloseTag(lower));
                } else {
                    tokens.push(Token::OpenTag(lower, attrs));
                }
            }
        } else {
            text_buf.push(ch);
            chars.next();
        }
    }
    if !text_buf.is_empty() {
        tokens.push(Token::Text(text_buf));
    }
    tokens
}

// ── Style state ─────────────────────────────────────────────

struct StyleState {
    color: u32,
    bg: u32,
    font_size: u32,
    bold: bool,
    indent: i32,
    in_list: bool,
    list_item: u32,
}

impl Default for StyleState {
    fn default() -> Self {
        StyleState {
            color: 0xFF_E0E0E0, // light grey text
            bg: 0xFF_0A0E17,    // dark blue background
            font_size: 16,
            bold: false,
            indent: 0,
            in_list: false,
            list_item: 0,
        }
    }
}

// ── Render ──────────────────────────────────────────────────

/// Render HTML into a list of drawing commands.
pub fn render_html(html: &str, viewport_w: u32) -> RenderedPage {
    let tokens = tokenise(html);
    let mut cmds: Vec<RenderCmd> = Vec::new();

    let margin = 12i32;
    let mut x = margin;
    let mut y = margin;
    let line_height: i32 = 18;
    let max_w = viewport_w as i32 - margin * 2;

    let mut style = StyleState::default();
    let mut title = String::new();
    let mut in_title = false;
    let mut in_head = false;
    let mut in_style_tag = false;
    let mut pending_newline = false;

    // Background fill
    let bg = style.bg;

    for token in &tokens {
        match token {
            Token::OpenTag(tag, attrs) => {
                match tag.as_str() {
                    "head" => in_head = true,
                    "title" => in_title = true,
                    "style" | "script" => in_style_tag = true,
                    "body" => {
                        in_head = false;
                        // Check for style attrs
                        for (k, v) in attrs {
                            if k == "style" && v.contains("background") {
                                // parse background color if present
                            }
                        }
                    }
                    "h1" => {
                        if y > margin { y += line_height / 2; }
                        style.font_size = 28;
                        style.bold = true;
                        style.color = 0xFF_4FC3F7; // accent blue
                        x = margin;
                    }
                    "h2" => {
                        if y > margin { y += line_height / 3; }
                        style.font_size = 22;
                        style.bold = true;
                        style.color = 0xFF_81D4FA;
                        x = margin;
                    }
                    "h3" => {
                        if y > margin { y += 4; }
                        style.font_size = 18;
                        style.bold = true;
                        style.color = 0xFF_B3E5FC;
                        x = margin;
                    }
                    "p" => {
                        if y > margin { y += line_height / 2; }
                        x = margin + style.indent;
                    }
                    "br" => {
                        y += line_height;
                        x = margin + style.indent;
                    }
                    "ul" | "ol" => {
                        style.in_list = true;
                        style.list_item = 0;
                        style.indent += 20;
                    }
                    "li" => {
                        y += line_height;
                        x = margin + style.indent;
                        style.list_item += 1;
                        // Draw bullet
                        cmds.push(RenderCmd::Text {
                            x,
                            y,
                            text: String::from("• "),
                            color: style.color,
                            bold: false,
                        });
                        x += 16;
                    }
                    "a" => {
                        style.color = 0xFF_81D4FA; // link blue
                    }
                    "strong" | "b" => {
                        style.bold = true;
                    }
                    "em" | "i" => {
                        // Italics rendered as different color
                        style.color = 0xFF_CE93D8;
                    }
                    "hr" => {
                        y += line_height / 2;
                        cmds.push(RenderCmd::HRule {
                            x: margin,
                            y,
                            w: max_w as u32,
                            color: 0xFF_333333,
                        });
                        y += line_height / 2;
                    }
                    "pre" | "code" => {
                        style.color = 0xFF_A5D6A7; // green for code
                    }
                    "div" | "section" | "article" | "main" | "header" | "footer" | "nav" => {
                        if y > margin { y += 4; }
                        x = margin + style.indent;
                    }
                    _ => {}
                }
            }
            Token::CloseTag(tag) => {
                match tag.as_str() {
                    "head" => in_head = false,
                    "title" => in_title = false,
                    "style" | "script" => in_style_tag = false,
                    "h1" | "h2" | "h3" => {
                        style.font_size = 16;
                        style.bold = false;
                        style.color = 0xFF_E0E0E0;
                        y += line_height + 4;
                        x = margin;
                    }
                    "p" => {
                        y += line_height;
                        x = margin;
                    }
                    "ul" | "ol" => {
                        style.in_list = false;
                        style.indent -= 20;
                        if style.indent < 0 { style.indent = 0; }
                        y += 4;
                    }
                    "li" => {
                        // newline already handled
                    }
                    "a" => {
                        style.color = 0xFF_E0E0E0;
                    }
                    "strong" | "b" => {
                        style.bold = false;
                    }
                    "em" | "i" => {
                        style.color = 0xFF_E0E0E0;
                    }
                    "pre" | "code" => {
                        style.color = 0xFF_E0E0E0;
                    }
                    "div" | "section" | "article" | "main" | "header" | "footer" | "nav" => {
                        y += 4;
                        x = margin;
                    }
                    "br" => {}
                    _ => {}
                }
            }
            Token::SelfClose(tag, _) => {
                match tag.as_str() {
                    "br" => {
                        y += line_height;
                        x = margin + style.indent;
                    }
                    "hr" => {
                        y += line_height / 2;
                        cmds.push(RenderCmd::HRule {
                            x: margin,
                            y,
                            w: max_w as u32,
                            color: 0xFF_333333,
                        });
                        y += line_height / 2;
                    }
                    "meta" | "link" | "img" => {}
                    _ => {}
                }
            }
            Token::Text(text) => {
                if in_style_tag || in_head { 
                    if in_title {
                        title = text.trim().into();
                    }
                    continue; 
                }
                let trimmed = collapse_whitespace(text);
                if trimmed.is_empty() { continue; }

                // Word-wrap
                let char_w = 8i32; // 8px per char in our bitmap font
                let avail = max_w - (x - margin);

                for word in trimmed.split(' ') {
                    if word.is_empty() { continue; }
                    let word_px = word.len() as i32 * char_w;
                    if x + word_px > margin + max_w && x > margin + style.indent {
                        y += line_height;
                        x = margin + style.indent;
                    }
                    cmds.push(RenderCmd::Text {
                        x,
                        y,
                        text: String::from(word),
                        color: style.color,
                        bold: style.bold,
                    });
                    x += word_px + char_w; // word + space
                }
            }
        }
    }

    RenderedPage {
        commands: cmds,
        total_height: (y + line_height) as u32,
        bg_color: bg,
        title,
    }
}

/// Collapse runs of whitespace / newlines into single spaces.
fn collapse_whitespace(s: &str) -> String {
    let mut out = String::new();
    let mut prev_ws = true; // trim leading
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                out.push(' ');
                prev_ws = true;
            }
        } else {
            out.push(ch);
            prev_ws = false;
        }
    }
    // trim trailing
    if out.ends_with(' ') { out.pop(); }
    out
}
