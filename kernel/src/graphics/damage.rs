//! Damage Tracking System
//!
//! Tracks which regions of the screen need to be redrawn.
//! This is critical for high-performance rendering - we only
//! redraw what has actually changed.

use alloc::vec::Vec;

/// A rectangular region that needs to be redrawn
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl DamageRect {
    /// Create a new damage rectangle
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create from bounds
    pub fn from_bounds(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        let x = x1.min(x2);
        let y = y1.min(y2);
        let width = (x1.max(x2) - x) as u32;
        let height = (y1.max(y2) - y) as u32;
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Get the right edge
    #[inline]
    pub fn right(&self) -> i32 {
        self.x + self.width as i32
    }

    /// Get the bottom edge
    #[inline]
    pub fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }

    /// Check if this rectangle intersects with another
    pub fn intersects(&self, other: &DamageRect) -> bool {
        !(self.right() <= other.x
            || other.right() <= self.x
            || self.bottom() <= other.y
            || other.bottom() <= self.y)
    }

    /// Get intersection with another rectangle
    pub fn intersection(&self, other: &DamageRect) -> Option<DamageRect> {
        if !self.intersects(other) {
            return None;
        }

        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());

        if right > x && bottom > y {
            Some(DamageRect {
                x,
                y,
                width: (right - x) as u32,
                height: (bottom - y) as u32,
            })
        } else {
            None
        }
    }

    /// Merge two rectangles into one bounding rectangle
    pub fn union(&self, other: &DamageRect) -> DamageRect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());

        DamageRect {
            x,
            y,
            width: (right - x) as u32,
            height: (bottom - y) as u32,
        }
    }

    /// Calculate area of the rectangle
    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// Check if rectangle contains a point
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }

    /// Check if this rectangle contains another
    pub fn contains_rect(&self, other: &DamageRect) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.right() >= other.right()
            && self.bottom() >= other.bottom()
    }

    /// Clip rectangle to bounds
    pub fn clip(&self, bounds: &DamageRect) -> Option<DamageRect> {
        self.intersection(bounds)
    }

    /// Expand rectangle by a margin
    pub fn expand(&self, margin: i32) -> DamageRect {
        DamageRect {
            x: self.x - margin,
            y: self.y - margin,
            width: self.width + 2 * margin as u32,
            height: self.height + 2 * margin as u32,
        }
    }
}

/// Damage tracking for efficient partial updates
pub struct DamageTracker {
    /// List of damaged rectangles
    rects: Vec<DamageRect>,
    /// Screen bounds for clipping
    bounds: DamageRect,
    /// Maximum number of damage rects before full redraw
    max_rects: usize,
    /// Whether full screen is damaged
    full_damage: bool,
    /// Statistics
    pub stats: DamageStats,
}

/// Statistics about damage tracking
#[derive(Debug, Clone, Default)]
pub struct DamageStats {
    /// Total damage rects added
    pub total_rects: u64,
    /// Damage rects merged
    pub merged_rects: u64,
    /// Full redraws triggered
    pub full_redraws: u64,
    /// Pixels in last damage set
    pub last_damage_pixels: u64,
    /// Total screen pixels
    pub screen_pixels: u64,
}

