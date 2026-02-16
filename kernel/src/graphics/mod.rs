//! Advanced Graphics System
//!
//! High-performance rendering infrastructure for KPIO OS.
//!
//! ## Architecture
//!
//! The graphics system is designed for high refresh rate and resolution:
//!
//! 1. **Compositor** - Composites all surfaces into final frame
//! 2. **Dirty Region Tracking** - Only re-render changed areas
//! 3. **Hardware Acceleration** - When available via GPU drivers
//! 4. **Frame Timing** - VSync-aware rendering for tear-free display
//!
//! ## Performance Optimizations
//!
//! - Partial updates: Only dirty rectangles are recomposited
//! - Layer caching: Static layers are rendered once and cached
//! - SIMD-optimized blitting: Fast memory copies using x86 intrinsics
//! - Async composition: Composition happens in parallel with frame display

pub mod animation;
pub mod blitter;
pub mod compositor;
pub mod damage;
pub mod memcpy;
pub mod pipeline;
pub mod surface;

pub use animation::{Animation, AnimationEngine, Easing};
pub use blitter::{BlitOp, Blitter};
pub use compositor::{Compositor, CompositorConfig};
pub use damage::{DamageRect, DamageTracker};
pub use memcpy::{copy_rect, fast_copy, fast_set, fast_set32, fill_rect_fast};
pub use pipeline::{FrameRateLimit, FrameTiming, RenderPipeline, RenderStats, VSyncMode};
pub use surface::{Surface, SurfaceFlags, SurfaceId};
