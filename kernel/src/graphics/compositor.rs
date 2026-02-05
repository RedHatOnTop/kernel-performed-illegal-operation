//! Surface Compositor
//!
//! Composites multiple surfaces into a final framebuffer output.
//! Supports damage tracking for efficient partial updates.

use alloc::vec::Vec;
use super::surface::{Surface, SurfaceId, PixelFormat};
use super::damage::{DamageTracker, DamageRect};
use super::blitter::{Blitter, BlitOp, BlendMode};
use super::animation::AnimationEngine;

/// Compositor configuration
#[derive(Debug, Clone)]
pub struct CompositorConfig {
    /// Screen width
    pub width: u32,
    /// Screen height
    pub height: u32,
    /// Target frame rate
    pub target_fps: u32,
    /// Enable damage tracking optimization
    pub damage_tracking: bool,
    /// Enable animations
    pub animations_enabled: bool,
    /// Background color (RGBA)
    pub background_color: [u8; 4],
}

impl Default for CompositorConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            target_fps: 60,
            damage_tracking: true,
            animations_enabled: true,
            background_color: [30, 30, 60, 255], // Dark blue background
        }
    }
}

/// Layer in the compositor
#[derive(Debug)]
pub struct Layer {
    /// Layer ID
    pub id: u64,
    /// Surface rendered to this layer
    pub surface: Surface,
    /// Whether layer is visible
    pub visible: bool,
    /// Opacity (0-255)
    pub opacity: u8,
}

/// Surface compositor
pub struct Compositor {
    /// Configuration
    config: CompositorConfig,
    /// Composition target (back buffer)
    target: Surface,
    /// Layers (sorted by z-order)
    layers: Vec<Layer>,
    /// Damage tracker
    damage: DamageTracker,
    /// Blitter for compositing
    blitter: Blitter,
    /// Animation engine
    animations: AnimationEngine,
    /// Frame counter
    frame_count: u64,
    /// Next layer ID
    next_layer_id: u64,
    /// Cursor surface (rendered on top)
    cursor: Option<Surface>,
    /// Cursor position
    cursor_x: i32,
    cursor_y: i32,
    /// Whether composition is needed
    needs_compose: bool,
}

impl Compositor {
    /// Create a new compositor
    pub fn new(config: CompositorConfig) -> Self {
        let target = Surface::new(config.width, config.height, PixelFormat::BGR888);
        let damage = DamageTracker::new(config.width, config.height);

        Self {
            config,
            target,
            layers: Vec::with_capacity(32),
            damage,
            blitter: Blitter::new(),
            animations: AnimationEngine::new(),
            frame_count: 0,
            next_layer_id: 1,
            cursor: None,
            cursor_x: 0,
            cursor_y: 0,
            needs_compose: true,
        }
    }

    /// Create a new surface and add it as a layer
    pub fn create_layer(&mut self, width: u32, height: u32, z_order: i32) -> SurfaceId {
        let mut surface = Surface::new(width, height, PixelFormat::BGRA8888);
        surface.z_order = z_order;
        
        let id = self.next_layer_id;
        self.next_layer_id += 1;

        self.layers.push(Layer {
            id,
            surface,
            visible: true,
            opacity: 255,
        });

        // Sort by z-order
        self.layers.sort_by_key(|l| l.surface.z_order);
        
        self.damage.mark_full_damage();
        self.needs_compose = true;

        SurfaceId(id)
    }

    /// Get mutable reference to a layer's surface
    pub fn get_surface_mut(&mut self, id: SurfaceId) -> Option<&mut Surface> {
        self.layers
            .iter_mut()
            .find(|l| l.id == id.0)
            .map(|l| &mut l.surface)
    }

    /// Get reference to a layer's surface
    pub fn get_surface(&self, id: SurfaceId) -> Option<&Surface> {
        self.layers
            .iter()
            .find(|l| l.id == id.0)
            .map(|l| &l.surface)
    }

    /// Remove a layer
    pub fn remove_layer(&mut self, id: SurfaceId) {
        if let Some(pos) = self.layers.iter().position(|l| l.id == id.0) {
            let layer = &self.layers[pos];
            // Damage the area where layer was
            self.damage.add_damage(DamageRect::new(
                layer.surface.x,
                layer.surface.y,
                layer.surface.width,
                layer.surface.height,
            ));
            self.layers.remove(pos);
            self.needs_compose = true;
        }
    }

