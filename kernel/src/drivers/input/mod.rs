//! Input Device Drivers
//!
//! Keyboard, mouse, touchpad, and touchscreen support.

pub mod hid;
pub mod keyboard;
pub mod mouse;
pub mod touchpad;

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

/// Input event types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEventType {
    /// Key press/release
    Key,
    /// Mouse button press/release
    MouseButton,
    /// Mouse movement (relative)
    MouseMove,
    /// Mouse wheel scroll
    MouseScroll,
    /// Absolute position (touchscreen/touchpad absolute mode)
    AbsolutePosition,
    /// Touch event
    Touch,
    /// Gesture event
    Gesture,
}

/// Input event
#[derive(Debug, Clone)]
pub struct InputEvent {
    /// Event type
    pub event_type: InputEventType,
    /// Timestamp (microseconds since boot)
    pub timestamp: u64,
    /// Event-specific data
    pub data: InputEventData,
}

/// Input event data variants
#[derive(Debug, Clone)]
pub enum InputEventData {
    /// Keyboard event
    Key(KeyEvent),
    /// Mouse button event
    MouseButton(MouseButtonEvent),
    /// Mouse movement event
    MouseMove(MouseMoveEvent),
    /// Mouse scroll event
    MouseScroll(MouseScrollEvent),
    /// Absolute position event
    AbsolutePosition(AbsolutePositionEvent),
    /// Touch event
    Touch(TouchEvent),
    /// Gesture event
    Gesture(GestureEvent),
}

/// Keyboard event
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    /// Scan code (hardware-specific)
    pub scancode: u16,
    /// Key code (virtual key)
    pub keycode: KeyCode,
    /// Is key pressed (true) or released (false)
    pub pressed: bool,
    /// Active modifiers
    pub modifiers: Modifiers,
}

/// Mouse button event
#[derive(Debug, Clone, Copy)]
pub struct MouseButtonEvent {
    /// Button
    pub button: MouseButton,
    /// Is button pressed
    pub pressed: bool,
    /// Current X position
    pub x: i32,
    /// Current Y position
    pub y: i32,
}

/// Mouse movement event
#[derive(Debug, Clone, Copy)]
pub struct MouseMoveEvent {
    /// Relative X movement
    pub dx: i32,
    /// Relative Y movement
    pub dy: i32,
    /// Current X position
    pub x: i32,
    /// Current Y position
    pub y: i32,
}

/// Mouse scroll event
#[derive(Debug, Clone, Copy)]
pub struct MouseScrollEvent {
    /// Horizontal scroll (positive = right)
    pub dx: i32,
    /// Vertical scroll (positive = up)
    pub dy: i32,
}

/// Absolute position event (touchscreen/tablet)
#[derive(Debug, Clone, Copy)]
pub struct AbsolutePositionEvent {
    /// X position (0.0 to 1.0)
    pub x: f32,
    /// Y position (0.0 to 1.0)
    pub y: f32,
    /// Pressure (0.0 to 1.0)
    pub pressure: f32,
}

/// Touch event
#[derive(Debug, Clone, Copy)]
pub struct TouchEvent {
    /// Touch ID (for multi-touch)
    pub id: u32,
    /// Touch phase
    pub phase: TouchPhase,
    /// X position (0.0 to 1.0)
    pub x: f32,
    /// Y position (0.0 to 1.0)
    pub y: f32,
    /// Pressure (0.0 to 1.0)
    pub pressure: f32,
}

/// Touch phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    /// Finger touched screen
    Started,
    /// Finger moved
    Moved,
    /// Finger lifted
    Ended,
    /// Touch cancelled
    Cancelled,
}

/// Gesture event
#[derive(Debug, Clone)]
pub struct GestureEvent {
    /// Gesture type
    pub gesture: GestureType,
    /// Gesture phase
    pub phase: GesturePhase,
}

