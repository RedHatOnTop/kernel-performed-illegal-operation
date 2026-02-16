//! Rendering Primitives
//!
//! This module defines the core primitive types used throughout the
//! WebRender pipeline.

use alloc::vec::Vec;

// Helper functions for no_std environment
#[inline]
pub fn floor_f32(x: f32) -> f32 {
    let xi = x as i32;
    if x < xi as f32 {
        (xi - 1) as f32
    } else {
        xi as f32
    }
}

#[inline]
pub fn ceil_f32(x: f32) -> f32 {
    let xi = x as i32;
    if x > xi as f32 {
        (xi + 1) as f32
    } else {
        xi as f32
    }
}

#[inline]
pub fn abs_f32(x: f32) -> f32 {
    if x < 0.0 {
        -x
    } else {
        x
    }
}

#[inline]
fn sin_f32(x: f32) -> f32 {
    // Taylor series approximation for sin
    let x = x % (2.0 * core::f32::consts::PI);
    let x3 = x * x * x;
    let x5 = x3 * x * x;
    let x7 = x5 * x * x;
    x - x3 / 6.0 + x5 / 120.0 - x7 / 5040.0
}

#[inline]
fn cos_f32(x: f32) -> f32 {
    sin_f32(x + core::f32::consts::FRAC_PI_2)
}

#[inline]
fn tan_f32(x: f32) -> f32 {
    sin_f32(x) / cos_f32(x)
}

/// A 2D point.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A 2D size.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Size = Size {
        width: 0.0,
        height: 0.0,
    };

    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// A 2D rectangle.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const ZERO: Rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    };

    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn from_origin_size(origin: Point, size: Size) -> Self {
        Self {
            x: origin.x,
            y: origin.y,
            w: size.width,
            h: size.height,
        }
    }

    pub fn origin(&self) -> Point {
        Point {
            x: self.x,
            y: self.y,
        }
    }

    pub fn size(&self) -> Size {
        Size {
            width: self.w,
            height: self.h,
        }
    }

    pub fn min_x(&self) -> f32 {
        self.x
    }
    pub fn min_y(&self) -> f32 {
        self.y
    }
    pub fn max_x(&self) -> f32 {
        self.x + self.w
    }
    pub fn max_y(&self) -> f32 {
        self.y + self.h
    }
    pub fn center(&self) -> Point {
        Point {
            x: self.x + self.w / 2.0,
            y: self.y + self.h / 2.0,
        }
    }

    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x
            && point.x < self.x + self.w
            && point.y >= self.y
            && point.y < self.y + self.h
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.w
            && self.x + self.w > other.x
            && self.y < other.y + other.h
            && self.y + self.h > other.y
    }

    pub fn intersect(&self, other: &Rect) -> Rect {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.w).min(other.x + other.w);
        let y2 = (self.y + self.h).min(other.y + other.h);

        if x2 > x1 && y2 > y1 {
            Rect {
                x: x1,
                y: y1,
                w: x2 - x1,
                h: y2 - y1,
            }
        } else {
            Rect::ZERO
        }
    }

    pub fn union(&self, other: &Rect) -> Rect {
        if self.w <= 0.0 || self.h <= 0.0 {
            return *other;
        }
        if other.w <= 0.0 || other.h <= 0.0 {
            return *self;
        }
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = (self.x + self.w).max(other.x + other.w);
        let y2 = (self.y + self.h).max(other.y + other.h);
        Rect {
            x: x1,
            y: y1,
            w: x2 - x1,
            h: y2 - y1,
        }
    }

    pub fn inflate(&self, amount: f32) -> Rect {
        Rect {
            x: self.x - amount,
            y: self.y - amount,
            w: self.w + amount * 2.0,
            h: self.h + amount * 2.0,
        }
    }
}

/// RGBA color.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    pub const BLACK: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const WHITE: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const RED: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const GREEN: Color = Color {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const BLUE: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    pub fn to_rgba8(&self) -> [u8; 4] {
        [
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
            (self.a * 255.0) as u8,
        ]
    }

    pub fn premultiply(&self) -> Color {
        Color {
            r: self.r * self.a,
            g: self.g * self.a,
            b: self.b * self.a,
            a: self.a,
        }
    }

    pub fn with_alpha(&self, alpha: f32) -> Color {
        Color { a: alpha, ..*self }
    }

    pub fn blend(&self, other: &Color) -> Color {
        let a = other.a + self.a * (1.0 - other.a);
        if a == 0.0 {
            return Color::TRANSPARENT;
        }
        Color {
            r: (other.r * other.a + self.r * self.a * (1.0 - other.a)) / a,
            g: (other.g * other.a + self.g * self.a * (1.0 - other.a)) / a,
            b: (other.b * other.a + self.b * self.a * (1.0 - other.a)) / a,
            a,
        }
    }
}

