//! Desktop Environment
//!
//! Modern desktop shell with smooth gradient background and stylish icons.

use super::render::{Color, Renderer};
use super::theme::{Surface, Text, Accent, IconColor, Radius, Spacing, Size};
use alloc::string::String;
use alloc::vec::Vec;

/// Desktop icon
#[derive(Debug, Clone)]
pub struct DesktopIcon {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub icon_type: IconType,
}

/// Icon types
#[derive(Debug, Clone, Copy)]
pub enum IconType {
    Files,
    Browser,
    Terminal,
    Settings,
    Trash,
}

impl IconType {
    /// Get icon pattern (16x16 bitmap)
    pub fn get_pattern(&self) -> [u16; 16] {
        match self {
            IconType::Files => [
                0b0111111111110000,
                0b0100000000010000,
                0b0101111111110000,
                0b0100000000010000,
                0b0100000000010000,
                0b0100000000010000,
                0b0100000000010000,
                0b0100000000010000,
                0b0100000000010000,
                0b0100000000010000,
                0b0100000000010000,
                0b0100000000010000,
                0b0100000000010000,
                0b0111111111110000,
                0b0000000000000000,
                0b0000000000000000,
            ],
            IconType::Browser => [
                0b0001111111100000,
                0b0011111111110000,
                0b0111111111111000,
                0b0110000110001000,
                0b0110000110001000,
                0b0111111111111000,
                0b0111111111111000,
                0b0110000000001000,
                0b0110000000001000,
                0b0110000000001000,
                0b0110000000001000,
                0b0111111111111000,
                0b0011111111110000,
                0b0001111111100000,
                0b0000000000000000,
                0b0000000000000000,
            ],
            IconType::Terminal => [
                0b0111111111111000,
                0b0100000000001000,
                0b0100000000001000,
                0b0100110000001000,
                0b0100011000001000,
                0b0100001100001000,
                0b0100000110001000,
                0b0100000011001000,
                0b0100000110001000,
                0b0100001100001000,
                0b0100011000001000,
                0b0100110000001000,
                0b0100000000001000,
                0b0111111111111000,
                0b0000000000000000,
                0b0000000000000000,
            ],
            IconType::Settings => [
                0b0000011000000000,
                0b0000111100000000,
                0b0011111111000000,
                0b0111100111100000,
                0b0110000001100000,
                0b0110011001100000,
                0b1110011001110000,
                0b1100011000110000,
                0b1100011000110000,
                0b1110011001110000,
                0b0110011001100000,
                0b0110000001100000,
                0b0111100111100000,
                0b0011111111000000,
                0b0000111100000000,
                0b0000011000000000,
            ],
            IconType::Trash => [
                0b0001111111100000,
                0b0000100001000000,
                0b0111111111111000,
                0b0100000000001000,
                0b0100100100101000,
                0b0100100100101000,
                0b0100100100101000,
                0b0100100100101000,
                0b0100100100101000,
                0b0100100100101000,
                0b0100100100101000,
                0b0100100100101000,
                0b0100000000001000,
                0b0011111111110000,
                0b0000000000000000,
                0b0000000000000000,
            ],
        }
    }
    
    /// Get the accent colour for this icon type
    pub fn color(&self) -> Color {
        match self {
            IconType::Files    => IconColor::FILES,
            IconType::Browser  => IconColor::BROWSER,
            IconType::Terminal => IconColor::TERMINAL,
            IconType::Settings => IconColor::SETTINGS,
            IconType::Trash    => IconColor::TRASH,
        }
    }
}

/// Desktop state
pub struct Desktop {
    pub width: u32,
    pub height: u32,
    pub icons: Vec<DesktopIcon>,
}

impl Desktop {
    /// Create new desktop
    pub fn new(width: u32, height: u32) -> Self {
        let gap = Size::DESKTOP_ICON_GAP as i32;
        let icons = alloc::vec![
            DesktopIcon { name: String::from("Files"),    x: 24, y: 24,              icon_type: IconType::Files },
            DesktopIcon { name: String::from("Browser"),  x: 24, y: 24 + gap,        icon_type: IconType::Browser },
            DesktopIcon { name: String::from("Terminal"), x: 24, y: 24 + gap * 2,    icon_type: IconType::Terminal },
            DesktopIcon { name: String::from("Settings"),x: 24, y: 24 + gap * 3,    icon_type: IconType::Settings },
            DesktopIcon { name: String::from("Trash"),   x: 24, y: 24 + gap * 4,    icon_type: IconType::Trash },
        ];
        Self { width, height, icons }
    }

    /// Render desktop with smooth gradient background
    pub fn render(&self, renderer: &mut Renderer) {
        // Smooth gradient background
        renderer.fill_gradient_v(0, 0, self.width, self.height,
            Surface::DESKTOP_TOP, Surface::DESKTOP_BOTTOM);

        // Render desktop icons
        for icon in &self.icons {
            self.render_icon(renderer, icon);
        }
    }

    /// Render a single modern-style desktop icon
    fn render_icon(&self, renderer: &mut Renderer, icon: &DesktopIcon) {
        let icon_area = Size::ICON_AREA;
        let icon_size = Size::ICON_SIZE;
        let pattern = icon.icon_type.get_pattern();
        let tint = icon.icon_type.color();

        // Translucent background pill behind the icon
        renderer.fill_rounded_rect_aa(
            icon.x - 4, icon.y - 4,
            icon_area, icon_area + 16,
            Radius::ICON,
            Color::rgba(255, 255, 255, 14),
        );

        // Draw the icon bitmap, scaled 2× with AA fringe
        let scale = 2i32;
        let ix = icon.x + (icon_area as i32 - 16 * scale) / 2;
        let iy = icon.y + 4;
        renderer.draw_icon_scaled(ix, iy, &pattern, tint, scale);

        // Icon label (centered below)
        let name_len = icon.name.len() as i32 * 8;
        let name_x = icon.x + (icon_area as i32 - name_len) / 2;
        let name_y = icon.y + icon_area as i32 - 6;
        // Text shadow for readability
        renderer.draw_text(name_x + 1, name_y + 1, &icon.name, Color::rgba(0, 0, 0, 120));
        renderer.draw_text(name_x, name_y, &icon.name, Text::ON_DARK);
    }
}
