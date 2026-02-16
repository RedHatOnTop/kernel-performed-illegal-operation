//! CSS Animation Module
//!
//! This module provides CSS animations and transitions support.
//! Implements keyframe animations, transitions, and timing functions.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::f32::consts::PI;

/// Animation timing function (easing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimingFunction {
    /// Linear interpolation.
    Linear,
    /// Ease (default CSS ease).
    Ease,
    /// Ease in (slow start).
    EaseIn,
    /// Ease out (slow end).
    EaseOut,
    /// Ease in and out.
    EaseInOut,
    /// Step function.
    Steps(u32, StepPosition),
    /// Cubic bezier curve.
    CubicBezier(f32, f32, f32, f32),
}

impl TimingFunction {
    /// Calculate the output value for a given input (0.0 to 1.0).
    pub fn calculate(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);

        match self {
            TimingFunction::Linear => t,
            TimingFunction::Ease => Self::cubic_bezier(0.25, 0.1, 0.25, 1.0, t),
            TimingFunction::EaseIn => Self::cubic_bezier(0.42, 0.0, 1.0, 1.0, t),
            TimingFunction::EaseOut => Self::cubic_bezier(0.0, 0.0, 0.58, 1.0, t),
            TimingFunction::EaseInOut => Self::cubic_bezier(0.42, 0.0, 0.58, 1.0, t),
            TimingFunction::Steps(steps, position) => Self::step_function(*steps, *position, t),
            TimingFunction::CubicBezier(x1, y1, x2, y2) => {
                Self::cubic_bezier(*x1, *y1, *x2, *y2, t)
            }
        }
    }

    /// Cubic bezier interpolation.
    fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
        // Newton-Raphson iteration to find t for x
        let mut guess = t;
        for _ in 0..8 {
            let x = Self::bezier_component(x1, x2, guess);
            let dx = Self::bezier_derivative(x1, x2, guess);
            if dx.abs() < 1e-6 {
                break;
            }
            guess -= (x - t) / dx;
            guess = guess.clamp(0.0, 1.0);
        }

        Self::bezier_component(y1, y2, guess)
    }

    /// Calculate bezier component.
    fn bezier_component(p1: f32, p2: f32, t: f32) -> f32 {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        3.0 * mt2 * t * p1 + 3.0 * mt * t2 * p2 + t3
    }

    /// Calculate bezier derivative.
    fn bezier_derivative(p1: f32, p2: f32, t: f32) -> f32 {
        let t2 = t * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;

        3.0 * mt2 * p1 + 6.0 * mt * t * (p2 - p1) + 3.0 * t2 * (1.0 - p2)
    }

    /// Step function interpolation.
    fn step_function(steps: u32, position: StepPosition, t: f32) -> f32 {
        let steps = steps.max(1) as f32;

        match position {
            StepPosition::Start | StepPosition::JumpStart => libm::ceilf(t * steps) / steps,
            StepPosition::End | StepPosition::JumpEnd => libm::floorf(t * steps) / steps,
            StepPosition::JumpNone => {
                let step = libm::floorf(t * steps);
                step / (steps - 1.0).max(1.0)
            }
            StepPosition::JumpBoth => {
                let step = libm::floorf(t * (steps + 1.0)).min(steps);
                step / steps
            }
        }
    }

    /// Parse from CSS string.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_ascii_lowercase();

        match s.as_str() {
            "linear" => Some(TimingFunction::Linear),
            "ease" => Some(TimingFunction::Ease),
            "ease-in" => Some(TimingFunction::EaseIn),
            "ease-out" => Some(TimingFunction::EaseOut),
            "ease-in-out" => Some(TimingFunction::EaseInOut),
            "step-start" => Some(TimingFunction::Steps(1, StepPosition::Start)),
            "step-end" => Some(TimingFunction::Steps(1, StepPosition::End)),
            _ if s.starts_with("steps(") => Self::parse_steps(&s),
            _ if s.starts_with("cubic-bezier(") => Self::parse_cubic_bezier(&s),
            _ => None,
        }
    }

    fn parse_steps(s: &str) -> Option<Self> {
        let inner = s.strip_prefix("steps(")?.strip_suffix(')')?;
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();

        let steps: u32 = parts.first()?.parse().ok()?;
        let position = if parts.len() > 1 {
            match parts[1] {
                "start" | "jump-start" => StepPosition::Start,
                "end" | "jump-end" => StepPosition::End,
                "jump-none" => StepPosition::JumpNone,
                "jump-both" => StepPosition::JumpBoth,
                _ => StepPosition::End,
            }
        } else {
            StepPosition::End
        };

        Some(TimingFunction::Steps(steps, position))
    }

    fn parse_cubic_bezier(s: &str) -> Option<Self> {
        let inner = s.strip_prefix("cubic-bezier(")?.strip_suffix(')')?;
        let parts: Vec<f32> = inner
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        if parts.len() == 4 {
            Some(TimingFunction::CubicBezier(
                parts[0], parts[1], parts[2], parts[3],
            ))
        } else {
            None
        }
    }
}

