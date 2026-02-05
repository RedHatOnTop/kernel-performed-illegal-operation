//! Animation Engine
//!
//! Provides smooth animations with various easing functions.
//! Used for window transitions, effects, and UI feedback.

use alloc::vec::Vec;
use core::f32::consts::PI;
use libm::{powf, sinf, sqrtf};

/// Easing function types for animations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    /// Constant speed
    Linear,
    /// Slow start
    EaseIn,
    /// Slow end
    EaseOut,
    /// Slow start and end
    EaseInOut,
    /// Bouncy effect
    Bounce,
    /// Elastic/spring effect
    Elastic,
    /// Back (overshoot) in
    BackIn,
    /// Back (overshoot) out
    BackOut,
    /// Circular easing in
    CircularIn,
    /// Circular easing out
    CircularOut,
    /// Exponential in
    ExpoIn,
    /// Exponential out
    ExpoOut,
}

impl Easing {
    /// Apply easing to a progress value (0.0 to 1.0)
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);

        match self {
            Easing::Linear => t,
            
            Easing::EaseIn => t * t,
            
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    let x = -2.0 * t + 2.0;
                    1.0 - (x * x) / 2.0
                }
            }
            
            Easing::Bounce => {
                let t = 1.0 - t;
                let result = if t < 1.0 / 2.75 {
                    7.5625 * t * t
                } else if t < 2.0 / 2.75 {
                    let t = t - 1.5 / 2.75;
                    7.5625 * t * t + 0.75
                } else if t < 2.5 / 2.75 {
                    let t = t - 2.25 / 2.75;
                    7.5625 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / 2.75;
                    7.5625 * t * t + 0.984375
                };
                1.0 - result
            }
            
            Easing::Elastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let p = 0.3;
                    let s = p / 4.0;
                    let t = t - 1.0;
                    -(powf(2.0, 10.0 * t) * sinf((t - s) * (2.0 * PI) / p))
                }
            }
            
            Easing::BackIn => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                c3 * t * t * t - c1 * t * t
            }
            
            Easing::BackOut => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                let x = t - 1.0;
                1.0 + c3 * x * x * x + c1 * x * x
            }
            
            Easing::CircularIn => {
                1.0 - sqrtf(1.0 - t * t)
            }
            
            Easing::CircularOut => {
                let x = t - 1.0;
                sqrtf(1.0 - x * x)
            }
            
            Easing::ExpoIn => {
                if t == 0.0 {
                    0.0
                } else {
                    powf(2.0, 10.0 * t - 10.0)
                }
            }
            
            Easing::ExpoOut => {
                if t == 1.0 {
                    1.0
                } else {
                    1.0 - powf(2.0, -10.0 * t)
                }
            }
        }
    }
}

/// Animation property to animate
#[derive(Debug, Clone, Copy)]
pub enum AnimationProperty {
    /// X position
    X,
    /// Y position
    Y,
    /// Width
    Width,
    /// Height
    Height,
    /// Opacity (0.0 - 1.0)
    Opacity,
    /// Rotation in degrees
    Rotation,
    /// Scale factor
    Scale,
    /// Background color R
    ColorR,
    /// Background color G
    ColorG,
    /// Background color B
    ColorB,
    /// Background color A
    ColorA,
}

/// Single animation definition
#[derive(Debug, Clone)]
pub struct Animation {
    /// Animation ID
    pub id: u64,
    /// Target surface ID
    pub target_id: u64,
    /// Property being animated
    pub property: AnimationProperty,
    /// Starting value
    pub from: f32,
    /// Ending value
    pub to: f32,
    /// Duration in milliseconds
    pub duration_ms: u32,
    /// Current progress (0.0 - 1.0)
    pub progress: f32,
    /// Easing function
    pub easing: Easing,
    /// Delay before start (ms)
    pub delay_ms: u32,
    /// Time elapsed in delay
    pub delay_elapsed: u32,
    /// Whether animation has started
    pub started: bool,
    /// Whether animation is complete
    pub complete: bool,
    /// Loop mode
    pub loop_mode: LoopMode,
    /// Number of loops completed
    pub loops_completed: u32,
    /// Callback on completion (stored as ID)
    pub on_complete: Option<u64>,
}

