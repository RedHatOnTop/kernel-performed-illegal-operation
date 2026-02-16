//! Rendering Pipeline
//!
//! High-performance rendering pipeline that integrates damage tracking,
//! frame rate control, and optimized buffer swapping.

use super::damage::{DamageRect, DamageTracker};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Frame timing information
#[derive(Debug, Clone, Copy)]
pub struct FrameTiming {
    /// Target frame time in microseconds (16667 for 60fps)
    pub target_frame_time_us: u64,
    /// Actual time spent rendering last frame
    pub last_render_time_us: u64,
    /// Total frames rendered
    pub frame_count: u64,
    /// Frames per second (calculated)
    pub fps: u32,
    /// Frame drops detected
    pub dropped_frames: u64,
}

impl Default for FrameTiming {
    fn default() -> Self {
        Self {
            target_frame_time_us: 16667, // ~60 FPS
            last_render_time_us: 0,
            frame_count: 0,
            fps: 60,
            dropped_frames: 0,
        }
    }
}

/// VSync mode for frame synchronization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VSyncMode {
    /// No synchronization (maximum fps, may tear)
    Off,
    /// Wait for vertical blank (smooth, may drop frames)
    On,
    /// Adaptive sync (switch between on/off based on frame time)
    Adaptive,
}

/// Frame rate limiting mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameRateLimit {
    /// No limit
    Unlimited,
    /// Fixed target fps
    Fixed(u32),
    /// Match display refresh rate
    VSync,
}

/// Rendering statistics
#[derive(Debug, Clone, Default)]
pub struct RenderStats {
    /// Total frames rendered
    pub total_frames: u64,
    /// Frames skipped due to no damage
    pub skipped_frames: u64,
    /// Partial redraws (only damaged areas)
    pub partial_redraws: u64,
    /// Full redraws
    pub full_redraws: u64,
    /// Total pixels drawn
    pub pixels_drawn: u64,
    /// Total pixels saved by partial updates
    pub pixels_saved: u64,
    /// Average render time (microseconds)
    pub avg_render_time_us: u64,
    /// Peak render time (microseconds)
    pub peak_render_time_us: u64,
}

/// High-performance rendering pipeline
pub struct RenderPipeline {
    /// Screen dimensions
    pub width: u32,
    pub height: u32,
    /// Bytes per pixel
    pub bpp: usize,
    /// Stride (pixels per row)
    pub stride: usize,
    /// Damage tracker for efficient updates
    pub damage: DamageTracker,
    /// Frame timing
    pub timing: FrameTiming,
    /// VSync mode
    pub vsync: VSyncMode,
    /// Frame rate limit
    pub frame_rate_limit: FrameRateLimit,
    /// Rendering statistics
    pub stats: RenderStats,
    /// Last frame start timestamp (TSC)
    last_frame_tsc: u64,
    /// TSC frequency (approx cycles per microsecond)
    tsc_freq_mhz: u64,
    /// Pending frame flag
    frame_pending: bool,
}

impl RenderPipeline {
    /// Create a new rendering pipeline
    pub fn new(width: u32, height: u32, bpp: usize, stride: usize) -> Self {
        Self {
            width,
            height,
            bpp,
            stride,
            damage: DamageTracker::new(width, height),
            timing: FrameTiming::default(),
            vsync: VSyncMode::Adaptive,
            frame_rate_limit: FrameRateLimit::Fixed(60),
            stats: RenderStats::default(),
            last_frame_tsc: 0,
            tsc_freq_mhz: estimate_tsc_frequency_mhz(),
            frame_pending: false,
        }
    }

    /// Mark a region as needing redraw
    pub fn damage_rect(&mut self, x: i32, y: i32, width: u32, height: u32) {
        self.damage.add_damage(DamageRect::new(x, y, width, height));
        self.frame_pending = true;
    }

    /// Mark a window area as damaged (for move/resize)
    pub fn damage_window(
        &mut self,
        old_x: i32,
        old_y: i32,
        new_x: i32,
        new_y: i32,
        width: u32,
        height: u32,
    ) {
        self.damage
            .add_surface_damage(old_x, old_y, new_x, new_y, width, height);
        self.frame_pending = true;
    }

    /// Mark entire screen for redraw
    pub fn damage_full(&mut self) {
        self.damage.mark_full_damage();
        self.frame_pending = true;
    }

    /// Check if a frame should be rendered
    pub fn should_render(&self) -> bool {
        if !self.frame_pending && !self.damage.has_damage() {
            return false;
        }

        // Frame rate limiting
        match self.frame_rate_limit {
            FrameRateLimit::Unlimited => true,
            FrameRateLimit::Fixed(target_fps) => {
                let target_interval_us = 1_000_000 / target_fps as u64;
                let elapsed = self.elapsed_us_since_last_frame();
                elapsed >= target_interval_us
            }
            FrameRateLimit::VSync => {
                // In real hardware, we'd check vblank signal
                // For now, assume 60Hz
                let elapsed = self.elapsed_us_since_last_frame();
                elapsed >= 16667
            }
        }
    }

    /// Get elapsed time since last frame in microseconds
    fn elapsed_us_since_last_frame(&self) -> u64 {
        let current_tsc = read_tsc();
        if self.last_frame_tsc == 0 || self.tsc_freq_mhz == 0 {
            return u64::MAX;
        }
        (current_tsc - self.last_frame_tsc) / self.tsc_freq_mhz
    }

