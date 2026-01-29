//! USB HID (Human Interface Device) Driver
//!
//! Implements USB HID class driver for keyboards, mice, gamepads, etc.

use super::{InputDevice, InputDeviceType, InputEvent, InputEventType, InputEventData,
            KeyEvent, KeyCode, Modifiers, MouseButtonEvent, MouseMoveEvent, MouseScrollEvent,
            MouseButton};
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

/// HID descriptor types
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidDescriptorType {
    Hid = 0x21,
    Report = 0x22,
    Physical = 0x23,
}

/// HID report types
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidReportType {
    Input = 1,
    Output = 2,
    Feature = 3,
}

/// HID protocol values
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidProtocol {
    Boot = 0,
    Report = 1,
}

/// HID usage page values
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidUsagePage {
    GenericDesktop = 0x01,
    Simulation = 0x02,
    Vr = 0x03,
    Sport = 0x04,
    Game = 0x05,
    GenericDevice = 0x06,
    Keyboard = 0x07,
    Led = 0x08,
    Button = 0x09,
    Ordinal = 0x0A,
    Consumer = 0x0C,
    Digitizer = 0x0D,
}

/// Generic desktop usage IDs
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenericDesktopUsage {
    Pointer = 0x01,
    Mouse = 0x02,
    Joystick = 0x04,
    Gamepad = 0x05,
    Keyboard = 0x06,
    Keypad = 0x07,
    MultiAxisController = 0x08,
    TabletPc = 0x09,
    X = 0x30,
    Y = 0x31,
    Z = 0x32,
    Rx = 0x33,
    Ry = 0x34,
    Rz = 0x35,
    Slider = 0x36,
    Dial = 0x37,
    Wheel = 0x38,
    HatSwitch = 0x39,
}

/// HID descriptor
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct HidDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub bcd_hid: u16,
    pub country_code: u8,
    pub num_descriptors: u8,
    pub report_descriptor_type: u8,
    pub report_descriptor_length: u16,
}

/// HID report field descriptor (parsed from report descriptor)
#[derive(Debug, Clone)]
pub struct HidReportField {
    /// Usage page
    pub usage_page: u16,
    /// Usage ID
    pub usage_id: u16,
    /// Report size in bits
    pub report_size: u8,
    /// Report count
    pub report_count: u8,
    /// Logical minimum
    pub logical_min: i32,
    /// Logical maximum
    pub logical_max: i32,
    /// Is relative value
    pub is_relative: bool,
    /// Is variable (vs array)
    pub is_variable: bool,
}

/// Parsed HID report descriptor
#[derive(Debug, Clone)]
pub struct HidReportDescriptor {
    /// Input fields
    pub input_fields: Vec<HidReportField>,
    /// Output fields
    pub output_fields: Vec<HidReportField>,
    /// Feature fields
    pub feature_fields: Vec<HidReportField>,
    /// Total input report size in bytes
    pub input_size: u16,
    /// Total output report size in bytes
    pub output_size: u16,
}

