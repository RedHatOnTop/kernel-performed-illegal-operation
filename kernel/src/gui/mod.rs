//! GUI Shell Module
//!
//! Simple graphical user interface for KPIO OS.
//! Provides a desktop environment with taskbar, windows, and basic apps.
//!
//! ## Architecture
//!
//! The GUI uses double-buffering and damage tracking to eliminate flickering
//! and maximize rendering performance:
//!
//! 1. All drawing happens to a back buffer in RAM
//! 2. Only damaged regions are redrawn (dirty rectangle optimization)
//! 3. Frame rate is controlled to prevent wasted CPU cycles
//! 4. When a frame is complete, the back buffer is copied to the display

pub mod boot_animation;
pub mod desktop;
pub mod font;
pub mod framebuffer;
pub mod html_render;
pub mod input;
pub mod mouse;
pub mod notification;
pub mod notification_panel;
pub mod render;
pub mod splash;
pub mod taskbar;
pub mod theme;
pub mod toast;
pub mod window;

use crate::graphics::{DamageRect, FrameRateLimit, RenderPipeline};
use alloc::vec::Vec;
use spin::Mutex;

pub use desktop::Desktop;
pub use font::Font8x8;
pub use framebuffer::FramebufferManager;
pub use mouse::MouseCursor;
pub use render::Renderer;
pub use taskbar::Taskbar;
pub use window::{Window, WindowId};

/// GUI System state
pub struct GuiSystem {
    /// Screen width
    pub width: u32,
    /// Screen height
    pub height: u32,
    /// Bytes per pixel
    pub bpp: usize,
    /// Stride (pixels per row)
    pub stride: usize,
    /// Framebuffer pointer (front buffer)
    pub framebuffer: *mut u8,
    /// Double-buffered framebuffer manager
    pub fb_manager: FramebufferManager,
    /// Rendering pipeline with damage tracking
    pub pipeline: RenderPipeline,
    /// Desktop
    pub desktop: Desktop,
    /// Taskbar
    pub taskbar: Taskbar,
    /// Windows
    pub windows: Vec<Window>,
    /// Active window
    pub active_window: Option<WindowId>,
    /// Mouse cursor
    pub mouse: MouseCursor,
    /// Needs redraw (legacy, now uses pipeline)
    pub dirty: bool,
    /// Dragging state
    pub dragging: Option<DragState>,
    /// Frame counter for performance tracking
    pub frame_count: u64,
    /// Last mouse position (for damage tracking)
    last_mouse_x: i32,
    last_mouse_y: i32,
}

/// Window drag state
#[derive(Debug, Clone, Copy)]
pub struct DragState {
    pub window_id: WindowId,
    pub offset_x: i32,
    pub offset_y: i32,
}

unsafe impl Send for GuiSystem {}
unsafe impl Sync for GuiSystem {}

impl GuiSystem {
    /// Create new GUI system
    pub fn new(width: u32, height: u32, bpp: usize, stride: usize, fb: *mut u8) -> Self {
        let taskbar_height = theme::Size::TASKBAR_HEIGHT;
        let fb_manager = FramebufferManager::new(fb, width, height, bpp, stride);
        let mut pipeline = RenderPipeline::new(width, height, bpp, stride);

        // Set default frame rate limit (60 FPS)
        pipeline.set_frame_rate_limit(FrameRateLimit::Fixed(60));

        // Mark full screen for initial render
        pipeline.damage_full();

        Self {
            width,
            height,
            bpp,
            stride,
            framebuffer: fb,
            fb_manager,
            pipeline,
            desktop: Desktop::new(width, height - taskbar_height),
            taskbar: Taskbar::new(width, taskbar_height),
            windows: Vec::new(),
            active_window: None,
            mouse: MouseCursor::new(width as i32 / 2, height as i32 / 2),
            dirty: true,
            dragging: None,
            frame_count: 0,
            last_mouse_x: width as i32 / 2,
            last_mouse_y: height as i32 / 2,
        }
    }

    /// Create a new window
    pub fn create_window(&mut self, title: &str, x: i32, y: i32, w: u32, h: u32) -> WindowId {
        let id = WindowId(self.windows.len() as u64 + 1);
        let window = Window::new(id, title, x, y, w, h);

        // Damage the new window area
        self.pipeline.damage_rect(x, y, w, h);

        self.windows.push(window);
        self.active_window = Some(id);
        self.taskbar.add_window(id, title);

        // Also damage taskbar area
        self.pipeline.damage_rect(
            0,
            (self.height - self.taskbar.height) as i32,
            self.width,
            self.taskbar.height,
        );
        self.dirty = true;
        id
    }

