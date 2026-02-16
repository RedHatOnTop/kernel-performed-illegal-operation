//! KPIO Design System
//!
//! A comprehensive design system for KPIO Browser OS.
//! Provides consistent theming, components, and design tokens.

pub mod animation;
pub mod browser;
pub mod components;
pub mod dialogs;
pub mod icons;
pub mod layout;
pub mod pages;
pub mod theme;
pub mod tokens;

pub use animation::*;
pub use browser::*;
pub use components::*;
pub use dialogs::*;
pub use icons::*;
pub use layout::*;
pub use pages::*;
pub use theme::*;
pub use tokens::*;

use alloc::string::String;

/// Design system version
pub const DESIGN_VERSION: &str = "1.0.0";

/// Initialize the design system
pub fn init() -> DesignSystem {
    DesignSystem::new()
}

/// Main design system manager
#[derive(Debug, Clone)]
pub struct DesignSystem {
    /// Current theme
    pub theme: Theme,
    /// Scale factor for UI (1.0 = 100%)
    pub scale: f32,
    /// Animation enabled
    pub animations_enabled: bool,
    /// Reduced motion preference
    pub reduced_motion: bool,
    /// High contrast mode
    pub high_contrast: bool,
}

impl DesignSystem {
    /// Create new design system with default theme
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
            scale: 1.0,
            animations_enabled: true,
            reduced_motion: false,
            high_contrast: false,
        }
    }

    /// Set theme
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Set scale factor
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale.max(0.5).min(2.0);
        self
    }

    /// Enable high contrast mode
    pub fn with_high_contrast(mut self, enabled: bool) -> Self {
        self.high_contrast = enabled;
        self
    }

    /// Scale a dimension value
    pub fn scale_dim(&self, value: u32) -> u32 {
        ((value as f32) * self.scale) as u32
    }

    /// Get effective animation duration (0 if disabled)
    pub fn animation_duration(&self, base_ms: u32) -> u32 {
        if self.animations_enabled && !self.reduced_motion {
            base_ms
        } else {
            0
        }
    }
}

impl Default for DesignSystem {
    fn default() -> Self {
        Self::new()
    }
}
