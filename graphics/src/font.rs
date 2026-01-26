//! Font Rendering Module
//!
//! This module provides bitmap and TrueType font rendering for the graphics system.
//! Supports basic text shaping, glyph caching, and font metrics.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

/// Font style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

impl Default for FontStyle {
    fn default() -> Self {
        FontStyle::Normal
    }
}

/// Font weight (100-900).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

/// Font metrics describing the dimensions of a font.
#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    /// Units per em (typically 1000 or 2048).
    pub units_per_em: u16,
    /// Ascender in font units.
    pub ascender: i16,
    /// Descender in font units (typically negative).
    pub descender: i16,
    /// Line gap in font units.
    pub line_gap: i16,
    /// Advance width of space character.
    pub space_advance: u16,
    /// x-height (height of lowercase 'x').
    pub x_height: i16,
    /// Cap height (height of capital letters).
    pub cap_height: i16,
}

impl FontMetrics {
    /// Calculate line height for a given font size.
    pub fn line_height(&self, font_size: f32) -> f32 {
        let scale = font_size / self.units_per_em as f32;
        ((self.ascender - self.descender) as f32 + self.line_gap as f32) * scale
    }
    
    /// Calculate ascender for a given font size.
    pub fn scaled_ascender(&self, font_size: f32) -> f32 {
        font_size * self.ascender as f32 / self.units_per_em as f32
    }
    
    /// Calculate descender for a given font size.
    pub fn scaled_descender(&self, font_size: f32) -> f32 {
        font_size * self.descender as f32 / self.units_per_em as f32
    }
}

impl Default for FontMetrics {
    fn default() -> Self {
        // Default metrics for 8x16 bitmap font
        Self {
            units_per_em: 16,
            ascender: 12,
            descender: -4,
            line_gap: 0,
            space_advance: 8,
            x_height: 8,
            cap_height: 12,
        }
    }
}

/// Glyph data for a single character.
#[derive(Debug, Clone)]
pub struct Glyph {
    /// Character code.
    pub codepoint: char,
    /// Glyph width in pixels.
    pub width: u32,
    /// Glyph height in pixels.
    pub height: u32,
    /// X bearing (offset from origin to left edge).
    pub bearing_x: i32,
    /// Y bearing (offset from baseline to top edge).
    pub bearing_y: i32,
    /// Horizontal advance width.
    pub advance: u32,
    /// Bitmap data (alpha values, row-major).
    pub bitmap: Vec<u8>,
}

impl Glyph {
    /// Create a new glyph.
    pub fn new(codepoint: char, width: u32, height: u32) -> Self {
        Self {
            codepoint,
            width,
            height,
            bearing_x: 0,
            bearing_y: height as i32,
            advance: width,
            bitmap: vec![0; (width * height) as usize],
        }
    }
    
    /// Get pixel alpha at (x, y).
    pub fn get_pixel(&self, x: u32, y: u32) -> u8 {
        if x < self.width && y < self.height {
            self.bitmap[(y * self.width + x) as usize]
        } else {
            0
        }
    }
    
    /// Set pixel alpha at (x, y).
    pub fn set_pixel(&mut self, x: u32, y: u32, alpha: u8) {
        if x < self.width && y < self.height {
            self.bitmap[(y * self.width + x) as usize] = alpha;
        }
    }
}

/// Bitmap font (fixed-width, 8x16 pixels per character).
pub struct BitmapFont {
    /// Font data (256 characters * 16 rows).
    data: Vec<u8>,
    /// Character width.
    pub char_width: u32,
    /// Character height.
    pub char_height: u32,
}

impl BitmapFont {
    /// Create a new bitmap font from raw data.
    pub fn new(data: &[u8], char_width: u32, char_height: u32) -> Self {
        Self {
            data: data.to_vec(),
            char_width,
            char_height,
        }
    }
    
    /// Create a default 8x16 bitmap font.
    pub fn default_font() -> Self {
        Self {
            data: Self::generate_default_font(),
            char_width: 8,
            char_height: 16,
        }
    }
    
