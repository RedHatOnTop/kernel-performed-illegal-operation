//! Window compositor using Vello.
//!
//! This module implements the window compositor that manages
//! surfaces from multiple applications and composites them
//! into the final display output.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use spin::Mutex;

use crate::surface::{Surface, SurfaceId};
use crate::{DisplayMode, GraphicsError, PixelFormat};

/// Global compositor instance.
static COMPOSITOR: Mutex<Option<Compositor>> = Mutex::new(None);

/// Initialize the compositor.
pub fn init() -> Result<(), GraphicsError> {
    let comp = Compositor::new()?;
    *COMPOSITOR.lock() = Some(comp);
    Ok(())
}

/// Get the current display mode.
pub fn get_display_mode() -> Option<DisplayMode> {
    COMPOSITOR.lock().as_ref().map(|c| c.display_mode)
}

/// Set the display mode.
pub fn set_display_mode(mode: DisplayMode) -> Result<(), GraphicsError> {
    if let Some(ref mut comp) = *COMPOSITOR.lock() {
        comp.set_display_mode(mode)?;
    }
    Ok(())
}

/// The window compositor.
pub struct Compositor {
    /// Current display mode.
    display_mode: DisplayMode,

    /// All surfaces managed by the compositor.
    surfaces: BTreeMap<SurfaceId, SurfaceState>,

    /// Surface stacking order (back to front).
    stacking_order: Vec<SurfaceId>,

    /// Focused surface.
    focused_surface: Option<SurfaceId>,

    /// Next surface ID.
    next_surface_id: u64,

    /// Damage regions for incremental updates.
    damage_regions: Vec<DamageRect>,

    /// Whether a frame is pending.
    frame_pending: bool,
}

impl Compositor {
    /// Create a new compositor.
    pub fn new() -> Result<Self, GraphicsError> {
        Ok(Compositor {
            display_mode: DisplayMode {
                width: 1920,
                height: 1080,
                refresh_rate: 60,
                format: PixelFormat::Bgra8Srgb,
            },
            surfaces: BTreeMap::new(),
            stacking_order: Vec::new(),
            focused_surface: None,
            next_surface_id: 1,
            damage_regions: Vec::new(),
            frame_pending: false,
        })
    }

    /// Set the display mode.
    pub fn set_display_mode(&mut self, mode: DisplayMode) -> Result<(), GraphicsError> {
        self.display_mode = mode;
        // Resize framebuffer
        self.damage_full_screen();
        Ok(())
    }

    /// Create a new surface.
    pub fn create_surface(&mut self, width: u32, height: u32) -> Result<SurfaceId, GraphicsError> {
        let id = SurfaceId(self.next_surface_id);
        self.next_surface_id += 1;

        let state = SurfaceState {
            x: 0,
            y: 0,
            width,
            height,
            visible: true,
            opacity: 1.0,
            buffer: None,
        };

        self.surfaces.insert(id, state);
        self.stacking_order.push(id);

        Ok(id)
    }

    /// Destroy a surface.
    pub fn destroy_surface(&mut self, id: SurfaceId) -> Result<(), GraphicsError> {
        self.surfaces.remove(&id);
        self.stacking_order.retain(|&s| s != id);

        if self.focused_surface == Some(id) {
            self.focused_surface = self.stacking_order.last().copied();
        }

        Ok(())
    }

    /// Move a surface to a new position.
    pub fn move_surface(&mut self, id: SurfaceId, x: i32, y: i32) -> Result<(), GraphicsError> {
        // First, read the old state values
        let damage_info = self
            .surfaces
            .get(&id)
            .map(|state| (state.x, state.y, state.width, state.height));

        if let Some((old_x, old_y, width, height)) = damage_info {
            // Damage old position
            self.add_damage(DamageRect {
                x: old_x,
                y: old_y,
                width,
                height,
            });

            // Update state
            if let Some(state) = self.surfaces.get_mut(&id) {
                state.x = x;
                state.y = y;
            }

            // Damage new position
            self.add_damage(DamageRect {
                x,
                y,
                width,
                height,
            });
        }

        Ok(())
    }