    /// Set layer visibility
    pub fn set_layer_visible(&mut self, id: SurfaceId, visible: bool) {
        if let Some(layer) = self.layers.iter_mut().find(|l| l.id == id.0) {
            if layer.visible != visible {
                layer.visible = visible;
                self.damage.add_damage(DamageRect::new(
                    layer.surface.x,
                    layer.surface.y,
                    layer.surface.width,
                    layer.surface.height,
                ));
                self.needs_compose = true;
            }
        }
    }

    /// Set layer position
    pub fn set_layer_position(&mut self, id: SurfaceId, x: i32, y: i32) {
        if let Some(layer) = self.layers.iter_mut().find(|l| l.id == id.0) {
            if layer.surface.x != x || layer.surface.y != y {
                // Damage old position
                self.damage.add_damage(DamageRect::new(
                    layer.surface.x,
                    layer.surface.y,
                    layer.surface.width,
                    layer.surface.height,
                ));
                
                layer.surface.x = x;
                layer.surface.y = y;
                
                // Damage new position
                self.damage.add_damage(DamageRect::new(
                    x,
                    y,
                    layer.surface.width,
                    layer.surface.height,
                ));
                self.needs_compose = true;
            }
        }
    }

    /// Set layer z-order
    pub fn set_layer_z_order(&mut self, id: SurfaceId, z_order: i32) {
        let mut damage_rect = None;
        
        if let Some(layer) = self.layers.iter_mut().find(|l| l.id == id.0) {
            layer.surface.z_order = z_order;
            damage_rect = Some(DamageRect::new(
                layer.surface.x,
                layer.surface.y,
                layer.surface.width,
                layer.surface.height,
            ));
        }
        
        if let Some(rect) = damage_rect {
            self.layers.sort_by_key(|l| l.surface.z_order);
            self.damage.add_damage(rect);
            self.needs_compose = true;
        }
    }

    /// Set layer opacity
    pub fn set_layer_opacity(&mut self, id: SurfaceId, opacity: u8) {
        if let Some(layer) = self.layers.iter_mut().find(|l| l.id == id.0) {
            if layer.opacity != opacity {
                layer.opacity = opacity;
                self.damage.add_damage(DamageRect::new(
                    layer.surface.x,
                    layer.surface.y,
                    layer.surface.width,
                    layer.surface.height,
                ));
                self.needs_compose = true;
            }
        }
    }

    /// Mark a region as damaged
    pub fn damage_region(&mut self, x: i32, y: i32, width: u32, height: u32) {
        self.damage.add_damage(DamageRect::new(x, y, width, height));
        self.needs_compose = true;
    }

    /// Mark layer as dirty (needs recomposition)
    pub fn mark_layer_dirty(&mut self, id: SurfaceId) {
        if let Some(layer) = self.layers.iter().find(|l| l.id == id.0) {
            self.damage.add_damage(DamageRect::new(
                layer.surface.x,
                layer.surface.y,
                layer.surface.width,
                layer.surface.height,
            ));
            self.needs_compose = true;
        }
    }

    /// Set cursor surface
    pub fn set_cursor(&mut self, cursor: Option<Surface>) {
        // Damage old cursor position
        if let Some(old_cursor) = &self.cursor {
            self.damage.add_damage(DamageRect::new(
                self.cursor_x,
                self.cursor_y,
                old_cursor.width,
                old_cursor.height,
            ));
        }

        self.cursor = cursor;

        // Damage new cursor position
        if let Some(new_cursor) = &self.cursor {
            self.damage.add_damage(DamageRect::new(
                self.cursor_x,
                self.cursor_y,
                new_cursor.width,
                new_cursor.height,
            ));
        }

        self.needs_compose = true;
    }

