//! Primitive Batching
//!
//! This module handles batching of primitives for efficient GPU rendering.
//! Primitives are grouped by type and rendered together to minimize state changes.

use super::primitives::Primitive;
use alloc::vec::Vec;

/// Key for identifying batch types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BatchKey {
    /// Solid rectangles.
    Rects,
    /// Rounded rectangles.
    RoundedRects,
    /// Text glyphs.
    Text,
    /// Images.
    Images,
    /// Borders.
    Borders,
    /// Box shadows.
    Shadows,
    /// Gradients.
    Gradients,
    /// Other primitives.
    Other,
}

/// A batch of primitives for efficient rendering.
#[derive(Debug)]
pub struct PrimitiveBatch {
    /// Batch key.
    pub key: BatchKey,
    /// Primitives in this batch.
    pub primitives: Vec<Primitive>,
    /// Whether the batch is opaque (can skip blending).
    pub opaque: bool,
    /// Z-index range.
    pub z_range: (i32, i32),
}

impl PrimitiveBatch {
    /// Create a new empty batch.
    pub fn new(key: BatchKey) -> Self {
        Self {
            key,
            primitives: Vec::new(),
            opaque: true,
            z_range: (0, 0),
        }
    }

    /// Create a batch with a single primitive.
    pub fn single(primitive: Primitive) -> Self {
        let key = Self::key_for_primitive(&primitive);
        let mut batch = Self::new(key);
        batch.add(primitive);
        batch
    }

    /// Create a batch from a list of primitives.
    pub fn from_primitives(key: BatchKey, primitives: Vec<Primitive>) -> Self {
        let opaque = primitives.iter().all(|p| Self::is_opaque(p));
        Self {
            key,
            primitives,
            opaque,
            z_range: (0, 0),
        }
    }

    /// Add a primitive to the batch.
    pub fn add(&mut self, primitive: Primitive) {
        if !Self::is_opaque(&primitive) {
            self.opaque = false;
        }
        self.primitives.push(primitive);
    }

    /// Get the batch key for a primitive.
    fn key_for_primitive(primitive: &Primitive) -> BatchKey {
        match primitive {
            Primitive::Rect { .. } => BatchKey::Rects,
            Primitive::RoundedRect { .. } => BatchKey::RoundedRects,
            Primitive::Text { .. } => BatchKey::Text,
            Primitive::Image { .. } => BatchKey::Images,
            Primitive::Border { .. } => BatchKey::Borders,
            Primitive::BoxShadow { .. } => BatchKey::Shadows,
            Primitive::LinearGradient { .. } | Primitive::RadialGradient { .. } => {
                BatchKey::Gradients
            }
        }
    }

    /// Check if a primitive is opaque.
    fn is_opaque(primitive: &Primitive) -> bool {
        match primitive {
            Primitive::Rect { color, .. } => color.a >= 1.0,
            Primitive::RoundedRect { color, .. } => color.a >= 1.0,
            Primitive::Text { color, .. } => color.a >= 1.0,
            Primitive::Image { .. } => false, // Images may have transparency
            Primitive::Border { colors, .. } => {
                colors.top.a >= 1.0
                    && colors.right.a >= 1.0
                    && colors.bottom.a >= 1.0
                    && colors.left.a >= 1.0
            }
            Primitive::BoxShadow { color, .. } => color.a >= 1.0,
            Primitive::LinearGradient { .. } => false, // Gradients may have transparency
            Primitive::RadialGradient { .. } => false,
        }
    }

    /// Check if the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.primitives.is_empty()
    }

    /// Get the number of primitives.
    pub fn len(&self) -> usize {
        self.primitives.len()
    }

    /// Get estimated vertex count for this batch.
    pub fn vertex_count(&self) -> usize {
        self.primitives
            .iter()
            .map(|p| Self::primitive_vertex_count(p))
            .sum()
    }

    fn primitive_vertex_count(primitive: &Primitive) -> usize {
        match primitive {
            Primitive::Rect { .. } => 6,                        // 2 triangles
            Primitive::RoundedRect { .. } => 6,                 // Shader-based
            Primitive::Text { glyphs, .. } => glyphs.len() * 6, // 2 triangles per glyph
            Primitive::Image { .. } => 6,
            Primitive::Border { .. } => 24, // 6 vertices * 4 sides
            Primitive::BoxShadow { .. } => 6,
            Primitive::LinearGradient { .. } => 6,
            Primitive::RadialGradient { .. } => 6,
        }
    }
}