    /// Resize a surface.
    pub fn resize_surface(
        &mut self,
        id: SurfaceId,
        width: u32,
        height: u32,
    ) -> Result<(), GraphicsError> {
        // First, read the current state values
        let damage_info = self
            .surfaces
            .get(&id)
            .map(|state| (state.x, state.y, state.width, state.height));

        if let Some((x, y, old_width, old_height)) = damage_info {
            self.add_damage(DamageRect {
                x,
                y,
                width: old_width.max(width),
                height: old_height.max(height),
            });

            if let Some(state) = self.surfaces.get_mut(&id) {
                state.width = width;
                state.height = height;
            }
        }

        Ok(())
    }

    /// Set surface visibility.
    pub fn set_surface_visible(
        &mut self,
        id: SurfaceId,
        visible: bool,
    ) -> Result<(), GraphicsError> {
        // First, read the current state values and update visibility
        let damage_info = if let Some(state) = self.surfaces.get_mut(&id) {
            state.visible = visible;
            Some((state.x, state.y, state.width, state.height))
        } else {
            None
        };

        if let Some((x, y, width, height)) = damage_info {
            self.add_damage(DamageRect {
                x,
                y,
                width,
                height,
            });
        }

        Ok(())
    }

    /// Raise a surface to the top.
    pub fn raise_surface(&mut self, id: SurfaceId) -> Result<(), GraphicsError> {
        self.stacking_order.retain(|&s| s != id);
        self.stacking_order.push(id);

        if let Some(state) = self.surfaces.get(&id) {
            self.add_damage(DamageRect {
                x: state.x,
                y: state.y,
                width: state.width,
                height: state.height,
            });
        }

        Ok(())
    }

    /// Set the focused surface.
    pub fn set_focus(&mut self, id: Option<SurfaceId>) {
        self.focused_surface = id;
    }

    /// Get the focused surface.
    pub fn focused_surface(&self) -> Option<SurfaceId> {
        self.focused_surface
    }

    /// Commit a surface buffer.
    pub fn commit_surface(&mut self, id: SurfaceId, buffer: u64) -> Result<(), GraphicsError> {
        // First, update state and get damage info
        let damage_info = if let Some(state) = self.surfaces.get_mut(&id) {
            state.buffer = Some(buffer);
            Some((state.x, state.y, state.width, state.height))
        } else {
            None
        };

        if let Some((x, y, width, height)) = damage_info {
            self.add_damage(DamageRect {
                x,
                y,
                width,
                height,
            });
            self.frame_pending = true;
        }

        Ok(())
    }

    /// Render a frame.
    pub fn render_frame(&mut self) -> Result<(), GraphicsError> {
        if !self.frame_pending {
            return Ok(());
        }

        // Composite all visible surfaces
        for &id in &self.stacking_order {
            if let Some(state) = self.surfaces.get(&id) {
                if state.visible && state.buffer.is_some() {
                    // Render surface to framebuffer
                    // (actual implementation would use Vello)
                }
            }
        }

        // Clear damage
        self.damage_regions.clear();
        self.frame_pending = false;

        Ok(())
    }

    /// Add a damage region.
    fn add_damage(&mut self, rect: DamageRect) {
        self.damage_regions.push(rect);
        self.frame_pending = true;
    }

    /// Damage the full screen.
    fn damage_full_screen(&mut self) {
        self.damage_regions.clear();
        self.damage_regions.push(DamageRect {
            x: 0,
            y: 0,
            width: self.display_mode.width,
            height: self.display_mode.height,
        });
        self.frame_pending = true;
    }
}

/// Surface state tracked by the compositor.
struct SurfaceState {
    /// X position.
    x: i32,
    /// Y position.
    y: i32,
    /// Width.
    width: u32,
    /// Height.
    height: u32,
    /// Visibility.
    visible: bool,
    /// Opacity (0.0-1.0).
    opacity: f32,
    /// Current buffer handle.
    buffer: Option<u64>,
}

/// Damage rectangle.
#[derive(Debug, Clone, Copy)]
struct DamageRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}
