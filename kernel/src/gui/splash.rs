//! PWA Splash Screen
//!
//! Renders a branded loading screen when a WebApp window is first opened:
//!   - `background_color` fills the entire content area
//!   - App icon centred (if available)
//!   - App name below the icon
//!
//! The splash is shown until the `start_url` finishes loading or a
//! 3-second timeout elapses.

use super::render::{Color, Renderer};
use super::theme::{Spacing, Text};

/// Render a PWA-style splash screen into the given content rectangle.
///
/// * `bg_color` — ARGB u32 from manifest `background_color`
/// * `name` — the application name
/// * `icon_data` — optional raw PNG bytes (rendered as simple bitmap if
///   a decoder is available; otherwise falls back to the first letter)
pub fn render_splash(
    renderer: &mut Renderer,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    bg_color: u32,
    name: &str,
    _icon_data: Option<&[u8]>,
) {
    // Fill background
    let bg = Color::rgba(
        (bg_color >> 16) as u8,
        (bg_color >> 8) as u8,
        bg_color as u8,
        255,
    );
    renderer.fill_rect(x, y, w, h, bg);

    let cx = x + w as i32 / 2;
    let cy = y + h as i32 / 2;

    // Icon placeholder — filled circle with the first letter
    let radius = 32i32;
    let icon_color = Color::rgba(255, 255, 255, 200);
    renderer.fill_rounded_rect_aa(
        cx - radius,
        cy - radius - 20,
        (radius * 2) as u32,
        (radius * 2) as u32,
        radius as u32,
        icon_color,
    );

    // First letter of name drawn on the circle
    if let Some(ch) = name.chars().next() {
        let letter = alloc::format!("{}", ch);
        let lx = cx - 4;
        let ly = cy - 20 - 6;
        renderer.draw_text(lx, ly, &letter, Color::rgba(0, 0, 0, 200));
    }

    // App name centered below the icon
    let name_len = name.len() as i32 * 8;
    let name_x = cx - name_len / 2;
    let name_y = cy + radius + 4;
    renderer.draw_text(name_x, name_y, name, Text::ON_DARK);
}