    /// Generate a simple default font.
    fn generate_default_font() -> Vec<u8> {
        let mut data = vec![0u8; 256 * 16];
        
        // Generate basic ASCII glyphs (simplified)
        // Space (32)
        // Already zeros
        
        // Generate simple block letters for A-Z (65-90)
        for c in 65u8..=90 {
            let idx = c as usize * 16;
            Self::draw_simple_char(&mut data[idx..idx+16], c);
        }
        
        // Generate a-z (97-122) as smaller versions
        for c in 97u8..=122 {
            let idx = c as usize * 16;
            Self::draw_simple_char(&mut data[idx..idx+16], c);
        }
        
        // Generate 0-9 (48-57)
        for c in 48u8..=57 {
            let idx = c as usize * 16;
            Self::draw_simple_char(&mut data[idx..idx+16], c);
        }
        
        // Common punctuation
        Self::draw_simple_char(&mut data[33*16..34*16], b'!');
        Self::draw_simple_char(&mut data[46*16..47*16], b'.');
        Self::draw_simple_char(&mut data[44*16..45*16], b',');
        Self::draw_simple_char(&mut data[58*16..59*16], b':');
        Self::draw_simple_char(&mut data[59*16..60*16], b';');
        Self::draw_simple_char(&mut data[63*16..64*16], b'?');
        
        data
    }
    