impl Default for TimingFunction {
    fn default() -> Self {
        TimingFunction::Ease
    }
}

/// Step position for step timing functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepPosition {
    Start,
    End,
    JumpStart,
    JumpEnd,
    JumpNone,
    JumpBoth,
}

/// Animation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationDirection {
    /// Normal direction.
    Normal,
    /// Reverse direction.
    Reverse,
    /// Alternate between normal and reverse.
    Alternate,
    /// Alternate starting with reverse.
    AlternateReverse,
}

impl AnimationDirection {
    /// Parse from CSS string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "normal" => Some(AnimationDirection::Normal),
            "reverse" => Some(AnimationDirection::Reverse),
            "alternate" => Some(AnimationDirection::Alternate),
            "alternate-reverse" => Some(AnimationDirection::AlternateReverse),
            _ => None,
        }
    }
}

impl Default for AnimationDirection {
    fn default() -> Self {
        AnimationDirection::Normal
    }
}

/// Animation fill mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillMode {
    /// No fill.
    None,
    /// Forward fill (keep end state).
    Forwards,
    /// Backward fill (apply start state during delay).
    Backwards,
    /// Both forward and backward.
    Both,
}

impl FillMode {
    /// Parse from CSS string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" => Some(FillMode::None),
            "forwards" => Some(FillMode::Forwards),
            "backwards" => Some(FillMode::Backwards),
            "both" => Some(FillMode::Both),
            _ => None,
        }
    }
}

impl Default for FillMode {
    fn default() -> Self {
        FillMode::None
    }
}

/// Animation play state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    Running,
    Paused,
}

impl Default for PlayState {
    fn default() -> Self {
        PlayState::Running
    }
}

/// A single keyframe in an animation.
#[derive(Debug, Clone)]
pub struct Keyframe {
    /// Offset (0.0 to 1.0, representing 0% to 100%).
    pub offset: f32,
    /// Properties at this keyframe.
    pub properties: BTreeMap<String, AnimatableValue>,
    /// Timing function to use from this keyframe to the next.
    pub timing_function: Option<TimingFunction>,
}

impl Keyframe {
    /// Create a new keyframe.
    pub fn new(offset: f32) -> Self {
        Self {
            offset: offset.clamp(0.0, 1.0),
            properties: BTreeMap::new(),
            timing_function: None,
        }
    }

    /// Add a property.
    pub fn property(mut self, name: &str, value: AnimatableValue) -> Self {
        self.properties.insert(name.to_string(), value);
        self
    }

    /// Set timing function.
    pub fn timing(mut self, timing: TimingFunction) -> Self {
        self.timing_function = Some(timing);
        self
    }
}

/// Values that can be animated.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimatableValue {
    /// Number (opacity, etc.).
    Number(f32),
    /// Length in pixels.
    Length(f32),
    /// Percentage.
    Percentage(f32),
    /// Color (ARGB).
    Color(u32),
    /// Transform (simplified).
    Transform(Transform),
}

