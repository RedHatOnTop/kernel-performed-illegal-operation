//! Renderer
//!
//! High-quality framebuffer rendering primitives with anti-aliasing.
//! Eliminates pixel-art appearance through sub-pixel coverage computation.

use super::font::Font8x8;
use crate::graphics::DamageRect;

/// Color in ARGB format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Blend this color over another (Porter-Duff Source Over)
    #[inline]
    pub fn blend_over(self, dst: Color) -> Color {
        if self.a == 255 {
            return self;
        }
        if self.a == 0 {
            return dst;
        }
        let sa = self.a as u32;
        let inv = 255 - sa;
        Color {
            r: ((self.r as u32 * sa + dst.r as u32 * inv + 128) / 255) as u8,
            g: ((self.g as u32 * sa + dst.g as u32 * inv + 128) / 255) as u8,
            b: ((self.b as u32 * sa + dst.b as u32 * inv + 128) / 255) as u8,
            a: (sa + (dst.a as u32 * inv + 128) / 255).min(255) as u8,
        }
    }

    /// Create the same colour with a different alpha
    #[inline]
    pub const fn with_alpha(self, a: u8) -> Color {
        Color {
            r: self.r,
            g: self.g,
            b: self.b,
            a,
        }
    }

    /// Darken by amount (0-255)
    pub fn darken(self, amount: u8) -> Color {
        let f = (255u32 - amount as u32);
        Color {
            r: (self.r as u32 * f / 255) as u8,
            g: (self.g as u32 * f / 255) as u8,
            b: (self.b as u32 * f / 255) as u8,
            a: self.a,
        }
    }

    /// Lighten by amount (0-255)
    pub fn lighten(self, amount: u8) -> Color {
        let a = amount as u32;
        Color {
            r: self
                .r
                .saturating_add(((255 - self.r as u32) * a / 255) as u8),
            g: self
                .g
                .saturating_add(((255 - self.g as u32) * a / 255) as u8),
            b: self
                .b
                .saturating_add(((255 - self.b as u32) * a / 255) as u8),
            a: self.a,
        }
    }

    /// Linearly interpolate between two colours, t in 0..256
    #[inline]
    pub fn lerp(a: Color, b: Color, t: u32) -> Color {
        let inv = 256 - t;
        Color {
            r: ((a.r as u32 * inv + b.r as u32 * t) >> 8) as u8,
            g: ((a.g as u32 * inv + b.g as u32 * t) >> 8) as u8,
            b: ((a.b as u32 * inv + b.b as u32 * t) >> 8) as u8,
            a: ((a.a as u32 * inv + b.a as u32 * t) >> 8) as u8,
        }
    }

    // ── Legacy/convenience aliases referenced by other modules ──
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const RED: Color = Color::rgb(255, 0, 0);
    pub const GREEN: Color = Color::rgb(0, 255, 0);
    pub const BLUE: Color = Color::rgb(0, 0, 255);
    pub const GRAY: Color = Color::rgb(128, 128, 128);
    pub const DARK_GRAY: Color = Color::rgb(64, 64, 64);
    pub const LIGHT_GRAY: Color = Color::rgb(192, 192, 192);
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);

    // Old theme references (kept for backward compat — prefer theme::*)
    pub const DESKTOP_BG: Color = Color::rgb(0, 30, 60);
    pub const TASKBAR_BG: Color = Color::rgb(30, 30, 30);
    pub const WINDOW_BG: Color = Color::rgb(250, 250, 252);
    pub const WINDOW_TITLE_ACTIVE: Color = Color::rgb(240, 240, 244);
    pub const WINDOW_TITLE_INACTIVE: Color = Color::rgb(230, 230, 234);
    pub const BUTTON_BG: Color = Color::rgb(225, 225, 225);
    pub const BUTTON_HOVER: Color = Color::rgb(200, 200, 200);
    pub const CLOSE_BUTTON_HOVER: Color = Color::rgb(235, 77, 75);
    pub const SELECTION: Color = Color::rgba(56, 132, 244, 180);
    pub const SHADOW: Color = Color::rgba(0, 0, 0, 40);
    pub const HIGHLIGHT: Color = Color::rgba(255, 255, 255, 128);
}

