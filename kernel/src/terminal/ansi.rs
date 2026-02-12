//! ANSI Escape Code Engine
//!
//! Parses ANSI/VT100 escape sequences and produces styled text spans.
//! Supports SGR (Select Graphic Rendition) for colors and attributes,
//! cursor movement codes, and screen/line clear sequences.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;

// ────────────────────────── Color ──────────────────────────

/// Terminal color — 16 standard, 256 indexed, or 24-bit RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnsiColor {
    /// Default terminal foreground / background
    Default,
    /// Standard 8 colors (0–7)
    Standard(u8),
    /// Bright/high-intensity variants (8–15)
    Bright(u8),
    /// 256-color palette index
    Indexed(u8),
    /// 24-bit true color
    Rgb(u8, u8, u8),
}

impl AnsiColor {
    // Standard color indices
    pub const BLACK: u8 = 0;
    pub const RED: u8 = 1;
    pub const GREEN: u8 = 2;
    pub const YELLOW: u8 = 3;
    pub const BLUE: u8 = 4;
    pub const MAGENTA: u8 = 5;
    pub const CYAN: u8 = 6;
    pub const WHITE: u8 = 7;

    /// Convert to approximate RGB tuple for rendering.
    pub fn to_rgb(self) -> (u8, u8, u8) {
        match self {
            AnsiColor::Default => (204, 204, 204), // light grey
            AnsiColor::Standard(c) | AnsiColor::Bright(c) => {
                let bright = matches!(self, AnsiColor::Bright(_));
                standard_to_rgb(c, bright)
            }
            AnsiColor::Indexed(idx) => indexed_to_rgb(idx),
            AnsiColor::Rgb(r, g, b) => (r, g, b),
        }
    }
}

/// Map a standard 3-bit colour to RGB.
fn standard_to_rgb(idx: u8, bright: bool) -> (u8, u8, u8) {
    if bright {
        match idx {
            0 => (85, 85, 85),    // bright black (grey)
            1 => (255, 85, 85),   // bright red
            2 => (85, 255, 85),   // bright green
            3 => (255, 255, 85),  // bright yellow
            4 => (85, 85, 255),   // bright blue
            5 => (255, 85, 255),  // bright magenta
            6 => (85, 255, 255),  // bright cyan
            7 => (255, 255, 255), // bright white
            _ => (204, 204, 204),
        }
    } else {
        match idx {
            0 => (0, 0, 0),       // black
            1 => (170, 0, 0),     // red
            2 => (0, 170, 0),     // green
            3 => (170, 170, 0),   // yellow / brown
            4 => (0, 0, 170),     // blue
            5 => (170, 0, 170),   // magenta
            6 => (0, 170, 170),   // cyan
            7 => (170, 170, 170), // white / light grey
            _ => (170, 170, 170),
        }
    }
}

/// Map 256-colour palette index to RGB.
fn indexed_to_rgb(idx: u8) -> (u8, u8, u8) {
    match idx {
        // 0–7: standard
        0..=7 => standard_to_rgb(idx, false),
        // 8–15: bright
        8..=15 => standard_to_rgb(idx - 8, true),
        // 16–231: 6×6×6 colour cube
        16..=231 => {
            let i = idx - 16;
            let r = i / 36;
            let g = (i % 36) / 6;
            let b = i % 6;
            let to_val = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            (to_val(r), to_val(g), to_val(b))
        }
        // 232–255: greyscale ramp (8–238)
        _ => {
            let v = 8 + 10 * (idx - 232);
            (v, v, v)
        }
    }
}

// ────────────────────────── Text Style ──────────────────────────

/// Text attributes for a styled span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextStyle {
    pub fg: AnsiColor,
    pub bg: AnsiColor,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub blink: bool,
    pub reverse: bool,
    pub strikethrough: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            fg: AnsiColor::Default,
            bg: AnsiColor::Default,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            blink: false,
            reverse: false,
            strikethrough: false,
        }
    }
}

