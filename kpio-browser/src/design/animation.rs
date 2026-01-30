//! Animation System
//!
//! Animation primitives and presets for KPIO Browser UI.

use super::tokens::{duration, easing, Easing};

/// Animation definition
#[derive(Debug, Clone)]
pub struct Animation {
    /// Duration in milliseconds
    pub duration: u32,
    /// Easing curve
    pub easing: Easing,
    /// Delay before start
    pub delay: u32,
    /// Number of iterations (0 = infinite)
    pub iterations: u32,
    /// Direction
    pub direction: AnimationDirection,
    /// Fill mode
    pub fill_mode: AnimationFillMode,
}

impl Animation {
    /// Create new animation
    pub fn new(duration: u32) -> Self {
        Self {
            duration,
            easing: easing::EASE,
            delay: 0,
            iterations: 1,
            direction: AnimationDirection::Normal,
            fill_mode: AnimationFillMode::None,
        }
    }

    /// Set easing
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Set delay
    pub fn delay(mut self, delay: u32) -> Self {
        self.delay = delay;
        self
    }

    /// Set iterations
    pub fn iterations(mut self, iterations: u32) -> Self {
        self.iterations = iterations;
        self
    }

    /// Set infinite
    pub fn infinite(mut self) -> Self {
        self.iterations = 0;
        self
    }

    /// Set alternate direction
    pub fn alternate(mut self) -> Self {
        self.direction = AnimationDirection::Alternate;
        self
    }
}

/// Animation direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationDirection {
    #[default]
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

/// Animation fill mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationFillMode {
    #[default]
    None,
    Forwards,
    Backwards,
    Both,
}

/// Transition definition
#[derive(Debug, Clone)]
pub struct Transition {
    /// Properties to transition
    pub properties: TransitionProperties,
    /// Duration in milliseconds
    pub duration: u32,
    /// Easing curve
    pub easing: Easing,
    /// Delay
    pub delay: u32,
}

impl Transition {
    /// Create new transition for all properties
    pub fn all(duration: u32) -> Self {
        Self {
            properties: TransitionProperties::All,
            duration,
            easing: easing::EASE,
            delay: 0,
        }
    }

    /// Create transition for specific properties
    pub fn properties(props: &[TransitionProperty], duration: u32) -> Self {
        Self {
            properties: TransitionProperties::Specific(props.to_vec()),
            duration,
            easing: easing::EASE,
            delay: 0,
        }
    }

    /// Set easing
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Common fast transition
    pub fn fast() -> Self {
        Self::all(duration::FAST)
    }

    /// Common normal transition
    pub fn normal() -> Self {
        Self::all(duration::NORMAL)
    }

    /// Common slow transition
    pub fn slow() -> Self {
        Self::all(duration::SLOW)
    }
}

use alloc::vec::Vec;

/// Transition properties
#[derive(Debug, Clone)]
pub enum TransitionProperties {
    All,
    None,
    Specific(Vec<TransitionProperty>),
}

/// Individual transition property
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionProperty {
    Opacity,
    Transform,
    BackgroundColor,
    BorderColor,
    Color,
    Width,
    Height,
    Padding,
    Margin,
    BoxShadow,
}

/// Transform operations
#[derive(Debug, Clone)]
pub struct Transform {
    /// Translation X
    pub translate_x: f32,
    /// Translation Y
    pub translate_y: f32,
    /// Scale X
    pub scale_x: f32,
    /// Scale Y
    pub scale_y: f32,
    /// Rotation in degrees
    pub rotate: f32,
    /// Skew X
    pub skew_x: f32,
    /// Skew Y
    pub skew_y: f32,
}

impl Transform {
    /// Identity transform
    pub fn identity() -> Self {
        Self {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotate: 0.0,
            skew_x: 0.0,
            skew_y: 0.0,
        }
    }

    /// Translate
    pub fn translate(x: f32, y: f32) -> Self {
        Self {
            translate_x: x,
            translate_y: y,
            ..Self::identity()
        }
    }

    /// Scale uniform
    pub fn scale(scale: f32) -> Self {
        Self {
            scale_x: scale,
            scale_y: scale,
            ..Self::identity()
        }
    }

    /// Scale non-uniform
    pub fn scale_xy(x: f32, y: f32) -> Self {
        Self {
            scale_x: x,
            scale_y: y,
            ..Self::identity()
        }
    }