    /// Draw a simple character glyph.
    fn draw_simple_char(glyph: &mut [u8], c: u8) {
        match c {
            // Uppercase letters (simplified block representation)
            b'A' => {
                glyph[2] = 0b00111100;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01100110;
                glyph[5] = 0b01111110;
                glyph[6] = 0b01100110;
                glyph[7] = 0b01100110;
                glyph[8] = 0b01100110;
            }
            b'B' => {
                glyph[2] = 0b01111100;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01111100;
                glyph[5] = 0b01100110;
                glyph[6] = 0b01100110;
                glyph[7] = 0b01100110;
                glyph[8] = 0b01111100;
            }
            b'C' => {
                glyph[2] = 0b00111100;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01100000;
                glyph[5] = 0b01100000;
                glyph[6] = 0b01100000;
                glyph[7] = 0b01100110;
                glyph[8] = 0b00111100;
            }
            b'D' => {
                glyph[2] = 0b01111000;
                glyph[3] = 0b01101100;
                glyph[4] = 0b01100110;
                glyph[5] = 0b01100110;
                glyph[6] = 0b01100110;
                glyph[7] = 0b01101100;
                glyph[8] = 0b01111000;
            }
            b'E' => {
                glyph[2] = 0b01111110;
                glyph[3] = 0b01100000;
                glyph[4] = 0b01100000;
                glyph[5] = 0b01111100;
                glyph[6] = 0b01100000;
                glyph[7] = 0b01100000;
                glyph[8] = 0b01111110;
            }
            b'F' => {
                glyph[2] = 0b01111110;
                glyph[3] = 0b01100000;
                glyph[4] = 0b01100000;
                glyph[5] = 0b01111100;
                glyph[6] = 0b01100000;
                glyph[7] = 0b01100000;
                glyph[8] = 0b01100000;
            }
            b'G' => {
                glyph[2] = 0b00111100;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01100000;
                glyph[5] = 0b01101110;
                glyph[6] = 0b01100110;
                glyph[7] = 0b01100110;
                glyph[8] = 0b00111100;
            }
            b'H' => {
                glyph[2] = 0b01100110;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01100110;
                glyph[5] = 0b01111110;
                glyph[6] = 0b01100110;
                glyph[7] = 0b01100110;
                glyph[8] = 0b01100110;
            }
            b'I' => {
                glyph[2] = 0b00111100;
                glyph[3] = 0b00011000;
                glyph[4] = 0b00011000;
                glyph[5] = 0b00011000;
                glyph[6] = 0b00011000;
                glyph[7] = 0b00011000;
                glyph[8] = 0b00111100;
            }
            b'J' => {
                glyph[2] = 0b00011110;
                glyph[3] = 0b00001100;
                glyph[4] = 0b00001100;
                glyph[5] = 0b00001100;
                glyph[6] = 0b01101100;
                glyph[7] = 0b01101100;
                glyph[8] = 0b00111000;
            }
            b'K' => {
                glyph[2] = 0b01100110;
                glyph[3] = 0b01101100;
                glyph[4] = 0b01111000;
                glyph[5] = 0b01110000;
                glyph[6] = 0b01111000;
                glyph[7] = 0b01101100;
                glyph[8] = 0b01100110;
            }
            b'L' => {
                glyph[2] = 0b01100000;
                glyph[3] = 0b01100000;
                glyph[4] = 0b01100000;
                glyph[5] = 0b01100000;
                glyph[6] = 0b01100000;
                glyph[7] = 0b01100000;
                glyph[8] = 0b01111110;
            }
            b'M' => {
                glyph[2] = 0b01100011;
                glyph[3] = 0b01110111;
                glyph[4] = 0b01111111;
                glyph[5] = 0b01101011;
                glyph[6] = 0b01100011;
                glyph[7] = 0b01100011;
                glyph[8] = 0b01100011;
            }
            b'N' => {
                glyph[2] = 0b01100011;
                glyph[3] = 0b01110011;
                glyph[4] = 0b01111011;
                glyph[5] = 0b01101111;
                glyph[6] = 0b01100111;
                glyph[7] = 0b01100011;
                glyph[8] = 0b01100011;
            }
            b'O' => {
                glyph[2] = 0b00111100;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01100110;
                glyph[5] = 0b01100110;
                glyph[6] = 0b01100110;
                glyph[7] = 0b01100110;
                glyph[8] = 0b00111100;
            }
            b'P' => {
                glyph[2] = 0b01111100;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01100110;
                glyph[5] = 0b01111100;
                glyph[6] = 0b01100000;
                glyph[7] = 0b01100000;
                glyph[8] = 0b01100000;
            }
            b'Q' => {
                glyph[2] = 0b00111100;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01100110;
                glyph[5] = 0b01100110;
                glyph[6] = 0b01101110;
                glyph[7] = 0b00111100;
                glyph[8] = 0b00000110;
            }
            b'R' => {
                glyph[2] = 0b01111100;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01100110;
                glyph[5] = 0b01111100;
                glyph[6] = 0b01101100;
                glyph[7] = 0b01100110;
                glyph[8] = 0b01100110;
            }
            b'S' => {
                glyph[2] = 0b00111100;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01110000;
                glyph[5] = 0b00111100;
                glyph[6] = 0b00001110;
                glyph[7] = 0b01100110;
                glyph[8] = 0b00111100;
            }
            b'T' => {
                glyph[2] = 0b01111110;
                glyph[3] = 0b00011000;
                glyph[4] = 0b00011000;
                glyph[5] = 0b00011000;
                glyph[6] = 0b00011000;
                glyph[7] = 0b00011000;
                glyph[8] = 0b00011000;
            }
            b'U' => {
                glyph[2] = 0b01100110;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01100110;
                glyph[5] = 0b01100110;
                glyph[6] = 0b01100110;
                glyph[7] = 0b01100110;
                glyph[8] = 0b00111100;
            }
            b'V' => {
                glyph[2] = 0b01100110;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01100110;
                glyph[5] = 0b01100110;
                glyph[6] = 0b00111100;
                glyph[7] = 0b00011000;
                glyph[8] = 0b00011000;
            }
            b'W' => {
                glyph[2] = 0b01100011;
                glyph[3] = 0b01100011;
                glyph[4] = 0b01100011;
                glyph[5] = 0b01101011;
                glyph[6] = 0b01111111;
                glyph[7] = 0b01110111;
                glyph[8] = 0b01100011;
            }
            b'X' => {
                glyph[2] = 0b01100110;
                glyph[3] = 0b01100110;
                glyph[4] = 0b00111100;
                glyph[5] = 0b00011000;
                glyph[6] = 0b00111100;
                glyph[7] = 0b01100110;
                glyph[8] = 0b01100110;
            }
            b'Y' => {
                glyph[2] = 0b01100110;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01100110;
                glyph[5] = 0b00111100;
                glyph[6] = 0b00011000;
                glyph[7] = 0b00011000;
                glyph[8] = 0b00011000;
            }
            b'Z' => {
                glyph[2] = 0b01111110;
                glyph[3] = 0b00000110;
                glyph[4] = 0b00001100;
                glyph[5] = 0b00011000;
                glyph[6] = 0b00110000;
                glyph[7] = 0b01100000;
                glyph[8] = 0b01111110;
            }
            // Lowercase letters (positioned lower)
            b'a'..=b'z' => {
                // Copy uppercase pattern but offset
                let upper = c - 32;
                let mut temp = [0u8; 16];
                Self::draw_simple_char(&mut temp, upper);
                for i in 0..16 {
                    glyph[i] = if i >= 3 && i < 11 { temp[i - 1] } else { 0 };
                }
            }
            // Numbers
            b'0' => {
                glyph[2] = 0b00111100;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01101110;
                glyph[5] = 0b01110110;
                glyph[6] = 0b01100110;
                glyph[7] = 0b01100110;
                glyph[8] = 0b00111100;
            }
            b'1' => {
                glyph[2] = 0b00011000;
                glyph[3] = 0b00111000;
                glyph[4] = 0b00011000;
                glyph[5] = 0b00011000;
                glyph[6] = 0b00011000;
                glyph[7] = 0b00011000;
                glyph[8] = 0b01111110;
            }
            b'2' => {
                glyph[2] = 0b00111100;
                glyph[3] = 0b01100110;
                glyph[4] = 0b00000110;
                glyph[5] = 0b00011100;
                glyph[6] = 0b00110000;
                glyph[7] = 0b01100000;
                glyph[8] = 0b01111110;
            }
            b'3' => {
                glyph[2] = 0b00111100;
                glyph[3] = 0b01100110;
                glyph[4] = 0b00000110;
                glyph[5] = 0b00011100;
                glyph[6] = 0b00000110;
                glyph[7] = 0b01100110;
                glyph[8] = 0b00111100;
            }
            b'4' => {
                glyph[2] = 0b00001100;
                glyph[3] = 0b00011100;
                glyph[4] = 0b00101100;
                glyph[5] = 0b01001100;
                glyph[6] = 0b01111110;
                glyph[7] = 0b00001100;
                glyph[8] = 0b00001100;
            }
            b'5' => {
                glyph[2] = 0b01111110;
                glyph[3] = 0b01100000;
                glyph[4] = 0b01111100;
                glyph[5] = 0b00000110;
                glyph[6] = 0b00000110;
                glyph[7] = 0b01100110;
                glyph[8] = 0b00111100;
            }
            b'6' => {
                glyph[2] = 0b00111100;
                glyph[3] = 0b01100000;
                glyph[4] = 0b01111100;
                glyph[5] = 0b01100110;
                glyph[6] = 0b01100110;
                glyph[7] = 0b01100110;
                glyph[8] = 0b00111100;
            }
            b'7' => {
                glyph[2] = 0b01111110;
                glyph[3] = 0b00000110;
                glyph[4] = 0b00001100;
                glyph[5] = 0b00011000;
                glyph[6] = 0b00110000;
                glyph[7] = 0b00110000;
                glyph[8] = 0b00110000;
            }
            b'8' => {
                glyph[2] = 0b00111100;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01100110;
                glyph[5] = 0b00111100;
                glyph[6] = 0b01100110;
                glyph[7] = 0b01100110;
                glyph[8] = 0b00111100;
            }
            b'9' => {
                glyph[2] = 0b00111100;
                glyph[3] = 0b01100110;
                glyph[4] = 0b01100110;
                glyph[5] = 0b00111110;
                glyph[6] = 0b00000110;
                glyph[7] = 0b00000110;
                glyph[8] = 0b00111100;
            }
            // Punctuation
            b'!' => {
                glyph[2] = 0b00011000;
                glyph[3] = 0b00011000;
                glyph[4] = 0b00011000;
                glyph[5] = 0b00011000;
                glyph[6] = 0b00000000;
                glyph[7] = 0b00000000;
                glyph[8] = 0b00011000;
            }
            b'.' => {
                glyph[8] = 0b00011000;
                glyph[9] = 0b00011000;
            }
            b',' => {
                glyph[8] = 0b00011000;
                glyph[9] = 0b00011000;
                glyph[10] = 0b00110000;
            }
            b':' => {
                glyph[4] = 0b00011000;
                glyph[5] = 0b00011000;
                glyph[7] = 0b00011000;
                glyph[8] = 0b00011000;
            }
            b';' => {
                glyph[4] = 0b00011000;
                glyph[5] = 0b00011000;
                glyph[7] = 0b00011000;
                glyph[8] = 0b00011000;
                glyph[9] = 0b00110000;
            }
            b'?' => {
                glyph[2] = 0b00111100;
                glyph[3] = 0b01100110;
                glyph[4] = 0b00000110;
                glyph[5] = 0b00001100;
                glyph[6] = 0b00011000;
                glyph[7] = 0b00000000;
                glyph[8] = 0b00011000;
            }
            _ => {}
        }
    }
    
