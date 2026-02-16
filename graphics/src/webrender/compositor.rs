//! Tile-based Compositor
//!
//! This module implements tile-based compositing for efficient GPU rendering.
//! Content is divided into tiles that can be cached and composited together.

use super::primitives::*;
use super::tile_cache::TileKey;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// Tile-based compositor.
pub struct WebRenderCompositor {
    /// Tile size in pixels.
    tile_size: u32,
    /// Active layers.
    layers: Vec<CompositeLayer>,
    /// Composite mode.
    mode: CompositeMode,
    /// Dirty regions that need recompositing.
    dirty_regions: Vec<Rect>,
    /// Current frame number.
    frame_number: u64,
}

impl WebRenderCompositor {
    /// Create a new compositor.
    pub fn new(tile_size: u32) -> Self {
        Self {
            tile_size,
            layers: Vec::new(),
            mode: CompositeMode::default(),
            dirty_regions: Vec::new(),
            frame_number: 0,
        }
    }

    /// Begin a new composite frame.
    pub fn begin_frame(&mut self) {
        self.frame_number += 1;
        self.layers.clear();
    }

    /// Add a layer to composite.
    pub fn add_layer(&mut self, layer: CompositeLayer) {
        self.layers.push(layer);
    }

    /// Mark a region as dirty.
    pub fn mark_dirty(&mut self, rect: Rect) {
        // Merge with existing dirty regions if overlapping
        let mut merged = false;
        for dirty in &mut self.dirty_regions {
            if dirty.intersects(&rect) {
                *dirty = dirty.union(&rect);
                merged = true;
                break;
            }
        }
        if !merged {
            self.dirty_regions.push(rect);
        }
    }

    /// Get tiles that need rendering.
    pub fn get_dirty_tiles(&self, viewport: Rect) -> Vec<TileKey> {
        let mut tiles = Vec::new();

        for layer in &self.layers {
            let layer_rect = layer.bounds.intersect(&viewport);
            if layer_rect.w <= 0.0 || layer_rect.h <= 0.0 {
                continue;
            }

            // Calculate tile range
            let start_x = floor_f32(layer_rect.x / self.tile_size as f32) as i32;
            let start_y = floor_f32(layer_rect.y / self.tile_size as f32) as i32;
            let end_x = ceil_f32((layer_rect.x + layer_rect.w) / self.tile_size as f32) as i32;
            let end_y = ceil_f32((layer_rect.y + layer_rect.h) / self.tile_size as f32) as i32;

            for ty in start_y..end_y {
                for tx in start_x..end_x {
                    let tile_rect = Rect {
                        x: tx as f32 * self.tile_size as f32,
                        y: ty as f32 * self.tile_size as f32,
                        w: self.tile_size as f32,
                        h: self.tile_size as f32,
                    };

                    // Check if tile is in dirty region
                    let is_dirty = self.dirty_regions.is_empty()
                        || self.dirty_regions.iter().any(|d| d.intersects(&tile_rect));

                    if is_dirty {
                        tiles.push(TileKey {
                            layer_id: layer.id,
                            x: tx,
                            y: ty,
                        });
                    }
                }
            }
        }

        tiles
    }

    /// Composite all layers into final output.
    pub fn composite(&mut self, viewport: Rect) -> CompositeResult {
        let mut result = CompositeResult {
            draw_calls: 0,
            tiles_rendered: 0,
            tiles_cached: 0,
        };

        // Sort layers by z-index
        self.layers.sort_by(|a, b| a.z_index.cmp(&b.z_index));

        // Generate composite operations
        for layer in &self.layers {
            let visible = layer.bounds.intersect(&viewport);
            if visible.w <= 0.0 || visible.h <= 0.0 {
                continue;
            }

            result.draw_calls += 1;
            result.tiles_rendered += self.count_tiles(&visible);
        }

        // Clear dirty regions
        self.dirty_regions.clear();

        result
    }

    fn count_tiles(&self, rect: &Rect) -> u32 {
        let cols = ceil_f32(rect.w / self.tile_size as f32) as u32;
        let rows = ceil_f32(rect.h / self.tile_size as f32) as u32;
        cols * rows
    }

    /// Get the composite mode.
    pub fn mode(&self) -> CompositeMode {
        self.mode
    }

    /// Set the composite mode.
    pub fn set_mode(&mut self, mode: CompositeMode) {
        self.mode = mode;
    }
}

/// Composite mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CompositeMode {
    /// Standard compositing.
    #[default]
    Normal,
    /// Low power mode (reduced quality).
    LowPower,
    /// High quality mode.
    HighQuality,
}

/// A layer to be composited.
#[derive(Debug, Clone)]
pub struct CompositeLayer {
    /// Layer ID.
    pub id: u64,
    /// Bounding rectangle.
    pub bounds: Rect,
    /// Z-index for ordering.
    pub z_index: i32,
    /// Transform applied to the layer.
    pub transform: Transform,
    /// Opacity (0.0 - 1.0).
    pub opacity: f32,
    /// Blend mode.
    pub blend_mode: LayerBlendMode,
    /// Whether this layer has transparency.
    pub has_transparency: bool,
    /// Tile sources for this layer.
    pub tiles: Vec<LayerTile>,
}