    /// Rotate
    pub fn rotate(degrees: f32) -> Self {
        Self {
            rotate: degrees,
            ..Self::identity()
        }
    }

    /// Combine with another transform
    pub fn then(mut self, other: Transform) -> Self {
        self.translate_x += other.translate_x;
        self.translate_y += other.translate_y;
        self.scale_x *= other.scale_x;
        self.scale_y *= other.scale_y;
        self.rotate += other.rotate;
        self.skew_x += other.skew_x;
        self.skew_y += other.skew_y;
        self
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

/// Preset animations
pub mod presets {
    use super::*;
    use super::super::tokens::{duration, easing};

    /// Fade in
    pub fn fade_in() -> Animation {
        Animation::new(duration::NORMAL)
            .easing(easing::EASE_OUT)
    }

    /// Fade out
    pub fn fade_out() -> Animation {
        Animation::new(duration::NORMAL)
            .easing(easing::EASE_IN)
    }

    /// Slide in from left
    pub fn slide_in_left() -> Animation {
        Animation::new(duration::SLOW)
            .easing(easing::SMOOTH)
    }

    /// Slide in from right
    pub fn slide_in_right() -> Animation {
        Animation::new(duration::SLOW)
            .easing(easing::SMOOTH)
    }

    /// Slide in from bottom
    pub fn slide_in_bottom() -> Animation {
        Animation::new(duration::SLOW)
            .easing(easing::SMOOTH)
    }

    /// Scale in (zoom)
    pub fn scale_in() -> Animation {
        Animation::new(duration::NORMAL)
            .easing(easing::BOUNCE_OUT)
    }

    /// Scale out
    pub fn scale_out() -> Animation {
        Animation::new(duration::FAST)
            .easing(easing::EASE_IN)
    }

    /// Bounce
    pub fn bounce() -> Animation {
        Animation::new(duration::SLOW)
            .easing(easing::BOUNCE_OUT)
    }

    /// Pulse (infinite)
    pub fn pulse() -> Animation {
        Animation::new(1000)
            .easing(easing::EASE_IN_OUT)
            .infinite()
            .alternate()
    }

    /// Spin (infinite)
    pub fn spin() -> Animation {
        Animation::new(1000)
            .easing(easing::LINEAR)
            .infinite()
    }

    /// Shake (error feedback)
    pub fn shake() -> Animation {
        Animation::new(400)
            .easing(easing::EASE_IN_OUT)
    }
}

/// Keyframe animation
#[derive(Debug, Clone)]
pub struct KeyframeAnimation {
    /// Keyframes
    pub keyframes: Vec<Keyframe>,
    /// Duration
    pub duration: u32,
    /// Iterations
    pub iterations: u32,
    /// Direction
    pub direction: AnimationDirection,
}

/// Single keyframe
#[derive(Debug, Clone)]
pub struct Keyframe {
    /// Position (0.0 to 1.0)
    pub position: f32,
    /// Opacity
    pub opacity: Option<f32>,
    /// Transform
    pub transform: Option<Transform>,
    /// Easing to next keyframe
    pub easing: Option<Easing>,
}

impl Keyframe {
    /// Create keyframe at position
    pub fn at(position: f32) -> Self {
        Self {
            position: position.max(0.0).min(1.0),
            opacity: None,
            transform: None,
            easing: None,
        }
    }

    /// Set opacity
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = Some(opacity);
        self
    }

    /// Set transform
    pub fn transform(mut self, transform: Transform) -> Self {
        self.transform = Some(transform);
        self
    }
}

impl KeyframeAnimation {
    /// Create new keyframe animation
    pub fn new(duration: u32) -> Self {
        Self {
            keyframes: Vec::new(),
            duration,
            iterations: 1,
            direction: AnimationDirection::Normal,
        }
    }

    /// Add keyframe
    pub fn keyframe(mut self, keyframe: Keyframe) -> Self {
        self.keyframes.push(keyframe);
        self.keyframes.sort_by(|a, b| {
            a.position.partial_cmp(&b.position).unwrap_or(core::cmp::Ordering::Equal)
        });
        self
    }

    /// Set infinite
    pub fn infinite(mut self) -> Self {
        self.iterations = 0;
        self
    }
}
