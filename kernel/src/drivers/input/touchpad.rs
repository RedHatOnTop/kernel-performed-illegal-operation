//! Touchpad Driver
//!
//! Driver for touchpad input with gesture support.

use super::{
    GestureEvent, GesturePhase, GestureType, InputDevice, InputDeviceType, InputEvent,
    InputEventData, InputEventType, MouseButton, MouseButtonEvent, MouseMoveEvent,
    MouseScrollEvent, SwipeDirection, TouchEvent, TouchPhase,
};
use alloc::string::String;
use alloc::vec::Vec;

/// Simple sqrt for f32 using Newton's method
fn sqrt_f32(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut guess = x / 2.0;
    for _ in 0..10 {
        guess = (guess + x / guess) / 2.0;
    }
    guess
}

/// Simple abs for f32
fn abs_f32(x: f32) -> f32 {
    if x < 0.0 {
        -x
    } else {
        x
    }
}

/// Touchpad finger state
#[derive(Debug, Clone, Copy, Default)]
struct FingerState {
    /// Is finger touching
    touching: bool,
    /// X position (0.0 to 1.0)
    x: f32,
    /// Y position (0.0 to 1.0)
    y: f32,
    /// Pressure
    pressure: f32,
    /// Previous X
    prev_x: f32,
    /// Previous Y
    prev_y: f32,
    /// Touch start time
    start_time: u64,
    /// Touch start position
    start_x: f32,
    start_y: f32,
}

/// Touchpad configuration
#[derive(Debug, Clone)]
pub struct TouchpadConfig {
    /// Enable tap to click
    pub tap_to_click: bool,
    /// Two-finger tap for right click
    pub two_finger_tap_right_click: bool,
    /// Two-finger scroll
    pub two_finger_scroll: bool,
    /// Natural scrolling (reverse)
    pub natural_scrolling: bool,
    /// Scroll speed multiplier
    pub scroll_speed: f32,
    /// Pointer speed multiplier
    pub pointer_speed: f32,
    /// Tap timeout in milliseconds
    pub tap_timeout_ms: u64,
    /// Three-finger gestures
    pub three_finger_gestures: bool,
    /// Four-finger gestures  
    pub four_finger_gestures: bool,
    /// Pinch to zoom
    pub pinch_to_zoom: bool,
}

impl Default for TouchpadConfig {
    fn default() -> Self {
        Self {
            tap_to_click: true,
            two_finger_tap_right_click: true,
            two_finger_scroll: true,
            natural_scrolling: true,
            scroll_speed: 1.0,
            pointer_speed: 1.0,
            tap_timeout_ms: 200,
            three_finger_gestures: true,
            four_finger_gestures: true,
            pinch_to_zoom: true,
        }
    }
}

/// Touchpad driver
pub struct Touchpad {
    /// Device name
    name: String,
    /// Configuration
    config: TouchpadConfig,
    /// Finger states (up to 5 fingers)
    fingers: [FingerState; 5],
    /// Number of fingers currently touching
    finger_count: u8,
    /// Screen width for coordinate mapping
    screen_width: i32,
    /// Screen height for coordinate mapping
    screen_height: i32,
    /// Current pointer X
    pointer_x: i32,
    /// Current pointer Y
    pointer_y: i32,
    /// Left button held (for dragging)
    left_button_down: bool,
    /// Pending events
    pending_events: Vec<InputEvent>,
    /// Gesture tracking state
    gesture_state: GestureState,
}

/// Gesture recognition state
#[derive(Debug, Clone, Default)]
struct GestureState {
    /// Active gesture type
    active_gesture: Option<GestureType>,
    /// Initial finger positions for gesture
    initial_fingers: [(f32, f32); 5],
    /// Initial distance between fingers (for pinch)
    initial_distance: f32,
    /// Initial angle between fingers (for rotate)
    initial_angle: f32,
    /// Cumulative scroll delta
    scroll_accumulator: (f32, f32),
}

impl Touchpad {
    /// Create a new touchpad
    pub fn new(name: &str, screen_width: i32, screen_height: i32) -> Self {
        Self {
            name: String::from(name),
            config: TouchpadConfig::default(),
            fingers: [FingerState::default(); 5],
            finger_count: 0,
            screen_width,
            screen_height,
            pointer_x: screen_width / 2,
            pointer_y: screen_height / 2,
            left_button_down: false,
            pending_events: Vec::new(),
            gesture_state: GestureState::default(),
        }
    }

