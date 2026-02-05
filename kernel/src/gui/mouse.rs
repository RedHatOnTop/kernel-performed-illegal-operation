//! Mouse Cursor
//!
//! Software-rendered mouse cursor.

use super::render::{Color, Renderer};

/// Mouse cursor state
pub struct MouseCursor {
    pub x: i32,
    pub y: i32,
    pub visible: bool,
}

impl MouseCursor {
    /// Create new cursor
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            visible: true,
        }
    }

    /// Move cursor by delta
    pub fn move_by(&mut self, dx: i32, dy: i32, max_x: i32, max_y: i32) {
        self.x = (self.x + dx).clamp(0, max_x - 1);
        self.y = (self.y + dy).clamp(0, max_y - 1);
    }

    /// Render cursor
    pub fn render(&self, renderer: &mut Renderer) {
        if !self.visible {
            return;
        }

        // Draw arrow cursor (12x18 pixels)
        let cursor: [u16; 18] = [
            0b1000000000000000,
            0b1100000000000000,
            0b1110000000000000,
            0b1111000000000000,
            0b1111100000000000,
            0b1111110000000000,
            0b1111111000000000,
            0b1111111100000000,
            0b1111111110000000,
            0b1111111111000000,
            0b1111110000000000,
            0b1101111000000000,
            0b1100111100000000,
            0b0000011110000000,
            0b0000011110000000,
            0b0000001111000000,
            0b0000001111000000,
            0b0000000110000000,
        ];

        let outline: [u16; 18] = [
            0b1000000000000000,
            0b1100000000000000,
            0b1010000000000000,
            0b1001000000000000,
            0b1000100000000000,
            0b1000010000000000,
            0b1000001000000000,
            0b1000000100000000,
            0b1000000010000000,
            0b1000000001000000,
            0b1000010000000000,
            0b1001001000000000,
            0b1100010100000000,
            0b0000000010000000,
            0b0000000010000000,
            0b0000000001000000,
            0b0000000001000000,
            0b0000000000000000,
        ];

        // Draw outline (black)
        for (row, &bits) in outline.iter().enumerate() {
            for col in 0..16 {
                if (bits >> (15 - col)) & 1 == 1 {
                    renderer.set_pixel(self.x + col, self.y + row as i32, Color::BLACK);
                }
            }
        }

        // Draw fill (white)
        for (row, &bits) in cursor.iter().enumerate() {
            for col in 0..16 {
                if (bits >> (15 - col)) & 1 == 1 {
                    if (outline[row] >> (15 - col)) & 1 == 0 {
                        renderer.set_pixel(self.x + col, self.y + row as i32, Color::WHITE);
                    }
                }
            }
        }
    }
}