/// Gesture types
#[derive(Debug, Clone)]
pub enum GestureType {
    /// Single tap
    Tap { x: f32, y: f32 },
    /// Double tap
    DoubleTap { x: f32, y: f32 },
    /// Long press
    LongPress { x: f32, y: f32 },
    /// Two-finger pinch (zoom)
    Pinch {
        scale: f32,
        center_x: f32,
        center_y: f32,
    },
    /// Two-finger rotate
    Rotate {
        angle: f32,
        center_x: f32,
        center_y: f32,
    },
    /// Two-finger scroll/pan
    Pan { dx: f32, dy: f32 },
    /// Three-finger swipe
    ThreeFingerSwipe { direction: SwipeDirection },
    /// Four-finger swipe
    FourFingerSwipe { direction: SwipeDirection },
}

/// Swipe direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Gesture phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GesturePhase {
    /// Gesture started
    Began,
    /// Gesture in progress
    Changed,
    /// Gesture completed
    Ended,
    /// Gesture cancelled
    Cancelled,
}

/// Modifier keys
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool, // Windows/Command key
    pub caps_lock: bool,
    pub num_lock: bool,
    pub scroll_lock: bool,
}

/// Mouse buttons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,    // X1
    Forward, // X2
    Other(u8),
}

/// Virtual key codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum KeyCode {
    // Letters
    A = 0x0004,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    // Numbers
    Num1 = 0x001E,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    Num0,
    // Number aliases for HID compatibility
    Key1 = 0x011E,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Key0,

    // Special keys
    Enter = 0x0028,
    Escape = 0x0029,
    Backspace = 0x002A,
    Tab = 0x002B,
    Space = 0x002C,
    Minus = 0x002D,
    Equal = 0x002E,
    LeftBracket = 0x002F,
    RightBracket = 0x0030,
    Backslash = 0x0031,
    Semicolon = 0x0033,
    Quote = 0x0034,
    Apostrophe = 0x0134, // Alias for Quote
    Grave = 0x0035,
    Comma = 0x0036,
    Period = 0x0037,
    Slash = 0x0038,
    CapsLock = 0x0039,

    // Function keys
    F1 = 0x003A,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // Control keys
    PrintScreen = 0x0046,
    ScrollLock = 0x0047,
    Pause = 0x0048,
    Insert = 0x0049,
    Home = 0x004A,
    PageUp = 0x004B,
    Delete = 0x004C,
    End = 0x004D,
    PageDown = 0x004E,
    Right = 0x004F,
    Left = 0x0050,
    Down = 0x0051,
    Up = 0x0052,
    NumLock = 0x0053,

    // Numpad
    NumpadDivide = 0x0054,
    NumpadMultiply = 0x0055,
    NumpadMinus = 0x0056,
    NumpadSubtract = 0x0156, // Alias
    NumpadPlus = 0x0057,
    NumpadAdd = 0x0157, // Alias
    NumpadEnter = 0x0058,
    Numpad1 = 0x0059,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    Numpad0,
    NumpadDecimal = 0x0063,

    // Modifiers
    LeftCtrl = 0x00E0,
    LeftShift = 0x00E1,
    LeftAlt = 0x00E2,
    LeftMeta = 0x00E3,
    LeftSuper = 0x01E3, // Alias for LeftMeta
    RightCtrl = 0x00E4,
    RightShift = 0x00E5,
    RightAlt = 0x00E6,
    RightMeta = 0x00E7,
    RightSuper = 0x01E7, // Alias for RightMeta

    // Media keys
    MediaPlayPause = 0x00E8,
    MediaStop = 0x00E9,
    MediaPrev = 0x00EA,
    MediaNext = 0x00EB,
    MediaMute = 0x00EC,
    MediaVolUp = 0x00ED,
    MediaVolDown = 0x00EE,

    // Unknown
    Unknown = 0xFFFF,
}

/// Input device trait
pub trait InputDevice: Send + Sync {
    /// Get device name
    fn name(&self) -> &str;

    /// Get device type
    fn device_type(&self) -> InputDeviceType;

    /// Poll for events (returns events since last poll)
    fn poll(&mut self) -> Vec<InputEvent>;

    /// Check if device is still connected
    fn is_connected(&self) -> bool;
}

/// Input device types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputDeviceType {
    Keyboard,
    Mouse,
    Touchpad,
    Touchscreen,
    Gamepad,
    GameController,
    Unknown,
    Other,
}