impl TextStyle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all attributes to default.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Resolved foreground RGB — if reverse, swap fg/bg.
    pub fn fg_rgb(&self) -> (u8, u8, u8) {
        let c = if self.reverse { self.bg } else { self.fg };
        let (r, g, b) = c.to_rgb();
        if self.dim {
            (r / 2, g / 2, b / 2)
        } else {
            (r, g, b)
        }
    }

    /// Resolved background RGB — if reverse, swap fg/bg.
    pub fn bg_rgb(&self) -> Option<(u8, u8, u8)> {
        let c = if self.reverse { self.fg } else { self.bg };
        if c == AnsiColor::Default {
            None // transparent — use terminal bg
        } else {
            Some(c.to_rgb())
        }
    }
}

// ────────────────────────── Styled Span ──────────────────────────

/// A contiguous run of text with uniform styling.
#[derive(Debug, Clone)]
pub struct StyledSpan {
    pub text: String,
    pub style: TextStyle,
}

impl StyledSpan {
    pub fn plain(text: String) -> Self {
        Self {
            text,
            style: TextStyle::default(),
        }
    }

    pub fn styled(text: String, style: TextStyle) -> Self {
        Self { text, style }
    }
}

/// A single terminal line composed of styled spans.
#[derive(Debug, Clone)]
pub struct StyledLine {
    pub spans: Vec<StyledSpan>,
}

impl StyledLine {
    pub fn new() -> Self {
        Self { spans: Vec::new() }
    }

    pub fn from_plain(text: &str) -> Self {
        Self {
            spans: alloc::vec![StyledSpan::plain(String::from(text))],
        }
    }

    pub fn push(&mut self, span: StyledSpan) {
        // Merge with previous span if styles match and text is non-empty
        if let Some(last) = self.spans.last_mut() {
            if last.style == span.style {
                last.text.push_str(&span.text);
                return;
            }
        }
        if !span.text.is_empty() {
            self.spans.push(span);
        }
    }

    /// The raw text content (without styling) for searching, etc.
    pub fn plain_text(&self) -> String {
        let mut s = String::new();
        for span in &self.spans {
            s.push_str(&span.text);
        }
        s
    }

    /// Total character count.
    pub fn len(&self) -> usize {
        self.spans.iter().map(|s| s.text.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.spans.iter().all(|s| s.text.is_empty())
    }
}

impl Default for StyledLine {
    fn default() -> Self {
        Self::new()
    }
}

// ────────────────────────── Special Actions ──────────────────────────

/// Actions that escape sequences can request beyond text output.
#[derive(Debug, Clone)]
pub enum AnsiAction {
    /// Append a styled line to the output buffer.
    Line(StyledLine),
    /// Clear the entire screen.
    ClearScreen,
    /// Clear from cursor to end of line (we treat as no-op during full-line parsing).
    ClearToEol,
    /// Bell / alert.
    Bell,
}

// ────────────────────────── Parser ──────────────────────────

/// Parser state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    /// Normal text output
    Normal,
    /// Received ESC (0x1B)
    Escape,
    /// Inside CSI sequence (ESC [ ...)
    Csi,
}

