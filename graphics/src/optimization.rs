//! Graphics Optimization Module
//!
//! Provides damage tracking, frame pacing, and rendering optimizations.

extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

/// Rectangle for damage regions
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// Create new rectangle
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    /// Check if rectangles overlap
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width as i32
            && self.x + self.width as i32 > other.x
            && self.y < other.y + other.height as i32
            && self.y + self.height as i32 > other.y
    }

    /// Get union of two rectangles
    pub fn union(&self, other: &Rect) -> Rect {
        let min_x = self.x.min(other.x);
        let min_y = self.y.min(other.y);
        let max_x = (self.x + self.width as i32).max(other.x + other.width as i32);
        let max_y = (self.y + self.height as i32).max(other.y + other.height as i32);
        
        Rect {
            x: min_x,
            y: min_y,
            width: (max_x - min_x) as u32,
            height: (max_y - min_y) as u32,
        }
    }

    /// Calculate area
    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

/// Damage tracker for incremental rendering
pub struct DamageTracker {
    /// Damage regions
    regions: Vec<Rect>,
    /// Whether full redraw is needed
    full_damage: bool,
    /// Screen dimensions
    screen_width: u32,
    screen_height: u32,
    /// Maximum regions before full damage
    max_regions: usize,
}

impl DamageTracker {
    /// Create new damage tracker
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        Self {
            regions: Vec::new(),
            full_damage: true, // Initial full redraw
            screen_width,
            screen_height,
            max_regions: 16,
        }
    }

    /// Mark region as damaged
    pub fn add_damage(&mut self, rect: Rect) {
        if self.full_damage {
            return;
        }

        // Merge with overlapping regions
        let mut i = 0;
        let mut merged_rect = rect;
        
        while i < self.regions.len() {
            if merged_rect.intersects(&self.regions[i]) {
                merged_rect = merged_rect.union(&self.regions[i]);
                self.regions.remove(i);
            } else {
                i += 1;
            }
        }
        
        self.regions.push(merged_rect);
        
        // If too many regions, switch to full damage
        if self.regions.len() > self.max_regions {
            self.set_full_damage();
        }
    }

    /// Mark entire screen as damaged
    pub fn set_full_damage(&mut self) {
        self.full_damage = true;
        self.regions.clear();
    }

    /// Get damage regions for rendering
    pub fn get_damage(&self) -> DamageResult {
        if self.full_damage {
            DamageResult::Full(Rect::new(0, 0, self.screen_width, self.screen_height))
        } else if self.regions.is_empty() {
            DamageResult::None
        } else {
            DamageResult::Regions(self.regions.as_slice())
        }
    }

    /// Clear damage after render
    pub fn clear(&mut self) {
        self.regions.clear();
        self.full_damage = false;
    }

    /// Check if any damage exists
    pub fn has_damage(&self) -> bool {
        self.full_damage || !self.regions.is_empty()
    }

    /// Get total damaged area
    pub fn damaged_area(&self) -> u64 {
        if self.full_damage {
            self.screen_width as u64 * self.screen_height as u64
        } else {
            self.regions.iter().map(|r| r.area()).sum()
        }
    }
}

/// Result of damage query
#[derive(Debug)]
pub enum DamageResult<'a> {
    /// No damage, skip rendering
    None,
    /// Full screen redraw needed
    Full(Rect),
    /// Partial regions need redraw
    Regions(&'a [Rect]),
}

/// Frame pacing for consistent frame timing
pub struct FramePacer {
    /// Target frames per second
    target_fps: u32,
    /// Target frame time in nanoseconds
    frame_time_ns: u64,
    /// Last frame timestamp
    last_frame_ns: AtomicU64,
    /// Frame time history for averaging
    frame_times: [u64; 60],
    /// Current frame time index
    frame_index: usize,
    /// Frames rendered
    frames_rendered: AtomicU64,
    /// Frames dropped
    frames_dropped: AtomicU64,
}

impl FramePacer {
    /// Nanoseconds per second
    const NS_PER_SEC: u64 = 1_000_000_000;

