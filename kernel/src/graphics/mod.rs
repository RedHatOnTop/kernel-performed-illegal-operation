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

pub mod compositor;
pub mod surface;
pub mod damage;
pub mod blitter;
pub mod animation;

pub use compositor::{Compositor, CompositorConfig};
pub use surface::{Surface, SurfaceId, SurfaceFlags};
pub use damage::{DamageTracker, DamageRect};
pub use blitter::{Blitter, BlitOp};
pub use animation::{AnimationEngine, Animation, Easing};