impl AnimatableValue {
    /// Interpolate between two values.
    pub fn interpolate(&self, other: &AnimatableValue, t: f32) -> Option<AnimatableValue> {
        match (self, other) {
            (AnimatableValue::Number(a), AnimatableValue::Number(b)) => {
                Some(AnimatableValue::Number(a + (b - a) * t))
            }
            (AnimatableValue::Length(a), AnimatableValue::Length(b)) => {
                Some(AnimatableValue::Length(a + (b - a) * t))
            }
            (AnimatableValue::Percentage(a), AnimatableValue::Percentage(b)) => {
                Some(AnimatableValue::Percentage(a + (b - a) * t))
            }
            (AnimatableValue::Color(a), AnimatableValue::Color(b)) => {
                Some(AnimatableValue::Color(Self::interpolate_color(*a, *b, t)))
            }
            (AnimatableValue::Transform(a), AnimatableValue::Transform(b)) => {
                Some(AnimatableValue::Transform(a.interpolate(b, t)))
            }
            _ => None,
        }
    }

    /// Interpolate colors.
    fn interpolate_color(from: u32, to: u32, t: f32) -> u32 {
        let fa = ((from >> 24) & 0xFF) as f32;
        let fr = ((from >> 16) & 0xFF) as f32;
        let fg = ((from >> 8) & 0xFF) as f32;
        let fb = (from & 0xFF) as f32;

        let ta = ((to >> 24) & 0xFF) as f32;
        let tr = ((to >> 16) & 0xFF) as f32;
        let tg = ((to >> 8) & 0xFF) as f32;
        let tb = (to & 0xFF) as f32;

        let a = (fa + (ta - fa) * t) as u32;
        let r = (fr + (tr - fr) * t) as u32;
        let g = (fg + (tg - fg) * t) as u32;
        let b = (fb + (tb - fb) * t) as u32;

        (a << 24) | (r << 16) | (g << 8) | b
    }
}

/// CSS Transform.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub translate_x: f32,
    pub translate_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotate: f32, // radians
    pub skew_x: f32,
    pub skew_y: f32,
}

impl Transform {
    /// Identity transform.
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

    /// Translation.
    pub fn translate(x: f32, y: f32) -> Self {
        let mut t = Self::identity();
        t.translate_x = x;
        t.translate_y = y;
        t
    }

    /// Scale.
    pub fn scale(x: f32, y: f32) -> Self {
        let mut t = Self::identity();
        t.scale_x = x;
        t.scale_y = y;
        t
    }

    /// Rotation (in degrees).
    pub fn rotate_deg(degrees: f32) -> Self {
        let mut t = Self::identity();
        t.rotate = degrees * PI / 180.0;
        t
    }

    /// Interpolate between transforms.
    pub fn interpolate(&self, other: &Transform, t: f32) -> Transform {
        Transform {
            translate_x: self.translate_x + (other.translate_x - self.translate_x) * t,
            translate_y: self.translate_y + (other.translate_y - self.translate_y) * t,
            scale_x: self.scale_x + (other.scale_x - self.scale_x) * t,
            scale_y: self.scale_y + (other.scale_y - self.scale_y) * t,
            rotate: self.rotate + (other.rotate - self.rotate) * t,
            skew_x: self.skew_x + (other.skew_x - self.skew_x) * t,
            skew_y: self.skew_y + (other.skew_y - self.skew_y) * t,
        }
    }