/// 2D affine transform matrix (3x2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub m11: f32,
    pub m12: f32,
    pub m21: f32,
    pub m22: f32,
    pub m31: f32,
    pub m32: f32,
}

impl Transform {
    pub fn identity() -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            m31: 0.0,
            m32: 0.0,
        }
    }

    pub fn translate(x: f32, y: f32) -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            m31: x,
            m32: y,
        }
    }

    pub fn scale(x: f32, y: f32) -> Self {
        Self {
            m11: x,
            m12: 0.0,
            m21: 0.0,
            m22: y,
            m31: 0.0,
            m32: 0.0,
        }
    }

    pub fn rotate(angle: f32) -> Self {
        let cos = cos_f32(angle);
        let sin = sin_f32(angle);
        Self {
            m11: cos,
            m12: sin,
            m21: -sin,
            m22: cos,
            m31: 0.0,
            m32: 0.0,
        }
    }

    pub fn skew(x: f32, y: f32) -> Self {
        Self {
            m11: 1.0,
            m12: tan_f32(y),
            m21: tan_f32(x),
            m22: 1.0,
            m31: 0.0,
            m32: 0.0,
        }
    }

    pub fn then(&self, other: &Transform) -> Transform {
        Transform {
            m11: self.m11 * other.m11 + self.m12 * other.m21,
            m12: self.m11 * other.m12 + self.m12 * other.m22,
            m21: self.m21 * other.m11 + self.m22 * other.m21,
            m22: self.m21 * other.m12 + self.m22 * other.m22,
            m31: self.m31 * other.m11 + self.m32 * other.m21 + other.m31,
            m32: self.m31 * other.m12 + self.m32 * other.m22 + other.m32,
        }
    }

    pub fn transform_point(&self, point: Point) -> Point {
        Point {
            x: self.m11 * point.x + self.m21 * point.y + self.m31,
            y: self.m12 * point.x + self.m22 * point.y + self.m32,
        }
    }

    pub fn transform_rect(&self, rect: Rect) -> Rect {
        let p1 = self.transform_point(Point {
            x: rect.x,
            y: rect.y,
        });
        let p2 = self.transform_point(Point {
            x: rect.x + rect.w,
            y: rect.y,
        });
        let p3 = self.transform_point(Point {
            x: rect.x,
            y: rect.y + rect.h,
        });
        let p4 = self.transform_point(Point {
            x: rect.x + rect.w,
            y: rect.y + rect.h,
        });

        let min_x = p1.x.min(p2.x).min(p3.x).min(p4.x);
        let min_y = p1.y.min(p2.y).min(p3.y).min(p4.y);
        let max_x = p1.x.max(p2.x).max(p3.x).max(p4.x);
        let max_y = p1.y.max(p2.y).max(p3.y).max(p4.y);

        Rect {
            x: min_x,
            y: min_y,
            w: max_x - min_x,
            h: max_y - min_y,
        }
    }

    pub fn inverse(&self) -> Option<Transform> {
        let det = self.m11 * self.m22 - self.m12 * self.m21;
        if abs_f32(det) < 1e-10 {
            return None;
        }
        let inv_det = 1.0 / det;
        Some(Transform {
            m11: self.m22 * inv_det,
            m12: -self.m12 * inv_det,
            m21: -self.m21 * inv_det,
            m22: self.m11 * inv_det,
            m31: (self.m21 * self.m32 - self.m22 * self.m31) * inv_det,
            m32: (self.m12 * self.m31 - self.m11 * self.m32) * inv_det,
        })
    }

    pub fn is_identity(&self) -> bool {
        *self == Transform::identity()
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

/// Border radius for rounded rectangles.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BorderRadius {
    pub top_left: Size,
    pub top_right: Size,
    pub bottom_left: Size,
    pub bottom_right: Size,
}

impl BorderRadius {
    pub const ZERO: BorderRadius = BorderRadius {
        top_left: Size::ZERO,
        top_right: Size::ZERO,
        bottom_left: Size::ZERO,
        bottom_right: Size::ZERO,
    };

    pub fn uniform(radius: f32) -> Self {
        let size = Size {
            width: radius,
            height: radius,
        };
        Self {
            top_left: size,
            top_right: size,
            bottom_left: size,
            bottom_right: size,
        }
    }

    pub fn is_zero(&self) -> bool {
        self.top_left == Size::ZERO
            && self.top_right == Size::ZERO
            && self.bottom_left == Size::ZERO
            && self.bottom_right == Size::ZERO
    }
}

/// Side offsets (for borders, padding, margin).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SideOffsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl SideOffsets {
    pub const ZERO: SideOffsets = SideOffsets {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    pub fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
}

