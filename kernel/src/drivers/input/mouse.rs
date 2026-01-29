//! PS/2 Mouse Driver
//!
//! Driver for PS/2 mouse input.

use super::{InputDevice, InputDeviceType, InputEvent, InputEventType, InputEventData, 
            MouseButtonEvent, MouseMoveEvent, MouseScrollEvent, MouseButton};
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

/// Mouse packet states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MouseState {
    /// Waiting for first byte
    WaitingForByte1,
    /// Waiting for second byte (X movement)
    WaitingForByte2,
    /// Waiting for third byte (Y movement)
    WaitingForByte3,
    /// Waiting for fourth byte (scroll wheel, if enabled)
    WaitingForByte4,
}

/// PS/2 mouse state
pub struct Ps2Mouse {
    /// Device name
    name: String,
    /// Current state in packet reception
    state: MouseState,
    /// Packet bytes
    packet: [u8; 4],
    /// Is scroll wheel enabled (Intellimouse)
    scroll_enabled: bool,
    /// Is 5-button mode enabled
    five_button_enabled: bool,
    /// Current X position
    x: i32,
    /// Current Y position
    y: i32,
    /// Screen width
    screen_width: i32,
    /// Screen height
    screen_height: i32,
    /// Current button state
    buttons: u8,
    /// Pending events
    pending_events: Vec<InputEvent>,
}

impl Ps2Mouse {
    /// Create a new PS/2 mouse
    pub fn new(screen_width: i32, screen_height: i32) -> Self {
        Self {
            name: String::from("PS/2 Mouse"),
            state: MouseState::WaitingForByte1,
            packet: [0; 4],
            scroll_enabled: false,
            five_button_enabled: false,
            x: screen_width / 2,
            y: screen_height / 2,
            screen_width,
            screen_height,
            buttons: 0,
            pending_events: Vec::new(),
        }
    }

    /// Handle incoming byte
    pub fn handle_byte(&mut self, byte: u8) {
        match self.state {
            MouseState::WaitingForByte1 => {
                // First byte: buttons and sign bits
                // Bit 3 should always be 1 for valid packets
                if (byte & 0x08) == 0 {
                    // Invalid packet, resync
                    return;
                }
                self.packet[0] = byte;
                self.state = MouseState::WaitingForByte2;
            }
            MouseState::WaitingForByte2 => {
                self.packet[1] = byte;
                self.state = MouseState::WaitingForByte3;
            }
            MouseState::WaitingForByte3 => {
                self.packet[2] = byte;
                if self.scroll_enabled {
                    self.state = MouseState::WaitingForByte4;
                } else {
                    self.process_packet();
                    self.state = MouseState::WaitingForByte1;
                }
            }
            MouseState::WaitingForByte4 => {
                self.packet[3] = byte;
                self.process_packet();
                self.state = MouseState::WaitingForByte1;
            }
        }
    }

