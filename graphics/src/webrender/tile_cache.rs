//! Tile Cache
//!
//! This module implements a tile-based cache for rendered content.
//! Tiles that haven't changed can be reused between frames.

use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use super::primitives::{Rect, floor_f32, ceil_f32};
use super::Frame;

/// Key for identifying a tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileKey {
    /// Layer ID.
    pub layer_id: u64,
    /// X coordinate in tile units.
    pub x: i32,
    /// Y coordinate in tile units.
    pub y: i32,
}

impl TileKey {
    pub fn new(layer_id: u64, x: i32, y: i32) -> Self {
        Self { layer_id, x, y }
    }
}

/// Tile cache for rendered content.
pub struct TileCache {
    /// Tile size in pixels.
    tile_size: u32,
    /// Cached tiles.
    tiles: BTreeMap<TileKey, CachedTile>,
    /// Maximum number of tiles.
    max_tiles: usize,
    /// LRU order for eviction.
    access_order: Vec<TileKey>,
    /// Invalidated regions.
    invalidated: Vec<Rect>,
    /// Current frame number.
    current_frame: u64,
}

impl TileCache {
    /// Create a new tile cache.
    pub fn new(tile_size: u32) -> Self {
        Self {
            tile_size,
            tiles: BTreeMap::new(),
            max_tiles: 1024,
            access_order: Vec::new(),
            invalidated: Vec::new(),
            current_frame: 0,
        }
    }
    
    /// Update the cache for a new frame.
    pub fn update(&mut self, frame: &Frame) {
        self.current_frame = frame.frame_number;
        
        // Process invalidated regions - collect first to avoid borrow conflict
        let regions: Vec<Rect> = self.invalidated.drain(..).collect();
        for rect in regions {
            self.invalidate_tiles_in_rect(rect);
        }
    }
    
    /// Get a cached tile.
    pub fn get(&mut self, key: TileKey) -> Option<&CachedTile> {
        if self.tiles.contains_key(&key) {
            // Update access order for LRU
            self.touch(key);
            self.tiles.get(&key)
        } else {
            None
        }
    }
    
    /// Insert a tile into the cache.
    pub fn insert(&mut self, key: TileKey, tile: CachedTile) {
        // Evict if necessary
        while self.tiles.len() >= self.max_tiles {
            self.evict_lru();
        }
        
        self.tiles.insert(key, tile);
        self.access_order.push(key);
    }
    
    /// Invalidate a region.
    pub fn invalidate(&mut self, rect: Rect) {
        self.invalidated.push(rect);
    }
    
    /// Invalidate tiles that intersect with a rectangle.
    fn invalidate_tiles_in_rect(&mut self, rect: Rect) {
        let start_x = floor_f32(rect.x / self.tile_size as f32) as i32;
        let start_y = floor_f32(rect.y / self.tile_size as f32) as i32;
        let end_x = ceil_f32((rect.x + rect.w) / self.tile_size as f32) as i32;
        let end_y = ceil_f32((rect.y + rect.h) / self.tile_size as f32) as i32;
        
        let keys_to_remove: Vec<TileKey> = self.tiles.keys()
            .filter(|k| k.x >= start_x && k.x < end_x && k.y >= start_y && k.y < end_y)
            .copied()
            .collect();
        
        for key in keys_to_remove {
            self.tiles.remove(&key);
            if let Some(pos) = self.access_order.iter().position(|k| *k == key) {
                self.access_order.remove(pos);
            }
        }
    }
    
    fn touch(&mut self, key: TileKey) {
        if let Some(pos) = self.access_order.iter().position(|k| *k == key) {
            self.access_order.remove(pos);
        }
        self.access_order.push(key);
    }
    
    fn evict_lru(&mut self) {
        if let Some(key) = self.access_order.first().copied() {
            self.access_order.remove(0);
            self.tiles.remove(&key);
        }
    }
    