/// Border colors for each side.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BorderColors {
    pub top: Color,
    pub right: Color,
    pub bottom: Color,
    pub left: Color,
}

impl BorderColors {
    pub fn uniform(color: Color) -> Self {
        Self {
            top: color,
            right: color,
            bottom: color,
            left: color,
        }
    }
}

/// Border styles for each side.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BorderStyles {
    pub top: BorderStyle,
    pub right: BorderStyle,
    pub bottom: BorderStyle,
    pub left: BorderStyle,
}

impl BorderStyles {
    pub fn uniform(style: BorderStyle) -> Self {
        Self {
            top: style,
            right: style,
            bottom: style,
            left: style,
        }
    }
}

/// Border style.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BorderStyle {
    #[default]
    None,
    Solid,
    Dotted,
    Dashed,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

/// Image key for texture references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImageKey(pub u64);

impl ImageKey {
    pub const INVALID: ImageKey = ImageKey(0);
}

/// Font key for font references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontKey(pub u64);

/// Font instance key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontInstanceKey(pub u64);

/// Glyph instance for text rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphInstance {
    pub index: u32,
    pub point: Point,
}

impl GlyphInstance {
    pub fn new(index: u32, x: f32, y: f32) -> Self {
        Self {
            index,
            point: Point { x, y },
        }
    }
}

/// Rendering primitive (internal representation).
#[derive(Debug, Clone)]
pub enum Primitive {
    Rect {
        rect: Rect,
        color: Color,
    },
    RoundedRect {
        rect: Rect,
        color: Color,
        radii: BorderRadius,
    },
    Text {
        rect: Rect,
        glyphs: Vec<GlyphInstance>,
        color: Color,
        font_size: f32,
    },
    Image {
        rect: Rect,
        image_key: ImageKey,
    },
    Border {
        rect: Rect,
        widths: SideOffsets,
        colors: BorderColors,
        styles: BorderStyles,
    },
    BoxShadow {
        rect: Rect,
        offset: Point,
        color: Color,
        blur_radius: f32,
        spread_radius: f32,
        inset: bool,
    },
    LinearGradient {
        rect: Rect,
        start: Point,
        end: Point,
        stops: Vec<(f32, Color)>,
    },
    RadialGradient {
        rect: Rect,
        center: Point,
        radius: Size,
        stops: Vec<(f32, Color)>,
    },
}

impl Primitive {
    /// Get the bounding rectangle.
    pub fn bounds(&self) -> Rect {
        match self {
            Primitive::Rect { rect, .. }
            | Primitive::RoundedRect { rect, .. }
            | Primitive::Text { rect, .. }
            | Primitive::Image { rect, .. }
            | Primitive::Border { rect, .. }
            | Primitive::LinearGradient { rect, .. }
            | Primitive::RadialGradient { rect, .. } => *rect,
            Primitive::BoxShadow {
                rect,
                offset,
                blur_radius,
                spread_radius,
                inset,
                ..
            } => {
                if *inset {
                    *rect
                } else {
                    Rect {
                        x: rect.x + offset.x - blur_radius - spread_radius,
                        y: rect.y + offset.y - blur_radius - spread_radius,
                        w: rect.w + 2.0 * (blur_radius + spread_radius),
                        h: rect.h + 2.0 * (blur_radius + spread_radius),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_operations() {
        let r1 = Rect::new(0.0, 0.0, 100.0, 100.0);
        let r2 = Rect::new(50.0, 50.0, 100.0, 100.0);

        assert!(r1.intersects(&r2));

        let intersection = r1.intersect(&r2);
        assert_eq!(intersection.x, 50.0);
        assert_eq!(intersection.y, 50.0);
        assert_eq!(intersection.w, 50.0);
        assert_eq!(intersection.h, 50.0);

        let union = r1.union(&r2);
        assert_eq!(union.x, 0.0);
        assert_eq!(union.y, 0.0);
        assert_eq!(union.w, 150.0);
        assert_eq!(union.h, 150.0);
    }

    #[test]
    fn test_transform() {
        let t = Transform::translate(10.0, 20.0);
        let p = Point::new(5.0, 5.0);
        let transformed = t.transform_point(p);
        assert_eq!(transformed.x, 15.0);
        assert_eq!(transformed.y, 25.0);
    }

    #[test]
    fn test_color_blend() {
        let bg = Color::WHITE;
        let fg = Color::new(1.0, 0.0, 0.0, 0.5);
        let blended = bg.blend(&fg);
        assert!(abs_f32(blended.r - 1.0) < 0.01);
        assert!(abs_f32(blended.g - 0.5) < 0.01);
    }
}