    /// Process a complete packet
    fn process_packet(&mut self) {
        let flags = self.packet[0];
        
        // X and Y movement (with sign extension)
        let mut dx = self.packet[1] as i32;
        let mut dy = self.packet[2] as i32;
        
        // Apply sign extension
        if (flags & 0x10) != 0 {
            dx |= !0xFF;
        }
        if (flags & 0x20) != 0 {
            dy |= !0xFF;
        }
        
        // Y is inverted in PS/2
        dy = -dy;
        
        // Check for overflow
        if (flags & 0x40) != 0 || (flags & 0x80) != 0 {
            // Overflow, discard
            return;
        }

        // Update position
        let old_x = self.x;
        let old_y = self.y;
        
        self.x = (self.x + dx).clamp(0, self.screen_width - 1);
        self.y = (self.y + dy).clamp(0, self.screen_height - 1);

        // Generate move event if position changed
        if self.x != old_x || self.y != old_y {
            self.pending_events.push(InputEvent {
                event_type: InputEventType::MouseMove,
                timestamp: get_timestamp(),
                data: InputEventData::MouseMove(MouseMoveEvent {
                    dx,
                    dy,
                    x: self.x,
                    y: self.y,
                }),
            });
        }

        // Handle buttons
        let new_buttons = flags & 0x07;
        let changed = self.buttons ^ new_buttons;
        
        if (changed & 0x01) != 0 {
            self.pending_events.push(InputEvent {
                event_type: InputEventType::MouseButton,
                timestamp: get_timestamp(),
                data: InputEventData::MouseButton(MouseButtonEvent {
                    button: MouseButton::Left,
                    pressed: (new_buttons & 0x01) != 0,
                    x: self.x,
                    y: self.y,
                }),
            });
        }
        
        if (changed & 0x02) != 0 {
            self.pending_events.push(InputEvent {
                event_type: InputEventType::MouseButton,
                timestamp: get_timestamp(),
                data: InputEventData::MouseButton(MouseButtonEvent {
                    button: MouseButton::Right,
                    pressed: (new_buttons & 0x02) != 0,
                    x: self.x,
                    y: self.y,
                }),
            });
        }
        
        if (changed & 0x04) != 0 {
            self.pending_events.push(InputEvent {
                event_type: InputEventType::MouseButton,
                timestamp: get_timestamp(),
                data: InputEventData::MouseButton(MouseButtonEvent {
                    button: MouseButton::Middle,
                    pressed: (new_buttons & 0x04) != 0,
                    x: self.x,
                    y: self.y,
                }),
            });
        }
        
        self.buttons = new_buttons;

        // Handle scroll wheel (if enabled)
        if self.scroll_enabled {
            let scroll_byte = self.packet[3];
            
            // Scroll data is in bits 0-3 with sign
            let scroll = if (scroll_byte & 0x08) != 0 {
                (scroll_byte | 0xF0) as i8 as i32
            } else {
                (scroll_byte & 0x0F) as i32
            };
            
            if scroll != 0 {
                self.pending_events.push(InputEvent {
                    event_type: InputEventType::MouseScroll,
                    timestamp: get_timestamp(),
                    data: InputEventData::MouseScroll(MouseScrollEvent {
                        dx: 0,
                        dy: scroll,
                    }),
                });
            }
            
            // Handle extra buttons (if 5-button mode)
            if self.five_button_enabled {
                let button4 = (scroll_byte & 0x10) != 0;
                let button5 = (scroll_byte & 0x20) != 0;
                
                // Would generate events for back/forward buttons
                let _ = (button4, button5);
            }
        }
    }

    /// Enable scroll wheel (Intellimouse initialization)
    pub fn enable_scroll(&mut self) {
        self.scroll_enabled = true;
    }

    /// Set screen dimensions
    pub fn set_screen_size(&mut self, width: i32, height: i32) {
        self.screen_width = width;
        self.screen_height = height;
        // Clamp current position
        self.x = self.x.clamp(0, width - 1);
        self.y = self.y.clamp(0, height - 1);
    }
}

impl InputDevice for Ps2Mouse {
    fn name(&self) -> &str {
        &self.name
    }

    fn device_type(&self) -> InputDeviceType {
        InputDeviceType::Mouse
    }

    fn poll(&mut self) -> Vec<InputEvent> {
        core::mem::take(&mut self.pending_events)
    }

    fn is_connected(&self) -> bool {
        true
    }
}

/// Get current timestamp (placeholder)
fn get_timestamp() -> u64 {
    0
}

/// Global PS/2 mouse instance
static PS2_MOUSE: Mutex<Option<Ps2Mouse>> = Mutex::new(None);

/// Initialize PS/2 mouse
pub fn init() {
    // Initialize with default screen size
    // Real implementation would get actual screen size
    let mouse = Ps2Mouse::new(1920, 1080);
    
    // Enable mouse in PS/2 controller
    // Write 0xA8 to command port to enable second port
    // Write 0xD4 then 0xF4 to enable mouse data reporting
    
    // Try to enable scroll wheel (Intellimouse)
    // This requires sending specific sample rate sequence:
    // 200, 100, 80 sample rates
    // Then check device ID
    
    *PS2_MOUSE.lock() = Some(mouse);
}

/// Handle mouse interrupt
pub fn handle_interrupt() {
    let byte = read_data();
    if let Some(ref mut mouse) = *PS2_MOUSE.lock() {
        mouse.handle_byte(byte);
    }
}

/// Read from PS/2 data port
fn read_data() -> u8 {
    unsafe {
        let value: u8;
        core::arch::asm!(
            "in al, dx",
            in("dx") 0x60u16,
            out("al") value,
        );
        value
    }
}

/// Get current mouse position
pub fn get_position() -> (i32, i32) {
    if let Some(ref mouse) = *PS2_MOUSE.lock() {
        (mouse.x, mouse.y)
    } else {
        (0, 0)
    }
}

/// Set screen size for mouse bounds
pub fn set_screen_size(width: i32, height: i32) {
    if let Some(ref mut mouse) = *PS2_MOUSE.lock() {
        mouse.set_screen_size(width, height);
    }
}
