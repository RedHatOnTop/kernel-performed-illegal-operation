//! Mouse Cursor
//!
//! Software-rendered mouse cursor with anti-aliased edges.

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

    /// Render cursor with anti-aliased edges
    pub fn render(&self, renderer: &mut Renderer) {
        if !self.visible {
            return;
        }

        // Arrow cursor masks (12x18 pixels)
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

        // Draw outline with AA fringe
        for (row, &bits) in outline.iter().enumerate() {
            for col in 0..16i32 {
                if (bits >> (15 - col)) & 1 == 1 {
                    let px = self.x + col;
                    let py = self.y + row as i32;
                    renderer.set_pixel(px, py, Color::BLACK);

                    // Add soft fringe around outline pixels for AA
                    let fringe = Color::rgba(0, 0, 0, 50);
                    // Check and add fringe to empty neighbours
                    for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                        let nx = col + dx;
                        let ny = row as i32 + dy;
                        if nx >= 0 && nx < 16 && ny >= 0 && ny < 18 {
                            let n_out = (outline[ny as usize] >> (15 - nx)) & 1;
                            let n_cur = (cursor[ny as usize] >> (15 - nx)) & 1;
                            if n_out == 0 && n_cur == 0 {
                                renderer.blend_pixel(self.x + nx, self.y + ny, fringe);
                            }
                        } else {
                            // Outside the bitmap — still draw fringe
                            renderer.blend_pixel(self.x + nx, self.y + ny, fringe);
                        }
                    }
                }
            }
        }

        // Draw fill (white)
        for (row, &bits) in cursor.iter().enumerate() {
            for col in 0..16i32 {
                if (bits >> (15 - col)) & 1 == 1 {
                    if (outline[row] >> (15 - col)) & 1 == 0 {
                        renderer.set_pixel(self.x + col, self.y + row as i32, Color::WHITE);
                    }
                }
            }
        }
    }
}
