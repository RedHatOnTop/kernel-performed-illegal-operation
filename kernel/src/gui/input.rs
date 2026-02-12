//! Input Buffer System
//!
//! Global input buffers for keyboard and mouse events.
//! Used to pass events from interrupt handlers to the GUI system.

use alloc::collections::VecDeque;
use spin::Mutex;

/// Maximum events in each buffer
const MAX_EVENTS: usize = 64;

/// Keyboard event
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub key: char,
    pub scancode: u8,
    pub pressed: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

/// Mouse event
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    pub dx: i16,
    pub dy: i16,
    pub buttons: MouseButtons,
}

/// Mouse button state
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseButtons {
    pub left: bool,
    pub right: bool,
    pub middle: bool,
}

/// Global keyboard buffer
static KEYBOARD_BUFFER: Mutex<VecDeque<KeyEvent>> = Mutex::new(VecDeque::new());

/// Global mouse buffer
static MOUSE_BUFFER: Mutex<VecDeque<MouseEvent>> = Mutex::new(VecDeque::new());

/// PS/2 mouse state machine
static MOUSE_STATE: Mutex<MouseState> = Mutex::new(MouseState::new());

/// Mouse packet state
struct MouseState {
    packet: [u8; 3],
    index: usize,
}

impl MouseState {
    const fn new() -> Self {
        Self {
            packet: [0; 3],
            index: 0,
        }
    }

    /// Process a byte from the mouse
    fn process_byte(&mut self, byte: u8) -> Option<MouseEvent> {
        self.packet[self.index] = byte;
        self.index += 1;

        if self.index == 3 {
            self.index = 0;

            let flags = self.packet[0];

            // Validate packet (bit 3 should always be 1)
            if flags & 0x08 == 0 {
                return None;
            }

            let mut dx = self.packet[1] as i16;
            let mut dy = self.packet[2] as i16;

            // Apply sign extension
            if flags & 0x10 != 0 {
                dx -= 256;
            }
            if flags & 0x20 != 0 {
                dy -= 256;
            }

            // Invert Y axis (screen coordinates)
            dy = -dy;

            let buttons = MouseButtons {
                left: flags & 0x01 != 0,
                right: flags & 0x02 != 0,
                middle: flags & 0x04 != 0,
            };

            Some(MouseEvent { dx, dy, buttons })
        } else {
            None
        }
    }
}

// ==================== Keyboard API ====================

/// Push a keyboard event (called from interrupt handler)
pub fn push_key_event(key: char, scancode: u8, pressed: bool, ctrl: bool, shift: bool, alt: bool) {
    let mut buffer = KEYBOARD_BUFFER.lock();
    if buffer.len() < MAX_EVENTS {
        buffer.push_back(KeyEvent {
            key,
            scancode,
            pressed,
            ctrl,
            shift,
            alt,
        });
    }
}

/// Pop a keyboard event (called from GUI)
pub fn pop_key_event() -> Option<KeyEvent> {
    KEYBOARD_BUFFER.lock().pop_front()
}

/// Check if keyboard buffer has events
pub fn has_key_events() -> bool {
    !KEYBOARD_BUFFER.lock().is_empty()
}

// ==================== Mouse API ====================

/// Process a mouse byte (called from interrupt handler)
pub fn process_mouse_byte(byte: u8) {
    let mut state = MOUSE_STATE.lock();
    if let Some(event) = state.process_byte(byte) {
        drop(state); // Release lock before acquiring buffer lock
        push_mouse_event(event);
    }
}

/// Push a mouse event
fn push_mouse_event(event: MouseEvent) {
    let mut buffer = MOUSE_BUFFER.lock();
    if buffer.len() < MAX_EVENTS {
        buffer.push_back(event);
    }
}

/// Pop a mouse event (called from GUI)
pub fn pop_mouse_event() -> Option<MouseEvent> {
    MOUSE_BUFFER.lock().pop_front()
}

/// Check if mouse buffer has events
pub fn has_mouse_events() -> bool {
    !MOUSE_BUFFER.lock().is_empty()
}

// ==================== Processing ====================

/// Process all pending input events and apply to GUI
pub fn process_all_events() {
    // Process keyboard events
    while let Some(event) = pop_key_event() {
        super::with_gui(|gui| {
            gui.on_key_event(&event);
        });
    }

    // Process mouse events
    while let Some(event) = pop_mouse_event() {
        super::with_gui(|gui| {
            gui.on_mouse_move(event.dx as i32, event.dy as i32);

            // Handle button clicks
            static mut PREV_LEFT: bool = false;
            static mut PREV_RIGHT: bool = false;

            unsafe {
                if event.buttons.left != PREV_LEFT {
                    gui.on_mouse_click(0, event.buttons.left);
                    PREV_LEFT = event.buttons.left;
                }
                if event.buttons.right != PREV_RIGHT {
                    gui.on_mouse_click(1, event.buttons.right);
                    PREV_RIGHT = event.buttons.right;
                }
            }
        });
    }
}