impl CompositeLayer {
    /// Create a new layer.
    pub fn new(id: u64, bounds: Rect) -> Self {
        Self {
            id,
            bounds,
            z_index: 0,
            transform: Transform::identity(),
            opacity: 1.0,
            blend_mode: LayerBlendMode::Normal,
            has_transparency: false,
            tiles: Vec::new(),
        }
    }

    /// Set z-index.
    pub fn with_z_index(mut self, z: i32) -> Self {
        self.z_index = z;
        self
    }

    /// Set transform.
    pub fn with_transform(mut self, t: Transform) -> Self {
        self.transform = t;
        self
    }

    /// Set opacity.
    pub fn with_opacity(mut self, o: f32) -> Self {
        self.opacity = o;
        self
    }
}

/// A tile within a layer.
#[derive(Debug, Clone)]
pub struct LayerTile {
    /// Tile coordinates.
    pub x: i32,
    pub y: i32,
    /// Texture ID for this tile.
    pub texture_id: u64,
    /// Whether the tile content is valid.
    pub valid: bool,
}

/// Blend mode for layers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LayerBlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Difference,
    Exclusion,
}

/// Result of compositing.
#[derive(Debug, Default)]
pub struct CompositeResult {
    pub draw_calls: u32,
    pub tiles_rendered: u32,
    pub tiles_cached: u32,
}

/// Picture cache for intermediate render results.
pub struct PictureCache {
    /// Cached pictures by key.
    pictures: BTreeMap<u64, CachedPicture>,
    /// Maximum cache size in bytes.
    max_size: usize,
    /// Current size.
    current_size: usize,
}

impl PictureCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            pictures: BTreeMap::new(),
            max_size,
            current_size: 0,
        }
    }

    /// Get a cached picture.
    pub fn get(&self, key: u64) -> Option<&CachedPicture> {
        self.pictures.get(&key)
    }

    /// Insert a picture into the cache.
    pub fn insert(&mut self, key: u64, picture: CachedPicture) {
        let size = picture.size_bytes();

        // Evict if necessary
        while self.current_size + size > self.max_size && !self.pictures.is_empty() {
            self.evict_one();
        }

        self.current_size += size;
        self.pictures.insert(key, picture);
    }

    /// Remove a picture from the cache.
    pub fn remove(&mut self, key: u64) -> Option<CachedPicture> {
        if let Some(pic) = self.pictures.remove(&key) {
            self.current_size -= pic.size_bytes();
            Some(pic)
        } else {
            None
        }
    }

    /// Invalidate pictures that intersect with a rect.
    pub fn invalidate(&mut self, rect: Rect) {
        let keys_to_remove: Vec<u64> = self
            .pictures
            .iter()
            .filter(|(_, p)| p.bounds.intersects(&rect))
            .map(|(k, _)| *k)
            .collect();

        for key in keys_to_remove {
            self.remove(key);
        }
    }

    fn evict_one(&mut self) {
        if let Some(&key) = self.pictures.keys().next() {
            self.remove(key);
        }
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.pictures.clear();
        self.current_size = 0;
    }
}

/// A cached picture (intermediate render result).
#[derive(Debug)]
pub struct CachedPicture {
    /// Texture ID.
    pub texture_id: u64,
    /// Bounds of the picture.
    pub bounds: Rect,
    /// Size in pixels.
    pub width: u32,
    pub height: u32,
    /// Frame when last used.
    pub last_used_frame: u64,
}

impl CachedPicture {
    fn size_bytes(&self) -> usize {
        (self.width * self.height * 4) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compositor_layers() {
        let mut compositor = WebRenderCompositor::new(256);
        compositor.begin_frame();

        let layer1 = CompositeLayer::new(1, Rect::new(0.0, 0.0, 800.0, 600.0)).with_z_index(0);
        let layer2 = CompositeLayer::new(2, Rect::new(100.0, 100.0, 200.0, 200.0))
            .with_z_index(1)
            .with_opacity(0.8);

        compositor.add_layer(layer1);
        compositor.add_layer(layer2);

        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let result = compositor.composite(viewport);

        assert_eq!(result.draw_calls, 2);
    }

    #[test]
    fn test_dirty_tiles() {
        let mut compositor = WebRenderCompositor::new(256);
        compositor.begin_frame();

        let layer = CompositeLayer::new(1, Rect::new(0.0, 0.0, 800.0, 600.0));
        compositor.add_layer(layer);
        compositor.mark_dirty(Rect::new(0.0, 0.0, 100.0, 100.0));

        let tiles = compositor.get_dirty_tiles(Rect::new(0.0, 0.0, 800.0, 600.0));
        assert!(!tiles.is_empty());
    }
}
