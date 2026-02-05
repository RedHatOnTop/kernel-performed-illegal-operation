//! Boot Animation Module
//!
//! Ubuntu-style boot splash screen with animated progress dots.

use super::render::{Color, Renderer};

/// Boot animation state
pub struct BootAnimation {
    /// Current frame (for animation)
    frame: u32,
    /// Animation phase (0-4 for dots)
    dot_phase: usize,
    /// Whether boot is complete
    pub complete: bool,
    /// Messages to display
    messages: [&'static str; 8],
    /// Current message index
    current_message: usize,
    /// Frames per message
    frames_per_message: u32,
    /// Screen dimensions
    width: u32,
    height: u32,
}

impl BootAnimation {
    /// Create new boot animation
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            frame: 0,
            dot_phase: 0,
            complete: false,
            messages: [
                "Initializing kernel...",
                "Loading drivers...",
                "Starting memory manager...",
                "Initializing interrupts...",
                "Loading GUI subsystem...",
                "Starting window manager...",
                "Preparing desktop...",
                "Welcome to KPIO",
            ],
            current_message: 0,
            frames_per_message: 15,
            width,
            height,
        }
    }

    /// Advance animation by one frame
    /// Returns true if animation is complete
    pub fn tick(&mut self) -> bool {
        if self.complete {
            return true;
        }

        self.frame += 1;
        
        // Update dot animation every 10 frames
        if self.frame % 10 == 0 {
            self.dot_phase = (self.dot_phase + 1) % 5;
        }

        // Advance message every frames_per_message frames
        if self.frame % self.frames_per_message == 0 {
            if self.current_message < self.messages.len() - 1 {
                self.current_message += 1;
            } else {
                self.complete = true;
            }
        }

        self.complete
    }

    /// Force complete the animation
    pub fn force_complete(&mut self) {
        self.complete = true;
    }

    /// Render the boot animation
    pub fn render(&self, renderer: &mut Renderer) {
        // Dark purple/navy background (Ubuntu-like)
        let bg_color = Color::rgb(48, 10, 36); // Dark purple
        
        // Fill entire screen with background
        renderer.fill_rect(0, 0, self.width, self.height, bg_color);

        let center_x = self.width as i32 / 2;
        let center_y = self.height as i32 / 2;

        // Draw KPIO logo (larger version)
        self.draw_logo(renderer, center_x - 100, center_y - 80);

        // Draw animated loading dots
        self.draw_loading_dots(renderer, center_x, center_y + 60);

        // Draw current status message
        self.draw_message(renderer, center_x, center_y + 100);

        // Draw progress bar
        self.draw_progress_bar(renderer, center_x, center_y + 140);
    }

    /// Draw the KPIO logo
    fn draw_logo(&self, renderer: &mut Renderer, x: i32, y: i32) {
        let white = Color::WHITE;
        let orange = Color::rgb(233, 84, 32); // Ubuntu orange

        // Draw "KPIO" text with pixel art
        let scale = 6;
        
        // K
        let k_pattern: [u8; 8] = [
            0b10000100,
            0b10001000,
            0b10010000,
            0b10100000,
            0b11100000,
            0b10010000,
            0b10001000,
            0b10000100,
        ];
        self.draw_char_scaled(renderer, x, y, &k_pattern, white, scale);

        // P
        let p_pattern: [u8; 8] = [
            0b11111000,
            0b10000100,
            0b10000100,
            0b11111000,
            0b10000000,
            0b10000000,
            0b10000000,
            0b10000000,
        ];
        self.draw_char_scaled(renderer, x + 50, y, &p_pattern, white, scale);

        // I
        let i_pattern: [u8; 8] = [
            0b11111110,
            0b00010000,
            0b00010000,
            0b00010000,
            0b00010000,
            0b00010000,
            0b00010000,
            0b11111110,
        ];
        self.draw_char_scaled(renderer, x + 100, y, &i_pattern, white, scale);

        // O
        let o_pattern: [u8; 8] = [
            0b00111100,
            0b01000010,
            0b10000001,
            0b10000001,
            0b10000001,
            0b10000001,
            0b01000010,
            0b00111100,
        ];
        self.draw_char_scaled(renderer, x + 150, y, &o_pattern, orange, scale);
    }

    /// Draw a character with scaling
    fn draw_char_scaled(&self, renderer: &mut Renderer, x: i32, y: i32, 
                        pattern: &[u8; 8], color: Color, scale: i32) {
        for (row, &bits) in pattern.iter().enumerate() {
            for col in 0..8 {
                if (bits >> (7 - col)) & 1 == 1 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            renderer.set_pixel(
                                x + col as i32 * scale + sx,
                                y + row as i32 * scale + sy,
                                color,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Draw animated loading dots
    fn draw_loading_dots(&self, renderer: &mut Renderer, center_x: i32, y: i32) {
        let dot_radius = 6;
        let dot_spacing = 24;
        let num_dots = 5;
        let start_x = center_x - (num_dots * dot_spacing) / 2;

        for i in 0..num_dots {
            let x = start_x + i * dot_spacing;
            
            // Calculate alpha based on animation phase
            let distance_from_active = ((i as isize) - (self.dot_phase as isize)).abs() as usize;
            let brightness = match distance_from_active {
                0 => 255,
                1 => 180,
                2 => 100,
                _ => 50,
            };
            
            let color = Color::rgb(brightness as u8, brightness as u8, brightness as u8);
            
            // Draw circle (filled)
            self.draw_filled_circle(renderer, x, y, dot_radius, color);
        }
    }

    /// Draw a filled circle
    fn draw_filled_circle(&self, renderer: &mut Renderer, cx: i32, cy: i32, r: i32, color: Color) {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    renderer.set_pixel(cx + dx, cy + dy, color);
                }
            }
        }
    }

    /// Draw the current status message
    fn draw_message(&self, renderer: &mut Renderer, center_x: i32, y: i32) {
        let message = self.messages[self.current_message];
        let char_width = 8;
        let text_width = message.len() as i32 * char_width;
        let start_x = center_x - text_width / 2;

        for (i, ch) in message.chars().enumerate() {
            renderer.draw_char(start_x + i as i32 * char_width, y, ch, Color::WHITE);
        }
    }

    /// Draw progress bar
    fn draw_progress_bar(&self, renderer: &mut Renderer, center_x: i32, y: i32) {
        let bar_width = 300;
        let bar_height = 8;
        let bar_x = center_x - bar_width / 2;

        // Background
        renderer.fill_rect(bar_x, y, bar_width as u32, bar_height as u32, Color::DARK_GRAY);

        // Progress
        let progress = (self.current_message + 1) as i32 * bar_width / self.messages.len() as i32;
        let progress_color = Color::rgb(233, 84, 32); // Ubuntu orange
        renderer.fill_rect(bar_x, y, progress as u32, bar_height as u32, progress_color);

        // Border
        let border_color = Color::GRAY;
        for px in bar_x..(bar_x + bar_width) {
            renderer.set_pixel(px, y, border_color);
            renderer.set_pixel(px, y + bar_height - 1, border_color);
        }
        for py in y..(y + bar_height) {
            renderer.set_pixel(bar_x, py, border_color);
            renderer.set_pixel(bar_x + bar_width - 1, py, border_color);
        }
    }
}