impl HidReportDescriptor {
    /// Parse a raw report descriptor
    pub fn parse(data: &[u8]) -> Result<Self, HidError> {
        let mut input_fields = Vec::new();
        let mut output_fields = Vec::new();
        let mut feature_fields = Vec::new();
        
        // Parser state
        let mut usage_page: u16 = 0;
        let mut usage_id: u16 = 0;
        let mut report_size: u8 = 0;
        let mut report_count: u8 = 0;
        let mut logical_min: i32 = 0;
        let mut logical_max: i32 = 0;
        
        let mut i = 0;
        while i < data.len() {
            let item = data[i];
            let size = match item & 0x03 {
                0 => 0,
                1 => 1,
                2 => 2,
                3 => 4,
                _ => 0,
            };
            
            let item_type = (item >> 2) & 0x03;
            let tag = (item >> 4) & 0x0F;
            
            // Read data
            let value = match size {
                0 => 0u32,
                1 if i + 1 < data.len() => data[i + 1] as u32,
                2 if i + 2 < data.len() => u16::from_le_bytes([data[i + 1], data[i + 2]]) as u32,
                4 if i + 4 < data.len() => u32::from_le_bytes([data[i + 1], data[i + 2], data[i + 3], data[i + 4]]),
                _ => 0,
            };
            
            match item_type {
                // Main items
                0 => {
                    match tag {
                        // Input
                        8 => {
                            input_fields.push(HidReportField {
                                usage_page,
                                usage_id,
                                report_size,
                                report_count,
                                logical_min,
                                logical_max,
                                is_relative: (value & 0x04) != 0,
                                is_variable: (value & 0x02) != 0,
                            });
                        }
                        // Output
                        9 => {
                            output_fields.push(HidReportField {
                                usage_page,
                                usage_id,
                                report_size,
                                report_count,
                                logical_min,
                                logical_max,
                                is_relative: (value & 0x04) != 0,
                                is_variable: (value & 0x02) != 0,
                            });
                        }
                        // Feature
                        11 => {
                            feature_fields.push(HidReportField {
                                usage_page,
                                usage_id,
                                report_size,
                                report_count,
                                logical_min,
                                logical_max,
                                is_relative: (value & 0x04) != 0,
                                is_variable: (value & 0x02) != 0,
                            });
                        }
                        _ => {}
                    }
                }
                // Global items
                1 => {
                    match tag {
                        0 => usage_page = value as u16,
                        1 => logical_min = value as i32,
                        2 => logical_max = value as i32,
                        7 => report_size = value as u8,
                        9 => report_count = value as u8,
                        _ => {}
                    }
                }
                // Local items
                2 => {
                    match tag {
                        0 => usage_id = value as u16,
                        _ => {}
                    }
                }
                _ => {}
            }
            
            i += 1 + size as usize;
        }
        
        // Calculate sizes
        let input_size: u16 = input_fields.iter()
            .map(|f| f.report_size as u16 * f.report_count as u16)
            .sum::<u16>() / 8;
        let output_size: u16 = output_fields.iter()
            .map(|f| f.report_size as u16 * f.report_count as u16)
            .sum::<u16>() / 8;
        
        Ok(Self {
            input_fields,
            output_fields,
            feature_fields,
            input_size,
            output_size,
        })
    }
}

/// HID device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidDeviceType {
    Keyboard,
    Mouse,
    Gamepad,
    Joystick,
    Touchscreen,
    Stylus,
    Consumer,
    Unknown,
}

/// HID error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidError {
    InvalidDescriptor,
    DeviceNotFound,
    TransferError,
    ProtocolError,
    BufferTooSmall,
}

/// USB HID device
pub struct UsbHidDevice {
    /// Device name
    name: String,
    /// HID device type
    device_type: HidDeviceType,
    /// USB address
    usb_address: u8,
    /// Endpoint address
    endpoint: u8,
    /// Report descriptor
    report_descriptor: HidReportDescriptor,
    /// Last report data
    last_report: Vec<u8>,
    /// Pending events
    pending_events: Vec<InputEvent>,
    /// Keyboard modifier state
    modifiers: u8,
    /// Mouse button state
    mouse_buttons: u8,
}

impl UsbHidDevice {
    /// Create a new HID device
    pub fn new(
        name: &str,
        device_type: HidDeviceType,
        usb_address: u8,
        endpoint: u8,
        report_descriptor: HidReportDescriptor,
    ) -> Self {
        let report_size = report_descriptor.input_size as usize;
        Self {
            name: String::from(name),
            device_type,
            usb_address,
            endpoint,
            report_descriptor,
            last_report: vec![0u8; report_size],
            pending_events: Vec::new(),
            modifiers: 0,
            mouse_buttons: 0,
        }
    }

    /// Process an input report
    pub fn process_report(&mut self, report: &[u8], timestamp: u64) {
        match self.device_type {
            HidDeviceType::Keyboard => self.process_keyboard_report(report, timestamp),
            HidDeviceType::Mouse => self.process_mouse_report(report, timestamp),
            HidDeviceType::Gamepad => self.process_gamepad_report(report, timestamp),
            _ => {}
        }
        
        // Save report
        if report.len() == self.last_report.len() {
            self.last_report.copy_from_slice(report);
        }
    }

