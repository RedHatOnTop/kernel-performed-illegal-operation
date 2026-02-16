//! Input Event Delivery
//!
//! This module handles input event routing from the kernel to browser tabs.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use spin::{Mutex, RwLock};

use super::coordinator::TabId;

/// Key code (subset of common keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum KeyCode {
    // Letters
    KeyA = 0x04,
    KeyB = 0x05,
    KeyC = 0x06,
    KeyD = 0x07,
    KeyE = 0x08,
    KeyF = 0x09,
    KeyG = 0x0A,
    KeyH = 0x0B,
    KeyI = 0x0C,
    KeyJ = 0x0D,
    KeyK = 0x0E,
    KeyL = 0x0F,
    KeyM = 0x10,
    KeyN = 0x11,
    KeyO = 0x12,
    KeyP = 0x13,
    KeyQ = 0x14,
    KeyR = 0x15,
    KeyS = 0x16,
    KeyT = 0x17,
    KeyU = 0x18,
    KeyV = 0x19,
    KeyW = 0x1A,
    KeyX = 0x1B,
    KeyY = 0x1C,
    KeyZ = 0x1D,

    // Numbers
    Digit1 = 0x1E,
    Digit2 = 0x1F,
    Digit3 = 0x20,
    Digit4 = 0x21,
    Digit5 = 0x22,
    Digit6 = 0x23,
    Digit7 = 0x24,
    Digit8 = 0x25,
    Digit9 = 0x26,
    Digit0 = 0x27,

    // Special keys
    Enter = 0x28,
    Escape = 0x29,
    Backspace = 0x2A,
    Tab = 0x2B,
    Space = 0x2C,

    // Modifiers
    LeftShift = 0xE1,
    LeftControl = 0xE0,
    LeftAlt = 0xE2,
    LeftMeta = 0xE3,
    RightShift = 0xE5,
    RightControl = 0xE4,
    RightAlt = 0xE6,
    RightMeta = 0xE7,

    // Navigation
    ArrowUp = 0x52,
    ArrowDown = 0x51,
    ArrowLeft = 0x50,
    ArrowRight = 0x4F,
    Home = 0x4A,
    End = 0x4D,
    PageUp = 0x4B,
    PageDown = 0x4E,

    // Function keys
    F1 = 0x3A,
    F2 = 0x3B,
    F3 = 0x3C,
    F4 = 0x3D,
    F5 = 0x3E,
    F6 = 0x3F,
    F7 = 0x40,
    F8 = 0x41,
    F9 = 0x42,
    F10 = 0x43,
    F11 = 0x44,
    F12 = 0x45,

    /// Unknown key.
    Unknown = 0xFF,
}

impl From<u32> for KeyCode {
    fn from(value: u32) -> Self {
        // Simplified - in reality would have full mapping
        match value {
            0x04..=0x1D => unsafe { core::mem::transmute(value) },
            0x1E..=0x27 => unsafe { core::mem::transmute(value) },
            0x28 => KeyCode::Enter,
            0x29 => KeyCode::Escape,
            0x2A => KeyCode::Backspace,
            0x2B => KeyCode::Tab,
            0x2C => KeyCode::Space,
            _ => KeyCode::Unknown,
        }
    }
}

/// Modifier key state.
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    /// Shift key pressed.
    pub shift: bool,
    /// Control key pressed.
    pub ctrl: bool,
    /// Alt key pressed.
    pub alt: bool,
    /// Meta/Super key pressed.
    pub meta: bool,
}

impl Modifiers {
    /// Create with no modifiers.
    pub fn none() -> Self {
        Self::default()
    }

    /// Create from raw bits.
    pub fn from_bits(bits: u8) -> Self {
        Modifiers {
            shift: bits & 0x01 != 0,
            ctrl: bits & 0x02 != 0,
            alt: bits & 0x04 != 0,
            meta: bits & 0x08 != 0,
        }
    }