    /// Convert to 2D matrix [a, b, c, d, e, f].
    pub fn to_matrix(&self) -> [f32; 6] {
        let cos_r = libm::cosf(self.rotate);
        let sin_r = libm::sinf(self.rotate);

        // Scale * Rotate
        let a = self.scale_x * cos_r;
        let b = self.scale_x * sin_r;
        let c = -self.scale_y * sin_r;
        let d = self.scale_y * cos_r;

        [a, b, c, d, self.translate_x, self.translate_y]
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

/// Keyframe animation definition.
#[derive(Debug, Clone)]
pub struct KeyframeAnimation {
    /// Animation name.
    pub name: String,
    /// Keyframes.
    pub keyframes: Vec<Keyframe>,
}

impl KeyframeAnimation {
    /// Create a new animation.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            keyframes: Vec::new(),
        }
    }

    /// Add a keyframe.
    pub fn keyframe(mut self, keyframe: Keyframe) -> Self {
        self.keyframes.push(keyframe);
        self.keyframes
            .sort_by(|a, b| a.offset.partial_cmp(&b.offset).unwrap());
        self
    }

    /// Get interpolated value at a given time (0.0 to 1.0).
    pub fn get_value(
        &self,
        property: &str,
        t: f32,
        timing: &TimingFunction,
    ) -> Option<AnimatableValue> {
        if self.keyframes.is_empty() {
            return None;
        }

        // Find the two keyframes to interpolate between
        let mut from_kf: Option<&Keyframe> = None;
        let mut to_kf: Option<&Keyframe> = None;

        for kf in &self.keyframes {
            if kf.offset <= t {
                from_kf = Some(kf);
            }
            if kf.offset >= t && to_kf.is_none() {
                to_kf = Some(kf);
            }
        }

        match (from_kf, to_kf) {
            (Some(from), Some(to)) if from.offset != to.offset => {
                let from_val = from.properties.get(property)?;
                let to_val = to.properties.get(property)?;

                let local_t = (t - from.offset) / (to.offset - from.offset);
                let eased_t = from
                    .timing_function
                    .as_ref()
                    .unwrap_or(timing)
                    .calculate(local_t);

                from_val.interpolate(to_val, eased_t)
            }
            (Some(kf), _) | (_, Some(kf)) => kf.properties.get(property).cloned(),
            _ => None,
        }
    }
}

/// Animation instance (running animation).
#[derive(Debug, Clone)]
pub struct Animation {
    /// Animation definition name.
    pub name: String,
    /// Duration in milliseconds.
    pub duration_ms: u32,
    /// Delay in milliseconds.
    pub delay_ms: u32,
    /// Timing function.
    pub timing_function: TimingFunction,
    /// Iteration count (None = infinite).
    pub iteration_count: Option<f32>,
    /// Direction.
    pub direction: AnimationDirection,
    /// Fill mode.
    pub fill_mode: FillMode,
    /// Play state.
    pub play_state: PlayState,
    /// Current time in milliseconds.
    current_time_ms: u32,
    /// Number of completed iterations.
    iterations_completed: f32,
    /// Is animation finished?
    finished: bool,
}

impl Animation {
    /// Create a new animation.
    pub fn new(name: &str, duration_ms: u32) -> Self {
        Self {
            name: name.to_string(),
            duration_ms,
            delay_ms: 0,
            timing_function: TimingFunction::Ease,
            iteration_count: Some(1.0),
            direction: AnimationDirection::Normal,
            fill_mode: FillMode::None,
            play_state: PlayState::Running,
            current_time_ms: 0,
            iterations_completed: 0.0,
            finished: false,
        }
    }

    /// Set delay.
    pub fn delay(mut self, delay_ms: u32) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    /// Set timing function.
    pub fn timing(mut self, timing: TimingFunction) -> Self {
        self.timing_function = timing;
        self
    }

    /// Set iteration count.
    pub fn iterations(mut self, count: Option<f32>) -> Self {
        self.iteration_count = count;
        self
    }

