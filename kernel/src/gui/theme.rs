//! Design System & Theme
//!
//! Centralized design tokens for the KPIO OS GUI.
//! Modern flat design inspired by Windows 11 / GNOME 4x.
//! All UI components reference these values for visual consistency.
//!
//! ## Design Philosophy
//! - Flat, clean surfaces with consistent corner radii
//! - Minimal depth — subtle single-layer shadows only
//! - High contrast text for readability
//! - Muted, professional colour palette with a single accent

use super::render::Color;

// ─────────────────────────── Color Palette ───────────────────────────

/// Primary brand colors
pub struct Accent;
impl Accent {
    pub const PRIMARY: Color = Color::rgb(56, 132, 244); // Bright blue
    pub const PRIMARY_DARK: Color = Color::rgb(36, 100, 200);
    pub const PRIMARY_LIGHT: Color = Color::rgb(100, 165, 255);
    pub const SECONDARY: Color = Color::rgb(100, 220, 170); // Teal-green
    pub const DANGER: Color = Color::rgb(232, 17, 35); // Windows-style red
    pub const DANGER_HOVER: Color = Color::rgb(241, 80, 80);
    pub const WARNING: Color = Color::rgb(249, 202, 36); // Yellow
    pub const SUCCESS: Color = Color::rgb(106, 176, 76); // Green
}

/// Neutral / surface colors (dark theme)
pub struct Surface;
impl Surface {
    // Desktop & root — very subtle gradient
    pub const DESKTOP_TOP: Color = Color::rgb(22, 22, 36);
    pub const DESKTOP_BOTTOM: Color = Color::rgb(28, 30, 44);

    // Taskbar — solid flat panel
    pub const TASKBAR: Color = Color::rgb(24, 24, 30);
    pub const TASKBAR_ITEM: Color = Color::rgba(255, 255, 255, 8);
    pub const TASKBAR_HOVER: Color = Color::rgba(255, 255, 255, 18);
    pub const TASKBAR_ACTIVE: Color = Color::rgba(255, 255, 255, 28);
    pub const TASKBAR_BORDER: Color = Color::rgba(255, 255, 255, 8);
    pub const TASKBAR_HIGHLIGHT: Color = Color::rgba(255, 255, 255, 6);

    // Window chrome — flat
    pub const WINDOW_BG: Color = Color::rgb(250, 250, 252);
    pub const WINDOW_TITLE_ACTIVE: Color = Color::rgb(240, 240, 244);
    pub const WINDOW_TITLE_INACTIVE: Color = Color::rgb(230, 230, 234);
    pub const WINDOW_BORDER_ACTIVE: Color = Color::rgba(0, 0, 0, 22);
    pub const WINDOW_BORDER_INACTIVE: Color = Color::rgba(0, 0, 0, 14);

    // Window control buttons (flat hover fills)
    pub const CLOSE_HOVER: Color = Color::rgb(232, 17, 35); // Windows-style red
    pub const BUTTON_HOVER: Color = Color::rgba(0, 0, 0, 15); // Subtle hover

    // Panels / containers
    pub const PANEL: Color = Color::rgb(245, 245, 248);
    pub const PANEL_ALT: Color = Color::rgb(238, 238, 242);
    pub const SIDEBAR: Color = Color::rgb(235, 236, 240);
    pub const INPUT_BG: Color = Color::rgb(255, 255, 255);
    pub const INPUT_BORDER: Color = Color::rgb(200, 200, 210);
    pub const INPUT_FOCUS: Color = Color::rgb(56, 132, 244);

    // Start menu — flat solid
    pub const MENU_BG: Color = Color::rgb(32, 32, 38);
    pub const MENU_HOVER: Color = Color::rgba(255, 255, 255, 12);
    pub const MENU_HEADER: Color = Color::rgba(56, 132, 244, 200);

    // Active window indicator on taskbar
    pub const ACTIVE_INDICATOR: Color = Color::rgb(56, 132, 244);
}

/// Text colors
pub struct Text;
impl Text {
    pub const PRIMARY: Color = Color::rgb(30, 30, 36);
    pub const SECONDARY: Color = Color::rgb(100, 100, 115);
    pub const MUTED: Color = Color::rgb(150, 150, 165);
    pub const ON_DARK: Color = Color::rgb(235, 235, 240);
    pub const ON_ACCENT: Color = Color::rgb(255, 255, 255);
    pub const LINK: Color = Color::rgb(56, 132, 244);
}