    /// Get glyph for a character.
    pub fn get_glyph(&self, c: char) -> Glyph {
        let code = c as u32;
        let idx = if code < 256 { code as usize } else { 0 };
        
        let mut glyph = Glyph::new(c, self.char_width, self.char_height);
        glyph.advance = self.char_width;
        glyph.bearing_y = 12; // Baseline offset
        
        // Convert bitmap row data to alpha values
        let row_start = idx * self.char_height as usize;
        for row in 0..self.char_height {
            let byte = self.data.get(row_start + row as usize).copied().unwrap_or(0);
            for col in 0..8 {
                if byte & (0x80 >> col) != 0 {
                    glyph.set_pixel(col, row, 255);
                }
            }
        }
        
        glyph
    }
    
    /// Get font metrics.
    pub fn metrics(&self) -> FontMetrics {
        FontMetrics {
            units_per_em: self.char_height as u16,
            ascender: 12,
            descender: -4,
            line_gap: 0,
            space_advance: self.char_width as u16,
            x_height: 8,
            cap_height: 12,
        }
    }
    
    /// Measure text width.
    pub fn measure_text(&self, text: &str) -> u32 {
        text.chars().count() as u32 * self.char_width
    }
}

impl Default for BitmapFont {
    fn default() -> Self {
        Self::default_font()
    }
}