    /// Get the number of cached tiles.
    pub fn len(&self) -> usize {
        self.tiles.len()
    }
    
    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }
    
    /// Get memory usage in bytes.
    pub fn memory_usage(&self) -> usize {
        let tile_bytes = (self.tile_size * self.tile_size * 4) as usize;
        self.tiles.len() * tile_bytes
    }
    
    /// Clear the cache.
    pub fn clear(&mut self) {
        self.tiles.clear();
        self.access_order.clear();
        self.invalidated.clear();
    }
    
    /// Get the tile bounds.
    pub fn tile_bounds(&self, key: TileKey) -> Rect {
        Rect {
            x: key.x as f32 * self.tile_size as f32,
            y: key.y as f32 * self.tile_size as f32,
            w: self.tile_size as f32,
            h: self.tile_size as f32,
        }
    }
    
    /// Get tiles that intersect with a viewport.
    pub fn get_visible_tiles(&self, viewport: Rect) -> Vec<TileKey> {
        let start_x = floor_f32(viewport.x / self.tile_size as f32) as i32;
        let start_y = floor_f32(viewport.y / self.tile_size as f32) as i32;
        let end_x = ceil_f32((viewport.x + viewport.w) / self.tile_size as f32) as i32;
        let end_y = ceil_f32((viewport.y + viewport.h) / self.tile_size as f32) as i32;
        
        self.tiles.keys()
            .filter(|k| k.x >= start_x && k.x < end_x && k.y >= start_y && k.y < end_y)
            .copied()
            .collect()
    }
    
    /// Check if a tile is cached.
    pub fn contains(&self, key: TileKey) -> bool {
        self.tiles.contains_key(&key)
    }
    
    /// Get tile size.
    pub fn tile_size(&self) -> u32 {
        self.tile_size
    }
}

/// A cached tile.
#[derive(Debug)]
pub struct CachedTile {
    /// Texture ID on GPU.
    pub texture_id: u64,
    /// Frame when this tile was created.
    pub created_frame: u64,
    /// Frame when last accessed.
    pub last_accessed_frame: u64,
    /// Content hash for invalidation detection.
    pub content_hash: u64,
    /// Whether the tile has transparency.
    pub has_transparency: bool,
    /// Tile state.
    pub state: TileState,
}

impl CachedTile {
    /// Create a new cached tile.
    pub fn new(texture_id: u64, frame: u64, content_hash: u64) -> Self {
        Self {
            texture_id,
            created_frame: frame,
            last_accessed_frame: frame,
            content_hash,
            has_transparency: false,
            state: TileState::Valid,
        }
    }
    
    /// Update the access time.
    pub fn touch(&mut self, frame: u64) {
        self.last_accessed_frame = frame;
    }
}

/// Tile state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileState {
    /// Tile is valid and can be used.
    Valid,
    /// Tile is dirty and needs re-rendering.
    Dirty,
    /// Tile is being rendered.
    Rendering,
    /// Tile failed to render.
    Failed,
}

/// Tile descriptor for rendering.
#[derive(Debug, Clone)]
pub struct TileDescriptor {
    /// Tile key.
    pub key: TileKey,
    /// Tile bounds in screen coordinates.
    pub bounds: Rect,
    /// Primitives that affect this tile.
    pub primitive_indices: Vec<usize>,
    /// Content hash.
    pub content_hash: u64,
}

impl TileDescriptor {
    pub fn new(key: TileKey, bounds: Rect) -> Self {
        Self {
            key,
            bounds,
            primitive_indices: Vec::new(),
            content_hash: 0,
        }
    }
    
    /// Add a primitive that affects this tile.
    pub fn add_primitive(&mut self, index: usize) {
        self.primitive_indices.push(index);
    }
    
    /// Calculate content hash.
    pub fn calculate_hash(&mut self) {
        // Simple hash combining primitive indices
        let mut hash: u64 = 0xcbf29ce484222325; // FNV-1a offset
        for &idx in &self.primitive_indices {
            hash ^= idx as u64;
            hash = hash.wrapping_mul(0x100000001b3); // FNV-1a prime
        }
        self.content_hash = hash;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tile_cache() {
        let mut cache = TileCache::new(256);
        
        let key = TileKey::new(1, 0, 0);
        let tile = CachedTile::new(123, 1, 0);
        cache.insert(key, tile);
        
        assert!(cache.contains(key));
        assert_eq!(cache.len(), 1);
        
        let cached = cache.get(key);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().texture_id, 123);
    }
    
    #[test]
    fn test_tile_invalidation() {
        let mut cache = TileCache::new(256);
        
        // Insert tiles in a 2x2 grid
        for y in 0..2 {
            for x in 0..2 {
                let key = TileKey::new(1, x, y);
                cache.insert(key, CachedTile::new(x as u64 * 10 + y as u64, 1, 0));
            }
        }
        
        assert_eq!(cache.len(), 4);
        
        // Invalidate a region covering some tiles
        cache.invalidate(Rect::new(0.0, 0.0, 257.0, 257.0));
        cache.update(&Frame::new(800, 600, 2));
        
        // Some tiles should be removed
        assert!(cache.len() < 4);
    }
    
    #[test]
    fn test_tile_bounds() {
        let cache = TileCache::new(256);
        let key = TileKey::new(1, 2, 3);
        let bounds = cache.tile_bounds(key);
        
        assert_eq!(bounds.x, 512.0);
        assert_eq!(bounds.y, 768.0);
        assert_eq!(bounds.w, 256.0);
        assert_eq!(bounds.h, 256.0);
    }
}