/// Input event queue
pub struct InputEventQueue {
    events: VecDeque<InputEvent>,
    max_size: usize,
}

impl InputEventQueue {
    /// Create a new event queue
    pub fn new(max_size: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    /// Push an event
    pub fn push(&mut self, event: InputEvent) {
        if self.events.len() >= self.max_size {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    /// Pop an event
    pub fn pop(&mut self) -> Option<InputEvent> {
        self.events.pop_front()
    }

    /// Peek at the next event without removing it
    pub fn peek(&self) -> Option<&InputEvent> {
        self.events.front()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Get number of pending events
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Clear all events
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

/// Input subsystem manager
pub struct InputManager {
    /// Registered devices
    devices: Vec<Box<dyn InputDevice>>,
    /// Global event queue
    event_queue: Mutex<InputEventQueue>,
    /// Current modifiers state
    modifiers: Mutex<Modifiers>,
    /// Current mouse position
    mouse_position: Mutex<(i32, i32)>,
}

impl InputManager {
    /// Create a new input manager
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            event_queue: Mutex::new(InputEventQueue::new(256)),
            modifiers: Mutex::new(Modifiers::default()),
            mouse_position: Mutex::new((0, 0)),
        }
    }

    /// Initialize input subsystem
    pub fn init(&mut self) {
        // Initialize PS/2 keyboard and mouse
        keyboard::init();
        mouse::init();

        // Initialize USB HID devices
        hid::init();
    }

    /// Register an input device
    pub fn register(&mut self, device: Box<dyn InputDevice>) {
        self.devices.push(device);
    }

    /// Poll all devices for events
    pub fn poll(&mut self) {
        let mut all_events = Vec::new();

        for device in &mut self.devices {
            if device.is_connected() {
                all_events.extend(device.poll());
            }
        }

        for event in all_events {
            self.process_event(&event);
            self.event_queue.lock().push(event);
        }
    }

    /// Process event to update internal state
    fn process_event(&self, event: &InputEvent) {
        match &event.data {
            InputEventData::Key(key_event) => {
                let mut modifiers = self.modifiers.lock();
                match key_event.keycode {
                    KeyCode::LeftShift | KeyCode::RightShift => {
                        modifiers.shift = key_event.pressed;
                    }
                    KeyCode::LeftCtrl | KeyCode::RightCtrl => {
                        modifiers.ctrl = key_event.pressed;
                    }
                    KeyCode::LeftAlt | KeyCode::RightAlt => {
                        modifiers.alt = key_event.pressed;
                    }
                    KeyCode::LeftMeta | KeyCode::RightMeta => {
                        modifiers.meta = key_event.pressed;
                    }
                    KeyCode::CapsLock if key_event.pressed => {
                        modifiers.caps_lock = !modifiers.caps_lock;
                    }
                    KeyCode::NumLock if key_event.pressed => {
                        modifiers.num_lock = !modifiers.num_lock;
                    }
                    KeyCode::ScrollLock if key_event.pressed => {
                        modifiers.scroll_lock = !modifiers.scroll_lock;
                    }
                    _ => {}
                }
            }
            InputEventData::MouseMove(move_event) => {
                let mut pos = self.mouse_position.lock();
                *pos = (move_event.x, move_event.y);
            }
            _ => {}
        }
    }

    /// Get next event from queue
    pub fn next_event(&self) -> Option<InputEvent> {
        self.event_queue.lock().pop()
    }

    /// Get current modifiers
    pub fn modifiers(&self) -> Modifiers {
        *self.modifiers.lock()
    }

    /// Get current mouse position
    pub fn mouse_position(&self) -> (i32, i32) {
        *self.mouse_position.lock()
    }
}

/// Global input manager
static mut INPUT_MANAGER: Option<InputManager> = None;

/// Initialize global input manager
pub fn init() {
    unsafe {
        let mut manager = InputManager::new();
        manager.init();
        INPUT_MANAGER = Some(manager);
    }
}

/// Get input manager reference
pub fn manager() -> Option<&'static InputManager> {
    unsafe { INPUT_MANAGER.as_ref() }
}