    /// Convert modifier byte to Modifiers struct
    fn byte_to_modifiers(byte: u8) -> Modifiers {
        Modifiers {
            ctrl: (byte & 0x11) != 0,  // Left or Right Ctrl
            shift: (byte & 0x22) != 0, // Left or Right Shift
            alt: (byte & 0x44) != 0,   // Left or Right Alt
            meta: (byte & 0x88) != 0,  // Left or Right Meta
            caps_lock: false,
            num_lock: false,
            scroll_lock: false,
        }
    }

    /// Process boot protocol keyboard report
    fn process_keyboard_report(&mut self, report: &[u8], timestamp: u64) {
        if report.len() < 8 {
            return;
        }

        // Boot protocol: [modifiers, reserved, key1, key2, key3, key4, key5, key6]
        let new_modifiers = report[0];
        
        // Check modifier changes
        let modifier_keys = [
            (0x01, KeyCode::LeftCtrl),
            (0x02, KeyCode::LeftShift),
            (0x04, KeyCode::LeftAlt),
            (0x08, KeyCode::LeftSuper),
            (0x10, KeyCode::RightCtrl),
            (0x20, KeyCode::RightShift),
            (0x40, KeyCode::RightAlt),
            (0x80, KeyCode::RightSuper),
        ];
        
        for (mask, keycode) in modifier_keys.iter() {
            let was_pressed = (self.modifiers & mask) != 0;
            let is_pressed = (new_modifiers & mask) != 0;
            
            if is_pressed != was_pressed {
                self.pending_events.push(InputEvent {
                    event_type: InputEventType::Key,
                    timestamp,
                    data: InputEventData::Key(KeyEvent {
                        scancode: *keycode as u16,
                        keycode: *keycode,
                        pressed: is_pressed,
                        modifiers: Self::byte_to_modifiers(new_modifiers),
                    }),
                });
            }
        }
        self.modifiers = new_modifiers;

        // Check key changes
        let old_keys = &self.last_report[2..8];
        let new_keys = &report[2..8];

        // Find released keys
        for &old_key in old_keys {
            if old_key != 0 && !new_keys.contains(&old_key) {
                if let Some(keycode) = self.hid_to_keycode(old_key) {
                    self.pending_events.push(InputEvent {
                        event_type: InputEventType::Key,
                        timestamp,
                        data: InputEventData::Key(KeyEvent {
                            scancode: old_key as u16,
                            keycode,
                            pressed: false,
                            modifiers: Self::byte_to_modifiers(new_modifiers),
                        }),
                    });
                }
            }
        }

        // Find pressed keys
        for &new_key in new_keys {
            if new_key != 0 && !old_keys.contains(&new_key) {
                if let Some(keycode) = self.hid_to_keycode(new_key) {
                    self.pending_events.push(InputEvent {
                        event_type: InputEventType::Key,
                        timestamp,
                        data: InputEventData::Key(KeyEvent {
                            scancode: new_key as u16,
                            keycode,
                            pressed: true,
                            modifiers: Self::byte_to_modifiers(new_modifiers),
                        }),
                    });
                }
            }
        }
    }

