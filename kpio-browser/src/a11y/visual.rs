//! Visual Accessibility
//!
//! High contrast, color adjustments, and visual enhancements.

use alloc::string::String;

/// Color scheme
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    /// Light theme
    Light,
    /// Dark theme
    Dark,
    /// System preference
    System,
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self::System
    }
}

/// High contrast mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighContrastMode {
    /// Off
    Off,
    /// More contrast
    More,
    /// Less contrast
    Less,
    /// Custom
    Custom,
}

impl Default for HighContrastMode {
    fn default() -> Self {
        Self::Off
    }
}

/// Color filter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorFilter {
    /// No filter
    None,
    /// Protanopia (red-blind)
    Protanopia,
    /// Deuteranopia (green-blind)
    Deuteranopia,
    /// Tritanopia (blue-blind)
    Tritanopia,
    /// Achromatopsia (complete color blindness)
    Achromatopsia,
    /// Invert colors
    Invert,
    /// Grayscale
    Grayscale,
}

impl Default for ColorFilter {
    fn default() -> Self {
        Self::None
    }
}

/// Visual settings
#[derive(Debug, Clone)]
pub struct VisualSettings {
    /// Color scheme
    pub color_scheme: ColorScheme,
    /// High contrast
    pub high_contrast: HighContrastMode,
    /// Color filter
    pub color_filter: ColorFilter,
    /// Font size multiplier (1.0 = 100%)
    pub font_scale: f32,
    /// Minimum font size in pixels
    pub min_font_size: u32,
    /// Force font family
    pub force_font: Option<String>,
    /// Underline links
    pub underline_links: bool,
    /// Reduce transparency
    pub reduce_transparency: bool,
    /// Reduce motion
    pub reduce_motion: bool,
    /// Focus highlight width
    pub focus_width: u32,
    /// Cursor size multiplier
    pub cursor_scale: f32,
    /// Caret width
    pub caret_width: u32,
    /// Animation speed (0.0 = instant, 1.0 = normal)
    pub animation_speed: f32,
}

impl Default for VisualSettings {
    fn default() -> Self {
        Self {
            color_scheme: ColorScheme::System,
            high_contrast: HighContrastMode::Off,
            color_filter: ColorFilter::None,
            font_scale: 1.0,
            min_font_size: 12,
            force_font: None,
            underline_links: false,
            reduce_transparency: false,
            reduce_motion: false,
            focus_width: 2,
            cursor_scale: 1.0,
            caret_width: 2,
            animation_speed: 1.0,
        }
    }
}

impl VisualSettings {
    /// Create new settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate effective font size
    pub fn effective_font_size(&self, base_size: f32) -> f32 {
        let scaled = base_size * self.font_scale;
        scaled.max(self.min_font_size as f32)
    }

    /// Should animations be disabled
    pub fn disable_animations(&self) -> bool {
        self.reduce_motion || self.animation_speed == 0.0
    }

    /// Get animation duration multiplier
    pub fn animation_duration_multiplier(&self) -> f32 {
        if self.reduce_motion {
            0.0
        } else {
            1.0 / self.animation_speed.max(0.01)
        }
    }
}

/// Color adjustments
pub struct ColorAdjuster {
    /// Color filter
    filter: ColorFilter,
    /// Contrast adjustment (-1.0 to 1.0)
    contrast: f32,
    /// Brightness adjustment (-1.0 to 1.0)
    brightness: f32,
    /// Saturation adjustment (0.0 to 2.0)
    saturation: f32,
}

impl ColorAdjuster {
    /// Create new adjuster
    pub fn new() -> Self {
        Self {
            filter: ColorFilter::None,
            contrast: 0.0,
            brightness: 0.0,
            saturation: 1.0,
        }
    }