/// Text renderer using a bitmap font.
pub struct TextRenderer {
    font: BitmapFont,
    /// Glyph cache.
    cache: BTreeMap<char, Glyph>,
}

impl TextRenderer {
    /// Create a new text renderer with the default font.
    pub fn new() -> Self {
        Self {
            font: BitmapFont::default(),
            cache: BTreeMap::new(),
        }
    }
    
    /// Create a text renderer with a custom font.
    pub fn with_font(font: BitmapFont) -> Self {
        Self {
            font,
            cache: BTreeMap::new(),
        }
    }
    
    /// Get a glyph, using cache if available.
    pub fn get_glyph(&mut self, c: char) -> &Glyph {
        if !self.cache.contains_key(&c) {
            let glyph = self.font.get_glyph(c);
            self.cache.insert(c, glyph);
        }
        self.cache.get(&c).unwrap()
    }
    
    /// Render text to a framebuffer.
    pub fn render_text(
        &mut self,
        text: &str,
        x: i32,
        y: i32,
        color: u32,
        framebuffer: &mut [u32],
        fb_width: u32,
        fb_height: u32,
    ) {
        let mut cursor_x = x;
        let baseline_y = y + self.font.metrics().ascender as i32;
        
        for c in text.chars() {
            let glyph = self.font.get_glyph(c);
            
            let glyph_x = cursor_x + glyph.bearing_x;
            let glyph_y = baseline_y - glyph.bearing_y;
            
            // Render glyph
            for gy in 0..glyph.height {
                for gx in 0..glyph.width {
                    let alpha = glyph.get_pixel(gx, gy);
                    if alpha > 0 {
                        let px = glyph_x + gx as i32;
                        let py = glyph_y + gy as i32;
                        
                        if px >= 0 && py >= 0 && (px as u32) < fb_width && (py as u32) < fb_height {
                            let idx = (py as u32 * fb_width + px as u32) as usize;
                            if idx < framebuffer.len() {
                                framebuffer[idx] = Self::blend_pixel(framebuffer[idx], color, alpha);
                            }
                        }
                    }
                }
            }
            
            cursor_x += glyph.advance as i32;
        }
    }
    
