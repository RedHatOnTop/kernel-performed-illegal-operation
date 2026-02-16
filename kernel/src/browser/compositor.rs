//! Compositor Interface
//!
//! This module provides the compositor layer management for browser rendering.
//! It coordinates with the GPU module to manage rendering layers.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use spin::{Mutex, RwLock};

use super::coordinator::TabId;
use super::gpu::GpuBufferHandle;

/// Layer identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerId(pub u32);

/// Layer blend mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BlendMode {
    /// Normal alpha blending.
    Normal = 0,
    /// Additive blending.
    Add = 1,
    /// Multiply blending.
    Multiply = 2,
    /// Screen blending.
    Screen = 3,
}

/// Layer transform.
#[derive(Debug, Clone, Copy)]
pub struct Transform2D {
    /// Scale X.
    pub scale_x: f32,
    /// Scale Y.
    pub scale_y: f32,
    /// Translation X.
    pub translate_x: f32,
    /// Translation Y.
    pub translate_y: f32,
    /// Rotation in radians.
    pub rotation: f32,
}

impl Default for Transform2D {
    fn default() -> Self {
        Transform2D {
            scale_x: 1.0,
            scale_y: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
            rotation: 0.0,
        }
    }
}

/// Rectangle.
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    /// X position.
    pub x: i32,
    /// Y position.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

impl Rect {
    /// Create new rect.
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Check if contains point.
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x
            && px < self.x + self.width as i32
            && py >= self.y
            && py < self.y + self.height as i32
    }

    /// Intersect with another rect.
    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = (self.x + self.width as i32).min(other.x + other.width as i32);
        let bottom = (self.y + self.height as i32).min(other.y + other.height as i32);

        if right > x && bottom > y {
            Some(Rect::new(x, y, (right - x) as u32, (bottom - y) as u32))
        } else {
            None
        }
    }
}

/// RGBA color.
#[derive(Debug, Clone, Copy, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Create from components.
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }

    /// Transparent.
    pub const TRANSPARENT: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    /// White.
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    /// Black.
    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
}

/// Compositor layer.
#[derive(Debug)]
pub struct Layer {
    /// Layer ID.
    pub id: LayerId,
    /// Owning tab.
    pub owner: TabId,
    /// Layer bounds.
    pub bounds: Rect,
    /// Content buffer.
    pub buffer: Option<GpuBufferHandle>,
    /// Buffer offset (for atlasing).
    pub buffer_offset: (u32, u32),
    /// Buffer stride.
    pub buffer_stride: u32,
    /// Layer transform.
    pub transform: Transform2D,
    /// Blend mode.
    pub blend_mode: BlendMode,
    /// Opacity (0.0 - 1.0).
    pub opacity: f32,
    /// Z-order.
    pub z_index: i32,
    /// Is layer visible.
    pub visible: bool,
    /// Needs repaint.
    pub dirty: bool,
    /// Background color.
    pub background: Color,
}

impl Layer {
    /// Create new layer.
    pub fn new(id: LayerId, owner: TabId, bounds: Rect) -> Self {
        Layer {
            id,
            owner,
            bounds,
            buffer: None,
            buffer_offset: (0, 0),
            buffer_stride: 0,
            transform: Transform2D::default(),
            blend_mode: BlendMode::Normal,
            opacity: 1.0,
            z_index: 0,
            visible: true,
            dirty: true,
            background: Color::TRANSPARENT,
        }
    }

    /// Set buffer.
    pub fn set_buffer(&mut self, buffer: GpuBufferHandle, stride: u32) {
        self.buffer = Some(buffer);
        self.buffer_stride = stride;
        self.dirty = true;
    }

    /// Mark dirty.
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }
}

/// Compositor manages layers for rendering.
pub struct Compositor {
    /// All layers.
    layers: BTreeMap<LayerId, Arc<Mutex<Layer>>>,
    /// Per-tab layers.
    tab_layers: BTreeMap<TabId, Vec<LayerId>>,
    /// Layer render order.
    render_order: Vec<LayerId>,
    /// Next layer ID.
    next_layer_id: AtomicU32,
    /// Frame number.
    frame_number: AtomicU64,
    /// Screen width.
    screen_width: u32,
    /// Screen height.
    screen_height: u32,
    /// Needs recomposite.
    needs_composite: bool,
}

impl Compositor {
    /// Create new compositor.
    pub fn new(width: u32, height: u32) -> Self {
        Compositor {
            layers: BTreeMap::new(),
            tab_layers: BTreeMap::new(),
            render_order: Vec::new(),
            next_layer_id: AtomicU32::new(1),
            frame_number: AtomicU64::new(0),
            screen_width: width,
            screen_height: height,
            needs_composite: true,
        }
    }

    /// Create a layer.
    pub fn create_layer(&mut self, owner: TabId, bounds: Rect) -> LayerId {
        let id = LayerId(self.next_layer_id.fetch_add(1, Ordering::Relaxed));

        let layer = Layer::new(id, owner, bounds);
        self.layers.insert(id, Arc::new(Mutex::new(layer)));

        self.tab_layers.entry(owner).or_default().push(id);
        self.render_order.push(id);
        self.needs_composite = true;

        crate::serial_println!(
            "[Compositor] Created layer {:?} for tab {} at ({}, {}) {}x{}",
            id,
            owner.0,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height
        );

        id
    }