    /// Convert to raw bits.
    pub fn to_bits(&self) -> u8 {
        let mut bits = 0u8;
        if self.shift {
            bits |= 0x01;
        }
        if self.ctrl {
            bits |= 0x02;
        }
        if self.alt {
            bits |= 0x04;
        }
        if self.meta {
            bits |= 0x08;
        }
        bits
    }
}

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MouseButton {
    /// Left button.
    Left = 0,
    /// Right button.
    Right = 1,
    /// Middle button.
    Middle = 2,
    /// Back button.
    Back = 3,
    /// Forward button.
    Forward = 4,
}

/// Touch point.
#[derive(Debug, Clone, Copy)]
pub struct TouchPoint {
    /// Touch identifier.
    pub id: u32,
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// Pressure (0.0 - 1.0).
    pub pressure: f32,
}

/// Input event.
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Key pressed.
    KeyDown {
        key: KeyCode,
        modifiers: Modifiers,
        repeat: bool,
    },
    /// Key released.
    KeyUp { key: KeyCode, modifiers: Modifiers },
    /// Character input.
    Char { character: char },
    /// Mouse moved.
    MouseMove {
        x: f32,
        y: f32,
        modifiers: Modifiers,
    },
    /// Mouse button pressed.
    MouseDown {
        button: MouseButton,
        x: f32,
        y: f32,
        modifiers: Modifiers,
    },
    /// Mouse button released.
    MouseUp {
        button: MouseButton,
        x: f32,
        y: f32,
        modifiers: Modifiers,
    },
    /// Mouse wheel scrolled.
    Scroll {
        delta_x: f32,
        delta_y: f32,
        x: f32,
        y: f32,
        modifiers: Modifiers,
    },
    /// Touch started.
    TouchStart { points: Vec<TouchPoint> },
    /// Touch moved.
    TouchMove { points: Vec<TouchPoint> },
    /// Touch ended.
    TouchEnd { points: Vec<TouchPoint> },
    /// Window focus changed.
    FocusChange { focused: bool },
}

/// Serialized input event for IPC.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct InputEventData {
    /// Event type.
    pub event_type: u32,
    /// Key code / button.
    pub code: u32,
    /// Modifier flags.
    pub modifiers: u8,
    /// Flags (repeat, etc.).
    pub flags: u8,
    /// Reserved.
    pub reserved: u16,
    /// X coordinate or delta.
    pub x: f32,
    /// Y coordinate or delta.
    pub y: f32,
    /// Timestamp.
    pub timestamp: u64,
}

impl InputEventData {
    /// Event type constants.
    pub const KEY_DOWN: u32 = 0;
    pub const KEY_UP: u32 = 1;
    pub const CHAR: u32 = 2;
    pub const MOUSE_MOVE: u32 = 3;
    pub const MOUSE_DOWN: u32 = 4;
    pub const MOUSE_UP: u32 = 5;
    pub const SCROLL: u32 = 6;
    pub const TOUCH_START: u32 = 7;
    pub const TOUCH_MOVE: u32 = 8;
    pub const TOUCH_END: u32 = 9;
    pub const FOCUS: u32 = 10;
}

/// Input queue for a tab.
struct InputQueue {
    /// Queued events.
    events: VecDeque<InputEvent>,
    /// Maximum queue size.
    max_size: usize,
    /// Events dropped due to overflow.
    dropped: u64,
}

impl InputQueue {
    fn new(max_size: usize) -> Self {
        InputQueue {
            events: VecDeque::with_capacity(max_size),
            max_size,
            dropped: 0,
        }
    }

    fn push(&mut self, event: InputEvent) {
        if self.events.len() >= self.max_size {
            self.events.pop_front();
            self.dropped += 1;
        }
        self.events.push_back(event);
    }

    fn pop(&mut self) -> Option<InputEvent> {
        self.events.pop_front()
    }

    fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// Input manager.
pub struct InputManager {
    /// Input queues per tab.
    queues: RwLock<alloc::collections::BTreeMap<TabId, Mutex<InputQueue>>>,
    /// Currently focused tab.
    focused_tab: Mutex<Option<TabId>>,
    /// Current modifier state.
    modifiers: Mutex<Modifiers>,
    /// Current mouse position.
    mouse_pos: Mutex<(f32, f32)>,
}

impl InputManager {
    /// Create new input manager.
    pub fn new() -> Self {
        InputManager {
            queues: RwLock::new(alloc::collections::BTreeMap::new()),
            focused_tab: Mutex::new(None),
            modifiers: Mutex::new(Modifiers::none()),
            mouse_pos: Mutex::new((0.0, 0.0)),
        }
    }

