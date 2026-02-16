//! Input Bridge Module
//!
//! This module provides input event bridging from the kernel HID drivers
//! to the browser event system.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

/// Key code enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum KeyCode {
    // Letters
    A = 0,
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
    Num0 = 30,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,

    // Function keys
    F1 = 40,
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

    // Modifiers
    LeftShift = 60,
    RightShift,
    LeftCtrl,
    RightCtrl,
    LeftAlt,
    RightAlt,
    LeftMeta,
    RightMeta,

    // Special keys
    Space = 70,
    Enter,
    Tab,
    Backspace,
    Escape,
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    CapsLock,
    NumLock,
    ScrollLock,
    PrintScreen,
    Pause,

    // Punctuation
    Minus = 100,
    Equals,
    LeftBracket,
    RightBracket,
    Backslash,
    Semicolon,
    Quote,
    Grave,
    Comma,
    Period,
    Slash,

    // Keypad
    KpNum0 = 120,
    KpNum1,
    KpNum2,
    KpNum3,
    KpNum4,
    KpNum5,
    KpNum6,
    KpNum7,
    KpNum8,
    KpNum9,
    KpPlus,
    KpMinus,
    KpMultiply,
    KpDivide,
    KpEnter,
    KpDecimal,

    // Unknown
    Unknown = 255,
}

impl From<u8> for KeyCode {
    fn from(val: u8) -> Self {
        // Safety: We handle all valid values and default to Unknown
        if val <= 25 {
            unsafe { core::mem::transmute(val) }
        } else if (30..40).contains(&val) {
            unsafe { core::mem::transmute(val) }
        } else if (40..52).contains(&val) {
            unsafe { core::mem::transmute(val) }
        } else if (60..68).contains(&val) {
            unsafe { core::mem::transmute(val) }
        } else if (70..89).contains(&val) {
            unsafe { core::mem::transmute(val) }
        } else if (100..111).contains(&val) {
            unsafe { core::mem::transmute(val) }
        } else if (120..136).contains(&val) {
            unsafe { core::mem::transmute(val) }
        } else {
            KeyCode::Unknown
        }
    }
}

/// Key state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    /// Key is pressed down
    Pressed,
    /// Key is released
    Released,
    /// Key is being held (repeat)
    Repeat,
}

/// Mouse button
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// Left mouse button
    Left,
    /// Right mouse button
    Right,
    /// Middle mouse button (scroll wheel click)
    Middle,
    /// Extra button 1 (back)
    Button4,
    /// Extra button 2 (forward)
    Button5,
}

/// Mouse button state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    /// Button pressed
    Pressed,
    /// Button released
    Released,
}

/// Modifier keys state
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    /// Shift key pressed
    pub shift: bool,
    /// Ctrl key pressed
    pub ctrl: bool,
    /// Alt key pressed
    pub alt: bool,
    /// Meta/Super/Windows key pressed
    pub meta: bool,
    /// Caps lock active
    pub caps_lock: bool,
    /// Num lock active
    pub num_lock: bool,
}

impl Modifiers {
    /// Check if any modifier is pressed
    pub fn any(&self) -> bool {
        self.shift || self.ctrl || self.alt || self.meta
    }

    /// Check if none modifiers are pressed
    pub fn none(&self) -> bool {
        !self.any()
    }
}