/// Clipping rectangle
#[derive(Debug, Clone, Copy)]
pub struct ClipRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
impl ClipRect {
    pub fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self {
            x,
            y,
            width: w,
            height: h,
        }
    }
    pub fn from_damage(r: &DamageRect) -> Self {
        Self {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
    #[inline]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width as i32
            && y >= self.y
            && y < self.y + self.height as i32
    }
}

// ════════════════════════════════════════════════════════════════════
//  Renderer
// ════════════════════════════════════════════════════════════════════

pub struct Renderer {
    buffer: *mut u8,
    width: u32,
    height: u32,
    bpp: usize,
    stride: usize,
}

impl Renderer {
    pub fn new(buffer: *mut u8, width: u32, height: u32, bpp: usize, stride: usize) -> Self {
        Self {
            buffer,
            width,
            height,
            bpp,
            stride,
        }
    }

    // ─────────── pixel-level ───────────

    #[inline]
    pub fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let off = (y as usize * self.stride + x as usize) * self.bpp;
        unsafe {
            *self.buffer.add(off) = color.b;
            *self.buffer.add(off + 1) = color.g;
            *self.buffer.add(off + 2) = color.r;
            if self.bpp == 4 {
                *self.buffer.add(off + 3) = color.a;
            }
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: i32, y: i32) -> Color {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return Color::BLACK;
        }
        let off = (y as usize * self.stride + x as usize) * self.bpp;
        unsafe {
            Color {
                b: *self.buffer.add(off),
                g: *self.buffer.add(off + 1),
                r: *self.buffer.add(off + 2),
                a: if self.bpp == 4 {
                    *self.buffer.add(off + 3)
                } else {
                    255
                },
            }
        }
    }

    /// Blend a single pixel with alpha onto the framebuffer (read-modify-write)
    #[inline]
    pub fn blend_pixel(&mut self, x: i32, y: i32, color: Color) {
        if color.a == 0 {
            return;
        }
        if color.a == 255 {
            self.set_pixel(x, y, color);
            return;
        }
        let dst = self.get_pixel(x, y);
        self.set_pixel(x, y, color.blend_over(dst));
    }

    // ─────────── rectangle primitives ───────────

    pub fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
        let x0 = x.max(0) as u32;
        let y0 = y.max(0) as u32;
        let x1 = ((x + w as i32) as u32).min(self.width);
        let y1 = ((y + h as i32) as u32).min(self.height);
        if color.a == 255 {
            for py in y0..y1 {
                let row_off = (py as usize * self.stride + x0 as usize) * self.bpp;
                for px in 0..(x1 - x0) as usize {
                    let off = row_off + px * self.bpp;
                    unsafe {
                        *self.buffer.add(off) = color.b;
                        *self.buffer.add(off + 1) = color.g;
                        *self.buffer.add(off + 2) = color.r;
                        if self.bpp == 4 {
                            *self.buffer.add(off + 3) = 255;
                        }
                    }
                }
            }
        } else {
            for py in y0..y1 {
                for px in x0..x1 {
                    self.blend_pixel(px as i32, py as i32, color);
                }
            }
        }
    }

    pub fn fill_rect_alpha(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
        self.fill_rect(x, y, w, h, color);
    }

    pub fn draw_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
        self.draw_hline(x, y, w, color);
        self.draw_hline(x, y + h as i32 - 1, w, color);
        self.draw_vline(x, y, h, color);
        self.draw_vline(x + w as i32 - 1, y, h, color);
    }

    pub fn draw_hline(&mut self, x: i32, y: i32, w: u32, color: Color) {
        for i in 0..w as i32 {
            self.blend_pixel(x + i, y, color);
        }
    }

    pub fn draw_vline(&mut self, x: i32, y: i32, h: u32, color: Color) {
        for i in 0..h as i32 {
            self.blend_pixel(x, y + i, color);
        }
    }

    pub fn clear(&mut self, color: Color) {
        self.fill_rect(0, 0, self.width, self.height, color);
    }

    // ─────────── anti-aliased rounded rectangles ───────────

    /// Fill a rounded rectangle with full anti-aliasing on the corners.
    /// This is the primary shape primitive for the modern UI.
    pub fn fill_rounded_rect_aa(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        radius: u32,
        color: Color,
    ) {
        if w == 0 || h == 0 {
            return;
        }
        let r = radius.min(w / 2).min(h / 2);
        if r == 0 {
            self.fill_rect(x, y, w, h, color);
            return;
        }

        let ri = r as i32;
        let wi = w as i32;
        let hi = h as i32;

        // For each scan-line, compute horizontal extent considering rounded corners
        for row in 0..hi {
            let (left_inset, right_inset) = if row < ri {
                // top corners
                let dy = ri - row;
                let inset = Self::circle_inset(ri, dy);
                (inset, inset)
            } else if row >= hi - ri {
                // bottom corners
                let dy = row - (hi - ri) + 1;
                let inset = Self::circle_inset(ri, dy);
                (inset, inset)
            } else {
                (0i32, 0i32)
            };

            let lx = x + left_inset;
            let rx = x + wi - right_inset;
            if lx < rx {
                // Inner opaque span
                for px in (lx + 1)..(rx - 1) {
                    self.blend_pixel(px, y + row, color);
                }
                // Left AA edge pixel
                let cov_l = self.corner_coverage(x, y, w, h, r, lx, y + row);
                self.blend_pixel(
                    lx,
                    y + row,
                    color.with_alpha(((color.a as u32 * cov_l) / 255) as u8),
                );
                // Right AA edge pixel
                let cov_r = self.corner_coverage(x, y, w, h, r, rx - 1, y + row);
                self.blend_pixel(
                    rx - 1,
                    y + row,
                    color.with_alpha(((color.a as u32 * cov_r) / 255) as u8),
                );
            }
        }
    }

    /// Stroke a rounded rectangle with anti-aliased edges (1px border)
    pub fn draw_rounded_rect_aa(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        radius: u32,
        color: Color,
    ) {
        if w == 0 || h == 0 {
            return;
        }
        let r = radius.min(w / 2).min(h / 2);
        if r == 0 {
            self.draw_rect(x, y, w, h, color);
            return;
        }

        let ri = r as i32;
        let wi = w as i32;
        let hi = h as i32;

        // Top / bottom straight portions
        for px in (x + ri)..(x + wi - ri) {
            self.blend_pixel(px, y, color);
            self.blend_pixel(px, y + hi - 1, color);
        }
        // Left / right straight portions
        for py in (y + ri)..(y + hi - ri) {
            self.blend_pixel(x, py, color);
            self.blend_pixel(x + wi - 1, py, color);
        }
        // Anti-aliased arcs at each corner
        self.draw_aa_arc(x + ri, y + ri, ri, 0, color);
        self.draw_aa_arc(x + wi - ri - 1, y + ri, ri, 1, color);
        self.draw_aa_arc(x + ri, y + hi - ri - 1, ri, 2, color);
        self.draw_aa_arc(x + wi - ri - 1, y + hi - ri - 1, ri, 3, color);
    }

    // ─────────── Legacy non-AA rounded rects (delegate to AA) ───────────

    pub fn fill_rounded_rect(&mut self, x: i32, y: i32, w: u32, h: u32, radius: u32, color: Color) {
        self.fill_rounded_rect_aa(x, y, w, h, radius, color);
    }

    pub fn draw_rounded_rect(&mut self, x: i32, y: i32, w: u32, h: u32, radius: u32, color: Color) {
        self.draw_rounded_rect_aa(x, y, w, h, radius, color);
    }

    // ─────────── anti-aliased circle / arc helpers ───────────

    /// Draw an anti-aliased quarter-circle arc
    /// quadrant: 0=TL, 1=TR, 2=BL, 3=BR
    fn draw_aa_arc(&mut self, cx: i32, cy: i32, r: i32, quadrant: u8, color: Color) {
        if r <= 0 {
            return;
        }
        // Walk the circle using the distance from edge for AA coverage
        for dy in 0..=r {
            for dx in 0..=r {
                let dist_sq = dx * dx + dy * dy;
                let r_sq = r * r;
                let r_inner_sq = (r - 1) * (r - 1);

                if dist_sq <= r_sq {
                    let (px, py) = match quadrant {
                        0 => (cx - dx, cy - dy),
                        1 => (cx + dx, cy - dy),
                        2 => (cx - dx, cy + dy),
                        _ => (cx + dx, cy + dy),
                    };

                    if dist_sq > r_inner_sq {
                        // Edge pixel — compute coverage for AA
                        // Use distance from the ideal circle boundary
                        let dist = Self::isqrt(dist_sq as u32) as i32;
                        let coverage = if dist <= r {
                            let frac = (r - dist) as u32;
                            (frac * 255 / r.max(1) as u32).min(255)
                        } else {
                            0
                        };
                        let a = ((color.a as u32 * coverage) / 255) as u8;
                        if a > 0 {
                            self.blend_pixel(px, py, color.with_alpha(a));
                        }
                    } else {
                        self.blend_pixel(px, py, color);
                    }
                }
            }
        }
    }

    /// Fill an anti-aliased circle  
    pub fn fill_circle_aa(&mut self, cx: i32, cy: i32, r: i32, color: Color) {
        if r <= 0 {
            return;
        }
        for dy in -r..=r {
            for dx in -r..=r {
                let dist_sq = (dx * dx + dy * dy) as u32;
                let r_sq = (r * r) as u32;
                if dist_sq <= r_sq {
                    let dist = Self::isqrt(dist_sq);
                    let ru = r as u32;
                    if dist >= ru.saturating_sub(1) {
                        // AA edge
                        let coverage = ((ru * 255).saturating_sub(dist * 255)) / ru.max(1);
                        let a = ((color.a as u32 * coverage.min(255)) / 255) as u8;
                        if a > 0 {
                            self.blend_pixel(cx + dx, cy + dy, color.with_alpha(a));
                        }
                    } else {
                        self.blend_pixel(cx + dx, cy + dy, color);
                    }
                }
            }
        }
    }

    /// Stroke an anti-aliased circle (1px)
    pub fn draw_circle_aa(&mut self, cx: i32, cy: i32, r: i32, color: Color) {
        if r <= 0 {
            self.blend_pixel(cx, cy, color);
            return;
        }
        for dy in -r - 1..=r + 1 {
            for dx in -r - 1..=r + 1 {
                let dist = Self::isqrt((dx * dx + dy * dy) as u32) as i32;
                let diff = (dist - r).abs();
                if diff <= 1 {
                    // Map distance to coverage
                    let coverage = (255 - diff as u32 * 180).min(255);
                    let a = ((color.a as u32 * coverage) / 255) as u8;
                    if a > 8 {
                        self.blend_pixel(cx + dx, cy + dy, color.with_alpha(a));
                    }
                }
            }
        }
    }

    // ─────────── Gradient fills ───────────

    /// Vertical linear gradient
    pub fn fill_gradient_v(&mut self, x: i32, y: i32, w: u32, h: u32, top: Color, bottom: Color) {
        if h == 0 {
            return;
        }
        for row in 0..h as i32 {
            let t = (row as u32 * 256) / (h - 1).max(1);
            let c = Color::lerp(top, bottom, t);
            self.draw_hline(x, y + row, w, c);
        }
    }

    /// Vertical gradient inside an AA rounded rect
    pub fn fill_rounded_gradient_v(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        radius: u32,
        top: Color,
        bottom: Color,
    ) {
        if w == 0 || h == 0 {
            return;
        }
        let r = radius.min(w / 2).min(h / 2);
        if r == 0 {
            self.fill_gradient_v(x, y, w, h, top, bottom);
            return;
        }

        let ri = r as i32;
        let wi = w as i32;
        let hi = h as i32;

        for row in 0..hi {
            let t = (row as u32 * 256) / (hi as u32 - 1).max(1);
            let c = Color::lerp(top, bottom, t);

            let inset = if row < ri {
                Self::circle_inset(ri, ri - row)
            } else if row >= hi - ri {
                Self::circle_inset(ri, row - (hi - ri) + 1)
            } else {
                0
            };
            let lx = x + inset;
            let rx = x + wi - inset;
            if lx < rx {
                for px in (lx + 1)..(rx - 1) {
                    self.blend_pixel(px, y + row, c);
                }
                // AA edge pixels
                let cov_l = self.corner_coverage(x, y, w, h, r, lx, y + row);
                self.blend_pixel(
                    lx,
                    y + row,
                    c.with_alpha(((c.a as u32 * cov_l) / 255) as u8),
                );
                let cov_r = self.corner_coverage(x, y, w, h, r, rx - 1, y + row);
                self.blend_pixel(
                    rx - 1,
                    y + row,
                    c.with_alpha(((c.a as u32 * cov_r) / 255) as u8),
                );
            }
        }
    }

    /// Horizontal linear gradient
    pub fn fill_gradient_h(&mut self, x: i32, y: i32, w: u32, h: u32, left: Color, right: Color) {
        if w == 0 {
            return;
        }
        for col in 0..w as i32 {
            let t = (col as u32 * 256) / (w - 1).max(1);
            let c = Color::lerp(left, right, t);
            self.draw_vline(x + col, y, h, c);
        }
    }

    /// Anti-aliased line using Xiaolin Wu's algorithm
    pub fn draw_line_aa(
        &mut self,
        mut x0: i32,
        mut y0: i32,
        mut x1: i32,
        mut y1: i32,
        color: Color,
    ) {
        let steep = (y1 - y0).abs() > (x1 - x0).abs();
        if steep {
            core::mem::swap(&mut x0, &mut y0);
            core::mem::swap(&mut x1, &mut y1);
        }
        if x0 > x1 {
            core::mem::swap(&mut x0, &mut x1);
            core::mem::swap(&mut y0, &mut y1);
        }

        let dx = x1 - x0;
        let dy = y1 - y0;
        let gradient = if dx == 0 { 256i32 } else { (dy * 256) / dx };

        // First endpoint
        let xend = x0;
        let yend = y0 * 256;
        if steep {
            self.blend_pixel(yend / 256, xend, color);
        } else {
            self.blend_pixel(xend, yend / 256, color);
        }
        let mut intery = yend + gradient;

        // Second endpoint
        let xend2 = x1;

        // Main loop
        for x in (xend + 1)..xend2 {
            let iy = intery / 256;
            let frac = ((intery & 0xFF) as u32).min(255);
            let inv = 255 - frac;
            if steep {
                self.blend_pixel(
                    iy,
                    x,
                    color.with_alpha(((color.a as u32 * inv) / 255) as u8),
                );
                self.blend_pixel(
                    iy + 1,
                    x,
                    color.with_alpha(((color.a as u32 * frac) / 255) as u8),
                );
            } else {
                self.blend_pixel(
                    x,
                    iy,
                    color.with_alpha(((color.a as u32 * inv) / 255) as u8),
                );
                self.blend_pixel(
                    x,
                    iy + 1,
                    color.with_alpha(((color.a as u32 * frac) / 255) as u8),
                );
            }
            intery += gradient;
        }
        // Last endpoint
        if steep {
            self.blend_pixel(intery / 256, xend2, color);
        } else {
            self.blend_pixel(xend2, intery / 256, color);
        }
    }

    /// Fill a triangle with a solid colour (flat-bottom/flat-top scanline algorithm)
    pub fn fill_triangle(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: Color,
    ) {
        // Sort vertices by y
        let (mut ax, mut ay) = (x0, y0);
        let (mut bx, mut by) = (x1, y1);
        let (mut cx, mut cy) = (x2, y2);
        if ay > by {
            core::mem::swap(&mut ax, &mut bx);
            core::mem::swap(&mut ay, &mut by);
        }
        if ay > cy {
            core::mem::swap(&mut ax, &mut cx);
            core::mem::swap(&mut ay, &mut cy);
        }
        if by > cy {
            core::mem::swap(&mut bx, &mut cx);
            core::mem::swap(&mut by, &mut cy);
        }

        let total_height = cy - ay;
        if total_height == 0 {
            return;
        }

        for y in ay..=cy {
            let second_half = y >= by;
            let seg_height = if second_half { cy - by } else { by - ay };
            if seg_height == 0 {
                continue;
            }

            let alpha = ((y - ay) * 256) / total_height;
            let beta = if second_half {
                ((y - by) * 256) / seg_height
            } else {
                ((y - ay) * 256) / seg_height
            };

            let mut sx = ax + (cx - ax) * alpha / 256;
            let mut ex = if second_half {
                bx + (cx - bx) * beta / 256
            } else {
                ax + (bx - ax) * beta / 256
            };
            if sx > ex {
                core::mem::swap(&mut sx, &mut ex);
            }
            for x in sx..=ex {
                self.blend_pixel(x, y, color);
            }
        }
    }

    // ─────────── Soft shadow ───────────

    /// Draw a blurred rectangular shadow under a rounded rectangle.
    /// Uses layered, expanding semi-transparent rectangles.
    pub fn draw_shadow_box(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        radius: u32,
        offset_x: i32,
        offset_y: i32,
        blur: u32,
        color: Color,
    ) {
        if blur == 0 {
            self.fill_rounded_rect_aa(x + offset_x, y + offset_y, w, h, radius, color);
            return;
        }
        // Approximate Gaussian blur with concentric expanding layers
        let layers = blur.min(12);
        for i in 0..layers {
            let spread = i as i32;
            let alpha = ((color.a as u32) * (layers - i) as u32 / (layers * layers) as u32) as u8;
            if alpha == 0 {
                continue;
            }
            let c = color.with_alpha(alpha);
            let r = radius + i;
            self.fill_rounded_rect_aa(
                x + offset_x - spread,
                y + offset_y - spread,
                w + 2 * spread as u32,
                h + 2 * spread as u32,
                r,
                c,
            );
        }
    }

    /// Legacy shadow API
    pub fn draw_shadow(&mut self, x: i32, y: i32, w: u32, h: u32, offset: i32, blur: u32) {
        self.draw_shadow_box(x, y, w, h, 0, offset, offset, blur, Color::SHADOW);
    }

    // ─────────── Text rendering ───────────

    pub fn draw_text(&mut self, x: i32, y: i32, text: &str, color: Color) {
        let mut cx = x;
        for ch in text.chars() {
            if ch == ' ' {
                cx += 8;
                continue;
            }
            self.draw_char_aa(cx, y, ch, color);
            cx += 8;
        }
    }

    /// Draw a single character (delegates to AA version)
    pub fn draw_char(&mut self, x: i32, y: i32, ch: char, color: Color) {
        self.draw_char_aa(x, y, ch, color);
    }

    pub fn draw_text_scaled(&mut self, x: i32, y: i32, text: &str, color: Color, scale: u32) {
        if scale <= 1 {
            self.draw_text(x, y, text, color);
            return;
        }
        let mut cx = x;
        for ch in text.chars() {
            if ch == ' ' {
                cx += 8 * scale as i32;
                continue;
            }
            self.draw_char_scaled_aa(cx, y, ch, color, scale);
            cx += 8 * scale as i32;
        }
    }

    /// Draw a character with basic anti-aliasing (sub-pixel fringe smoothing).
    /// Uses the 8x8 glyph but blends edge pixels at partial opacity.
    fn draw_char_aa(&mut self, x: i32, y: i32, ch: char, color: Color) {
        let glyph = Font8x8::get_glyph(ch);

        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..8i32 {
                let px = x + col;
                let py = y + row as i32;
                if (bits >> (7 - col)) & 1 == 1 {
                    // Foreground pixel — check neighbours for edge softening
                    let is_edge = self.is_glyph_edge(&glyph, row, col as usize);
                    if is_edge {
                        self.blend_pixel(
                            px,
                            py,
                            color.with_alpha(((color.a as u32 * 210) / 255) as u8),
                        );
                    } else {
                        self.blend_pixel(px, py, color);
                    }
                } else {
                    // Background pixel next to foreground → fringe AA
                    let adj = self.count_glyph_neighbours(&glyph, row, col as usize);
                    if adj > 0 {
                        let strength = (adj as u32 * 35).min(90);
                        let a = ((color.a as u32 * strength) / 255) as u8;
                        if a > 0 {
                            self.blend_pixel(px, py, color.with_alpha(a));
                        }
                    }
                }
            }
        }
    }

    /// Scaled character with AA
    fn draw_char_scaled_aa(&mut self, x: i32, y: i32, ch: char, color: Color, scale: u32) {
        let glyph = Font8x8::get_glyph(ch);
        let s = scale as i32;

        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if (bits >> (7 - col)) & 1 == 1 {
                    let bx = x + col * s;
                    let by = y + row as i32 * s;
                    // Fill the inner block
                    self.fill_rect(bx, by, scale, scale, color);

                    // Add AA fringe around edges
                    let is_edge = self.is_glyph_edge(&glyph, row, col as usize);
                    if is_edge {
                        let fringe = color.with_alpha(((color.a as u32 * 60) / 255) as u8);
                        // Check which sides are exposed and add fringe
                        if col == 0 || (bits >> (7 - col + 1)) & 1 == 0 {
                            for fy in 0..s {
                                self.blend_pixel(bx - 1, by + fy, fringe);
                            }
                        }
                        if col == 7 || (bits >> (7 - col - 1)) & 1 == 0 {
                            for fy in 0..s {
                                self.blend_pixel(bx + s, by + fy, fringe);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Check if a glyph pixel is on the edge (has at least one empty neighbour)
    fn is_glyph_edge(&self, glyph: &[u8; 8], row: usize, col: usize) -> bool {
        let dirs: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        for (dr, dc) in dirs {
            let nr = row as i32 + dr;
            let nc = col as i32 + dc;
            if nr < 0 || nr >= 8 || nc < 0 || nc >= 8 {
                return true;
            }
            if (glyph[nr as usize] >> (7 - nc)) & 1 == 0 {
                return true;
            }
        }
        false
    }

    /// Count how many set-pixel neighbours a background pixel has
    fn count_glyph_neighbours(&self, glyph: &[u8; 8], row: usize, col: usize) -> u32 {
        let dirs: [(i32, i32); 8] = [
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, -1),
            (0, 1),
            (1, -1),
            (1, 0),
            (1, 1),
        ];
        let mut count = 0u32;
        for (dr, dc) in dirs {
            let nr = row as i32 + dr;
            let nc = col as i32 + dc;
            if nr >= 0 && nr < 8 && nc >= 0 && nc < 8 {
                if (glyph[nr as usize] >> (7 - nc)) & 1 == 1 {
                    count += 1;
                }
            }
        }
        count
    }

    // ─────────── Icon rendering ───────────

    /// Draw a 16x16 bitmap icon with anti-aliased edges
    pub fn draw_icon(&mut self, x: i32, y: i32, icon: &[u16; 16], color: Color) {
        for (row, &bits) in icon.iter().enumerate() {
            for col in 0..16i32 {
                let px = x + col;
                let py = y + row as i32;
                if (bits >> (15 - col)) & 1 == 1 {
                    self.blend_pixel(px, py, color);
                }
            }
        }
    }

    /// Draw a 16x16 icon scaled up with AA fringe
    pub fn draw_icon_scaled(&mut self, x: i32, y: i32, icon: &[u16; 16], color: Color, scale: i32) {
        for (row, &bits) in icon.iter().enumerate() {
            for col in 0..16 {
                if (bits >> (15 - col)) & 1 == 1 {
                    let bx = x + col * scale;
                    let by = y + row as i32 * scale;
                    // Fill inner
                    self.fill_rect(bx, by, scale as u32, scale as u32, color);
                    // Fringe around outer edges
                    let fringe = color.with_alpha(((color.a as u32 * 50) / 255) as u8);
                    let left = col == 0 || (bits >> (15 - col + 1)) & 1 == 0;
                    let right = col == 15 || (bits >> (15 - col - 1)) & 1 == 0;
                    let top = row == 0 || (icon[row - 1] >> (15 - col)) & 1 == 0;
                    let bot = row == 15 || (icon[row + 1] >> (15 - col)) & 1 == 0;
                    if left {
                        for i in 0..scale {
                            self.blend_pixel(bx - 1, by + i, fringe);
                        }
                    }
                    if right {
                        for i in 0..scale {
                            self.blend_pixel(bx + scale, by + i, fringe);
                        }
                    }
                    if top {
                        for i in 0..scale {
                            self.blend_pixel(bx + i, by - 1, fringe);
                        }
                    }
                    if bot {
                        for i in 0..scale {
                            self.blend_pixel(bx + i, by + scale, fringe);
                        }
                    }
                }
            }
        }
    }

    // ─────────── Helper math ───────────

    /// Integer square root (good enough for coverage computation)
    #[inline]
    fn isqrt(n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let mut x = n;
        let mut y = (x + 1) / 2;
        while y < x {
            x = y;
            y = (x + n / x) / 2;
        }
        x
    }

    /// How many pixels the circle of given radius is inset at a certain dy from center
    #[inline]
    fn circle_inset(r: i32, dy: i32) -> i32 {
        if dy > r {
            return r;
        }
        // inset = r - sqrt(r² - dy²)
        let sq = (r * r - dy * dy).max(0) as u32;
        r - Self::isqrt(sq) as i32
    }

    /// Compute AA coverage for a pixel relative to the rounded-rect boundary.
    /// Returns 0-255 coverage.
    fn corner_coverage(
        &self,
        rx: i32,
        ry: i32,
        rw: u32,
        rh: u32,
        radius: u32,
        px: i32,
        py: i32,
    ) -> u32 {
        let r = radius as i32;
        // Determine which corner this point is in
        let (cx, cy) = if px < rx + r && py < ry + r {
            (rx + r, ry + r) // TL
        } else if px >= rx + rw as i32 - r && py < ry + r {
            (rx + rw as i32 - r - 1, ry + r) // TR
        } else if px < rx + r && py >= ry + rh as i32 - r {
            (rx + r, ry + rh as i32 - r - 1) // BL
        } else if px >= rx + rw as i32 - r && py >= ry + rh as i32 - r {
            (rx + rw as i32 - r - 1, ry + rh as i32 - r - 1) // BR
        } else {
            return 255; // Not in a corner — full coverage
        };

        let dx = px - cx;
        let dy = py - cy;
        let dist_sq = (dx * dx + dy * dy) as u32;
        let r_sq = (r * r) as u32;

        if dist_sq > r_sq {
            0
        } else {
            let dist = Self::isqrt(dist_sq);
            let ru = r as u32;
            if dist >= ru.saturating_sub(1) {
                ((ru * 255).saturating_sub(dist * 255)) / ru.max(1)
            } else {
                255
            }
        }
    }

    // Legacy helpers kept for backward compatibility
    fn draw_corner(&mut self, cx: i32, cy: i32, r: i32, quadrant: u8, color: Color) {
        self.draw_aa_arc(cx, cy, r, quadrant, color);
    }
    fn fill_corner(&mut self, cx: i32, cy: i32, r: i32, quadrant: u8, color: Color) {
        // AA quarter-disc fill via draw_aa_arc with filled interior
        self.draw_aa_arc(cx, cy, r, quadrant, color);
    }
}