    /// Process boot protocol mouse report
    fn process_mouse_report(&mut self, report: &[u8], timestamp: u64) {
        if report.len() < 3 {
            return;
        }

        // Boot protocol: [buttons, dx, dy] or [buttons, dx, dy, wheel]
        let buttons = report[0];
        let dx = report[1] as i8 as i32;
        let dy = report[2] as i8 as i32;
        let wheel = if report.len() > 3 { report[3] as i8 as i32 } else { 0 };

        // Check button changes
        let button_map = [
            (0x01, MouseButton::Left),
            (0x02, MouseButton::Right),
            (0x04, MouseButton::Middle),
            (0x08, MouseButton::Back),
            (0x10, MouseButton::Forward),
        ];

        for (mask, button) in button_map.iter() {
            let was_pressed = (self.mouse_buttons & mask) != 0;
            let is_pressed = (buttons & mask) != 0;

            if is_pressed != was_pressed {
                self.pending_events.push(InputEvent {
                    event_type: InputEventType::MouseButton,
                    timestamp,
                    data: InputEventData::MouseButton(MouseButtonEvent {
                        button: *button,
                        pressed: is_pressed,
                        x: 0,
                        y: 0,
                    }),
                });
            }
        }
        self.mouse_buttons = buttons;

        // Generate movement event
        if dx != 0 || dy != 0 {
            self.pending_events.push(InputEvent {
                event_type: InputEventType::MouseMove,
                timestamp,
                data: InputEventData::MouseMove(MouseMoveEvent {
                    dx,
                    dy,
                    x: 0,
                    y: 0,
                }),
            });
        }

        // Generate scroll event
        if wheel != 0 {
            self.pending_events.push(InputEvent {
                event_type: InputEventType::MouseScroll,
                timestamp,
                data: InputEventData::MouseScroll(MouseScrollEvent {
                    dx: 0,
                    dy: wheel * 3,
                }),
            });
        }
    }

    /// Process gamepad report
    fn process_gamepad_report(&mut self, _report: &[u8], _timestamp: u64) {
        // Gamepad reports vary significantly by device
        // Would need device-specific handling
    }

    /// Convert HID usage to KeyCode
    fn hid_to_keycode(&self, usage: u8) -> Option<KeyCode> {
        match usage {
            0x04 => Some(KeyCode::A),
            0x05 => Some(KeyCode::B),
            0x06 => Some(KeyCode::C),
            0x07 => Some(KeyCode::D),
            0x08 => Some(KeyCode::E),
            0x09 => Some(KeyCode::F),
            0x0A => Some(KeyCode::G),
            0x0B => Some(KeyCode::H),
            0x0C => Some(KeyCode::I),
            0x0D => Some(KeyCode::J),
            0x0E => Some(KeyCode::K),
            0x0F => Some(KeyCode::L),
            0x10 => Some(KeyCode::M),
            0x11 => Some(KeyCode::N),
            0x12 => Some(KeyCode::O),
            0x13 => Some(KeyCode::P),
            0x14 => Some(KeyCode::Q),
            0x15 => Some(KeyCode::R),
            0x16 => Some(KeyCode::S),
            0x17 => Some(KeyCode::T),
            0x18 => Some(KeyCode::U),
            0x19 => Some(KeyCode::V),
            0x1A => Some(KeyCode::W),
            0x1B => Some(KeyCode::X),
            0x1C => Some(KeyCode::Y),
            0x1D => Some(KeyCode::Z),
            0x1E => Some(KeyCode::Key1),
            0x1F => Some(KeyCode::Key2),
            0x20 => Some(KeyCode::Key3),
            0x21 => Some(KeyCode::Key4),
            0x22 => Some(KeyCode::Key5),
            0x23 => Some(KeyCode::Key6),
            0x24 => Some(KeyCode::Key7),
            0x25 => Some(KeyCode::Key8),
            0x26 => Some(KeyCode::Key9),
            0x27 => Some(KeyCode::Key0),
            0x28 => Some(KeyCode::Enter),
            0x29 => Some(KeyCode::Escape),
            0x2A => Some(KeyCode::Backspace),
            0x2B => Some(KeyCode::Tab),
            0x2C => Some(KeyCode::Space),
            0x2D => Some(KeyCode::Minus),
            0x2E => Some(KeyCode::Equal),
            0x2F => Some(KeyCode::LeftBracket),
            0x30 => Some(KeyCode::RightBracket),
            0x31 => Some(KeyCode::Backslash),
            0x33 => Some(KeyCode::Semicolon),
            0x34 => Some(KeyCode::Apostrophe),
            0x35 => Some(KeyCode::Grave),
            0x36 => Some(KeyCode::Comma),
            0x37 => Some(KeyCode::Period),
            0x38 => Some(KeyCode::Slash),
            0x39 => Some(KeyCode::CapsLock),
            0x3A => Some(KeyCode::F1),
            0x3B => Some(KeyCode::F2),
            0x3C => Some(KeyCode::F3),
            0x3D => Some(KeyCode::F4),
            0x3E => Some(KeyCode::F5),
            0x3F => Some(KeyCode::F6),
            0x40 => Some(KeyCode::F7),
            0x41 => Some(KeyCode::F8),
            0x42 => Some(KeyCode::F9),
            0x43 => Some(KeyCode::F10),
            0x44 => Some(KeyCode::F11),
            0x45 => Some(KeyCode::F12),
            0x46 => Some(KeyCode::PrintScreen),
            0x47 => Some(KeyCode::ScrollLock),
            0x48 => Some(KeyCode::Pause),
            0x49 => Some(KeyCode::Insert),
            0x4A => Some(KeyCode::Home),
            0x4B => Some(KeyCode::PageUp),
            0x4C => Some(KeyCode::Delete),
            0x4D => Some(KeyCode::End),
            0x4E => Some(KeyCode::PageDown),
            0x4F => Some(KeyCode::Right),
            0x50 => Some(KeyCode::Left),
            0x51 => Some(KeyCode::Down),
            0x52 => Some(KeyCode::Up),
            0x53 => Some(KeyCode::NumLock),
            0x54 => Some(KeyCode::NumpadDivide),
            0x55 => Some(KeyCode::NumpadMultiply),
            0x56 => Some(KeyCode::NumpadSubtract),
            0x57 => Some(KeyCode::NumpadAdd),
            0x58 => Some(KeyCode::NumpadEnter),
            0x59 => Some(KeyCode::Numpad1),
            0x5A => Some(KeyCode::Numpad2),
            0x5B => Some(KeyCode::Numpad3),
            0x5C => Some(KeyCode::Numpad4),
            0x5D => Some(KeyCode::Numpad5),
            0x5E => Some(KeyCode::Numpad6),
            0x5F => Some(KeyCode::Numpad7),
            0x60 => Some(KeyCode::Numpad8),
            0x61 => Some(KeyCode::Numpad9),
            0x62 => Some(KeyCode::Numpad0),
            0x63 => Some(KeyCode::NumpadDecimal),
            _ => None,
        }
    }
}