    /// Set color filter
    pub fn with_filter(mut self, filter: ColorFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Set contrast
    pub fn with_contrast(mut self, contrast: f32) -> Self {
        self.contrast = contrast.clamp(-1.0, 1.0);
        self
    }

    /// Set brightness
    pub fn with_brightness(mut self, brightness: f32) -> Self {
        self.brightness = brightness.clamp(-1.0, 1.0);
        self
    }

    /// Adjust color
    pub fn adjust(&self, color: u32) -> u32 {
        let a = (color >> 24) & 0xFF;
        let r = ((color >> 16) & 0xFF) as f32;
        let g = ((color >> 8) & 0xFF) as f32;
        let b = (color & 0xFF) as f32;

        // Apply filter first
        let (r, g, b) = self.apply_filter(r, g, b);

        // Apply brightness
        let r = r + (255.0 * self.brightness);
        let g = g + (255.0 * self.brightness);
        let b = b + (255.0 * self.brightness);

        // Apply contrast
        let factor = (1.0 + self.contrast) / (1.0 - self.contrast.min(0.99));
        let r = ((r - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u32;
        let g = ((g - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u32;
        let b = ((b - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u32;

        (a << 24) | (r << 16) | (g << 8) | b
    }

    /// Apply color filter
    fn apply_filter(&self, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        match self.filter {
            ColorFilter::None => (r, g, b),
            ColorFilter::Grayscale => {
                let gray = r * 0.299 + g * 0.587 + b * 0.114;
                (gray, gray, gray)
            }
            ColorFilter::Invert => (255.0 - r, 255.0 - g, 255.0 - b),
            ColorFilter::Protanopia => {
                // Simulate red-blindness
                let new_r = 0.567 * r + 0.433 * g;
                let new_g = 0.558 * r + 0.442 * g;
                let new_b = 0.242 * g + 0.758 * b;
                (new_r, new_g, new_b)
            }
            ColorFilter::Deuteranopia => {
                // Simulate green-blindness
                let new_r = 0.625 * r + 0.375 * g;
                let new_g = 0.700 * r + 0.300 * g;
                let new_b = 0.300 * g + 0.700 * b;
                (new_r, new_g, new_b)
            }
            ColorFilter::Tritanopia => {
                // Simulate blue-blindness
                let new_r = 0.950 * r + 0.050 * g;
                let new_g = 0.433 * g + 0.567 * b;
                let new_b = 0.475 * g + 0.525 * b;
                (new_r, new_g, new_b)
            }
            ColorFilter::Achromatopsia => {
                // Complete color blindness (rod monochromacy)
                let gray = r * 0.212 + g * 0.715 + b * 0.072;
                (gray, gray, gray)
            }
        }
    }
}

impl Default for ColorAdjuster {
    fn default() -> Self {
        Self::new()
    }
}

/// Text spacing settings
#[derive(Debug, Clone)]
pub struct TextSpacing {
    /// Line height multiplier
    pub line_height: f32,
    /// Letter spacing in em
    pub letter_spacing: f32,
    /// Word spacing in em
    pub word_spacing: f32,
    /// Paragraph spacing in em
    pub paragraph_spacing: f32,
}

impl Default for TextSpacing {
    fn default() -> Self {
        Self {
            line_height: 1.5,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            paragraph_spacing: 2.0,
        }
    }
}

impl TextSpacing {
    /// WCAG 2.1 minimum spacing
    pub fn wcag_minimum() -> Self {
        Self {
            line_height: 1.5,
            letter_spacing: 0.12,
            word_spacing: 0.16,
            paragraph_spacing: 2.0,
        }
    }

    /// Dyslexia-friendly spacing
    pub fn dyslexia_friendly() -> Self {
        Self {
            line_height: 2.0,
            letter_spacing: 0.35,
            word_spacing: 0.35,
            paragraph_spacing: 2.5,
        }
    }
}

/// Reading guide
#[derive(Debug, Clone)]
pub struct ReadingGuide {
    /// Enabled
    pub enabled: bool,
    /// Width of visible area
    pub width: u32,
    /// Height of visible area  
    pub height: u32,
    /// Tint color
    pub tint: u32,
    /// Opacity of mask
    pub mask_opacity: f32,
}

impl Default for ReadingGuide {
    fn default() -> Self {
        Self {
            enabled: false,
            width: 800,
            height: 100,
            tint: 0xFFFFFACD, // Light yellow
            mask_opacity: 0.5,
        }
    }
}
