//! Design Tokens
//!
//! Core design values used throughout the UI.

use alloc::string::String;

/// Color in RGBA format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Create new color
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create opaque color
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// Create from hex value (0xRRGGBB)
    pub const fn from_hex(hex: u32) -> Self {
        Self::rgb(
            ((hex >> 16) & 0xFF) as u8,
            ((hex >> 8) & 0xFF) as u8,
            (hex & 0xFF) as u8,
        )
    }

    /// With alpha value
    pub const fn with_alpha(self, a: u8) -> Self {
        Self::new(self.r, self.g, self.b, a)
    }

    /// Transparent
    pub const fn transparent() -> Self {
        Self::new(0, 0, 0, 0)
    }

    /// White
    pub const fn white() -> Self {
        Self::rgb(255, 255, 255)
    }

    /// Black
    pub const fn black() -> Self {
        Self::rgb(0, 0, 0)
    }

    /// Convert to u32 (ARGB)
    pub const fn to_argb(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    /// Linear interpolation between two colors
    pub fn lerp(&self, other: &Color, t: f32) -> Color {
        let t = t.max(0.0).min(1.0);
        let inv_t = 1.0 - t;
        Color::new(
            (self.r as f32 * inv_t + other.r as f32 * t) as u8,
            (self.g as f32 * inv_t + other.g as f32 * t) as u8,
            (self.b as f32 * inv_t + other.b as f32 * t) as u8,
            (self.a as f32 * inv_t + other.a as f32 * t) as u8,
        )
    }
}

/// Color palette - KPIO brand colors
pub mod palette {
    use super::Color;

    // Primary brand colors
    pub const PRIMARY_50: Color = Color::from_hex(0xEFF6FF);
    pub const PRIMARY_100: Color = Color::from_hex(0xDBEAFE);
    pub const PRIMARY_200: Color = Color::from_hex(0xBFDBFE);
    pub const PRIMARY_300: Color = Color::from_hex(0x93C5FD);
    pub const PRIMARY_400: Color = Color::from_hex(0x60A5FA);
    pub const PRIMARY_500: Color = Color::from_hex(0x3B82F6);
    pub const PRIMARY_600: Color = Color::from_hex(0x2563EB);
    pub const PRIMARY_700: Color = Color::from_hex(0x1D4ED8);
    pub const PRIMARY_800: Color = Color::from_hex(0x1E40AF);
    pub const PRIMARY_900: Color = Color::from_hex(0x1E3A8A);

    // Neutral grays
    pub const GRAY_50: Color = Color::from_hex(0xF9FAFB);
    pub const GRAY_100: Color = Color::from_hex(0xF3F4F6);
    pub const GRAY_200: Color = Color::from_hex(0xE5E7EB);
    pub const GRAY_300: Color = Color::from_hex(0xD1D5DB);
    pub const GRAY_400: Color = Color::from_hex(0x9CA3AF);
    pub const GRAY_500: Color = Color::from_hex(0x6B7280);
    pub const GRAY_600: Color = Color::from_hex(0x4B5563);
    pub const GRAY_700: Color = Color::from_hex(0x374151);
    pub const GRAY_800: Color = Color::from_hex(0x1F2937);
    pub const GRAY_900: Color = Color::from_hex(0x111827);
    pub const GRAY_950: Color = Color::from_hex(0x030712);

    // Semantic colors
    pub const SUCCESS_50: Color = Color::from_hex(0xF0FDF4);
    pub const SUCCESS_500: Color = Color::from_hex(0x22C55E);
    pub const SUCCESS_700: Color = Color::from_hex(0x15803D);

    pub const WARNING_50: Color = Color::from_hex(0xFFFBEB);
    pub const WARNING_500: Color = Color::from_hex(0xF59E0B);
    pub const WARNING_700: Color = Color::from_hex(0xB45309);

    pub const ERROR_50: Color = Color::from_hex(0xFEF2F2);
    pub const ERROR_500: Color = Color::from_hex(0xEF4444);
    pub const ERROR_700: Color = Color::from_hex(0xB91C1C);

    pub const INFO_50: Color = Color::from_hex(0xEFF6FF);
    pub const INFO_500: Color = Color::from_hex(0x3B82F6);
    pub const INFO_700: Color = Color::from_hex(0x1D4ED8);
}

/// Spacing scale (in pixels at 1x scale)
pub mod spacing {
    /// 0px
    pub const NONE: u32 = 0;
    /// 4px
    pub const XS: u32 = 4;
    /// 8px
    pub const SM: u32 = 8;
    /// 12px
    pub const MD: u32 = 12;
    /// 16px
    pub const LG: u32 = 16;
    /// 24px
    pub const XL: u32 = 24;
    /// 32px
    pub const XXL: u32 = 32;
    /// 48px
    pub const XXXL: u32 = 48;
    /// 64px
    pub const HUGE: u32 = 64;
}

/// Border radius values
pub mod radius {
    /// No radius
    pub const NONE: u32 = 0;
    /// 2px - subtle
    pub const XS: u32 = 2;
    /// 4px - small elements
    pub const SM: u32 = 4;
    /// 6px - medium elements
    pub const MD: u32 = 6;
    /// 8px - buttons, cards
    pub const LG: u32 = 8;
    /// 12px - larger cards
    pub const XL: u32 = 12;
    /// 16px - modals
    pub const XXL: u32 = 16;
    /// Full circle
    pub const FULL: u32 = 9999;
}