    /// Set direction.
    pub fn direction(mut self, direction: AnimationDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Set fill mode.
    pub fn fill(mut self, fill: FillMode) -> Self {
        self.fill_mode = fill;
        self
    }

    /// Update animation.
    pub fn update(&mut self, delta_ms: u32) {
        if self.play_state == PlayState::Paused || self.finished {
            return;
        }

        self.current_time_ms += delta_ms;

        // Check if past delay
        if self.current_time_ms < self.delay_ms {
            return;
        }

        let active_time = self.current_time_ms - self.delay_ms;

        if self.duration_ms > 0 {
            self.iterations_completed = active_time as f32 / self.duration_ms as f32;

            if let Some(max_iterations) = self.iteration_count {
                if self.iterations_completed >= max_iterations {
                    self.iterations_completed = max_iterations;
                    self.finished = true;
                }
            }
        } else {
            self.iterations_completed = 1.0;
            self.finished = true;
        }
    }

    /// Get current progress (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        if self.current_time_ms < self.delay_ms {
            return match self.fill_mode {
                FillMode::Backwards | FillMode::Both => 0.0,
                _ => 0.0,
            };
        }

        if self.finished {
            return match self.fill_mode {
                FillMode::Forwards | FillMode::Both => 1.0,
                _ => 0.0,
            };
        }

        let floor_val = libm::floorf(self.iterations_completed);
        let iteration_progress = self.iterations_completed - floor_val;
        let current_iteration = floor_val as u32;

        // Apply direction
        let should_reverse = match self.direction {
            AnimationDirection::Normal => false,
            AnimationDirection::Reverse => true,
            AnimationDirection::Alternate => current_iteration % 2 == 1,
            AnimationDirection::AlternateReverse => current_iteration % 2 == 0,
        };

        if should_reverse {
            1.0 - iteration_progress
        } else {
            iteration_progress
        }
    }

    /// Check if animation is finished.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Check if animation is active (past delay, not finished or has fill).
    pub fn is_active(&self) -> bool {
        if self.current_time_ms >= self.delay_ms && !self.finished {
            return true;
        }

        match self.fill_mode {
            FillMode::Backwards | FillMode::Both if self.current_time_ms < self.delay_ms => true,
            FillMode::Forwards | FillMode::Both if self.finished => true,
            _ => false,
        }
    }

    /// Pause the animation.
    pub fn pause(&mut self) {
        self.play_state = PlayState::Paused;
    }

    /// Resume the animation.
    pub fn resume(&mut self) {
        self.play_state = PlayState::Running;
    }

    /// Reset the animation.
    pub fn reset(&mut self) {
        self.current_time_ms = 0;
        self.iterations_completed = 0.0;
        self.finished = false;
    }
}

/// CSS Transition.
#[derive(Debug, Clone)]
pub struct Transition {
    /// Property being transitioned.
    pub property: String,
    /// Duration in milliseconds.
    pub duration_ms: u32,
    /// Delay in milliseconds.
    pub delay_ms: u32,
    /// Timing function.
    pub timing_function: TimingFunction,
    /// Start value.
    pub from: AnimatableValue,
    /// End value.
    pub to: AnimatableValue,
    /// Current time in milliseconds.
    current_time_ms: u32,
    /// Is transition finished?
    finished: bool,
}

impl Transition {
    /// Create a new transition.
    pub fn new(
        property: &str,
        from: AnimatableValue,
        to: AnimatableValue,
        duration_ms: u32,
    ) -> Self {
        Self {
            property: property.to_string(),
            duration_ms,
            delay_ms: 0,
            timing_function: TimingFunction::Ease,
            from,
            to,
            current_time_ms: 0,
            finished: false,
        }
    }

    /// Set delay.
    pub fn delay(mut self, delay_ms: u32) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    /// Set timing function.
    pub fn timing(mut self, timing: TimingFunction) -> Self {
        self.timing_function = timing;
        self
    }

    /// Update transition.
    pub fn update(&mut self, delta_ms: u32) {
        if self.finished {
            return;
        }

        self.current_time_ms += delta_ms;

        if self.current_time_ms >= self.delay_ms + self.duration_ms {
            self.finished = true;
        }
    }