    /// Begin a new frame
    pub fn begin_frame(&mut self) -> FrameContext {
        let frame_start = read_tsc();

        FrameContext {
            start_tsc: frame_start,
            is_full_redraw: self.damage.is_full_damage(),
            damage_rects: if self.damage.is_full_damage() {
                vec![self.damage.screen_bounds()]
            } else {
                self.damage.get_rects().to_vec()
            },
        }
    }

    /// End a frame (update timing, clear damage)
    pub fn end_frame(&mut self, ctx: FrameContext) {
        let frame_end = read_tsc();
        let frame_time_us = if self.tsc_freq_mhz > 0 {
            (frame_end - ctx.start_tsc) / self.tsc_freq_mhz
        } else {
            0
        };

        // Update timing
        self.timing.last_render_time_us = frame_time_us;
        self.timing.frame_count += 1;
        self.last_frame_tsc = frame_end;

        // Update stats
        self.stats.total_frames += 1;
        if ctx.is_full_redraw {
            self.stats.full_redraws += 1;
        } else {
            self.stats.partial_redraws += 1;
        }

        // Update rolling average
        self.stats.avg_render_time_us = (self.stats.avg_render_time_us * 7 + frame_time_us) / 8;

        if frame_time_us > self.stats.peak_render_time_us {
            self.stats.peak_render_time_us = frame_time_us;
        }

        // Check for dropped frames
        if frame_time_us > self.timing.target_frame_time_us * 2 {
            self.timing.dropped_frames += 1;
        }

        // Calculate FPS (smoothed)
        if frame_time_us > 0 {
            let instant_fps = (1_000_000 / frame_time_us) as u32;
            self.timing.fps = (self.timing.fps * 7 + instant_fps) / 8;
        }

        // Clear damage for next frame
        self.damage.clear();
        self.frame_pending = false;
    }

    /// Skip this frame (no changes)
    pub fn skip_frame(&mut self) {
        self.stats.skipped_frames += 1;
        self.last_frame_tsc = read_tsc();
    }

    /// Set frame rate limit
    pub fn set_frame_rate_limit(&mut self, limit: FrameRateLimit) {
        self.frame_rate_limit = limit;
        match limit {
            FrameRateLimit::Fixed(fps) => {
                self.timing.target_frame_time_us = 1_000_000 / fps as u64;
            }
            FrameRateLimit::VSync => {
                self.timing.target_frame_time_us = 16667; // Assume 60Hz
            }
            FrameRateLimit::Unlimited => {
                self.timing.target_frame_time_us = 0;
            }
        }
    }

    /// Get rendering efficiency (0.0 = all full redraws, 1.0 = all partial)
    pub fn efficiency(&self) -> f32 {
        if self.stats.total_frames == 0 {
            return 0.0;
        }
        self.stats.partial_redraws as f32 / self.stats.total_frames as f32
    }
}

/// Context for a single frame
pub struct FrameContext {
    /// TSC at frame start
    pub start_tsc: u64,
    /// Whether this is a full redraw
    pub is_full_redraw: bool,
    /// Damaged rectangles to redraw
    pub damage_rects: Vec<DamageRect>,
}

/// Read CPU timestamp counter
#[inline]
fn read_tsc() -> u64 {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::x86_64::_rdtsc()
    }
    #[cfg(not(target_arch = "x86_64"))]
    0
}

/// Estimate TSC frequency (very rough approximation)
/// In a real kernel, we'd calibrate this against a known timer
fn estimate_tsc_frequency_mhz() -> u64 {
    // Assume ~2GHz CPU = 2000 cycles per microsecond
    // This is a rough estimate; real implementation would calibrate
    2000
}

/// Frame rate controller for consistent animation timing
pub struct FrameRateController {
    /// Target FPS
    target_fps: u32,
    /// Accumulated time for frame pacing
    accumulated_time_us: u64,
    /// Time per frame in microseconds
    frame_time_us: u64,
    /// Last tick timestamp
    last_tick_tsc: u64,
    /// TSC frequency
    tsc_freq_mhz: u64,
}

impl FrameRateController {
    /// Create a new frame rate controller
    pub fn new(target_fps: u32) -> Self {
        Self {
            target_fps,
            accumulated_time_us: 0,
            frame_time_us: 1_000_000 / target_fps as u64,
            last_tick_tsc: read_tsc(),
            tsc_freq_mhz: estimate_tsc_frequency_mhz(),
        }
    }

    /// Tick the controller, returns number of frames to advance
    pub fn tick(&mut self) -> u32 {
        let now = read_tsc();
        let delta_tsc = now.saturating_sub(self.last_tick_tsc);
        self.last_tick_tsc = now;

        let delta_us = if self.tsc_freq_mhz > 0 {
            delta_tsc / self.tsc_freq_mhz
        } else {
            self.frame_time_us // Fallback: assume 1 frame
        };

        self.accumulated_time_us += delta_us;

        let frames = (self.accumulated_time_us / self.frame_time_us) as u32;
        self.accumulated_time_us %= self.frame_time_us;

        frames.min(4) // Cap at 4 frames to prevent spiral of death
    }

    /// Get interpolation factor for smooth rendering (0.0 - 1.0)
    pub fn interpolation(&self) -> f32 {
        self.accumulated_time_us as f32 / self.frame_time_us as f32
    }

    /// Set target FPS
    pub fn set_target_fps(&mut self, fps: u32) {
        self.target_fps = fps;
        self.frame_time_us = 1_000_000 / fps as u64;
    }
}