/// Input event
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Keyboard event
    Key {
        /// Key code
        code: KeyCode,
        /// Key state
        state: KeyState,
        /// Active modifiers
        modifiers: Modifiers,
        /// Character produced (if any)
        character: Option<char>,
    },

    /// Mouse move event
    MouseMove {
        /// X position
        x: i32,
        /// Y position
        y: i32,
        /// Delta X since last event
        dx: i32,
        /// Delta Y since last event
        dy: i32,
    },

    /// Mouse button event
    MouseButton {
        /// Button
        button: MouseButton,
        /// State
        state: ButtonState,
        /// X position
        x: i32,
        /// Y position
        y: i32,
        /// Active modifiers
        modifiers: Modifiers,
    },

    /// Mouse wheel event
    MouseWheel {
        /// Horizontal scroll delta
        dx: i32,
        /// Vertical scroll delta
        dy: i32,
        /// X position
        x: i32,
        /// Y position
        y: i32,
    },

    /// Touch event
    Touch {
        /// Touch ID (for multi-touch)
        id: u32,
        /// Touch phase
        phase: TouchPhase,
        /// X position
        x: i32,
        /// Y position
        y: i32,
        /// Pressure (0.0-1.0)
        pressure: f32,
    },

    /// Gamepad button event
    GamepadButton {
        /// Gamepad ID
        gamepad_id: u32,
        /// Button ID
        button: u32,
        /// State
        state: ButtonState,
    },

    /// Gamepad axis event
    GamepadAxis {
        /// Gamepad ID
        gamepad_id: u32,
        /// Axis ID
        axis: u32,
        /// Value (-1.0 to 1.0)
        value: f32,
    },

    /// Focus event
    Focus {
        /// Whether focused
        focused: bool,
    },
}

/// Touch phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    /// Touch started
    Start,
    /// Touch moved
    Move,
    /// Touch ended
    End,
    /// Touch cancelled
    Cancel,
}

/// Input bridge for kernel HID events
pub struct InputBridge {
    /// Event queue
    events: spin::Mutex<VecDeque<InputEvent>>,
    /// Current mouse position
    mouse_x: core::sync::atomic::AtomicI32,
    mouse_y: core::sync::atomic::AtomicI32,
    /// Current modifiers
    modifiers: spin::Mutex<Modifiers>,
    /// Whether input is enabled
    enabled: AtomicBool,
    /// Focused window ID
    focused_window: core::sync::atomic::AtomicU32,
}

impl InputBridge {
    /// Create a new input bridge
    pub const fn new() -> Self {
        Self {
            events: spin::Mutex::new(VecDeque::new()),
            mouse_x: core::sync::atomic::AtomicI32::new(0),
            mouse_y: core::sync::atomic::AtomicI32::new(0),
            modifiers: spin::Mutex::new(Modifiers {
                shift: false,
                ctrl: false,
                alt: false,
                meta: false,
                caps_lock: false,
                num_lock: false,
            }),
            enabled: AtomicBool::new(true),
            focused_window: core::sync::atomic::AtomicU32::new(0),
        }
    }

    /// Enable or disable input
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    /// Check if input is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// Get current mouse position
    pub fn mouse_position(&self) -> (i32, i32) {
        (
            self.mouse_x.load(Ordering::SeqCst),
            self.mouse_y.load(Ordering::SeqCst),
        )
    }

    /// Get current modifiers
    pub fn modifiers(&self) -> Modifiers {
        *self.modifiers.lock()
    }

    /// Set focused window
    pub fn set_focus(&self, window_id: u32) {
        let old = self.focused_window.swap(window_id, Ordering::SeqCst);

        if old != window_id {
            // Queue focus events
            if old != 0 {
                self.events
                    .lock()
                    .push_back(InputEvent::Focus { focused: false });
            }
            if window_id != 0 {
                self.events
                    .lock()
                    .push_back(InputEvent::Focus { focused: true });
            }
        }
    }

    /// Get focused window
    pub fn focused_window(&self) -> u32 {
        self.focused_window.load(Ordering::SeqCst)
    }