    /// Create new frame pacer
    pub fn new(target_fps: u32) -> Self {
        let frame_time_ns = Self::NS_PER_SEC / target_fps as u64;
        
        Self {
            target_fps,
            frame_time_ns,
            last_frame_ns: AtomicU64::new(0),
            frame_times: [frame_time_ns; 60],
            frame_index: 0,
            frames_rendered: AtomicU64::new(0),
            frames_dropped: AtomicU64::new(0),
        }
    }

    /// Check if it's time for next frame
    pub fn should_render(&self, current_time_ns: u64) -> bool {
        let last = self.last_frame_ns.load(Ordering::Relaxed);
        current_time_ns >= last + self.frame_time_ns
    }

    /// Record frame completion
    pub fn frame_complete(&mut self, current_time_ns: u64) {
        let last = self.last_frame_ns.swap(current_time_ns, Ordering::Relaxed);
        let frame_time = current_time_ns.saturating_sub(last);
        
        // Record frame time
        self.frame_times[self.frame_index] = frame_time;
        self.frame_index = (self.frame_index + 1) % 60;
        
        self.frames_rendered.fetch_add(1, Ordering::Relaxed);
        
        // Check for dropped frames
        if frame_time > self.frame_time_ns * 2 {
            let dropped = (frame_time / self.frame_time_ns).saturating_sub(1);
            self.frames_dropped.fetch_add(dropped, Ordering::Relaxed);
        }
    }

    /// Get average frame time
    pub fn average_frame_time_ns(&self) -> u64 {
        let sum: u64 = self.frame_times.iter().sum();
        sum / 60
    }

    /// Get current FPS
    pub fn current_fps(&self) -> f32 {
        let avg = self.average_frame_time_ns();
        if avg == 0 {
            self.target_fps as f32
        } else {
            Self::NS_PER_SEC as f32 / avg as f32
        }
    }

    /// Get frame statistics
    pub fn stats(&self) -> FrameStats {
        FrameStats {
            target_fps: self.target_fps,
            current_fps: self.current_fps(),
            frames_rendered: self.frames_rendered.load(Ordering::Relaxed),
            frames_dropped: self.frames_dropped.load(Ordering::Relaxed),
            avg_frame_time_ms: self.average_frame_time_ns() as f32 / 1_000_000.0,
        }
    }
}

/// Frame statistics
#[derive(Debug, Clone)]
pub struct FrameStats {
    pub target_fps: u32,
    pub current_fps: f32,
    pub frames_rendered: u64,
    pub frames_dropped: u64,
    pub avg_frame_time_ms: f32,
}

/// Layer cache for static content
pub struct LayerCache {
    /// Cached layers
    layers: Vec<CachedLayer>,
    /// Maximum cache size in bytes
    max_size: usize,
    /// Current cache size
    current_size: usize,
    /// Cache hits
    hits: AtomicU64,
    /// Cache misses
    misses: AtomicU64,
}

/// A cached layer
struct CachedLayer {
    id: u64,
    data: Vec<u8>,
    bounds: Rect,
    last_access: u64,
}

