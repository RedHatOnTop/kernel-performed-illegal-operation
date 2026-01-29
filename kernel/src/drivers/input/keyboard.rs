//! PS/2 Keyboard Driver
//!
//! Driver for PS/2 keyboard input.

use super::{InputDevice, InputDeviceType, InputEvent, InputEventType, InputEventData, KeyEvent, KeyCode, Modifiers};
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

/// PS/2 data port
const PS2_DATA_PORT: u16 = 0x60;
/// PS/2 status/command port
const PS2_STATUS_PORT: u16 = 0x64;

/// PS/2 keyboard state
pub struct Ps2Keyboard {
    /// Device name
    name: String,
    /// Current modifiers
    modifiers: Modifiers,
    /// Extended scancode prefix received
    extended: bool,
    /// Release prefix received
    release: bool,
    /// Pending events
    pending_events: Vec<InputEvent>,
}

impl Ps2Keyboard {
    /// Create a new PS/2 keyboard
    pub fn new() -> Self {
        Self {
            name: String::from("PS/2 Keyboard"),
            modifiers: Modifiers::default(),
            extended: false,
            release: false,
            pending_events: Vec::new(),
        }
    }

    /// Handle incoming scancode
    pub fn handle_scancode(&mut self, scancode: u8) {
        // Check for special prefixes
        if scancode == 0xE0 {
            self.extended = true;
            return;
        }
        if scancode == 0xE1 {
            // Pause key (special sequence) - skip for now
            return;
        }

        let release = (scancode & 0x80) != 0;
        let code = scancode & 0x7F;

        let keycode = if self.extended {
            self.extended = false;
            translate_extended_scancode(code)
        } else {
            translate_scancode(code)
        };

        // Update modifiers
        match keycode {
            KeyCode::LeftShift | KeyCode::RightShift => {
                self.modifiers.shift = !release;
            }
            KeyCode::LeftCtrl | KeyCode::RightCtrl => {
                self.modifiers.ctrl = !release;
            }
            KeyCode::LeftAlt | KeyCode::RightAlt => {
                self.modifiers.alt = !release;
            }
            KeyCode::LeftMeta | KeyCode::RightMeta => {
                self.modifiers.meta = !release;
            }
            KeyCode::CapsLock if !release => {
                self.modifiers.caps_lock = !self.modifiers.caps_lock;
            }
            KeyCode::NumLock if !release => {
                self.modifiers.num_lock = !self.modifiers.num_lock;
            }
            KeyCode::ScrollLock if !release => {
                self.modifiers.scroll_lock = !self.modifiers.scroll_lock;
            }
            _ => {}
        }

        let event = InputEvent {
            event_type: InputEventType::Key,
            timestamp: get_timestamp(),
            data: InputEventData::Key(KeyEvent {
                scancode: scancode as u16,
                keycode,
                pressed: !release,
                modifiers: self.modifiers,
            }),
        };

        self.pending_events.push(event);
    }
}

impl InputDevice for Ps2Keyboard {
    fn name(&self) -> &str {
        &self.name
    }

    fn device_type(&self) -> InputDeviceType {
        InputDeviceType::Keyboard
    }

    fn poll(&mut self) -> Vec<InputEvent> {
        core::mem::take(&mut self.pending_events)
    }

    fn is_connected(&self) -> bool {
        true // PS/2 keyboard is always considered connected if initialized
    }
}

/// Translate PS/2 set 1 scancode to KeyCode
fn translate_scancode(code: u8) -> KeyCode {
    match code {
        0x01 => KeyCode::Escape,
        0x02 => KeyCode::Num1,
        0x03 => KeyCode::Num2,
        0x04 => KeyCode::Num3,
        0x05 => KeyCode::Num4,
        0x06 => KeyCode::Num5,
        0x07 => KeyCode::Num6,
        0x08 => KeyCode::Num7,
        0x09 => KeyCode::Num8,
        0x0A => KeyCode::Num9,
        0x0B => KeyCode::Num0,
        0x0C => KeyCode::Minus,
        0x0D => KeyCode::Equal,
        0x0E => KeyCode::Backspace,
        0x0F => KeyCode::Tab,
        0x10 => KeyCode::Q,
        0x11 => KeyCode::W,
        0x12 => KeyCode::E,
        0x13 => KeyCode::R,
        0x14 => KeyCode::T,
        0x15 => KeyCode::Y,
        0x16 => KeyCode::U,
        0x17 => KeyCode::I,
        0x18 => KeyCode::O,
        0x19 => KeyCode::P,
        0x1A => KeyCode::LeftBracket,
        0x1B => KeyCode::RightBracket,
        0x1C => KeyCode::Enter,
        0x1D => KeyCode::LeftCtrl,
        0x1E => KeyCode::A,
        0x1F => KeyCode::S,
        0x20 => KeyCode::D,
        0x21 => KeyCode::F,
        0x22 => KeyCode::G,
        0x23 => KeyCode::H,
        0x24 => KeyCode::J,
        0x25 => KeyCode::K,
        0x26 => KeyCode::L,
        0x27 => KeyCode::Semicolon,
        0x28 => KeyCode::Quote,
        0x29 => KeyCode::Grave,
        0x2A => KeyCode::LeftShift,
        0x2B => KeyCode::Backslash,
        0x2C => KeyCode::Z,
        0x2D => KeyCode::X,
        0x2E => KeyCode::C,
        0x2F => KeyCode::V,
        0x30 => KeyCode::B,
        0x31 => KeyCode::N,
        0x32 => KeyCode::M,
        0x33 => KeyCode::Comma,
        0x34 => KeyCode::Period,
        0x35 => KeyCode::Slash,
        0x36 => KeyCode::RightShift,
        0x37 => KeyCode::NumpadMultiply,
        0x38 => KeyCode::LeftAlt,
        0x39 => KeyCode::Space,
        0x3A => KeyCode::CapsLock,
        0x3B => KeyCode::F1,
        0x3C => KeyCode::F2,
        0x3D => KeyCode::F3,
        0x3E => KeyCode::F4,
        0x3F => KeyCode::F5,
        0x40 => KeyCode::F6,
        0x41 => KeyCode::F7,
        0x42 => KeyCode::F8,
        0x43 => KeyCode::F9,
        0x44 => KeyCode::F10,
        0x45 => KeyCode::NumLock,
        0x46 => KeyCode::ScrollLock,
        0x47 => KeyCode::Numpad7,
        0x48 => KeyCode::Numpad8,
        0x49 => KeyCode::Numpad9,
        0x4A => KeyCode::NumpadMinus,
        0x4B => KeyCode::Numpad4,
        0x4C => KeyCode::Numpad5,
        0x4D => KeyCode::Numpad6,
        0x4E => KeyCode::NumpadPlus,
        0x4F => KeyCode::Numpad1,
        0x50 => KeyCode::Numpad2,
        0x51 => KeyCode::Numpad3,
        0x52 => KeyCode::Numpad0,
        0x53 => KeyCode::NumpadDecimal,
        0x57 => KeyCode::F11,
        0x58 => KeyCode::F12,
        _ => KeyCode::Unknown,
    }
}

