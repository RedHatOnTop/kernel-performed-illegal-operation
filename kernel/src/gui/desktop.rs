//! Desktop Environment
//!
//! Main desktop shell with wallpaper and icons.

use super::render::{Color, Renderer};
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
        let icons = alloc::vec![
            DesktopIcon {
                name: String::from("Files"),
                x: 20,
                y: 20,
                icon_type: IconType::Files,
            },
            DesktopIcon {
                name: String::from("Browser"),
                x: 20,
                y: 100,
                icon_type: IconType::Browser,
            },
            DesktopIcon {
                name: String::from("Terminal"),
                x: 20,
                y: 180,
                icon_type: IconType::Terminal,
            },
            DesktopIcon {
                name: String::from("Settings"),
                x: 20,
                y: 260,
                icon_type: IconType::Settings,
            },
            DesktopIcon {
                name: String::from("Trash"),
                x: 20,
                y: 340,
                icon_type: IconType::Trash,
            },
        ];

        Self {
            width,
            height,
            icons,
        }
    }

    /// Render desktop
    pub fn render(&self, renderer: &mut Renderer) {
        // Draw background gradient
        for y in 0..self.height {
            let factor = y as f32 / self.height as f32;
            let r = (0.0 + factor * 20.0) as u8;
            let g = (30.0 + factor * 30.0) as u8;
            let b = (60.0 + factor * 60.0) as u8;
            let color = Color::rgb(r, g, b);
            
            renderer.draw_hline(0, y as i32, self.width, color);
        }

        // Draw icons
        for icon in &self.icons {
            self.render_icon(renderer, icon);
        }
    }

    /// Render a single icon
    fn render_icon(&self, renderer: &mut Renderer, icon: &DesktopIcon) {
        let pattern = icon.icon_type.get_pattern();
        
        // Draw icon background
        renderer.fill_rect(icon.x - 4, icon.y - 4, 56, 64, Color::rgba(255, 255, 255, 30));
        
        // Draw icon (scaled 3x)
        let scale = 3;
        for (row, &bits) in pattern.iter().enumerate() {
            for col in 0..16 {
                if (bits >> (15 - col)) & 1 == 1 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            renderer.set_pixel(
                                icon.x + col * scale + sx,
                                icon.y + row as i32 * scale + sy,
                                Color::WHITE,
                            );
                        }
                    }
                }
            }
        }

        // Draw icon name
        let text_x = icon.x;
        let text_y = icon.y + 50;
        renderer.draw_text(text_x, text_y, &icon.name, Color::WHITE);
    }
}