    /// Inject a key event (for testing/simulation)
    pub fn inject_key(&self, code: KeyCode, state: KeyState) {
        if !self.is_enabled() {
            return;
        }

        // Update modifiers
        {
            let mut mods = self.modifiers.lock();
            let pressed = matches!(state, KeyState::Pressed);
            match code {
                KeyCode::LeftShift | KeyCode::RightShift => mods.shift = pressed,
                KeyCode::LeftCtrl | KeyCode::RightCtrl => mods.ctrl = pressed,
                KeyCode::LeftAlt | KeyCode::RightAlt => mods.alt = pressed,
                KeyCode::LeftMeta | KeyCode::RightMeta => mods.meta = pressed,
                KeyCode::CapsLock if pressed => mods.caps_lock = !mods.caps_lock,
                KeyCode::NumLock if pressed => mods.num_lock = !mods.num_lock,
                _ => {}
            }
        }

        let modifiers = *self.modifiers.lock();
        let character = self.key_to_char(code, &modifiers);

        self.events.lock().push_back(InputEvent::Key {
            code,
            state,
            modifiers,
            character,
        });
    }

    /// Inject a mouse move event
    pub fn inject_mouse_move(&self, x: i32, y: i32) {
        if !self.is_enabled() {
            return;
        }

        let old_x = self.mouse_x.swap(x, Ordering::SeqCst);
        let old_y = self.mouse_y.swap(y, Ordering::SeqCst);

        self.events.lock().push_back(InputEvent::MouseMove {
            x,
            y,
            dx: x - old_x,
            dy: y - old_y,
        });
    }

    /// Inject a mouse button event
    pub fn inject_mouse_button(&self, button: MouseButton, state: ButtonState) {
        if !self.is_enabled() {
            return;
        }

        let (x, y) = self.mouse_position();
        let modifiers = *self.modifiers.lock();

        self.events.lock().push_back(InputEvent::MouseButton {
            button,
            state,
            x,
            y,
            modifiers,
        });
    }

    /// Inject a mouse wheel event
    pub fn inject_mouse_wheel(&self, dx: i32, dy: i32) {
        if !self.is_enabled() {
            return;
        }

        let (x, y) = self.mouse_position();

        self.events
            .lock()
            .push_back(InputEvent::MouseWheel { dx, dy, x, y });
    }

    /// Inject a touch event
    pub fn inject_touch(&self, id: u32, phase: TouchPhase, x: i32, y: i32, pressure: f32) {
        if !self.is_enabled() {
            return;
        }

        self.events.lock().push_back(InputEvent::Touch {
            id,
            phase,
            x,
            y,
            pressure,
        });
    }

    /// Poll for events
    pub fn poll_events(&self) -> Vec<InputEvent> {
        let mut events = self.events.lock();
        events.drain(..).collect()
    }

    /// Check if there are pending events
    pub fn has_events(&self) -> bool {
        !self.events.lock().is_empty()
    }

    /// Clear all pending events
    pub fn clear_events(&self) {
        self.events.lock().clear();
    }