    /// Update finger state
    pub fn update_finger(
        &mut self,
        slot: u8,
        touching: bool,
        x: f32,
        y: f32,
        pressure: f32,
        timestamp: u64,
    ) {
        if slot as usize >= self.fingers.len() {
            return;
        }

        let finger = &mut self.fingers[slot as usize];
        let was_touching = finger.touching;

        finger.prev_x = finger.x;
        finger.prev_y = finger.y;
        finger.touching = touching;
        finger.x = x;
        finger.y = y;
        finger.pressure = pressure;

        if touching && !was_touching {
            // Finger down
            finger.start_time = timestamp;
            finger.start_x = x;
            finger.start_y = y;
            self.finger_count += 1;
            self.on_finger_down(slot, timestamp);
        } else if !touching && was_touching {
            // Finger up
            self.finger_count = self.finger_count.saturating_sub(1);
            self.on_finger_up(slot, timestamp);
        } else if touching {
            // Finger moved
            self.on_finger_move(slot, timestamp);
        }
    }

    /// Handle finger down event
    fn on_finger_down(&mut self, slot: u8, timestamp: u64) {
        // Generate touch event
        let finger = &self.fingers[slot as usize];
        self.pending_events.push(InputEvent {
            event_type: InputEventType::Touch,
            timestamp,
            data: InputEventData::Touch(TouchEvent {
                id: slot as u32,
                phase: TouchPhase::Started,
                x: finger.x,
                y: finger.y,
                pressure: finger.pressure,
            }),
        });

        // Initialize gesture if multiple fingers
        if self.finger_count >= 2 {
            self.start_multi_finger_gesture();
        }
    }

    /// Handle finger up event
    fn on_finger_up(&mut self, slot: u8, timestamp: u64) {
        let finger = &self.fingers[slot as usize];

        // Generate touch event
        self.pending_events.push(InputEvent {
            event_type: InputEventType::Touch,
            timestamp,
            data: InputEventData::Touch(TouchEvent {
                id: slot as u32,
                phase: TouchPhase::Ended,
                x: finger.x,
                y: finger.y,
                pressure: 0.0,
            }),
        });

        // Check for tap
        if self.config.tap_to_click && slot == 0 {
            let duration = timestamp.saturating_sub(finger.start_time);
            let dx = finger.x - finger.start_x;
            let dy = finger.y - finger.start_y;
            let distance = sqrt_f32(dx * dx + dy * dy);

            if duration < self.config.tap_timeout_ms * 1000 && distance < 0.02 {
                // It's a tap!
                let button = if self.finger_count == 0 && self.config.two_finger_tap_right_click {
                    // Check if there was a second finger recently
                    MouseButton::Left
                } else {
                    MouseButton::Left
                };

                // Generate click (press and release)
                self.pending_events.push(InputEvent {
                    event_type: InputEventType::MouseButton,
                    timestamp,
                    data: InputEventData::MouseButton(MouseButtonEvent {
                        button,
                        pressed: true,
                        x: self.pointer_x,
                        y: self.pointer_y,
                    }),
                });
                self.pending_events.push(InputEvent {
                    event_type: InputEventType::MouseButton,
                    timestamp: timestamp + 50,
                    data: InputEventData::MouseButton(MouseButtonEvent {
                        button,
                        pressed: false,
                        x: self.pointer_x,
                        y: self.pointer_y,
                    }),
                });
            }
        }

        // End gesture if all fingers up
        if self.finger_count == 0 {
            self.end_gesture(timestamp);
        }
    }

    /// Handle finger move event
    fn on_finger_move(&mut self, slot: u8, timestamp: u64) {
        let finger = &self.fingers[slot as usize];

        // Generate touch event
        self.pending_events.push(InputEvent {
            event_type: InputEventType::Touch,
            timestamp,
            data: InputEventData::Touch(TouchEvent {
                id: slot as u32,
                phase: TouchPhase::Moved,
                x: finger.x,
                y: finger.y,
                pressure: finger.pressure,
            }),
        });

        // Handle based on finger count
        match self.finger_count {
            1 => self.handle_single_finger_move(timestamp),
            2 => self.handle_two_finger_move(timestamp),
            3 => self.handle_three_finger_move(timestamp),
            4 => self.handle_four_finger_move(timestamp),
            _ => {}
        }
    }