/// Animation loop behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    /// Play once
    Once,
    /// Loop forever
    Forever,
    /// Loop N times
    Count(u32),
    /// Ping-pong (reverse direction at end)
    PingPong,
    /// Ping-pong N times
    PingPongCount(u32),
}

impl Animation {
    /// Create a new animation
    pub fn new(
        target_id: u64,
        property: AnimationProperty,
        from: f32,
        to: f32,
        duration_ms: u32,
    ) -> Self {
        static NEXT_ID: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);

        Self {
            id: NEXT_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
            target_id,
            property,
            from,
            to,
            duration_ms,
            progress: 0.0,
            easing: Easing::EaseOut,
            delay_ms: 0,
            delay_elapsed: 0,
            started: false,
            complete: false,
            loop_mode: LoopMode::Once,
            loops_completed: 0,
            on_complete: None,
        }
    }

    /// Set easing function
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Set delay before animation starts
    pub fn with_delay(mut self, delay_ms: u32) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    /// Set loop mode
    pub fn with_loop(mut self, mode: LoopMode) -> Self {
        self.loop_mode = mode;
        self
    }

    /// Get current animated value
    pub fn current_value(&self) -> f32 {
        if !self.started {
            return self.from;
        }
        if self.complete {
            return self.to;
        }

        let eased = self.easing.apply(self.progress);
        self.from + (self.to - self.from) * eased
    }

    /// Advance animation by delta time (milliseconds)
    pub fn tick(&mut self, delta_ms: u32) {
        if self.complete {
            return;
        }

        // Handle delay
        if !self.started {
            if self.delay_elapsed < self.delay_ms {
                self.delay_elapsed += delta_ms;
                return;
            }
            self.started = true;
        }

        // Advance progress
        if self.duration_ms > 0 {
            self.progress += delta_ms as f32 / self.duration_ms as f32;
        } else {
            self.progress = 1.0;
        }

        // Check completion
        if self.progress >= 1.0 {
            match self.loop_mode {
                LoopMode::Once => {
                    self.progress = 1.0;
                    self.complete = true;
                }
                LoopMode::Forever => {
                    self.progress = 0.0;
                    self.loops_completed += 1;
                }
                LoopMode::Count(n) => {
                    self.loops_completed += 1;
                    if self.loops_completed >= n {
                        self.progress = 1.0;
                        self.complete = true;
                    } else {
                        self.progress = 0.0;
                    }
                }
                LoopMode::PingPong => {
                    self.progress = 0.0;
                    core::mem::swap(&mut self.from, &mut self.to);
                    self.loops_completed += 1;
                }
                LoopMode::PingPongCount(n) => {
                    self.loops_completed += 1;
                    if self.loops_completed >= n * 2 {
                        self.progress = 1.0;
                        self.complete = true;
                    } else {
                        self.progress = 0.0;
                        core::mem::swap(&mut self.from, &mut self.to);
                    }
                }
            }
        }
    }

    /// Reset animation to beginning
    pub fn reset(&mut self) {
        self.progress = 0.0;
        self.delay_elapsed = 0;
        self.started = false;
        self.complete = false;
        self.loops_completed = 0;
    }
}

/// Animation engine manages all active animations
pub struct AnimationEngine {
    /// Active animations
    animations: Vec<Animation>,
    /// Completed animation callbacks (callback_id, target_id, property)
    callbacks: Vec<(u64, u64, AnimationProperty)>,
    /// Current time in milliseconds
    current_time_ms: u64,
    /// Last update time
    last_update_ms: u64,
}

impl AnimationEngine {
    /// Create a new animation engine
    pub fn new() -> Self {
        Self {
            animations: Vec::with_capacity(64),
            callbacks: Vec::new(),
            current_time_ms: 0,
            last_update_ms: 0,
        }
    }

    /// Add a new animation
    pub fn add(&mut self, animation: Animation) -> u64 {
        let id = animation.id;
        self.animations.push(animation);
        id
    }

    /// Cancel an animation by ID
    pub fn cancel(&mut self, id: u64) {
        self.animations.retain(|a| a.id != id);
    }