impl DamageTracker {
    /// Create a new damage tracker for given screen size
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            rects: Vec::with_capacity(32),
            bounds: DamageRect::new(0, 0, width, height),
            max_rects: 32,
            full_damage: true, // Start with full damage for initial draw
            stats: DamageStats {
                screen_pixels: width as u64 * height as u64,
                ..Default::default()
            },
        }
    }

    /// Add a damaged region
    pub fn add_damage(&mut self, rect: DamageRect) {
        // Clip to screen bounds
        let Some(clipped) = rect.clip(&self.bounds) else {
            return;
        };

        if clipped.width == 0 || clipped.height == 0 {
            return;
        }

        self.stats.total_rects += 1;

        // If already full damage, no need to track more
        if self.full_damage {
            return;
        }

        // Check if new rect is contained in an existing one
        for existing in &self.rects {
            if existing.contains_rect(&clipped) {
                return;
            }
        }

        // Try to merge with an overlapping rect
        let mut merged = false;
        for existing in &mut self.rects {
            if existing.intersects(&clipped) {
                *existing = existing.union(&clipped);
                self.stats.merged_rects += 1;
                merged = true;
                break;
            }
        }

        if !merged {
            if self.rects.len() >= self.max_rects {
                // Too many rects, switch to full redraw
                self.mark_full_damage();
            } else {
                self.rects.push(clipped);
            }
        }

        // Consolidate overlapping rects
        self.consolidate();
    }

    /// Add damage for a surface that moved
    pub fn add_surface_damage(
        &mut self,
        old_x: i32,
        old_y: i32,
        new_x: i32,
        new_y: i32,
        width: u32,
        height: u32,
    ) {
        // Damage old position
        self.add_damage(DamageRect::new(old_x, old_y, width, height));
        // Damage new position
        self.add_damage(DamageRect::new(new_x, new_y, width, height));
    }

    /// Mark entire screen as damaged
    pub fn mark_full_damage(&mut self) {
        self.full_damage = true;
        self.rects.clear();
        self.stats.full_redraws += 1;
    }

    /// Clear all damage (after rendering)
    pub fn clear(&mut self) {
        // Update stats
        self.stats.last_damage_pixels = self.total_damage_area();

        self.rects.clear();
        self.full_damage = false;
    }

    /// Check if there's any damage to process
    pub fn has_damage(&self) -> bool {
        self.full_damage || !self.rects.is_empty()
    }

    /// Check if full screen redraw is needed
    pub fn is_full_damage(&self) -> bool {
        self.full_damage
    }

    /// Get iterator over damaged rectangles
    /// Returns the full bounds if full damage, otherwise returns the individual rects
    pub fn damage_rects_full(&self) -> Option<DamageRect> {
        if self.full_damage {
            Some(self.bounds)
        } else {
            None
        }
    }

    /// Get damage rectangles as a slice
    pub fn get_rects(&self) -> &[DamageRect] {
        if self.full_damage {
            // Caller should check is_full_damage() first
            &[]
        } else {
            &self.rects
        }
    }

    /// Get screen bounds
    pub fn screen_bounds(&self) -> DamageRect {
        self.bounds
    }

    /// Calculate total damaged area
    pub fn total_damage_area(&self) -> u64 {
        if self.full_damage {
            self.bounds.area()
        } else {
            self.rects.iter().map(|r| r.area()).sum()
        }
    }

    /// Calculate damage ratio (0.0 - 1.0)
    pub fn damage_ratio(&self) -> f32 {
        if self.stats.screen_pixels == 0 {
            return 1.0;
        }
        self.total_damage_area() as f32 / self.stats.screen_pixels as f32
    }

    /// Consolidate overlapping damage rectangles
    fn consolidate(&mut self) {
        if self.rects.len() < 2 {
            return;
        }

        // Simple O(n²) consolidation - fine for small rect counts
        let mut i = 0;
        while i < self.rects.len() {
            let mut j = i + 1;
            while j < self.rects.len() {
                if self.rects[i].intersects(&self.rects[j]) {
                    // Merge rects
                    self.rects[i] = self.rects[i].union(&self.rects[j]);
                    self.rects.remove(j);
                    self.stats.merged_rects += 1;
                    // Check against remaining rects again
                    continue;
                }
                j += 1;
            }
            i += 1;
        }

        // If merged rects cover most of the screen, just do full redraw
        if self.damage_ratio() > 0.5 {
            self.mark_full_damage();
        }
    }

    /// Resize the screen bounds
    pub fn resize(&mut self, width: u32, height: u32) {
        self.bounds = DamageRect::new(0, 0, width, height);
        self.stats.screen_pixels = width as u64 * height as u64;
        self.mark_full_damage();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_intersection() {
        let r1 = DamageRect::new(0, 0, 100, 100);
        let r2 = DamageRect::new(50, 50, 100, 100);

        assert!(r1.intersects(&r2));

        let intersection = r1.intersection(&r2).unwrap();
        assert_eq!(intersection.x, 50);
        assert_eq!(intersection.y, 50);
        assert_eq!(intersection.width, 50);
        assert_eq!(intersection.height, 50);
    }

    #[test]
    fn test_rect_union() {
        let r1 = DamageRect::new(0, 0, 50, 50);
        let r2 = DamageRect::new(100, 100, 50, 50);

        let union = r1.union(&r2);
        assert_eq!(union.x, 0);
        assert_eq!(union.y, 0);
        assert_eq!(union.width, 150);
        assert_eq!(union.height, 150);
    }
}