    /// Handle single finger movement (pointer movement)
    fn handle_single_finger_move(&mut self, timestamp: u64) {
        let finger = &self.fingers[0];
        let dx = (finger.x - finger.prev_x) * self.screen_width as f32 * self.config.pointer_speed;
        let dy = (finger.y - finger.prev_y) * self.screen_height as f32 * self.config.pointer_speed;

        self.pointer_x = (self.pointer_x + dx as i32).clamp(0, self.screen_width - 1);
        self.pointer_y = (self.pointer_y + dy as i32).clamp(0, self.screen_height - 1);

        self.pending_events.push(InputEvent {
            event_type: InputEventType::MouseMove,
            timestamp,
            data: InputEventData::MouseMove(MouseMoveEvent {
                dx: dx as i32,
                dy: dy as i32,
                x: self.pointer_x,
                y: self.pointer_y,
            }),
        });
    }

    /// Handle two-finger movement (scroll or pinch)
    fn handle_two_finger_move(&mut self, timestamp: u64) {
        if !self.config.two_finger_scroll {
            return;
        }

        let f1 = &self.fingers[0];
        let f2 = &self.fingers[1];

        // Calculate average movement for scroll
        let avg_dx = ((f1.x - f1.prev_x) + (f2.x - f2.prev_x)) / 2.0;
        let avg_dy = ((f1.y - f1.prev_y) + (f2.y - f2.prev_y)) / 2.0;

        // Apply natural scrolling if enabled
        let scroll_dy = if self.config.natural_scrolling {
            -avg_dy
        } else {
            avg_dy
        };
        let scroll_dx = if self.config.natural_scrolling {
            -avg_dx
        } else {
            avg_dx
        };

        // Accumulate and generate scroll events
        self.gesture_state.scroll_accumulator.0 += scroll_dx * self.config.scroll_speed * 20.0;
        self.gesture_state.scroll_accumulator.1 += scroll_dy * self.config.scroll_speed * 20.0;

        let scroll_x = self.gesture_state.scroll_accumulator.0 as i32;
        let scroll_y = self.gesture_state.scroll_accumulator.1 as i32;

        if scroll_x != 0 || scroll_y != 0 {
            self.gesture_state.scroll_accumulator.0 -= scroll_x as f32;
            self.gesture_state.scroll_accumulator.1 -= scroll_y as f32;

            self.pending_events.push(InputEvent {
                event_type: InputEventType::MouseScroll,
                timestamp,
                data: InputEventData::MouseScroll(MouseScrollEvent {
                    dx: scroll_x,
                    dy: scroll_y,
                }),
            });
        }

        // Check for pinch gesture
        if self.config.pinch_to_zoom {
            let dx = f1.x - f2.x;
            let dy = f1.y - f2.y;
            let current_distance = sqrt_f32(dx * dx + dy * dy);

            if self.gesture_state.initial_distance > 0.0 {
                let scale = current_distance / self.gesture_state.initial_distance;
                let center_x = (f1.x + f2.x) / 2.0;
                let center_y = (f1.y + f2.y) / 2.0;

                self.pending_events.push(InputEvent {
                    event_type: InputEventType::Gesture,
                    timestamp,
                    data: InputEventData::Gesture(GestureEvent {
                        gesture: GestureType::Pinch {
                            scale,
                            center_x,
                            center_y,
                        },
                        phase: GesturePhase::Changed,
                    }),
                });
            }
        }
    }