    /// Blend a pixel with alpha.
    fn blend_pixel(bg: u32, fg: u32, alpha: u8) -> u32 {
        if alpha == 255 {
            return fg;
        }
        if alpha == 0 {
            return bg;
        }
        
        let a = alpha as u32;
        let inv_a = 255 - a;
        
        let bg_r = (bg >> 16) & 0xFF;
        let bg_g = (bg >> 8) & 0xFF;
        let bg_b = bg & 0xFF;
        
        let fg_r = (fg >> 16) & 0xFF;
        let fg_g = (fg >> 8) & 0xFF;
        let fg_b = fg & 0xFF;
        
        let r = (fg_r * a + bg_r * inv_a) / 255;
        let g = (fg_g * a + bg_g * inv_a) / 255;
        let b = (fg_b * a + bg_b * inv_a) / 255;
        
        0xFF000000 | (r << 16) | (g << 8) | b
    }
    
    /// Measure text dimensions.
    pub fn measure_text(&self, text: &str) -> (u32, u32) {
        let width = self.font.measure_text(text);
        let height = self.font.char_height;
        (width, height)
    }
    
    /// Get line height.
    pub fn line_height(&self) -> u32 {
        self.font.char_height
    }
    
    /// Get font metrics.
    pub fn metrics(&self) -> FontMetrics {
        self.font.metrics()
    }
}

impl Default for TextRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Scaled font for rendering at different sizes.
pub struct ScaledFont {
    /// Base bitmap font.
    base_font: BitmapFont,
    /// Target font size.
    target_size: f32,
    /// Scale factor.
    scale: f32,
}

impl ScaledFont {
    /// Create a scaled font.
    pub fn new(base_font: BitmapFont, target_size: f32) -> Self {
        let scale = target_size / base_font.char_height as f32;
        Self {
            base_font,
            target_size,
            scale,
        }
    }
    
    /// Get scaled glyph.
    pub fn get_glyph(&self, c: char) -> Glyph {
        let base_glyph = self.base_font.get_glyph(c);
        
        if (self.scale - 1.0).abs() < 0.01 {
            return base_glyph;
        }
        
        let new_width = ((base_glyph.width as f32 * self.scale) as u32).max(1);
        let new_height = ((base_glyph.height as f32 * self.scale) as u32).max(1);
        
        let mut scaled = Glyph::new(c, new_width, new_height);
        scaled.advance = (base_glyph.advance as f32 * self.scale) as u32;
        scaled.bearing_x = (base_glyph.bearing_x as f32 * self.scale) as i32;
        scaled.bearing_y = (base_glyph.bearing_y as f32 * self.scale) as i32;
        
        // Simple nearest-neighbor scaling
        for y in 0..new_height {
            for x in 0..new_width {
                let src_x = (x as f32 / self.scale) as u32;
                let src_y = (y as f32 / self.scale) as u32;
                let alpha = base_glyph.get_pixel(src_x, src_y);
                scaled.set_pixel(x, y, alpha);
            }
        }
        
        scaled
    }
    
    /// Measure text at scaled size.
    pub fn measure_text(&self, text: &str) -> (f32, f32) {
        let base_width = self.base_font.measure_text(text) as f32;
        (base_width * self.scale, self.target_size)
    }
}
