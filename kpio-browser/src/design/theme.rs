//! Theme System
//!
//! Light and dark themes with semantic color mappings.

use alloc::string::String;
use super::tokens::{Color, palette};

/// Theme mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
}

/// Complete theme definition
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme mode
    pub mode: ThemeMode,
    /// Theme name
    pub name: String,
    /// Color scheme
    pub colors: ThemeColors,
}

impl Theme {
    /// Create light theme
    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            name: String::from("KPIO Light"),
            colors: ThemeColors::light(),
        }
    }

    /// Create dark theme
    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            name: String::from("KPIO Dark"),
            colors: ThemeColors::dark(),
        }
    }

    /// Check if dark mode
    pub fn is_dark(&self) -> bool {
        self.mode == ThemeMode::Dark
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

/// Semantic color scheme
#[derive(Debug, Clone)]
pub struct ThemeColors {
    // Backgrounds
    /// Main background
    pub bg_primary: Color,
    /// Secondary background (cards, sections)
    pub bg_secondary: Color,
    /// Tertiary background (inputs, highlights)
    pub bg_tertiary: Color,
    /// Hover state background
    pub bg_hover: Color,
    /// Active/pressed state background
    pub bg_active: Color,
    /// Selected item background
    pub bg_selected: Color,
    
    // Surfaces
    /// Card/panel surface
    pub surface: Color,
    /// Elevated surface (dropdowns, modals)
    pub surface_elevated: Color,
    /// Overlay background
    pub overlay: Color,
    
    // Text
    /// Primary text
    pub text_primary: Color,
    /// Secondary text
    pub text_secondary: Color,
    /// Tertiary/muted text
    pub text_tertiary: Color,
    /// Disabled text
    pub text_disabled: Color,
    /// Text on primary color
    pub text_on_primary: Color,
    
    // Borders
    /// Default border
    pub border: Color,
    /// Strong/focused border
    pub border_strong: Color,
    /// Subtle border
    pub border_subtle: Color,
    /// Focused border color
    pub border_focus: Color,
    
    // Brand
    /// Primary brand color
    pub primary: Color,
    /// Primary hover
    pub primary_hover: Color,
    /// Primary active
    pub primary_active: Color,
    /// Subtle primary (tinted backgrounds)
    pub primary_subtle: Color,
    
    // Semantic
    /// Success color
    pub success: Color,
    /// Success background
    pub success_subtle: Color,
    /// Warning color
    pub warning: Color,
    /// Warning background
    pub warning_subtle: Color,
    /// Error/danger color
    pub error: Color,
    /// Error background
    pub error_subtle: Color,
    /// Info color
    pub info: Color,
    /// Info background
    pub info_subtle: Color,
    
    // Interactive
    /// Link color
    pub link: Color,
    /// Link hover
    pub link_hover: Color,
    /// Focus ring color
    pub focus_ring: Color,
    
    // Browser specific
    /// Tab bar background
    pub tab_bar: Color,
    /// Active tab
    pub tab_active: Color,
    /// Inactive tab
    pub tab_inactive: Color,
    /// Address bar background
    pub address_bar: Color,
    /// Toolbar background
    pub toolbar: Color,
}

impl ThemeColors {
    /// Light theme colors
    pub fn light() -> Self {
        Self {
            // Backgrounds
            bg_primary: palette::GRAY_50,
            bg_secondary: Color::white(),
            bg_tertiary: palette::GRAY_100,
            bg_hover: palette::GRAY_100,
            bg_active: palette::GRAY_200,
            bg_selected: palette::PRIMARY_50,
            
            // Surfaces
            surface: Color::white(),
            surface_elevated: Color::white(),
            overlay: Color::black().with_alpha(128),
            
            // Text
            text_primary: palette::GRAY_900,
            text_secondary: palette::GRAY_600,
            text_tertiary: palette::GRAY_500,
            text_disabled: palette::GRAY_400,
            text_on_primary: Color::white(),
            
            // Borders
            border: palette::GRAY_200,
            border_strong: palette::GRAY_300,
            border_subtle: palette::GRAY_100,
            border_focus: palette::PRIMARY_500,
            
            // Brand
            primary: palette::PRIMARY_500,
            primary_hover: palette::PRIMARY_600,
            primary_active: palette::PRIMARY_700,
            primary_subtle: palette::PRIMARY_50,
            
            // Semantic
            success: palette::SUCCESS_500,
            success_subtle: palette::SUCCESS_50,
            warning: palette::WARNING_500,
            warning_subtle: palette::WARNING_50,
            error: palette::ERROR_500,
            error_subtle: palette::ERROR_50,
            info: palette::INFO_500,
            info_subtle: palette::INFO_50,
            
            // Interactive
            link: palette::PRIMARY_600,
            link_hover: palette::PRIMARY_700,
            focus_ring: palette::PRIMARY_500.with_alpha(128),
            
            // Browser
            tab_bar: palette::GRAY_100,
            tab_active: Color::white(),
            tab_inactive: palette::GRAY_100,
            address_bar: Color::white(),
            toolbar: palette::GRAY_50,
        }
    }

    /// Dark theme colors
    pub fn dark() -> Self {
        Self {
            // Backgrounds
            bg_primary: palette::GRAY_950,
            bg_secondary: palette::GRAY_900,
            bg_tertiary: palette::GRAY_800,
            bg_hover: palette::GRAY_800,
            bg_active: palette::GRAY_700,
            bg_selected: Color::from_hex(0x1E3A5F), // Dark blue
            
            // Surfaces
            surface: palette::GRAY_900,
            surface_elevated: palette::GRAY_800,
            overlay: Color::black().with_alpha(180),
            
            // Text
            text_primary: palette::GRAY_50,
            text_secondary: palette::GRAY_400,
            text_tertiary: palette::GRAY_500,
            text_disabled: palette::GRAY_600,
            text_on_primary: Color::white(),
            
            // Borders
            border: palette::GRAY_700,
            border_strong: palette::GRAY_600,
            border_subtle: palette::GRAY_800,
            border_focus: palette::PRIMARY_400,
            
            // Brand
            primary: palette::PRIMARY_400,
            primary_hover: palette::PRIMARY_300,
            primary_active: palette::PRIMARY_500,
            primary_subtle: Color::from_hex(0x1E3A5F),
            
            // Semantic
            success: Color::from_hex(0x4ADE80),
            success_subtle: Color::from_hex(0x14532D),
            warning: Color::from_hex(0xFBBF24),
            warning_subtle: Color::from_hex(0x713F12),
            error: Color::from_hex(0xF87171),
            error_subtle: Color::from_hex(0x7F1D1D),
            info: Color::from_hex(0x60A5FA),
            info_subtle: Color::from_hex(0x1E3A8A),
            
            // Interactive
            link: palette::PRIMARY_400,
            link_hover: palette::PRIMARY_300,
            focus_ring: palette::PRIMARY_400.with_alpha(128),
            
            // Browser
            tab_bar: palette::GRAY_900,
            tab_active: palette::GRAY_800,
            tab_inactive: palette::GRAY_900,
            address_bar: palette::GRAY_800,
            toolbar: palette::GRAY_900,
        }
    }
}