    /// Get current value.
    pub fn current_value(&self) -> AnimatableValue {
        if self.current_time_ms < self.delay_ms {
            return self.from.clone();
        }

        if self.finished {
            return self.to.clone();
        }

        let elapsed = self.current_time_ms - self.delay_ms;
        let t = elapsed as f32 / self.duration_ms as f32;
        let eased_t = self.timing_function.calculate(t);

        self.from
            .interpolate(&self.to, eased_t)
            .unwrap_or_else(|| self.to.clone())
    }

    /// Check if transition is finished.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Get progress (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        if self.current_time_ms < self.delay_ms {
            return 0.0;
        }

        let elapsed = self.current_time_ms - self.delay_ms;
        (elapsed as f32 / self.duration_ms as f32).min(1.0)
    }
}

/// Animation controller for managing multiple animations.
pub struct AnimationController {
    /// Keyframe animation definitions.
    definitions: BTreeMap<String, KeyframeAnimation>,
    /// Active animations.
    animations: Vec<Animation>,
    /// Active transitions.
    transitions: Vec<Transition>,
}

impl AnimationController {
    /// Create a new animation controller.
    pub fn new() -> Self {
        Self {
            definitions: BTreeMap::new(),
            animations: Vec::new(),
            transitions: Vec::new(),
        }
    }

    /// Register a keyframe animation.
    pub fn register(&mut self, animation: KeyframeAnimation) {
        self.definitions.insert(animation.name.clone(), animation);
    }

    /// Start an animation.
    pub fn start_animation(&mut self, animation: Animation) {
        self.animations.push(animation);
    }

    /// Start a transition.
    pub fn start_transition(&mut self, transition: Transition) {
        // Remove any existing transition for the same property
        self.transitions
            .retain(|t| t.property != transition.property);
        self.transitions.push(transition);
    }

    /// Update all animations.
    pub fn update(&mut self, delta_ms: u32) {
        for anim in &mut self.animations {
            anim.update(delta_ms);
        }

        for trans in &mut self.transitions {
            trans.update(delta_ms);
        }

        // Remove finished animations and transitions
        self.animations
            .retain(|a| !a.is_finished() || a.fill_mode != FillMode::None);
        self.transitions.retain(|t| !t.is_finished());
    }

    /// Get current value for a property.
    pub fn get_value(&self, property: &str) -> Option<AnimatableValue> {
        // Check transitions first
        for trans in &self.transitions {
            if trans.property == property {
                return Some(trans.current_value());
            }
        }

        // Check animations
        for anim in &self.animations {
            if anim.is_active() {
                if let Some(def) = self.definitions.get(&anim.name) {
                    let t = anim.progress();
                    if let Some(value) = def.get_value(property, t, &anim.timing_function) {
                        return Some(value);
                    }
                }
            }
        }

        None
    }

    /// Check if any animations are active.
    pub fn has_active_animations(&self) -> bool {
        !self.animations.is_empty() || !self.transitions.is_empty()
    }

    /// Clear all animations and transitions.
    pub fn clear(&mut self) {
        self.animations.clear();
        self.transitions.clear();
    }
}

impl Default for AnimationController {
    fn default() -> Self {
        Self::new()
    }
}

/// Create common animations.
pub mod presets {
    use super::*;

    /// Fade in animation.
    pub fn fade_in() -> KeyframeAnimation {
        KeyframeAnimation::new("fadeIn")
            .keyframe(Keyframe::new(0.0).property("opacity", AnimatableValue::Number(0.0)))
            .keyframe(Keyframe::new(1.0).property("opacity", AnimatableValue::Number(1.0)))
    }

    /// Fade out animation.
    pub fn fade_out() -> KeyframeAnimation {
        KeyframeAnimation::new("fadeOut")
            .keyframe(Keyframe::new(0.0).property("opacity", AnimatableValue::Number(1.0)))
            .keyframe(Keyframe::new(1.0).property("opacity", AnimatableValue::Number(0.0)))
    }