/// Shadow / overlay colors (minimal for flat design)
pub struct Shadow;
impl Shadow {
    pub const LIGHT: Color = Color::rgba(0, 0, 0, 10);
    pub const MEDIUM: Color = Color::rgba(0, 0, 0, 20);
    pub const HEAVY: Color = Color::rgba(0, 0, 0, 35);
    pub const DROP: Color = Color::rgba(0, 0, 0, 25);
}

/// Icon / accent tints
pub struct IconColor;
impl IconColor {
    pub const FILES: Color = Color::rgb(255, 179, 64); // Orange
    pub const BROWSER: Color = Color::rgb(56, 132, 244); // Blue
    pub const TERMINAL: Color = Color::rgb(100, 220, 170); // Teal
    pub const SETTINGS: Color = Color::rgb(150, 150, 165); // Gray
    pub const TRASH: Color = Color::rgb(160, 160, 175); // Cool gray
    pub const FOLDER: Color = Color::rgb(255, 196, 64); // Golden
    pub const FILE: Color = Color::rgb(180, 190, 210); // Light steel
}

/// Terminal-specific
pub struct TermTheme;
impl TermTheme {
    pub const BG: Color = Color::rgb(24, 24, 32);
    pub const FG: Color = Color::rgb(200, 210, 220);
    pub const PROMPT: Color = Color::rgb(100, 220, 170);
    pub const ERROR: Color = Color::rgb(235, 77, 75);
    pub const PATH: Color = Color::rgb(130, 170, 255);
    pub const CURSOR: Color = Color::rgb(200, 210, 220);
}

/// System tray icon colours
pub struct TrayColor;
impl TrayColor {
    pub const ICON: Color = Color::rgb(180, 180, 195);
    pub const ACTIVE: Color = Color::rgb(56, 132, 244);
}

// ─────────────────────────── Spacing ───────────────────────────

/// Spacing constants (in pixels)
pub struct Spacing;
impl Spacing {
    pub const XXXS: u32 = 2;
    pub const XXS: u32 = 4;
    pub const XS: u32 = 6;
    pub const SM: u32 = 8;
    pub const MD: u32 = 12;
    pub const LG: u32 = 16;
    pub const XL: u32 = 24;
    pub const XXL: u32 = 32;
}

// ─────────────────────────── Corner Radii ───────────────────────────

/// Border radius values
pub struct Radius;
impl Radius {
    pub const NONE: u32 = 0;
    pub const SM: u32 = 4;
    pub const MD: u32 = 6;
    pub const LG: u32 = 8;
    pub const XL: u32 = 12;
    pub const PILL: u32 = 999; // fully rounded

    pub const WINDOW: u32 = 6;
    pub const BUTTON: u32 = 4;
    pub const INPUT: u32 = 4;
    pub const MENU: u32 = 6;
    pub const ICON: u32 = 8;
    pub const TASKBAR_ITEM: u32 = 4;
}

// ─────────────────────────── Sizes ───────────────────────────

/// Component sizes
pub struct Size;
impl Size {
    pub const TITLE_BAR_HEIGHT: u32 = 36;
    pub const TASKBAR_HEIGHT: u32 = 48;
    pub const BUTTON_HEIGHT: u32 = 28;
    pub const INPUT_HEIGHT: u32 = 28;
    pub const ICON_SIZE: u32 = 36;
    pub const ICON_AREA: u32 = 72;
    pub const DESKTOP_ICON_GAP: u32 = 90;
    pub const MENU_ITEM_HEIGHT: u32 = 38;
    pub const MENU_WIDTH: u32 = 240;
    pub const SCROLL_BAR_W: u32 = 6;

    // Window control buttons
    pub const WIN_BTN_W: u32 = 46;
    pub const WIN_BTN_H: u32 = 36;
}

// ─────────────────────────── Shadows ───────────────────────────

/// Shadow specification
#[derive(Debug, Clone, Copy)]
pub struct ShadowSpec {
    pub offset_x: i32,
    pub offset_y: i32,
    pub blur: u32,
    pub color: Color,
}

pub struct Shadows;
impl Shadows {
    pub const WINDOW: ShadowSpec = ShadowSpec {
        offset_x: 0,
        offset_y: 2,
        blur: 8,
        color: Color::rgba(0, 0, 0, 25),
    };
    pub const MENU: ShadowSpec = ShadowSpec {
        offset_x: 0,
        offset_y: 4,
        blur: 12,
        color: Color::rgba(0, 0, 0, 30),
    };
    pub const BUTTON: ShadowSpec = ShadowSpec {
        offset_x: 0,
        offset_y: 1,
        blur: 2,
        color: Color::rgba(0, 0, 0, 12),
    };
}