    /// Register a tab for input.
    pub fn register_tab(&self, tab: TabId) {
        let mut queues = self.queues.write();
        queues.insert(tab, Mutex::new(InputQueue::new(256)));
    }

    /// Unregister a tab.
    pub fn unregister_tab(&self, tab: TabId) {
        let mut queues = self.queues.write();
        queues.remove(&tab);

        let mut focused = self.focused_tab.lock();
        if *focused == Some(tab) {
            *focused = None;
        }
    }

    /// Set focused tab.
    pub fn set_focus(&self, tab: TabId) {
        let mut focused = self.focused_tab.lock();

        // Send focus lost to previous tab
        if let Some(prev_tab) = *focused {
            if prev_tab != tab {
                self.push_to_tab(prev_tab, InputEvent::FocusChange { focused: false });
            }
        }

        // Send focus gained to new tab
        self.push_to_tab(tab, InputEvent::FocusChange { focused: true });
        *focused = Some(tab);
    }

    /// Push event to focused tab.
    pub fn push(&self, event: InputEvent) {
        // Update modifier state
        match &event {
            InputEvent::KeyDown { key, modifiers, .. } | InputEvent::KeyUp { key, modifiers } => {
                *self.modifiers.lock() = *modifiers;
            }
            InputEvent::MouseMove { x, y, .. } => {
                *self.mouse_pos.lock() = (*x, *y);
            }
            _ => {}
        }

        if let Some(tab) = *self.focused_tab.lock() {
            self.push_to_tab(tab, event);
        }
    }

    /// Push event to specific tab.
    pub fn push_to_tab(&self, tab: TabId, event: InputEvent) {
        let queues = self.queues.read();
        if let Some(queue) = queues.get(&tab) {
            queue.lock().push(event);
        }
    }

    /// Pop event from tab queue.
    pub fn pop(&self, tab: TabId) -> Option<InputEvent> {
        let queues = self.queues.read();
        let queue = queues.get(&tab)?;
        let result = queue.lock().pop();
        result
    }

    /// Check if tab has pending events.
    pub fn has_events(&self, tab: TabId) -> bool {
        let queues = self.queues.read();
        if let Some(q) = queues.get(&tab) {
            !q.lock().is_empty()
        } else {
            false
        }
    }

    /// Get current modifiers.
    pub fn current_modifiers(&self) -> Modifiers {
        *self.modifiers.lock()
    }

    /// Get current mouse position.
    pub fn mouse_position(&self) -> (f32, f32) {
        *self.mouse_pos.lock()
    }
}

/// Global input manager.
static INPUT_MANAGER: RwLock<Option<InputManager>> = RwLock::new(None);

/// Initialize input manager.
pub fn init() {
    let mut mgr = INPUT_MANAGER.write();
    *mgr = Some(InputManager::new());
    crate::serial_println!("[Input] Input manager initialized");
}

/// Register tab for input.
pub fn register_tab(tab: TabId) {
    if let Some(mgr) = INPUT_MANAGER.read().as_ref() {
        mgr.register_tab(tab);
    }
}

/// Unregister tab.
pub fn unregister_tab(tab: TabId) {
    if let Some(mgr) = INPUT_MANAGER.read().as_ref() {
        mgr.unregister_tab(tab);
    }
}

/// Set focused tab.
pub fn set_focus(tab: TabId) {
    if let Some(mgr) = INPUT_MANAGER.read().as_ref() {
        mgr.set_focus(tab);
    }
}

/// Push input event.
pub fn push_event(event: InputEvent) {
    if let Some(mgr) = INPUT_MANAGER.read().as_ref() {
        mgr.push(event);
    }
}

/// Pop input event for tab.
pub fn pop_event(tab: TabId) -> Option<InputEvent> {
    INPUT_MANAGER.read().as_ref()?.pop(tab)
}

/// Check for pending events.
pub fn has_events(tab: TabId) -> bool {
    INPUT_MANAGER
        .read()
        .as_ref()
        .map(|m| m.has_events(tab))
        .unwrap_or(false)
}