    /// Close a window
    pub fn close_window(&mut self, id: WindowId) {
        // Get window bounds for damage before removing
        if let Some(window) = self.windows.iter().find(|w| w.id == id) {
            self.pipeline
                .damage_rect(window.x, window.y, window.width, window.height);
        }

        self.windows.retain(|w| w.id != id);
        self.taskbar.remove_window(id);
        if self.active_window == Some(id) {
            self.active_window = self.windows.last().map(|w| w.id);
        }

        // Damage taskbar
        self.pipeline.damage_rect(
            0,
            (self.height - self.taskbar.height) as i32,
            self.width,
            self.taskbar.height,
        );
        self.dirty = true;
    }

    /// Handle mouse move
    pub fn on_mouse_move(&mut self, dx: i32, dy: i32) {
        // Track old mouse position for damage
        let old_x = self.mouse.x;
        let old_y = self.mouse.y;

        self.mouse
            .move_by(dx, dy, self.width as i32, self.height as i32);

        // Damage old and new cursor positions (cursor is typically 16x16)
        const CURSOR_SIZE: u32 = 20;
        self.pipeline
            .damage_rect(old_x - 2, old_y - 2, CURSOR_SIZE, CURSOR_SIZE);
        self.pipeline
            .damage_rect(self.mouse.x - 2, self.mouse.y - 2, CURSOR_SIZE, CURSOR_SIZE);

        // Handle window dragging
        if let Some(drag) = self.dragging {
            if let Some(window) = self.windows.iter_mut().find(|w| w.id == drag.window_id) {
                let old_wx = window.x;
                let old_wy = window.y;

                window.x = self.mouse.x - drag.offset_x;
                window.y = self.mouse.y - drag.offset_y;

                // Keep window on screen
                window.x = window.x.max(0).min(self.width as i32 - 50);
                window.y = window.y.max(0).min(self.height as i32 - 50);

                // Damage both old and new window positions
                self.pipeline.damage_window(
                    old_wx,
                    old_wy,
                    window.x,
                    window.y,
                    window.width,
                    window.height,
                );
            }
        }

        // Desktop icon hover detection
        let hover_idx = self.desktop.icon_at(self.mouse.x, self.mouse.y);
        self.desktop.set_hover(hover_idx);

        self.dirty = true;
    }