    /// Update cursor position
    pub fn move_cursor(&mut self, x: i32, y: i32) {
        if let Some(cursor) = &self.cursor {
            // Damage old position
            self.damage.add_damage(DamageRect::new(
                self.cursor_x,
                self.cursor_y,
                cursor.width,
                cursor.height,
            ));

            self.cursor_x = x;
            self.cursor_y = y;

            // Damage new position
            self.damage.add_damage(DamageRect::new(
                x,
                y,
                cursor.width,
                cursor.height,
            ));
            
            self.needs_compose = true;
        } else {
            self.cursor_x = x;
            self.cursor_y = y;
        }
    }

    /// Add an animation
    pub fn add_animation(&mut self, animation: super::animation::Animation) -> u64 {
        self.animations.add(animation)
    }

    /// Update animations
    pub fn update_animations(&mut self, time_ms: u64) {
        if !self.config.animations_enabled {
            return;
        }

        self.animations.update(time_ms);

        // Apply animated values to surfaces
        for layer in &mut self.layers {
            let id = layer.id;

            if let Some(opacity) = self.animations.get_value(id, super::animation::AnimationProperty::Opacity) {
                let new_opacity = (opacity * 255.0).clamp(0.0, 255.0) as u8;
                if layer.opacity != new_opacity {
                    layer.opacity = new_opacity;
                    self.needs_compose = true;
                }
            }

            if let Some(x) = self.animations.get_value(id, super::animation::AnimationProperty::X) {
                let new_x = x as i32;
                if layer.surface.x != new_x {
                    self.damage.add_damage(DamageRect::new(
                        layer.surface.x, layer.surface.y,
                        layer.surface.width, layer.surface.height,
                    ));
                    layer.surface.x = new_x;
                    self.damage.add_damage(DamageRect::new(
                        new_x, layer.surface.y,
                        layer.surface.width, layer.surface.height,
                    ));
                    self.needs_compose = true;
                }
            }

            if let Some(y) = self.animations.get_value(id, super::animation::AnimationProperty::Y) {
                let new_y = y as i32;
                if layer.surface.y != new_y {
                    self.damage.add_damage(DamageRect::new(
                        layer.surface.x, layer.surface.y,
                        layer.surface.width, layer.surface.height,
                    ));
                    layer.surface.y = new_y;
                    self.damage.add_damage(DamageRect::new(
                        layer.surface.x, new_y,
                        layer.surface.width, layer.surface.height,
                    ));
                    self.needs_compose = true;
                }
            }
        }

        // If animations are running, we always need to compose
        if self.animations.has_animations() {
            self.needs_compose = true;
        }
    }

    /// Compose all layers into the target buffer
    pub fn compose(&mut self) -> bool {
        if !self.needs_compose && !self.damage.has_damage() {
            return false;
        }

        if self.damage.is_full_damage() {
            self.compose_full();
        } else {
            self.compose_damaged();
        }

        self.damage.clear();
        self.needs_compose = false;
        self.frame_count += 1;

        true
    }

    /// Full screen composition
    fn compose_full(&mut self) {
        // Clear to background color
        let [r, g, b, a] = self.config.background_color;
        self.target.fill(&[b, g, r, a]); // BGR format

        // Composite each visible layer
        for layer in &self.layers {
            if !layer.visible || layer.opacity == 0 {
                continue;
            }

            let op = BlitOp {
                src_rect: DamageRect::new(0, 0, layer.surface.width, layer.surface.height),
                dst_x: layer.surface.x,
                dst_y: layer.surface.y,
                blend_mode: if layer.surface.flags.alpha {
                    BlendMode::AlphaBlend
                } else {
                    BlendMode::Copy
                },
                opacity: layer.opacity,
            };

            self.blitter.blit(&layer.surface, &mut self.target, &op);
        }

        // Composite cursor on top
        if let Some(cursor) = &self.cursor {
            let op = BlitOp {
                src_rect: DamageRect::new(0, 0, cursor.width, cursor.height),
                dst_x: self.cursor_x,
                dst_y: self.cursor_y,
                blend_mode: BlendMode::AlphaBlend,
                opacity: 255,
            };

            self.blitter.blit(cursor, &mut self.target, &op);
        }
    }