/// Translate extended PS/2 scancode to KeyCode
fn translate_extended_scancode(code: u8) -> KeyCode {
    match code {
        0x10 => KeyCode::MediaPrev,
        0x19 => KeyCode::MediaNext,
        0x1C => KeyCode::NumpadEnter,
        0x1D => KeyCode::RightCtrl,
        0x20 => KeyCode::MediaMute,
        0x22 => KeyCode::MediaPlayPause,
        0x24 => KeyCode::MediaStop,
        0x2E => KeyCode::MediaVolDown,
        0x30 => KeyCode::MediaVolUp,
        0x35 => KeyCode::NumpadDivide,
        0x38 => KeyCode::RightAlt,
        0x47 => KeyCode::Home,
        0x48 => KeyCode::Up,
        0x49 => KeyCode::PageUp,
        0x4B => KeyCode::Left,
        0x4D => KeyCode::Right,
        0x4F => KeyCode::End,
        0x50 => KeyCode::Down,
        0x51 => KeyCode::PageDown,
        0x52 => KeyCode::Insert,
        0x53 => KeyCode::Delete,
        0x5B => KeyCode::LeftMeta,
        0x5C => KeyCode::RightMeta,
        _ => KeyCode::Unknown,
    }
}

/// Get current timestamp (placeholder)
fn get_timestamp() -> u64 {
    // In real implementation, get from kernel timer
    0
}

/// Global PS/2 keyboard instance
static PS2_KEYBOARD: Mutex<Option<Ps2Keyboard>> = Mutex::new(None);

/// Initialize PS/2 keyboard
pub fn init() {
    // Enable PS/2 keyboard interrupts
    // In real implementation:
    // 1. Disable devices
    // 2. Flush output buffer
    // 3. Set controller configuration
    // 4. Perform self-test
    // 5. Enable device
    // 6. Reset device
    // 7. Set scan code set
    
    let mut keyboard = PS2_KEYBOARD.lock();
    *keyboard = Some(Ps2Keyboard::new());
}

/// Handle keyboard interrupt
pub fn handle_interrupt() {
    let scancode = read_scancode();
    if let Some(ref mut keyboard) = *PS2_KEYBOARD.lock() {
        keyboard.handle_scancode(scancode);
    }
}

/// Read scancode from PS/2 data port
fn read_scancode() -> u8 {
    unsafe {
        let value: u8;
        core::arch::asm!(
            "in al, dx",
            in("dx") PS2_DATA_PORT,
            out("al") value,
        );
        value
    }
}

/// Write to PS/2 data port
fn write_data(data: u8) {
    // Wait for input buffer to be empty
    while (read_status() & 0x02) != 0 {}
    
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") PS2_DATA_PORT,
            in("al") data,
        );
    }
}

/// Read PS/2 status register
fn read_status() -> u8 {
    unsafe {
        let value: u8;
        core::arch::asm!(
            "in al, dx",
            in("dx") PS2_STATUS_PORT,
            out("al") value,
        );
        value
    }
}

/// Write to PS/2 command register
fn write_command(command: u8) {
    // Wait for input buffer to be empty
    while (read_status() & 0x02) != 0 {}
    
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") PS2_STATUS_PORT,
            in("al") command,
        );
    }
}

/// Set keyboard LEDs
pub fn set_leds(scroll_lock: bool, num_lock: bool, caps_lock: bool) {
    let mut led_byte = 0u8;
    if scroll_lock { led_byte |= 1; }
    if num_lock { led_byte |= 2; }
    if caps_lock { led_byte |= 4; }
    
    write_data(0xED);  // Set LED command
    write_data(led_byte);
}