/// Global boot animation state
use spin::Mutex;
pub static BOOT_ANIMATION: Mutex<Option<BootAnimation>> = Mutex::new(None);

/// Initialize boot animation
pub fn init(width: u32, height: u32) {
    let mut anim = BOOT_ANIMATION.lock();
    *anim = Some(BootAnimation::new(width, height));
}

/// Check if boot animation is complete (safe for interrupt context)
pub fn is_complete() -> bool {
    // Use try_lock to avoid deadlock when called from interrupt context
    if let Some(guard) = BOOT_ANIMATION.try_lock() {
        return guard.as_ref().map(|a| a.complete).unwrap_or(true);
    }
    false // Return not complete if we couldn't get lock
}

/// Tick the boot animation (safe for interrupt context)
pub fn tick() -> bool {
    // Use try_lock to avoid deadlock when called from interrupt context
    if let Some(mut guard) = BOOT_ANIMATION.try_lock() {
        if let Some(ref mut anim) = *guard {
            return anim.tick();
        }
    }
    // Return false (not complete) if we couldn't get the lock
    false
}

/// Force complete the boot animation (safe for interrupt context)
pub fn complete() {
    // Use try_lock to avoid deadlock when called from interrupt context
    if let Some(mut guard) = BOOT_ANIMATION.try_lock() {
        if let Some(ref mut anim) = *guard {
            anim.force_complete();
        }
    }
}

/// Render boot animation to framebuffer (safe for interrupt context)
pub fn render(renderer: &mut Renderer) {
    // Use try_lock to avoid deadlock when called from interrupt context
    if let Some(guard) = BOOT_ANIMATION.try_lock() {
        if let Some(ref anim) = *guard {
            anim.render(renderer);
        }
    }
}