    /// Handle three-finger movement
    fn handle_three_finger_move(&mut self, timestamp: u64) {
        if !self.config.three_finger_gestures {
            return;
        }

        // Calculate average movement
        let avg_dx: f32 = self.fingers[0..3]
            .iter()
            .map(|f| f.x - f.prev_x)
            .sum::<f32>()
            / 3.0;
        let avg_dy: f32 = self.fingers[0..3]
            .iter()
            .map(|f| f.y - f.prev_y)
            .sum::<f32>()
            / 3.0;

        // Accumulate movement
        let total_dx: f32 = self.fingers[0..3]
            .iter()
            .map(|f| f.x - f.start_x)
            .sum::<f32>()
            / 3.0;
        let total_dy: f32 = self.fingers[0..3]
            .iter()
            .map(|f| f.y - f.start_y)
            .sum::<f32>()
            / 3.0;

        // Detect swipe direction
        let threshold = 0.1;
        if abs_f32(total_dx) > threshold || abs_f32(total_dy) > threshold {
            let direction = if abs_f32(total_dx) > abs_f32(total_dy) {
                if total_dx > 0.0 {
                    SwipeDirection::Right
                } else {
                    SwipeDirection::Left
                }
            } else {
                if total_dy > 0.0 {
                    SwipeDirection::Down
                } else {
                    SwipeDirection::Up
                }
            };

            self.pending_events.push(InputEvent {
                event_type: InputEventType::Gesture,
                timestamp,
                data: InputEventData::Gesture(GestureEvent {
                    gesture: GestureType::ThreeFingerSwipe { direction },
                    phase: GesturePhase::Changed,
                }),
            });
        }
    }

    /// Handle four-finger movement
    fn handle_four_finger_move(&mut self, timestamp: u64) {
        if !self.config.four_finger_gestures {
            return;
        }

        // Similar to three-finger but for desktop/app switching
        let total_dx: f32 = self.fingers[0..4]
            .iter()
            .map(|f| f.x - f.start_x)
            .sum::<f32>()
            / 4.0;
        let total_dy: f32 = self.fingers[0..4]
            .iter()
            .map(|f| f.y - f.start_y)
            .sum::<f32>()
            / 4.0;

        let threshold = 0.1;
        if abs_f32(total_dx) > threshold || abs_f32(total_dy) > threshold {
            let direction = if abs_f32(total_dx) > abs_f32(total_dy) {
                if total_dx > 0.0 {
                    SwipeDirection::Right
                } else {
                    SwipeDirection::Left
                }
            } else {
                if total_dy > 0.0 {
                    SwipeDirection::Down
                } else {
                    SwipeDirection::Up
                }
            };

            self.pending_events.push(InputEvent {
                event_type: InputEventType::Gesture,
                timestamp,
                data: InputEventData::Gesture(GestureEvent {
                    gesture: GestureType::FourFingerSwipe { direction },
                    phase: GesturePhase::Changed,
                }),
            });
        }
    }

    /// Start tracking multi-finger gesture
    fn start_multi_finger_gesture(&mut self) {
        // Store initial finger positions
        for (i, finger) in self.fingers.iter().enumerate() {
            self.gesture_state.initial_fingers[i] = (finger.x, finger.y);
        }

        // Calculate initial distance for pinch
        if self.finger_count >= 2 {
            let f1 = &self.fingers[0];
            let f2 = &self.fingers[1];
            let dx = f1.x - f2.x;
            let dy = f1.y - f2.y;
            self.gesture_state.initial_distance = sqrt_f32(dx * dx + dy * dy);
        }

        self.gesture_state.scroll_accumulator = (0.0, 0.0);
    }

    /// End gesture tracking
    fn end_gesture(&mut self, timestamp: u64) {
        if let Some(ref gesture) = self.gesture_state.active_gesture {
            self.pending_events.push(InputEvent {
                event_type: InputEventType::Gesture,
                timestamp,
                data: InputEventData::Gesture(GestureEvent {
                    gesture: gesture.clone(),
                    phase: GesturePhase::Ended,
                }),
            });
        }
        self.gesture_state = GestureState::default();
    }

    /// Set configuration
    pub fn set_config(&mut self, config: TouchpadConfig) {
        self.config = config;
    }
}

impl InputDevice for Touchpad {
    fn name(&self) -> &str {
        &self.name
    }

    fn device_type(&self) -> InputDeviceType {
        InputDeviceType::Touchpad
    }

    fn poll(&mut self) -> Vec<InputEvent> {
        core::mem::take(&mut self.pending_events)
    }

    fn is_connected(&self) -> bool {
        true
    }
}