impl LayerCache {
    /// Create new layer cache
    pub fn new(max_size_bytes: usize) -> Self {
        Self {
            layers: Vec::new(),
            max_size: max_size_bytes,
            current_size: 0,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Get cached layer
    pub fn get(&self, id: u64) -> Option<&[u8]> {
        if let Some(layer) = self.layers.iter().find(|l| l.id == id) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            Some(&layer.data)
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    /// Insert layer into cache
    pub fn insert(&mut self, id: u64, data: Vec<u8>, bounds: Rect, timestamp: u64) {
        let size = data.len();
        
        // Evict if needed
        while self.current_size + size > self.max_size && !self.layers.is_empty() {
            self.evict_oldest();
        }
        
        // Remove existing entry with same ID
        if let Some(pos) = self.layers.iter().position(|l| l.id == id) {
            self.current_size -= self.layers[pos].data.len();
            self.layers.remove(pos);
        }
        
        self.layers.push(CachedLayer {
            id,
            data,
            bounds,
            last_access: timestamp,
        });
        self.current_size += size;
    }

    /// Evict oldest entry
    fn evict_oldest(&mut self) {
        if let Some(pos) = self.layers.iter()
            .enumerate()
            .min_by_key(|(_, l)| l.last_access)
            .map(|(i, _)| i) 
        {
            self.current_size -= self.layers[pos].data.len();
            self.layers.remove(pos);
        }
    }

    /// Invalidate specific layer
    pub fn invalidate(&mut self, id: u64) {
        if let Some(pos) = self.layers.iter().position(|l| l.id == id) {
            self.current_size -= self.layers[pos].data.len();
            self.layers.remove(pos);
        }
    }

    /// Clear entire cache
    pub fn clear(&mut self) {
        self.layers.clear();
        self.current_size = 0;
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        
        CacheStats {
            entries: self.layers.len(),
            size_bytes: self.current_size,
            max_size_bytes: self.max_size,
            hits,
            misses,
            hit_rate: if total > 0 { hits as f32 / total as f32 } else { 0.0 },
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub size_bytes: usize,
    pub max_size_bytes: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f32,
}

/// Font glyph cache
pub struct GlyphCache {
    /// Cached glyphs
    glyphs: Vec<CachedGlyph>,
    /// Maximum glyphs to cache
    max_glyphs: usize,
}

struct CachedGlyph {
    codepoint: u32,
    font_id: u16,
    size_px: u16,
    bitmap: Vec<u8>,
    width: u16,
    height: u16,
}

impl GlyphCache {
    /// Create new glyph cache
    pub fn new(max_glyphs: usize) -> Self {
        Self {
            glyphs: Vec::with_capacity(max_glyphs),
            max_glyphs,
        }
    }

    /// Get cached glyph
    pub fn get(&self, codepoint: u32, font_id: u16, size_px: u16) -> Option<&[u8]> {
        self.glyphs.iter()
            .find(|g| g.codepoint == codepoint && g.font_id == font_id && g.size_px == size_px)
            .map(|g| g.bitmap.as_slice())
    }

    /// Cache a glyph
    pub fn insert(&mut self, codepoint: u32, font_id: u16, size_px: u16, 
                  bitmap: Vec<u8>, width: u16, height: u16) {
        // Evict if at capacity
        if self.glyphs.len() >= self.max_glyphs {
            self.glyphs.remove(0);
        }
        
        self.glyphs.push(CachedGlyph {
            codepoint,
            font_id,
            size_px,
            bitmap,
            width,
            height,
        });
    }

    /// Number of cached glyphs
    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_intersects() {
        let r1 = Rect::new(0, 0, 100, 100);
        let r2 = Rect::new(50, 50, 100, 100);
        let r3 = Rect::new(200, 200, 100, 100);
        
        assert!(r1.intersects(&r2));
        assert!(!r1.intersects(&r3));
    }

    #[test]
    fn test_damage_tracker() {
        let mut tracker = DamageTracker::new(1920, 1080);
        tracker.clear(); // Clear initial full damage
        
        assert!(!tracker.has_damage());
        
        tracker.add_damage(Rect::new(0, 0, 100, 100));
        assert!(tracker.has_damage());
        
        tracker.clear();
        assert!(!tracker.has_damage());
    }

    #[test]
    fn test_frame_pacer() {
        let pacer = FramePacer::new(60);
        
        assert_eq!(pacer.target_fps, 60);
        assert!(pacer.should_render(pacer.frame_time_ns + 1));
    }

    #[test]
    fn test_layer_cache() {
        let mut cache = LayerCache::new(1000);
        
        cache.insert(1, vec![0u8; 100], Rect::new(0, 0, 10, 10), 0);
        assert!(cache.get(1).is_some());
        assert!(cache.get(2).is_none());
        
        cache.invalidate(1);
        assert!(cache.get(1).is_none());
    }
}