    /// Partial composition (only damaged regions)
    fn compose_damaged(&mut self) {
        let damage_rects: Vec<DamageRect> = self.damage.get_rects().to_vec();

        for damage_rect in damage_rects {
            // Clear damaged region to background
            let [r, g, b, a] = self.config.background_color;
            self.blitter.fill_rect(&mut self.target, &damage_rect, r, g, b, a);

            // Composite intersecting layers
            for layer in &self.layers {
                if !layer.visible || layer.opacity == 0 {
                    continue;
                }

                // Check if layer intersects damage rect
                let layer_rect = DamageRect::new(
                    layer.surface.x,
                    layer.surface.y,
                    layer.surface.width,
                    layer.surface.height,
                );

                if let Some(intersection) = layer_rect.intersection(&damage_rect) {
                    // Calculate source rect within layer
                    let src_x = (intersection.x - layer.surface.x).max(0) as u32;
                    let src_y = (intersection.y - layer.surface.y).max(0) as u32;

                    let op = BlitOp {
                        src_rect: DamageRect::new(
                            src_x as i32,
                            src_y as i32,
                            intersection.width,
                            intersection.height,
                        ),
                        dst_x: intersection.x,
                        dst_y: intersection.y,
                        blend_mode: if layer.surface.flags.alpha {
                            BlendMode::AlphaBlend
                        } else {
                            BlendMode::Copy
                        },
                        opacity: layer.opacity,
                    };

                    self.blitter.blit(&layer.surface, &mut self.target, &op);
                }
            }

            // Composite cursor if it intersects
            if let Some(cursor) = &self.cursor {
                let cursor_rect = DamageRect::new(
                    self.cursor_x,
                    self.cursor_y,
                    cursor.width,
                    cursor.height,
                );

                if let Some(intersection) = cursor_rect.intersection(&damage_rect) {
                    let src_x = (intersection.x - self.cursor_x).max(0) as u32;
                    let src_y = (intersection.y - self.cursor_y).max(0) as u32;

                    let op = BlitOp {
                        src_rect: DamageRect::new(
                            src_x as i32,
                            src_y as i32,
                            intersection.width,
                            intersection.height,
                        ),
                        dst_x: intersection.x,
                        dst_y: intersection.y,
                        blend_mode: BlendMode::AlphaBlend,
                        opacity: 255,
                    };

                    self.blitter.blit(cursor, &mut self.target, &op);
                }
            }
        }
    }

    /// Get composed target buffer
    pub fn target(&self) -> &Surface {
        &self.target
    }

    /// Get composed target buffer (mutable)
    pub fn target_mut(&mut self) -> &mut Surface {
        &mut self.target
    }

    /// Get frame count
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get damage statistics
    pub fn damage_stats(&self) -> &super::damage::DamageStats {
        &self.damage.stats
    }

    /// Check if composition is needed
    pub fn needs_composition(&self) -> bool {
        self.needs_compose || self.damage.has_damage()
    }

    /// Get config
    pub fn config(&self) -> &CompositorConfig {
        &self.config
    }

    /// Resize compositor
    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.target = Surface::new(width, height, PixelFormat::BGR888);
        self.damage.resize(width, height);
        self.needs_compose = true;
    }

    /// Copy target to framebuffer
    pub fn copy_to_framebuffer(&self, framebuffer: *mut u8, stride: usize) {
        let src_bpp = self.target.format.bytes_per_pixel();
        let dst_bpp = 3; // Assuming BGR888 framebuffer

        for y in 0..self.config.height {
            let src_row = self.target.row(y);
            let dst_offset = y as usize * stride * dst_bpp;

            unsafe {
                let dst_ptr = framebuffer.add(dst_offset);
                
                if src_bpp == dst_bpp {
                    // Same format, direct copy
                    core::ptr::copy_nonoverlapping(
                        src_row.as_ptr(),
                        dst_ptr,
                        self.config.width as usize * dst_bpp,
                    );
                } else {
                    // Convert row
                    for x in 0..self.config.width as usize {
                        let src_pixel = &src_row[x * src_bpp..];
                        let dst_pixel = dst_ptr.add(x * dst_bpp);
                        
                        *dst_pixel = src_pixel[0];
                        *dst_pixel.add(1) = src_pixel[1];
                        *dst_pixel.add(2) = src_pixel[2];
                    }
                }
            }
        }
    }
}