/// Parse a raw string containing ANSI escape sequences into styled lines.
///
/// Each `\n` produces a new `StyledLine`. Escape codes update the current
/// style which is then applied to subsequent text.
pub fn parse_ansi(input: &str) -> Vec<AnsiAction> {
    let mut actions: Vec<AnsiAction> = Vec::new();
    let mut current_line = StyledLine::new();
    let mut current_text = String::new();
    let mut style = TextStyle::default();
    let mut state = ParserState::Normal;
    let mut csi_params = String::new();

    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match state {
            ParserState::Normal => {
                let ch = chars[i];
                match ch {
                    '\x1B' => {
                        // Flush accumulated text
                        if !current_text.is_empty() {
                            current_line.push(StyledSpan::styled(
                                core::mem::take(&mut current_text),
                                style,
                            ));
                        }
                        state = ParserState::Escape;
                    }
                    '\n' => {
                        // Flush text, then emit line
                        if !current_text.is_empty() {
                            current_line.push(StyledSpan::styled(
                                core::mem::take(&mut current_text),
                                style,
                            ));
                        }
                        actions.push(AnsiAction::Line(core::mem::take(&mut current_line)));
                    }
                    '\r' => {
                        // Carriage return — ignore (we normalise to \n line breaks)
                    }
                    '\x07' => {
                        // BEL
                        actions.push(AnsiAction::Bell);
                    }
                    '\t' => {
                        // Tab → spaces (8-column stops)
                        let col = current_text.len() % 8;
                        let spaces = 8 - col;
                        for _ in 0..spaces {
                            current_text.push(' ');
                        }
                    }
                    _ => {
                        current_text.push(ch);
                    }
                }
            }
            ParserState::Escape => {
                match chars[i] {
                    '[' => {
                        // Enter CSI mode
                        state = ParserState::Csi;
                        csi_params.clear();
                    }
                    _ => {
                        // Unknown escape — ignore and go back to normal
                        state = ParserState::Normal;
                    }
                }
            }
            ParserState::Csi => {
                let ch = chars[i];
                if ch.is_ascii_digit() || ch == ';' || ch == ':' {
                    csi_params.push(ch);
                } else {
                    // ch is the final byte — dispatch
                    match ch {
                        'm' => {
                            // SGR — Select Graphic Rendition
                            apply_sgr(&csi_params, &mut style);
                        }
                        'J' => {
                            // Erase in Display
                            let n = parse_param_default(&csi_params, 0);
                            if n == 2 || n == 3 {
                                actions.push(AnsiAction::ClearScreen);
                            }
                        }
                        'K' => {
                            // Erase in Line
                            actions.push(AnsiAction::ClearToEol);
                        }
                        'H' | 'f' => {
                            // Cursor position — we don't handle absolute positioning
                            // in a line-oriented model. Ignore.
                        }
                        'A' | 'B' | 'C' | 'D' => {
                            // Cursor movement — ignore in line-oriented model
                        }
                        _ => {
                            // Unknown CSI sequence — ignore
                        }
                    }
                    state = ParserState::Normal;
                }
            }
        }
        i += 1;
    }

    // Flush remaining text
    if !current_text.is_empty() {
        current_line.push(StyledSpan::styled(current_text, style));
    }
    if !current_line.is_empty() {
        actions.push(AnsiAction::Line(current_line));
    }

    actions
}

/// Convenience: parse and extract only the styled lines (discard clear/bell actions).
pub fn parse_ansi_lines(input: &str) -> Vec<StyledLine> {
    parse_ansi(input)
        .into_iter()
        .filter_map(|a| match a {
            AnsiAction::Line(l) => Some(l),
            _ => None,
        })
        .collect()
}

/// Apply SGR (Select Graphic Rendition) parameters to a style.
fn apply_sgr(params: &str, style: &mut TextStyle) {
    if params.is_empty() {
        style.reset();
        return;
    }

    let codes: Vec<u32> = params
        .split(';')
        .filter_map(|s| s.parse::<u32>().ok())
        .collect();

    let mut j = 0;
    while j < codes.len() {
        match codes[j] {
            0 => style.reset(),
            1 => style.bold = true,
            2 => style.dim = true,
            3 => style.italic = true,
            4 => style.underline = true,
            5 | 6 => style.blink = true,
            7 => style.reverse = true,
            8 => {} // hidden — not supported
            9 => style.strikethrough = true,
            21 => style.bold = false, // doubly-underlined or bold off
            22 => {
                style.bold = false;
                style.dim = false;
            }
            23 => style.italic = false,
            24 => style.underline = false,
            25 => style.blink = false,
            27 => style.reverse = false,
            29 => style.strikethrough = false,

            // Foreground standard colours 30–37
            30..=37 => style.fg = AnsiColor::Standard((codes[j] - 30) as u8),
            // Extended foreground: 38;5;n or 38;2;r;g;b
            38 => {
                if j + 1 < codes.len() {
                    match codes[j + 1] {
                        5 if j + 2 < codes.len() => {
                            style.fg = AnsiColor::Indexed(codes[j + 2] as u8);
                            j += 2;
                        }
                        2 if j + 4 < codes.len() => {
                            style.fg = AnsiColor::Rgb(
                                codes[j + 2] as u8,
                                codes[j + 3] as u8,
                                codes[j + 4] as u8,
                            );
                            j += 4;
                        }
                        _ => {}
                    }
                }
            }
            39 => style.fg = AnsiColor::Default,

            // Background standard colours 40–47
            40..=47 => style.bg = AnsiColor::Standard((codes[j] - 40) as u8),
            // Extended background: 48;5;n or 48;2;r;g;b
            48 => {
                if j + 1 < codes.len() {
                    match codes[j + 1] {
                        5 if j + 2 < codes.len() => {
                            style.bg = AnsiColor::Indexed(codes[j + 2] as u8);
                            j += 2;
                        }
                        2 if j + 4 < codes.len() => {
                            style.bg = AnsiColor::Rgb(
                                codes[j + 2] as u8,
                                codes[j + 3] as u8,
                                codes[j + 4] as u8,
                            );
                            j += 4;
                        }
                        _ => {}
                    }
                }
            }
            49 => style.bg = AnsiColor::Default,

            // Bright foreground 90–97
            90..=97 => style.fg = AnsiColor::Bright((codes[j] - 90) as u8),
            // Bright background 100–107
            100..=107 => style.bg = AnsiColor::Bright((codes[j] - 100) as u8),

            _ => {} // unknown — ignore
        }
        j += 1;
    }
}