    /// Slide in from left.
    pub fn slide_in_left() -> KeyframeAnimation {
        KeyframeAnimation::new("slideInLeft")
            .keyframe(
                Keyframe::new(0.0)
                    .property(
                        "transform",
                        AnimatableValue::Transform(Transform::translate(-100.0, 0.0)),
                    )
                    .property("opacity", AnimatableValue::Number(0.0)),
            )
            .keyframe(
                Keyframe::new(1.0)
                    .property(
                        "transform",
                        AnimatableValue::Transform(Transform::identity()),
                    )
                    .property("opacity", AnimatableValue::Number(1.0)),
            )
    }

    /// Slide in from right.
    pub fn slide_in_right() -> KeyframeAnimation {
        KeyframeAnimation::new("slideInRight")
            .keyframe(
                Keyframe::new(0.0)
                    .property(
                        "transform",
                        AnimatableValue::Transform(Transform::translate(100.0, 0.0)),
                    )
                    .property("opacity", AnimatableValue::Number(0.0)),
            )
            .keyframe(
                Keyframe::new(1.0)
                    .property(
                        "transform",
                        AnimatableValue::Transform(Transform::identity()),
                    )
                    .property("opacity", AnimatableValue::Number(1.0)),
            )
    }

    /// Scale in animation.
    pub fn scale_in() -> KeyframeAnimation {
        KeyframeAnimation::new("scaleIn")
            .keyframe(
                Keyframe::new(0.0)
                    .property(
                        "transform",
                        AnimatableValue::Transform(Transform::scale(0.0, 0.0)),
                    )
                    .property("opacity", AnimatableValue::Number(0.0)),
            )
            .keyframe(
                Keyframe::new(1.0)
                    .property(
                        "transform",
                        AnimatableValue::Transform(Transform::identity()),
                    )
                    .property("opacity", AnimatableValue::Number(1.0)),
            )
    }

    /// Bounce animation.
    pub fn bounce() -> KeyframeAnimation {
        KeyframeAnimation::new("bounce")
            .keyframe(Keyframe::new(0.0).property(
                "transform",
                AnimatableValue::Transform(Transform::translate(0.0, 0.0)),
            ))
            .keyframe(Keyframe::new(0.2).property(
                "transform",
                AnimatableValue::Transform(Transform::translate(0.0, -30.0)),
            ))
            .keyframe(Keyframe::new(0.4).property(
                "transform",
                AnimatableValue::Transform(Transform::translate(0.0, 0.0)),
            ))
            .keyframe(Keyframe::new(0.6).property(
                "transform",
                AnimatableValue::Transform(Transform::translate(0.0, -15.0)),
            ))
            .keyframe(Keyframe::new(0.8).property(
                "transform",
                AnimatableValue::Transform(Transform::translate(0.0, 0.0)),
            ))
            .keyframe(Keyframe::new(1.0).property(
                "transform",
                AnimatableValue::Transform(Transform::translate(0.0, 0.0)),
            ))
    }

    /// Pulse animation.
    pub fn pulse() -> KeyframeAnimation {
        KeyframeAnimation::new("pulse")
            .keyframe(Keyframe::new(0.0).property(
                "transform",
                AnimatableValue::Transform(Transform::scale(1.0, 1.0)),
            ))
            .keyframe(Keyframe::new(0.5).property(
                "transform",
                AnimatableValue::Transform(Transform::scale(1.1, 1.1)),
            ))
            .keyframe(Keyframe::new(1.0).property(
                "transform",
                AnimatableValue::Transform(Transform::scale(1.0, 1.0)),
            ))
    }

    /// Spin animation.
    pub fn spin() -> KeyframeAnimation {
        KeyframeAnimation::new("spin")
            .keyframe(Keyframe::new(0.0).property(
                "transform",
                AnimatableValue::Transform(Transform::rotate_deg(0.0)),
            ))
            .keyframe(Keyframe::new(1.0).property(
                "transform",
                AnimatableValue::Transform(Transform::rotate_deg(360.0)),
            ))
    }
}