    /// Handle mouse click
    pub fn on_mouse_click(&mut self, button: u8, pressed: bool) {
        let x = self.mouse.x;
        let y = self.mouse.y;

        // Handle drag end
        if !pressed && button == 0 {
            self.dragging = None;
        }

        if pressed && button == 0 {
            // Check start menu click first
            if let Some(app_type) = self.taskbar.check_start_menu_click(x, y, self.height) {
                self.launch_app(app_type);
                self.dirty = true;
                return;
            }

            // Close start menu if clicking elsewhere
            if self.taskbar.start_menu_open {
                let taskbar_y = (self.height - self.taskbar.height) as i32;
                let in_menu = x < 200 && y >= taskbar_y - 200 && y < taskbar_y;
                let in_start_button = x < 48 && y >= taskbar_y;

                if !in_menu && !in_start_button {
                    self.taskbar.start_menu_open = false;
                    self.dirty = true;
                    return;
                }
            }

            // Check taskbar click
            if y >= (self.height - self.taskbar.height) as i32 {
                let clicked_id = self
                    .taskbar
                    .on_click(x, y - (self.height - self.taskbar.height) as i32);

                // If clicked a taskbar item, focus that window or restore if minimized
                if let Some(idx) = clicked_id {
                    if idx < self.taskbar.items.len() {
                        let window_id = self.taskbar.items[idx].window_id;

                        // Restore if minimized
                        if let Some(window) = self.windows.iter_mut().find(|w| w.id == window_id) {
                            if !window.is_visible() {
                                window.restore();
                            }
                        }

                        self.active_window = Some(window_id);

                        // Bring window to front
                        if let Some(pos) = self.windows.iter().position(|w| w.id == window_id) {
                            let window = self.windows.remove(pos);
                            self.windows.push(window);
                        }
                    }
                }

                self.dirty = true;
                return;
            }

            // Check desktop icon clicks (below taskbar, no window hit)
            let taskbar_top = (self.height - self.taskbar.height) as i32;
            if y < taskbar_top {
                if let Some(icon_idx) = self.desktop.icon_at(x, y) {
                    // Check if a window would intercept first
                    let any_window = self
                        .windows
                        .iter()
                        .rev()
                        .any(|w| w.is_visible() && w.contains(x, y));
                    if !any_window {
                        let app_type = match &self.desktop.icons[icon_idx].icon_type {
                            desktop::IconType::Files => taskbar::AppType::Files,
                            desktop::IconType::Browser => taskbar::AppType::Browser,
                            desktop::IconType::Terminal => taskbar::AppType::Terminal,
                            desktop::IconType::Settings => taskbar::AppType::Settings,
                            desktop::IconType::Trash => return, // Trash does nothing
                            desktop::IconType::InstalledApp { app_id, .. } => {
                                // Look up the app descriptor to get launch info
                                let reg = crate::app::registry::APP_REGISTRY.lock();
                                if let Some(desc) =
                                    reg.get(crate::app::registry::KernelAppId(*app_id))
                                {
                                    let (scope, _) = match &desc.app_type {
                                        crate::app::registry::KernelAppType::WebApp {
                                            scope,
                                            offline_capable,
                                        } => (scope.clone(), *offline_capable),
                                        _ => (alloc::string::String::new(), false),
                                    };
                                    taskbar::AppType::WebApp {
                                        app_id: *app_id,
                                        name: desc.name.clone(),
                                        start_url: desc.entry_point.clone(),
                                        scope,
                                        theme_color: None,
                                    }
                                } else {
                                    return;
                                }
                            }
                        };
                        self.launch_app(app_type);
                        self.dirty = true;
                        return;
                    }
                }
            }

            // Check window clicks (reverse order for top window first, only visible)
            let mut clicked_window_id = None;
            for window in self.windows.iter().rev() {
                if window.is_visible() && window.contains(x, y) {
                    clicked_window_id = Some(window.id);
                    break;
                }
            }

            if let Some(id) = clicked_window_id {
                self.active_window = Some(id);

                // Find window and process click
                let screen_w = self.width;
                let screen_h = self.height;
                let taskbar_h = self.taskbar.height;

                if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
                    let local_x = x - window.x;
                    let local_y = y - window.y;

                    let bw = theme::Size::WIN_BTN_W as i32;
                    let tb = theme::Size::TITLE_BAR_HEIGHT as i32;
                    // Check title bar for dragging (excluding button area)
                    if local_y < tb && local_x < window.width as i32 - bw * 3 {
                        self.dragging = Some(DragState {
                            window_id: id,
                            offset_x: local_x,
                            offset_y: local_y,
                        });
                    }
                    // Check close button
                    else if local_y < tb && local_x >= window.width as i32 - bw {
                        let id_to_close = id;
                        self.windows.retain(|w| w.id != id_to_close);
                        self.taskbar.remove_window(id_to_close);
                        if self.active_window == Some(id_to_close) {
                            self.active_window = self.windows.last().map(|w| w.id);
                        }
                    }
                    // Check maximize button
                    else if local_y < tb
                        && local_x >= window.width as i32 - bw * 2
                        && local_x < window.width as i32 - bw
                    {
                        window.maximize(screen_w, screen_h, taskbar_h);
                    }
                    // Check minimize button
                    else if local_y < tb
                        && local_x >= window.width as i32 - bw * 3
                        && local_x < window.width as i32 - bw * 2
                    {
                        window.minimize();
                        // Set active to another visible window
                        self.active_window = self
                            .windows
                            .iter()
                            .rev()
                            .find(|w| w.is_visible() && w.id != id)
                            .map(|w| w.id);
                    } else {
                        window.on_click(local_x, local_y, pressed);
                    }
                }

                // Bring window to front
                if let Some(pos) = self.windows.iter().position(|w| w.id == id) {
                    let window = self.windows.remove(pos);
                    self.windows.push(window);
                }

                self.dirty = true;
            }
        }
    }

    /// Handle key press
    pub fn on_key_event(&mut self, event: &input::KeyEvent) {
        if let Some(id) = self.active_window {
            if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
                window.on_key_event(event);
                self.dirty = true;
            }
        }
    }

    /// Next window ID counter
    fn next_window_id(&self) -> WindowId {
        let max_id = self.windows.iter().map(|w| w.id.0).max().unwrap_or(0);
        WindowId(max_id + 1)
    }

    /// Launch an application
    pub fn launch_app(&mut self, app_type: taskbar::AppType) {
        use taskbar::AppType;

        let id = self.next_window_id();

        // Offset new windows slightly
        let offset = (self.windows.len() as i32 % 5) * 30;

        let window = match app_type {
            AppType::Browser => Window::new_browser(id, 100 + offset, 50 + offset),
            AppType::Terminal => Window::new_terminal(id, 150 + offset, 100 + offset),
            AppType::Files => Window::new_files(id, 200 + offset, 80 + offset),
            AppType::Settings => Window::new_settings(id, 180 + offset, 120 + offset),
            AppType::WebApp {
                app_id,
                name,
                start_url,
                scope,
                theme_color,
            } => Window::new_webapp(
                id,
                app_id,
                &name,
                &start_url,
                &scope,
                theme_color,
                window::PwaDisplayMode::Standalone,
                120 + offset,
                60 + offset,
            ),
        };

        let title = window.title.clone();
        self.windows.push(window);
        self.taskbar.add_window(id, &title);
        self.active_window = Some(id);
        self.dirty = true;
    }

    /// Render the GUI using double buffering and damage tracking
    pub fn render(&mut self) {
        // Check if we should render this frame (frame rate limiting)
        if !self.dirty && !self.pipeline.should_render() {
            self.pipeline.skip_frame();
            return;
        }

        // Begin frame (captures timing and damage info)
        let frame_ctx = self.pipeline.begin_frame();

        // Create renderer targeting the BACK buffer (not the display)
        let mut renderer = Renderer::new(
            self.fb_manager.back_buffer(),
            self.width,
            self.height,
            self.bpp,
            self.stride,
        );

        // For now, always do full redraws (damage-aware rendering can be
        // optimized further, but requires more complex clipping logic)
        // The frame rate limiting still provides significant performance gains

        // Sync state to taskbar
        self.taskbar.frame_count = self.frame_count;
        self.taskbar.set_active(self.active_window);

        // Draw desktop background
        self.desktop.render(&mut renderer);

        // Draw windows (skip minimized)
        for window in &self.windows {
            if !window.is_visible() {
                continue;
            }
            let is_active = self.active_window == Some(window.id);
            window.render(&mut renderer, is_active);
        }

        // Draw taskbar
        self.taskbar
            .render(&mut renderer, self.height - self.taskbar.height);

        // Draw mouse cursor (always on top)
        self.mouse.render(&mut renderer);

        // CRITICAL: Copy back buffer to front buffer atomically
        self.fb_manager.swap_buffers();

        // End frame (update timing stats, clear damage)
        self.pipeline.end_frame(frame_ctx);

        self.dirty = false;
        self.frame_count += 1;
    }

    /// Get render statistics for debugging/profiling
    pub fn get_render_stats(&self) -> &crate::graphics::RenderStats {
        &self.pipeline.stats
    }

    /// Get current FPS
    pub fn get_fps(&self) -> u32 {
        self.pipeline.timing.fps
    }
}

/// Global GUI system
pub static GUI: Mutex<Option<GuiSystem>> = Mutex::new(None);

/// Initialize GUI system
pub fn init(width: u32, height: u32, bpp: usize, stride: usize, fb: *mut u8) {
    let mut gui = GUI.lock();
    *gui = Some(GuiSystem::new(width, height, bpp, stride, fb));
    crate::serial_println!("[GUI] Initialized {}x{} @ {} bpp", width, height, bpp);
}

/// Get GUI system reference
pub fn with_gui<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut GuiSystem) -> R,
{
    let mut gui = GUI.lock();
    gui.as_mut().map(f)
}

/// Render GUI
pub fn render() {
    with_gui(|gui| gui.render());
}

/// Handle mouse movement
pub fn mouse_move(dx: i32, dy: i32) {
    with_gui(|gui| gui.on_mouse_move(dx, dy));
}

/// Handle mouse click
pub fn mouse_click(button: u8, pressed: bool) {
    with_gui(|gui| gui.on_mouse_click(button, pressed));
}

/// Handle key press
pub fn key_press(event: &input::KeyEvent) {
    with_gui(|gui| gui.on_key_event(event));
}