/// Parse a single numeric parameter with a default.
fn parse_param_default(params: &str, default: u32) -> u32 {
    if params.is_empty() {
        default
    } else {
        params.parse::<u32>().unwrap_or(default)
    }
}

// ────────────────────────── Convenience Escape Builders ──────────────────────────

/// Build an SGR escape string for the given parameters.
/// Usage: `sgr(&[1, 31])` → `"\x1B[1;31m"` (bold red foreground)
pub fn sgr(params: &[u8]) -> String {
    let mut s = String::from("\x1B[");
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            s.push(';');
        }
        push_u8_str(&mut s, *p);
    }
    s.push('m');
    s
}

/// Reset all attributes.
pub fn reset() -> &'static str {
    "\x1B[0m"
}

/// Bold text.
pub fn bold() -> &'static str {
    "\x1B[1m"
}

/// Dim text.
pub fn dim() -> &'static str {
    "\x1B[2m"
}

/// Standard foreground color.
pub fn fg(color: u8) -> String {
    alloc::format!("\x1B[{}m", 30 + color)
}

/// Bright foreground color.
pub fn fg_bright(color: u8) -> String {
    alloc::format!("\x1B[{}m", 90 + color)
}

/// Standard background color.
pub fn bg(color: u8) -> String {
    alloc::format!("\x1B[{}m", 40 + color)
}

/// Clear screen.
pub fn clear_screen() -> &'static str {
    "\x1B[2J"
}

/// Clear to end of line.
pub fn clear_eol() -> &'static str {
    "\x1B[K"
}

// Named convenience functions for common colours
pub fn red() -> String {
    fg(AnsiColor::RED)
}
pub fn green() -> String {
    fg(AnsiColor::GREEN)
}
pub fn yellow() -> String {
    fg(AnsiColor::YELLOW)
}
pub fn blue() -> String {
    fg(AnsiColor::BLUE)
}
pub fn magenta() -> String {
    fg(AnsiColor::MAGENTA)
}
pub fn cyan() -> String {
    fg(AnsiColor::CYAN)
}
pub fn white() -> String {
    fg(AnsiColor::WHITE)
}

/// Push a u8 as decimal digits onto a string (no alloc formatting).
fn push_u8_str(s: &mut String, mut n: u8) {
    if n >= 100 {
        s.push((b'0' + n / 100) as char);
        n %= 100;
        s.push((b'0' + n / 10) as char);
        s.push((b'0' + n % 10) as char);
    } else if n >= 10 {
        s.push((b'0' + n / 10) as char);
        s.push((b'0' + n % 10) as char);
    } else {
        s.push((b'0' + n) as char);
    }
}