/// Typography scale
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Typography {
    pub size: u32,
    pub line_height: u32,
    pub weight: FontWeight,
    pub letter_spacing: i32,  // in 1/100 em
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Thin = 100,
    Light = 300,
    Regular = 400,
    Medium = 500,
    SemiBold = 600,
    Bold = 700,
    Black = 900,
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::Regular
    }
}

/// Typography presets
pub mod typography {
    use super::{Typography, FontWeight};

    /// Display - Large headlines
    pub const DISPLAY_LG: Typography = Typography {
        size: 48,
        line_height: 56,
        weight: FontWeight::Bold,
        letter_spacing: -2,
    };

    pub const DISPLAY_MD: Typography = Typography {
        size: 36,
        line_height: 44,
        weight: FontWeight::Bold,
        letter_spacing: -2,
    };

    pub const DISPLAY_SM: Typography = Typography {
        size: 30,
        line_height: 38,
        weight: FontWeight::Bold,
        letter_spacing: -1,
    };

    /// Headings
    pub const HEADING_1: Typography = Typography {
        size: 24,
        line_height: 32,
        weight: FontWeight::SemiBold,
        letter_spacing: 0,
    };

    pub const HEADING_2: Typography = Typography {
        size: 20,
        line_height: 28,
        weight: FontWeight::SemiBold,
        letter_spacing: 0,
    };

    pub const HEADING_3: Typography = Typography {
        size: 18,
        line_height: 24,
        weight: FontWeight::SemiBold,
        letter_spacing: 0,
    };

    /// Body text
    pub const BODY_LG: Typography = Typography {
        size: 16,
        line_height: 24,
        weight: FontWeight::Regular,
        letter_spacing: 0,
    };

    pub const BODY_MD: Typography = Typography {
        size: 14,
        line_height: 20,
        weight: FontWeight::Regular,
        letter_spacing: 0,
    };

    pub const BODY_SM: Typography = Typography {
        size: 12,
        line_height: 16,
        weight: FontWeight::Regular,
        letter_spacing: 0,
    };

    /// Labels and captions
    pub const LABEL_LG: Typography = Typography {
        size: 14,
        line_height: 20,
        weight: FontWeight::Medium,
        letter_spacing: 0,
    };

    pub const LABEL_MD: Typography = Typography {
        size: 12,
        line_height: 16,
        weight: FontWeight::Medium,
        letter_spacing: 0,
    };

    pub const LABEL_SM: Typography = Typography {
        size: 11,
        line_height: 14,
        weight: FontWeight::Medium,
        letter_spacing: 1,
    };

    pub const CAPTION: Typography = Typography {
        size: 11,
        line_height: 14,
        weight: FontWeight::Regular,
        letter_spacing: 0,
    };
}

/// Shadow definitions
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub x: i32,
    pub y: i32,
    pub blur: u32,
    pub spread: i32,
    pub color: Color,
}

impl Shadow {
    pub const fn new(x: i32, y: i32, blur: u32, spread: i32, color: Color) -> Self {
        Self { x, y, blur, spread, color }
    }
}

/// Shadow presets
pub mod shadows {
    use super::{Shadow, Color};

    pub const NONE: Shadow = Shadow::new(0, 0, 0, 0, Color::transparent());

    pub const SM: Shadow = Shadow::new(
        0, 1, 2, 0,
        Color::new(0, 0, 0, 15)
    );

    pub const MD: Shadow = Shadow::new(
        0, 4, 6, -1,
        Color::new(0, 0, 0, 25)
    );

    pub const LG: Shadow = Shadow::new(
        0, 10, 15, -3,
        Color::new(0, 0, 0, 25)
    );

    pub const XL: Shadow = Shadow::new(
        0, 20, 25, -5,
        Color::new(0, 0, 0, 25)
    );

    pub const XXL: Shadow = Shadow::new(
        0, 25, 50, -12,
        Color::new(0, 0, 0, 64)
    );

    /// Inner shadow for inset effects
    pub const INNER: Shadow = Shadow::new(
        0, 2, 4, 0,
        Color::new(0, 0, 0, 15)
    );
}

/// Z-index layers
pub mod z_index {
    pub const BASE: i32 = 0;
    pub const DROPDOWN: i32 = 100;
    pub const STICKY: i32 = 200;
    pub const FIXED: i32 = 300;
    pub const MODAL_BACKDROP: i32 = 400;
    pub const MODAL: i32 = 500;
    pub const POPOVER: i32 = 600;
    pub const TOOLTIP: i32 = 700;
    pub const TOAST: i32 = 800;
    pub const SPLASH: i32 = 1000;
}

/// Transition durations in milliseconds
pub mod duration {
    pub const INSTANT: u32 = 0;
    pub const FAST: u32 = 100;
    pub const NORMAL: u32 = 200;
    pub const SLOW: u32 = 300;
    pub const SLOWER: u32 = 500;
}

/// Easing curves (cubic bezier control points)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Easing {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl Easing {
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }
}

pub mod easing {
    use super::Easing;

    pub const LINEAR: Easing = Easing::new(0.0, 0.0, 1.0, 1.0);
    pub const EASE: Easing = Easing::new(0.25, 0.1, 0.25, 1.0);
    pub const EASE_IN: Easing = Easing::new(0.42, 0.0, 1.0, 1.0);
    pub const EASE_OUT: Easing = Easing::new(0.0, 0.0, 0.58, 1.0);
    pub const EASE_IN_OUT: Easing = Easing::new(0.42, 0.0, 0.58, 1.0);
    
    // Custom easing curves
    pub const BOUNCE_OUT: Easing = Easing::new(0.34, 1.56, 0.64, 1.0);
    pub const SMOOTH: Easing = Easing::new(0.4, 0.0, 0.2, 1.0);
}