    /// Destroy a layer.
    pub fn destroy_layer(&mut self, id: LayerId) -> bool {
        if let Some(layer) = self.layers.remove(&id) {
            let owner = layer.lock().owner;

            if let Some(layers) = self.tab_layers.get_mut(&owner) {
                layers.retain(|l| *l != id);
            }

            self.render_order.retain(|l| *l != id);
            self.needs_composite = true;

            crate::serial_println!("[Compositor] Destroyed layer {:?}", id);
            true
        } else {
            false
        }
    }

    /// Get layer.
    pub fn get_layer(&self, id: LayerId) -> Option<Arc<Mutex<Layer>>> {
        self.layers.get(&id).cloned()
    }

    /// Update layer.
    pub fn update_layer<F>(&self, id: LayerId, f: F) -> bool
    where
        F: FnOnce(&mut Layer),
    {
        if let Some(layer) = self.layers.get(&id) {
            f(&mut layer.lock());
            true
        } else {
            false
        }
    }

    /// Set layer z-index and update render order.
    pub fn set_layer_z_index(&mut self, id: LayerId, z_index: i32) {
        if let Some(layer) = self.layers.get(&id) {
            layer.lock().z_index = z_index;
            self.sort_layers();
        }
    }

    /// Sort layers by z-index.
    fn sort_layers(&mut self) {
        self.render_order.sort_by(|a, b| {
            let za = self.layers.get(a).map(|l| l.lock().z_index).unwrap_or(0);
            let zb = self.layers.get(b).map(|l| l.lock().z_index).unwrap_or(0);
            za.cmp(&zb)
        });
        self.needs_composite = true;
    }

    /// Destroy all layers for a tab.
    pub fn destroy_tab_layers(&mut self, tab: TabId) {
        if let Some(layers) = self.tab_layers.remove(&tab) {
            for id in layers {
                self.layers.remove(&id);
                self.render_order.retain(|l| *l != id);
            }
            self.needs_composite = true;
        }
    }

    /// Get layers to render.
    pub fn render_list(&self) -> Vec<LayerId> {
        self.render_order
            .iter()
            .filter(|id| {
                self.layers
                    .get(id)
                    .map(|l| {
                        let layer = l.lock();
                        layer.visible && layer.opacity > 0.0
                    })
                    .unwrap_or(false)
            })
            .copied()
            .collect()
    }

    /// Begin frame.
    pub fn begin_frame(&mut self) -> u64 {
        self.frame_number.fetch_add(1, Ordering::Relaxed)
    }

    /// End frame (composite).
    pub fn end_frame(&mut self) {
        // Mark all layers as clean after composite
        for layer in self.layers.values() {
            layer.lock().dirty = false;
        }
        self.needs_composite = false;
    }

    /// Check if composite needed.
    pub fn needs_composite(&self) -> bool {
        self.needs_composite || self.layers.values().any(|l| l.lock().dirty)
    }

    /// Get screen size.
    pub fn screen_size(&self) -> (u32, u32) {
        (self.screen_width, self.screen_height)
    }

    /// Resize screen.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
        self.needs_composite = true;
    }

    /// Hit test at point.
    pub fn hit_test(&self, x: i32, y: i32) -> Option<(LayerId, TabId)> {
        // Check in reverse render order (front to back)
        for id in self.render_order.iter().rev() {
            if let Some(layer) = self.layers.get(id) {
                let layer = layer.lock();
                if layer.visible && layer.bounds.contains(x, y) {
                    return Some((*id, layer.owner));
                }
            }
        }
        None
    }
}

/// Global compositor.
static COMPOSITOR: RwLock<Option<Compositor>> = RwLock::new(None);

/// Initialize compositor.
pub fn init(width: u32, height: u32) {
    let mut comp = COMPOSITOR.write();
    *comp = Some(Compositor::new(width, height));
    crate::serial_println!("[Compositor] Initialized: {}x{}", width, height);
}

/// Create layer.
pub fn create_layer(owner: TabId, bounds: Rect) -> Option<LayerId> {
    Some(COMPOSITOR.write().as_mut()?.create_layer(owner, bounds))
}

/// Destroy layer.
pub fn destroy_layer(id: LayerId) -> bool {
    COMPOSITOR
        .write()
        .as_mut()
        .map(|c| c.destroy_layer(id))
        .unwrap_or(false)
}

/// Get layer.
pub fn get_layer(id: LayerId) -> Option<Arc<Mutex<Layer>>> {
    COMPOSITOR.read().as_ref()?.get_layer(id)
}

/// Update layer.
pub fn update_layer<F>(id: LayerId, f: F) -> bool
where
    F: FnOnce(&mut Layer),
{
    COMPOSITOR
        .read()
        .as_ref()
        .map(|c| c.update_layer(id, f))
        .unwrap_or(false)
}

/// Begin frame.
pub fn begin_frame() -> Option<u64> {
    Some(COMPOSITOR.write().as_mut()?.begin_frame())
}

/// End frame.
pub fn end_frame() {
    if let Some(comp) = COMPOSITOR.write().as_mut() {
        comp.end_frame();
    }
}

/// Get render list.
pub fn render_list() -> Vec<LayerId> {
    COMPOSITOR
        .read()
        .as_ref()
        .map(|c| c.render_list())
        .unwrap_or_default()
}

/// Hit test.
pub fn hit_test(x: i32, y: i32) -> Option<(LayerId, TabId)> {
    COMPOSITOR.read().as_ref()?.hit_test(x, y)
}