impl InputDevice for UsbHidDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn device_type(&self) -> InputDeviceType {
        match self.device_type {
            HidDeviceType::Keyboard => InputDeviceType::Keyboard,
            HidDeviceType::Mouse => InputDeviceType::Mouse,
            HidDeviceType::Gamepad => InputDeviceType::Gamepad,
            HidDeviceType::Touchscreen => InputDeviceType::Touchscreen,
            _ => InputDeviceType::Unknown,
        }
    }

    fn poll(&mut self) -> Vec<InputEvent> {
        core::mem::take(&mut self.pending_events)
    }

    fn is_connected(&self) -> bool {
        true
    }
}

/// HID manager for handling multiple HID devices
pub struct HidManager {
    /// Connected devices
    devices: Vec<UsbHidDevice>,
}

impl HidManager {
    /// Create a new HID manager
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    /// Add a device
    pub fn add_device(&mut self, device: UsbHidDevice) {
        self.devices.push(device);
    }

    /// Remove a device by USB address
    pub fn remove_device(&mut self, usb_address: u8) {
        self.devices.retain(|d| d.usb_address != usb_address);
    }

    /// Process report for a device
    pub fn process_report(&mut self, usb_address: u8, report: &[u8], timestamp: u64) {
        for device in &mut self.devices {
            if device.usb_address == usb_address {
                device.process_report(report, timestamp);
                break;
            }
        }
    }

    /// Poll all devices
    pub fn poll_all(&mut self) -> Vec<InputEvent> {
        let mut events = Vec::new();
        for device in &mut self.devices {
            events.extend(device.poll());
        }
        events
    }
}

impl Default for HidManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global HID manager
static mut HID_MANAGER: Option<HidManager> = None;

/// Initialize HID subsystem
pub fn init() {
    unsafe {
        HID_MANAGER = Some(HidManager::new());
    }
}

/// Get HID manager reference
pub fn manager() -> Option<&'static HidManager> {
    unsafe { HID_MANAGER.as_ref() }
}