/// Batch builder for constructing optimal batches.
pub struct BatchBuilder {
    /// Current batches being built.
    batches: Vec<PrimitiveBatch>,
    /// Maximum primitives per batch.
    max_batch_size: usize,
    /// Maximum vertices per batch.
    max_vertices: usize,
}

impl BatchBuilder {
    /// Create a new batch builder.
    pub fn new() -> Self {
        Self {
            batches: Vec::new(),
            max_batch_size: 1000,
            max_vertices: 65535,
        }
    }

    /// Add a primitive.
    pub fn add(&mut self, primitive: Primitive) {
        let key = PrimitiveBatch::key_for_primitive(&primitive);
        let prim_vertices = PrimitiveBatch::primitive_vertex_count(&primitive);

        // Find an existing compatible batch
        let batch_idx = self.batches.iter().position(|b| {
            b.key == key
                && b.len() < self.max_batch_size
                && b.vertex_count() + prim_vertices <= self.max_vertices
        });

        if let Some(idx) = batch_idx {
            self.batches[idx].add(primitive);
        } else {
            // Create a new batch
            let mut batch = PrimitiveBatch::new(key);
            batch.add(primitive);
            self.batches.push(batch);
        }
    }

    /// Finish building and return the batches.
    pub fn finish(self) -> Vec<PrimitiveBatch> {
        self.batches
    }

    /// Sort batches for optimal rendering order.
    pub fn optimize(&mut self) {
        // Sort opaque batches first (front-to-back), then transparent (back-to-front)
        self.batches.sort_by(|a, b| match (a.opaque, b.opaque) {
            (true, false) => core::cmp::Ordering::Less,
            (false, true) => core::cmp::Ordering::Greater,
            _ => a.key.cmp(&b.key),
        });
    }
}

impl Default for BatchBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialOrd for BatchKey {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BatchKey {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        fn key_order(key: &BatchKey) -> u8 {
            match key {
                BatchKey::Rects => 0,
                BatchKey::RoundedRects => 1,
                BatchKey::Images => 2,
                BatchKey::Borders => 3,
                BatchKey::Shadows => 4,
                BatchKey::Gradients => 5,
                BatchKey::Text => 6,
                BatchKey::Other => 7,
            }
        }
        key_order(self).cmp(&key_order(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webrender::primitives::{Color, Rect};

    #[test]
    fn test_batch_creation() {
        let mut batch = PrimitiveBatch::new(BatchKey::Rects);

        batch.add(Primitive::Rect {
            rect: Rect::new(0.0, 0.0, 100.0, 100.0),
            color: Color::RED,
        });
        batch.add(Primitive::Rect {
            rect: Rect::new(50.0, 50.0, 100.0, 100.0),
            color: Color::BLUE,
        });

        assert_eq!(batch.len(), 2);
        assert!(batch.opaque);
    }

    #[test]
    fn test_batch_builder() {
        let mut builder = BatchBuilder::new();

        // Add multiple rects
        for i in 0..10 {
            builder.add(Primitive::Rect {
                rect: Rect::new(i as f32 * 10.0, 0.0, 10.0, 10.0),
                color: Color::WHITE,
            });
        }

        // Add some text
        builder.add(Primitive::Text {
            rect: Rect::new(0.0, 100.0, 100.0, 20.0),
            glyphs: Vec::new(),
            color: Color::BLACK,
            font_size: 14.0,
        });

        let batches = builder.finish();

        // Should have at least 2 batches (rects and text)
        assert!(batches.len() >= 2);
    }

    #[test]
    fn test_transparency_detection() {
        let opaque = Primitive::Rect {
            rect: Rect::ZERO,
            color: Color::new(1.0, 0.0, 0.0, 1.0),
        };
        let transparent = Primitive::Rect {
            rect: Rect::ZERO,
            color: Color::new(1.0, 0.0, 0.0, 0.5),
        };

        let batch1 = PrimitiveBatch::single(opaque);
        let batch2 = PrimitiveBatch::single(transparent);

        assert!(batch1.opaque);
        assert!(!batch2.opaque);
    }
}