    /// Cancel all animations for a target
    pub fn cancel_for_target(&mut self, target_id: u64) {
        self.animations.retain(|a| a.target_id != target_id);
    }

    /// Cancel all animations
    pub fn cancel_all(&mut self) {
        self.animations.clear();
    }

    /// Update all animations with current time (milliseconds since boot)
    pub fn update(&mut self, current_time_ms: u64) {
        let delta = if self.last_update_ms == 0 {
            16 // Default to ~60fps for first frame
        } else {
            (current_time_ms - self.last_update_ms) as u32
        };

        self.current_time_ms = current_time_ms;
        self.last_update_ms = current_time_ms;

        // Update all animations
        for animation in &mut self.animations {
            animation.tick(delta);

            // Collect completion callbacks
            if animation.complete {
                if let Some(callback_id) = animation.on_complete {
                    self.callbacks.push((callback_id, animation.target_id, animation.property));
                }
            }
        }

        // Remove completed animations
        self.animations.retain(|a| !a.complete);
    }

    /// Get current value for a target's property
    pub fn get_value(&self, target_id: u64, property: AnimationProperty) -> Option<f32> {
        self.animations
            .iter()
            .find(|a| a.target_id == target_id && matches_property(&a.property, &property))
            .map(|a| a.current_value())
    }

    /// Check if target has any active animations
    pub fn is_animating(&self, target_id: u64) -> bool {
        self.animations.iter().any(|a| a.target_id == target_id)
    }

    /// Check if any animations are active
    pub fn has_animations(&self) -> bool {
        !self.animations.is_empty()
    }

    /// Get number of active animations
    pub fn count(&self) -> usize {
        self.animations.len()
    }

    /// Drain completed callbacks
    pub fn drain_callbacks(&mut self) -> impl Iterator<Item = (u64, u64, AnimationProperty)> + '_ {
        self.callbacks.drain(..)
    }

    /// Create common fade-in animation
    pub fn fade_in(target_id: u64, duration_ms: u32) -> Animation {
        Animation::new(target_id, AnimationProperty::Opacity, 0.0, 1.0, duration_ms)
            .with_easing(Easing::EaseOut)
    }

    /// Create common fade-out animation
    pub fn fade_out(target_id: u64, duration_ms: u32) -> Animation {
        Animation::new(target_id, AnimationProperty::Opacity, 1.0, 0.0, duration_ms)
            .with_easing(Easing::EaseIn)
    }

    /// Create slide-in from left animation
    pub fn slide_in_left(target_id: u64, start_x: f32, end_x: f32, duration_ms: u32) -> Animation {
        Animation::new(target_id, AnimationProperty::X, start_x, end_x, duration_ms)
            .with_easing(Easing::EaseOut)
    }

    /// Create slide-in from top animation
    pub fn slide_in_top(target_id: u64, start_y: f32, end_y: f32, duration_ms: u32) -> Animation {
        Animation::new(target_id, AnimationProperty::Y, start_y, end_y, duration_ms)
            .with_easing(Easing::EaseOut)
    }

    /// Create scale animation
    pub fn scale(target_id: u64, from: f32, to: f32, duration_ms: u32) -> Animation {
        Animation::new(target_id, AnimationProperty::Scale, from, to, duration_ms)
            .with_easing(Easing::EaseInOut)
    }

    /// Create bounce animation
    pub fn bounce(target_id: u64, property: AnimationProperty, amount: f32, duration_ms: u32) -> Animation {
        Animation::new(target_id, property, 0.0, amount, duration_ms)
            .with_easing(Easing::Bounce)
    }

    /// Create pulse animation (scale up and down)
    pub fn pulse(target_id: u64, intensity: f32, duration_ms: u32) -> Animation {
        Animation::new(target_id, AnimationProperty::Scale, 1.0, 1.0 + intensity, duration_ms)
            .with_easing(Easing::EaseInOut)
            .with_loop(LoopMode::PingPong)
    }
}

impl Default for AnimationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to check if properties match
fn matches_property(a: &AnimationProperty, b: &AnimationProperty) -> bool {
    core::mem::discriminant(a) == core::mem::discriminant(b)
}