    /// Convert key code to character
    fn key_to_char(&self, code: KeyCode, modifiers: &Modifiers) -> Option<char> {
        let shift = modifiers.shift ^ modifiers.caps_lock;

        match code {
            KeyCode::A => Some(if shift { 'A' } else { 'a' }),
            KeyCode::B => Some(if shift { 'B' } else { 'b' }),
            KeyCode::C => Some(if shift { 'C' } else { 'c' }),
            KeyCode::D => Some(if shift { 'D' } else { 'd' }),
            KeyCode::E => Some(if shift { 'E' } else { 'e' }),
            KeyCode::F => Some(if shift { 'F' } else { 'f' }),
            KeyCode::G => Some(if shift { 'G' } else { 'g' }),
            KeyCode::H => Some(if shift { 'H' } else { 'h' }),
            KeyCode::I => Some(if shift { 'I' } else { 'i' }),
            KeyCode::J => Some(if shift { 'J' } else { 'j' }),
            KeyCode::K => Some(if shift { 'K' } else { 'k' }),
            KeyCode::L => Some(if shift { 'L' } else { 'l' }),
            KeyCode::M => Some(if shift { 'M' } else { 'm' }),
            KeyCode::N => Some(if shift { 'N' } else { 'n' }),
            KeyCode::O => Some(if shift { 'O' } else { 'o' }),
            KeyCode::P => Some(if shift { 'P' } else { 'p' }),
            KeyCode::Q => Some(if shift { 'Q' } else { 'q' }),
            KeyCode::R => Some(if shift { 'R' } else { 'r' }),
            KeyCode::S => Some(if shift { 'S' } else { 's' }),
            KeyCode::T => Some(if shift { 'T' } else { 't' }),
            KeyCode::U => Some(if shift { 'U' } else { 'u' }),
            KeyCode::V => Some(if shift { 'V' } else { 'v' }),
            KeyCode::W => Some(if shift { 'W' } else { 'w' }),
            KeyCode::X => Some(if shift { 'X' } else { 'x' }),
            KeyCode::Y => Some(if shift { 'Y' } else { 'y' }),
            KeyCode::Z => Some(if shift { 'Z' } else { 'z' }),

            KeyCode::Num0 => Some(if modifiers.shift { ')' } else { '0' }),
            KeyCode::Num1 => Some(if modifiers.shift { '!' } else { '1' }),
            KeyCode::Num2 => Some(if modifiers.shift { '@' } else { '2' }),
            KeyCode::Num3 => Some(if modifiers.shift { '#' } else { '3' }),
            KeyCode::Num4 => Some(if modifiers.shift { '$' } else { '4' }),
            KeyCode::Num5 => Some(if modifiers.shift { '%' } else { '5' }),
            KeyCode::Num6 => Some(if modifiers.shift { '^' } else { '6' }),
            KeyCode::Num7 => Some(if modifiers.shift { '&' } else { '7' }),
            KeyCode::Num8 => Some(if modifiers.shift { '*' } else { '8' }),
            KeyCode::Num9 => Some(if modifiers.shift { '(' } else { '9' }),

            KeyCode::Space => Some(' '),
            KeyCode::Enter => Some('\n'),
            KeyCode::Tab => Some('\t'),

            KeyCode::Minus => Some(if modifiers.shift { '_' } else { '-' }),
            KeyCode::Equals => Some(if modifiers.shift { '+' } else { '=' }),
            KeyCode::LeftBracket => Some(if modifiers.shift { '{' } else { '[' }),
            KeyCode::RightBracket => Some(if modifiers.shift { '}' } else { ']' }),
            KeyCode::Backslash => Some(if modifiers.shift { '|' } else { '\\' }),
            KeyCode::Semicolon => Some(if modifiers.shift { ':' } else { ';' }),
            KeyCode::Quote => Some(if modifiers.shift { '"' } else { '\'' }),
            KeyCode::Grave => Some(if modifiers.shift { '~' } else { '`' }),
            KeyCode::Comma => Some(if modifiers.shift { '<' } else { ',' }),
            KeyCode::Period => Some(if modifiers.shift { '>' } else { '.' }),
            KeyCode::Slash => Some(if modifiers.shift { '?' } else { '/' }),

            _ => None,
        }
    }
}

impl Default for InputBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Global input bridge instance
static INPUT_BRIDGE: InputBridge = InputBridge::new();

/// Get the global input bridge
pub fn input_bridge() -> &'static InputBridge {
    &INPUT_BRIDGE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_event() {
        let input = InputBridge::new();
        input.inject_key(KeyCode::A, KeyState::Pressed);
        let events = input.poll_events();
        assert!(events.iter().any(|e| matches!(
            e,
            InputEvent::Key {
                code: KeyCode::A,
                ..
            }
        )));
    }

    #[test]
    fn test_mouse_event() {
        let input = InputBridge::new();
        input.inject_mouse_move(100, 200);
        let events = input.poll_events();
        assert!(events
            .iter()
            .any(|e| matches!(e, InputEvent::MouseMove { x: 100, y: 200, .. })));
    }

    #[test]
    fn test_modifiers() {
        let input = InputBridge::new();

        // Press shift
        input.inject_key(KeyCode::LeftShift, KeyState::Pressed);
        assert!(input.modifiers().shift);

        // Release shift
        input.inject_key(KeyCode::LeftShift, KeyState::Released);
        assert!(!input.modifiers().shift);
    }
}
