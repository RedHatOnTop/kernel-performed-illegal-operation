//! Input Handling System
//!
//! This module provides comprehensive input event processing for the browser.
//! It converts raw keyboard and mouse events to DOM events and dispatches them
//! to the appropriate elements based on hit testing.

use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::events::{Event, EventPhase, EventType, KeyboardEvent, MouseEvent};

/// Raw keyboard scancode to key mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scancode(pub u8);

impl Scancode {
    // Common scancodes (PS/2 Set 1)
    pub const ESCAPE: Scancode = Scancode(0x01);
    pub const KEY_1: Scancode = Scancode(0x02);
    pub const KEY_2: Scancode = Scancode(0x03);
    pub const KEY_3: Scancode = Scancode(0x04);
    pub const KEY_4: Scancode = Scancode(0x05);
    pub const KEY_5: Scancode = Scancode(0x06);
    pub const KEY_6: Scancode = Scancode(0x07);
    pub const KEY_7: Scancode = Scancode(0x08);
    pub const KEY_8: Scancode = Scancode(0x09);
    pub const KEY_9: Scancode = Scancode(0x0A);
    pub const KEY_0: Scancode = Scancode(0x0B);
    pub const MINUS: Scancode = Scancode(0x0C);
    pub const EQUALS: Scancode = Scancode(0x0D);
    pub const BACKSPACE: Scancode = Scancode(0x0E);
    pub const TAB: Scancode = Scancode(0x0F);
    pub const KEY_Q: Scancode = Scancode(0x10);
    pub const KEY_W: Scancode = Scancode(0x11);
    pub const KEY_E: Scancode = Scancode(0x12);
    pub const KEY_R: Scancode = Scancode(0x13);
    pub const KEY_T: Scancode = Scancode(0x14);
    pub const KEY_Y: Scancode = Scancode(0x15);
    pub const KEY_U: Scancode = Scancode(0x16);
    pub const KEY_I: Scancode = Scancode(0x17);
    pub const KEY_O: Scancode = Scancode(0x18);
    pub const KEY_P: Scancode = Scancode(0x19);
    pub const LEFT_BRACKET: Scancode = Scancode(0x1A);
    pub const RIGHT_BRACKET: Scancode = Scancode(0x1B);
    pub const ENTER: Scancode = Scancode(0x1C);
    pub const LEFT_CTRL: Scancode = Scancode(0x1D);
    pub const KEY_A: Scancode = Scancode(0x1E);
    pub const KEY_S: Scancode = Scancode(0x1F);
    pub const KEY_D: Scancode = Scancode(0x20);
    pub const KEY_F: Scancode = Scancode(0x21);
    pub const KEY_G: Scancode = Scancode(0x22);
    pub const KEY_H: Scancode = Scancode(0x23);
    pub const KEY_J: Scancode = Scancode(0x24);
    pub const KEY_K: Scancode = Scancode(0x25);
    pub const KEY_L: Scancode = Scancode(0x26);
    pub const SEMICOLON: Scancode = Scancode(0x27);
    pub const APOSTROPHE: Scancode = Scancode(0x28);
    pub const GRAVE: Scancode = Scancode(0x29);
    pub const LEFT_SHIFT: Scancode = Scancode(0x2A);
    pub const BACKSLASH: Scancode = Scancode(0x2B);
    pub const KEY_Z: Scancode = Scancode(0x2C);
    pub const KEY_X: Scancode = Scancode(0x2D);
    pub const KEY_C: Scancode = Scancode(0x2E);
    pub const KEY_V: Scancode = Scancode(0x2F);
    pub const KEY_B: Scancode = Scancode(0x30);
    pub const KEY_N: Scancode = Scancode(0x31);
    pub const KEY_M: Scancode = Scancode(0x32);
    pub const COMMA: Scancode = Scancode(0x33);
    pub const PERIOD: Scancode = Scancode(0x34);
    pub const SLASH: Scancode = Scancode(0x35);
    pub const RIGHT_SHIFT: Scancode = Scancode(0x36);
    pub const LEFT_ALT: Scancode = Scancode(0x38);
    pub const SPACE: Scancode = Scancode(0x39);
    pub const CAPS_LOCK: Scancode = Scancode(0x3A);
    pub const F1: Scancode = Scancode(0x3B);
    pub const F2: Scancode = Scancode(0x3C);
    pub const F3: Scancode = Scancode(0x3D);
    pub const F4: Scancode = Scancode(0x3E);
    pub const F5: Scancode = Scancode(0x3F);
    pub const F6: Scancode = Scancode(0x40);
    pub const F7: Scancode = Scancode(0x41);
    pub const F8: Scancode = Scancode(0x42);
    pub const F9: Scancode = Scancode(0x43);
    pub const F10: Scancode = Scancode(0x44);
    pub const F11: Scancode = Scancode(0x57);
    pub const F12: Scancode = Scancode(0x58);
    pub const ARROW_UP: Scancode = Scancode(0x48);
    pub const ARROW_LEFT: Scancode = Scancode(0x4B);
    pub const ARROW_RIGHT: Scancode = Scancode(0x4D);
    pub const ARROW_DOWN: Scancode = Scancode(0x50);
    pub const HOME: Scancode = Scancode(0x47);
    pub const END: Scancode = Scancode(0x4F);
    pub const PAGE_UP: Scancode = Scancode(0x49);
    pub const PAGE_DOWN: Scancode = Scancode(0x51);
    pub const INSERT: Scancode = Scancode(0x52);
    pub const DELETE: Scancode = Scancode(0x53);
}

/// Virtual key codes (similar to JavaScript key codes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualKey(pub u32);

impl VirtualKey {
    pub const BACKSPACE: VirtualKey = VirtualKey(8);
    pub const TAB: VirtualKey = VirtualKey(9);
    pub const ENTER: VirtualKey = VirtualKey(13);
    pub const SHIFT: VirtualKey = VirtualKey(16);
    pub const CTRL: VirtualKey = VirtualKey(17);
    pub const ALT: VirtualKey = VirtualKey(18);
    pub const PAUSE: VirtualKey = VirtualKey(19);
    pub const CAPS_LOCK: VirtualKey = VirtualKey(20);
    pub const ESCAPE: VirtualKey = VirtualKey(27);
    pub const SPACE: VirtualKey = VirtualKey(32);
    pub const PAGE_UP: VirtualKey = VirtualKey(33);
    pub const PAGE_DOWN: VirtualKey = VirtualKey(34);
    pub const END: VirtualKey = VirtualKey(35);
    pub const HOME: VirtualKey = VirtualKey(36);
    pub const ARROW_LEFT: VirtualKey = VirtualKey(37);
    pub const ARROW_UP: VirtualKey = VirtualKey(38);
    pub const ARROW_RIGHT: VirtualKey = VirtualKey(39);
    pub const ARROW_DOWN: VirtualKey = VirtualKey(40);
    pub const INSERT: VirtualKey = VirtualKey(45);
    pub const DELETE: VirtualKey = VirtualKey(46);
    pub const KEY_0: VirtualKey = VirtualKey(48);
    pub const KEY_1: VirtualKey = VirtualKey(49);
    pub const KEY_2: VirtualKey = VirtualKey(50);
    pub const KEY_3: VirtualKey = VirtualKey(51);
    pub const KEY_4: VirtualKey = VirtualKey(52);
    pub const KEY_5: VirtualKey = VirtualKey(53);
    pub const KEY_6: VirtualKey = VirtualKey(54);
    pub const KEY_7: VirtualKey = VirtualKey(55);
    pub const KEY_8: VirtualKey = VirtualKey(56);
    pub const KEY_9: VirtualKey = VirtualKey(57);
    pub const KEY_A: VirtualKey = VirtualKey(65);
    pub const KEY_B: VirtualKey = VirtualKey(66);
    pub const KEY_C: VirtualKey = VirtualKey(67);
    pub const KEY_D: VirtualKey = VirtualKey(68);
    pub const KEY_E: VirtualKey = VirtualKey(69);
    pub const KEY_F: VirtualKey = VirtualKey(70);
    pub const KEY_G: VirtualKey = VirtualKey(71);
    pub const KEY_H: VirtualKey = VirtualKey(72);
    pub const KEY_I: VirtualKey = VirtualKey(73);
    pub const KEY_J: VirtualKey = VirtualKey(74);
    pub const KEY_K: VirtualKey = VirtualKey(75);
    pub const KEY_L: VirtualKey = VirtualKey(76);
    pub const KEY_M: VirtualKey = VirtualKey(77);
    pub const KEY_N: VirtualKey = VirtualKey(78);
    pub const KEY_O: VirtualKey = VirtualKey(79);
    pub const KEY_P: VirtualKey = VirtualKey(80);
    pub const KEY_Q: VirtualKey = VirtualKey(81);
    pub const KEY_R: VirtualKey = VirtualKey(82);
    pub const KEY_S: VirtualKey = VirtualKey(83);
    pub const KEY_T: VirtualKey = VirtualKey(84);
    pub const KEY_U: VirtualKey = VirtualKey(85);
    pub const KEY_V: VirtualKey = VirtualKey(86);
    pub const KEY_W: VirtualKey = VirtualKey(87);
    pub const KEY_X: VirtualKey = VirtualKey(88);
    pub const KEY_Y: VirtualKey = VirtualKey(89);
    pub const KEY_Z: VirtualKey = VirtualKey(90);
    pub const F1: VirtualKey = VirtualKey(112);
    pub const F2: VirtualKey = VirtualKey(113);
    pub const F3: VirtualKey = VirtualKey(114);
    pub const F4: VirtualKey = VirtualKey(115);
    pub const F5: VirtualKey = VirtualKey(116);
    pub const F6: VirtualKey = VirtualKey(117);
    pub const F7: VirtualKey = VirtualKey(118);
    pub const F8: VirtualKey = VirtualKey(119);
    pub const F9: VirtualKey = VirtualKey(120);
    pub const F10: VirtualKey = VirtualKey(121);
    pub const F11: VirtualKey = VirtualKey(122);
    pub const F12: VirtualKey = VirtualKey(123);
}

/// Modifier key state.
#[derive(Debug, Clone, Copy, Default)]
pub struct ModifierState {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
    pub caps_lock: bool,
}

impl ModifierState {
    /// Create new modifier state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update modifier state based on scancode.
    pub fn update(&mut self, scancode: Scancode, pressed: bool) {
        match scancode {
            Scancode::LEFT_CTRL => self.ctrl = pressed,
            Scancode::LEFT_SHIFT | Scancode::RIGHT_SHIFT => self.shift = pressed,
            Scancode::LEFT_ALT => self.alt = pressed,
            Scancode::CAPS_LOCK if pressed => self.caps_lock = !self.caps_lock,
            _ => {}
        }
    }

    /// Check if any modifier is pressed.
    pub fn any(&self) -> bool {
        self.ctrl || self.alt || self.shift || self.meta
    }
}

/// Raw input event from the kernel.
#[derive(Debug, Clone)]
pub enum RawInputEvent {
    /// Keyboard scancode event.
    Keyboard { scancode: Scancode, pressed: bool },
    /// Mouse button event.
    MouseButton {
        button: u8,
        pressed: bool,
        x: i32,
        y: i32,
    },
    /// Mouse move event.
    MouseMove { x: i32, y: i32, dx: i32, dy: i32 },
    /// Mouse wheel event.
    MouseWheel {
        delta_x: i32,
        delta_y: i32,
        x: i32,
        y: i32,
    },
}

/// Processed input event for the DOM.
#[derive(Debug, Clone)]
pub enum DomInputEvent {
    /// Mouse event.
    Mouse(MouseEvent),
    /// Keyboard event.
    Keyboard(KeyboardEvent),
    /// Scroll event.
    Scroll {
        delta_x: i32,
        delta_y: i32,
        target_id: u64,
    },
}

/// Hit test result.
#[derive(Debug, Clone)]
pub struct HitTestResult {
    /// Element ID at the point.
    pub element_id: u64,
    /// Element path from root to target.
    pub path: Vec<u64>,
    /// Is element focusable?
    pub focusable: bool,
    /// Is element clickable (link, button, etc.)?
    pub clickable: bool,
    /// Cursor to display.
    pub cursor: CursorType,
}

/// Cursor types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorType {
    Default,
    Pointer,
    Text,
    Move,
    NotAllowed,
    Crosshair,
    Wait,
    Progress,
    ResizeNS,
    ResizeEW,
    ResizeNESW,
    ResizeNWSE,
}

impl Default for CursorType {
    fn default() -> Self {
        CursorType::Default
    }
}

/// Input manager for the browser.
pub struct InputManager {
    /// Current modifier state.
    modifiers: ModifierState,
    /// Event queue.
    event_queue: VecDeque<DomInputEvent>,
    /// Current mouse position.
    mouse_x: i32,
    mouse_y: i32,
    /// Current mouse button state.
    mouse_buttons: u8,
    /// Currently focused element ID.
    focused_element: Option<u64>,
    /// Currently hovered element ID.
    hovered_element: Option<u64>,
    /// Last click time for double-click detection.
    last_click_time: u64,
    /// Last click position.
    last_click_pos: (i32, i32),
    /// Click count.
    click_count: u8,
    /// Double-click threshold in ms.
    double_click_threshold: u64,
    /// Current timestamp.
    current_time: u64,
}

impl InputManager {
    /// Create a new input manager.
    pub fn new() -> Self {
        Self {
            modifiers: ModifierState::new(),
            event_queue: VecDeque::new(),
            mouse_x: 0,
            mouse_y: 0,
            mouse_buttons: 0,
            focused_element: None,
            hovered_element: None,
            last_click_time: 0,
            last_click_pos: (0, 0),
            click_count: 0,
            double_click_threshold: 500,
            current_time: 0,
        }
    }

    /// Update current timestamp.
    pub fn set_time(&mut self, time: u64) {
        self.current_time = time;
    }

    /// Process a raw input event.
    pub fn process_raw_event(
        &mut self,
        event: RawInputEvent,
        hit_test: impl Fn(i32, i32) -> Option<HitTestResult>,
    ) {
        match event {
            RawInputEvent::Keyboard { scancode, pressed } => {
                self.process_keyboard(scancode, pressed);
            }
            RawInputEvent::MouseButton {
                button,
                pressed,
                x,
                y,
            } => {
                self.mouse_x = x;
                self.mouse_y = y;
                self.process_mouse_button(button, pressed, &hit_test);
            }
            RawInputEvent::MouseMove { x, y, .. } => {
                self.mouse_x = x;
                self.mouse_y = y;
                self.process_mouse_move(&hit_test);
            }
            RawInputEvent::MouseWheel {
                delta_x,
                delta_y,
                x,
                y,
            } => {
                self.mouse_x = x;
                self.mouse_y = y;
                self.process_mouse_wheel(delta_x, delta_y, &hit_test);
            }
        }
    }

    /// Process keyboard event.
    fn process_keyboard(&mut self, scancode: Scancode, pressed: bool) {
        // Update modifiers
        self.modifiers.update(scancode, pressed);

        // Convert scancode to virtual key and key string
        let (vkey, key, code) = self.scancode_to_key(scancode);

        // Create keyboard event
        let event_type = if pressed {
            EventType::KeyDown
        } else {
            EventType::KeyUp
        };
        let mut kb_event = KeyboardEvent::new(event_type, &key, &code, vkey.0);
        kb_event.ctrl_key = self.modifiers.ctrl;
        kb_event.shift_key = self.modifiers.shift;
        kb_event.alt_key = self.modifiers.alt;
        kb_event.meta_key = self.modifiers.meta;
        kb_event.event.timestamp = self.current_time;
        kb_event.event.is_trusted = true;

        // Set target to focused element
        if let Some(focused) = self.focused_element {
            kb_event.event.target_id = focused;
        }

        self.event_queue
            .push_back(DomInputEvent::Keyboard(kb_event.clone()));

        // Generate keypress event for printable characters
        if pressed && self.is_printable_key(vkey) {
            let mut keypress = kb_event;
            keypress.event.event_type = EventType::KeyPress;
            keypress.char_code = self.key_to_char(vkey, self.modifiers.shift);
            self.event_queue
                .push_back(DomInputEvent::Keyboard(keypress));
        }
    }

    /// Process mouse button event.
    fn process_mouse_button(
        &mut self,
        button: u8,
        pressed: bool,
        hit_test: impl Fn(i32, i32) -> Option<HitTestResult>,
    ) {
        let target = hit_test(self.mouse_x, self.mouse_y);
        let target_id = target.as_ref().map(|t| t.element_id).unwrap_or(0);

        // Update button state
        if pressed {
            self.mouse_buttons |= 1 << button;
        } else {
            self.mouse_buttons &= !(1 << button);
        }

        // Generate mousedown/mouseup event
        let event_type = if pressed {
            EventType::MouseDown
        } else {
            EventType::MouseUp
        };
        let mut mouse_event = MouseEvent::new(event_type, self.mouse_x, self.mouse_y, button);
        mouse_event.buttons = self.mouse_buttons;
        mouse_event.ctrl_key = self.modifiers.ctrl;
        mouse_event.shift_key = self.modifiers.shift;
        mouse_event.alt_key = self.modifiers.alt;
        mouse_event.meta_key = self.modifiers.meta;
        mouse_event.event.target_id = target_id;
        mouse_event.event.timestamp = self.current_time;
        mouse_event.event.is_trusted = true;

        self.event_queue
            .push_back(DomInputEvent::Mouse(mouse_event));

        // Generate click event on mouse up
        if !pressed && button == 0 {
            self.generate_click_event(target_id);

            // Update focus
            if let Some(ref hit) = target {
                if hit.focusable && self.focused_element != Some(hit.element_id) {
                    self.set_focus(Some(hit.element_id));
                }
            }
        }
    }

    /// Generate click event.
    fn generate_click_event(&mut self, target_id: u64) {
        // Check for double click
        let time_diff = self.current_time.saturating_sub(self.last_click_time);
        let dist_x = (self.mouse_x - self.last_click_pos.0).abs();
        let dist_y = (self.mouse_y - self.last_click_pos.1).abs();

        if time_diff < self.double_click_threshold && dist_x < 5 && dist_y < 5 {
            self.click_count += 1;
        } else {
            self.click_count = 1;
        }

        self.last_click_time = self.current_time;
        self.last_click_pos = (self.mouse_x, self.mouse_y);

        // Generate click event
        let mut click_event = MouseEvent::new(EventType::Click, self.mouse_x, self.mouse_y, 0);
        click_event.event.target_id = target_id;
        click_event.event.timestamp = self.current_time;
        click_event.event.is_trusted = true;
        click_event.ctrl_key = self.modifiers.ctrl;
        click_event.shift_key = self.modifiers.shift;
        click_event.alt_key = self.modifiers.alt;
        click_event.meta_key = self.modifiers.meta;

        self.event_queue
            .push_back(DomInputEvent::Mouse(click_event));

        // Generate dblclick on second click
        if self.click_count == 2 {
            let mut dblclick_event =
                MouseEvent::new(EventType::DblClick, self.mouse_x, self.mouse_y, 0);
            dblclick_event.event.target_id = target_id;
            dblclick_event.event.timestamp = self.current_time;
            dblclick_event.event.is_trusted = true;

            self.event_queue
                .push_back(DomInputEvent::Mouse(dblclick_event));
            self.click_count = 0;
        }
    }

    /// Process mouse move event.
    fn process_mouse_move(&mut self, hit_test: impl Fn(i32, i32) -> Option<HitTestResult>) {
        let target = hit_test(self.mouse_x, self.mouse_y);
        let target_id = target.as_ref().map(|t| t.element_id).unwrap_or(0);

        // Generate mousemove event
        let mut move_event = MouseEvent::new(EventType::MouseMove, self.mouse_x, self.mouse_y, 0);
        move_event.buttons = self.mouse_buttons;
        move_event.event.target_id = target_id;
        move_event.event.timestamp = self.current_time;
        move_event.event.is_trusted = true;

        self.event_queue.push_back(DomInputEvent::Mouse(move_event));

        // Generate mouseenter/mouseleave events
        let new_hovered = Some(target_id);
        if self.hovered_element != new_hovered {
            // Mouse leave old element
            if let Some(old_id) = self.hovered_element {
                let mut leave_event =
                    MouseEvent::new(EventType::MouseLeave, self.mouse_x, self.mouse_y, 0);
                leave_event.event.target_id = old_id;
                leave_event.event.timestamp = self.current_time;
                leave_event.event.is_trusted = true;
                self.event_queue
                    .push_back(DomInputEvent::Mouse(leave_event));
            }

            // Mouse enter new element
            if target_id != 0 {
                let mut enter_event =
                    MouseEvent::new(EventType::MouseEnter, self.mouse_x, self.mouse_y, 0);
                enter_event.event.target_id = target_id;
                enter_event.event.timestamp = self.current_time;
                enter_event.event.is_trusted = true;
                self.event_queue
                    .push_back(DomInputEvent::Mouse(enter_event));
            }

            self.hovered_element = new_hovered;
        }
    }

    /// Process mouse wheel event.
    fn process_mouse_wheel(
        &mut self,
        delta_x: i32,
        delta_y: i32,
        hit_test: impl Fn(i32, i32) -> Option<HitTestResult>,
    ) {
        let target = hit_test(self.mouse_x, self.mouse_y);
        let target_id = target.as_ref().map(|t| t.element_id).unwrap_or(0);

        // Generate wheel event as mouse event
        let mut wheel_event = MouseEvent::new(EventType::Wheel, self.mouse_x, self.mouse_y, 0);
        wheel_event.event.target_id = target_id;
        wheel_event.event.timestamp = self.current_time;
        wheel_event.event.is_trusted = true;

        self.event_queue
            .push_back(DomInputEvent::Mouse(wheel_event));

        // Also queue scroll event
        self.event_queue.push_back(DomInputEvent::Scroll {
            delta_x,
            delta_y,
            target_id,
        });
    }

    /// Set focus to an element.
    pub fn set_focus(&mut self, element_id: Option<u64>) {
        if self.focused_element == element_id {
            return;
        }

        // Blur old element
        if let Some(old_id) = self.focused_element {
            let mut blur_event = Event::new(EventType::Blur, old_id);
            blur_event.timestamp = self.current_time;
            blur_event.is_trusted = true;

            let kb_event = KeyboardEvent {
                event: blur_event,
                key_code: 0,
                char_code: 0,
                key: String::new(),
                code: String::new(),
                ctrl_key: false,
                shift_key: false,
                alt_key: false,
                meta_key: false,
                repeat: false,
            };
            self.event_queue
                .push_back(DomInputEvent::Keyboard(kb_event));
        }

        // Focus new element
        if let Some(new_id) = element_id {
            let mut focus_event = Event::new(EventType::Focus, new_id);
            focus_event.timestamp = self.current_time;
            focus_event.is_trusted = true;

            let kb_event = KeyboardEvent {
                event: focus_event,
                key_code: 0,
                char_code: 0,
                key: String::new(),
                code: String::new(),
                ctrl_key: false,
                shift_key: false,
                alt_key: false,
                meta_key: false,
                repeat: false,
            };
            self.event_queue
                .push_back(DomInputEvent::Keyboard(kb_event));
        }

        self.focused_element = element_id;
    }

    /// Get pending events.
    pub fn poll_events(&mut self) -> Vec<DomInputEvent> {
        self.event_queue.drain(..).collect()
    }

    /// Get current mouse position.
    pub fn mouse_position(&self) -> (i32, i32) {
        (self.mouse_x, self.mouse_y)
    }

    /// Get focused element.
    pub fn focused_element(&self) -> Option<u64> {
        self.focused_element
    }

    /// Get hovered element.
    pub fn hovered_element(&self) -> Option<u64> {
        self.hovered_element
    }

    /// Get current modifiers.
    pub fn modifiers(&self) -> ModifierState {
        self.modifiers
    }

    /// Convert scancode to virtual key and key strings.
    fn scancode_to_key(&self, scancode: Scancode) -> (VirtualKey, String, String) {
        let (vkey, key, code) = match scancode {
            Scancode::ESCAPE => (VirtualKey::ESCAPE, "Escape", "Escape"),
            Scancode::KEY_1 => (
                VirtualKey::KEY_1,
                if self.modifiers.shift { "!" } else { "1" },
                "Digit1",
            ),
            Scancode::KEY_2 => (
                VirtualKey::KEY_2,
                if self.modifiers.shift { "@" } else { "2" },
                "Digit2",
            ),
            Scancode::KEY_3 => (
                VirtualKey::KEY_3,
                if self.modifiers.shift { "#" } else { "3" },
                "Digit3",
            ),
            Scancode::KEY_4 => (
                VirtualKey::KEY_4,
                if self.modifiers.shift { "$" } else { "4" },
                "Digit4",
            ),
            Scancode::KEY_5 => (
                VirtualKey::KEY_5,
                if self.modifiers.shift { "%" } else { "5" },
                "Digit5",
            ),
            Scancode::KEY_6 => (
                VirtualKey::KEY_6,
                if self.modifiers.shift { "^" } else { "6" },
                "Digit6",
            ),
            Scancode::KEY_7 => (
                VirtualKey::KEY_7,
                if self.modifiers.shift { "&" } else { "7" },
                "Digit7",
            ),
            Scancode::KEY_8 => (
                VirtualKey::KEY_8,
                if self.modifiers.shift { "*" } else { "8" },
                "Digit8",
            ),
            Scancode::KEY_9 => (
                VirtualKey::KEY_9,
                if self.modifiers.shift { "(" } else { "9" },
                "Digit9",
            ),
            Scancode::KEY_0 => (
                VirtualKey::KEY_0,
                if self.modifiers.shift { ")" } else { "0" },
                "Digit0",
            ),
            Scancode::BACKSPACE => (VirtualKey::BACKSPACE, "Backspace", "Backspace"),
            Scancode::TAB => (VirtualKey::TAB, "Tab", "Tab"),
            Scancode::KEY_Q => (VirtualKey::KEY_Q, self.key_char("q"), "KeyQ"),
            Scancode::KEY_W => (VirtualKey::KEY_W, self.key_char("w"), "KeyW"),
            Scancode::KEY_E => (VirtualKey::KEY_E, self.key_char("e"), "KeyE"),
            Scancode::KEY_R => (VirtualKey::KEY_R, self.key_char("r"), "KeyR"),
            Scancode::KEY_T => (VirtualKey::KEY_T, self.key_char("t"), "KeyT"),
            Scancode::KEY_Y => (VirtualKey::KEY_Y, self.key_char("y"), "KeyY"),
            Scancode::KEY_U => (VirtualKey::KEY_U, self.key_char("u"), "KeyU"),
            Scancode::KEY_I => (VirtualKey::KEY_I, self.key_char("i"), "KeyI"),
            Scancode::KEY_O => (VirtualKey::KEY_O, self.key_char("o"), "KeyO"),
            Scancode::KEY_P => (VirtualKey::KEY_P, self.key_char("p"), "KeyP"),
            Scancode::ENTER => (VirtualKey::ENTER, "Enter", "Enter"),
            Scancode::LEFT_CTRL => (VirtualKey::CTRL, "Control", "ControlLeft"),
            Scancode::KEY_A => (VirtualKey::KEY_A, self.key_char("a"), "KeyA"),
            Scancode::KEY_S => (VirtualKey::KEY_S, self.key_char("s"), "KeyS"),
            Scancode::KEY_D => (VirtualKey::KEY_D, self.key_char("d"), "KeyD"),
            Scancode::KEY_F => (VirtualKey::KEY_F, self.key_char("f"), "KeyF"),
            Scancode::KEY_G => (VirtualKey::KEY_G, self.key_char("g"), "KeyG"),
            Scancode::KEY_H => (VirtualKey::KEY_H, self.key_char("h"), "KeyH"),
            Scancode::KEY_J => (VirtualKey::KEY_J, self.key_char("j"), "KeyJ"),
            Scancode::KEY_K => (VirtualKey::KEY_K, self.key_char("k"), "KeyK"),
            Scancode::KEY_L => (VirtualKey::KEY_L, self.key_char("l"), "KeyL"),
            Scancode::LEFT_SHIFT => (VirtualKey::SHIFT, "Shift", "ShiftLeft"),
            Scancode::RIGHT_SHIFT => (VirtualKey::SHIFT, "Shift", "ShiftRight"),
            Scancode::KEY_Z => (VirtualKey::KEY_Z, self.key_char("z"), "KeyZ"),
            Scancode::KEY_X => (VirtualKey::KEY_X, self.key_char("x"), "KeyX"),
            Scancode::KEY_C => (VirtualKey::KEY_C, self.key_char("c"), "KeyC"),
            Scancode::KEY_V => (VirtualKey::KEY_V, self.key_char("v"), "KeyV"),
            Scancode::KEY_B => (VirtualKey::KEY_B, self.key_char("b"), "KeyB"),
            Scancode::KEY_N => (VirtualKey::KEY_N, self.key_char("n"), "KeyN"),
            Scancode::KEY_M => (VirtualKey::KEY_M, self.key_char("m"), "KeyM"),
            Scancode::LEFT_ALT => (VirtualKey::ALT, "Alt", "AltLeft"),
            Scancode::SPACE => (VirtualKey::SPACE, " ", "Space"),
            Scancode::CAPS_LOCK => (VirtualKey::CAPS_LOCK, "CapsLock", "CapsLock"),
            Scancode::F1 => (VirtualKey::F1, "F1", "F1"),
            Scancode::F2 => (VirtualKey::F2, "F2", "F2"),
            Scancode::F3 => (VirtualKey::F3, "F3", "F3"),
            Scancode::F4 => (VirtualKey::F4, "F4", "F4"),
            Scancode::F5 => (VirtualKey::F5, "F5", "F5"),
            Scancode::F6 => (VirtualKey::F6, "F6", "F6"),
            Scancode::F7 => (VirtualKey::F7, "F7", "F7"),
            Scancode::F8 => (VirtualKey::F8, "F8", "F8"),
            Scancode::F9 => (VirtualKey::F9, "F9", "F9"),
            Scancode::F10 => (VirtualKey::F10, "F10", "F10"),
            Scancode::F11 => (VirtualKey::F11, "F11", "F11"),
            Scancode::F12 => (VirtualKey::F12, "F12", "F12"),
            Scancode::ARROW_UP => (VirtualKey::ARROW_UP, "ArrowUp", "ArrowUp"),
            Scancode::ARROW_LEFT => (VirtualKey::ARROW_LEFT, "ArrowLeft", "ArrowLeft"),
            Scancode::ARROW_RIGHT => (VirtualKey::ARROW_RIGHT, "ArrowRight", "ArrowRight"),
            Scancode::ARROW_DOWN => (VirtualKey::ARROW_DOWN, "ArrowDown", "ArrowDown"),
            Scancode::HOME => (VirtualKey::HOME, "Home", "Home"),
            Scancode::END => (VirtualKey::END, "End", "End"),
            Scancode::PAGE_UP => (VirtualKey::PAGE_UP, "PageUp", "PageUp"),
            Scancode::PAGE_DOWN => (VirtualKey::PAGE_DOWN, "PageDown", "PageDown"),
            Scancode::INSERT => (VirtualKey::INSERT, "Insert", "Insert"),
            Scancode::DELETE => (VirtualKey::DELETE, "Delete", "Delete"),
            _ => (VirtualKey(0), "Unidentified", "Unidentified"),
        };

        (vkey, key.to_string(), code.to_string())
    }

    /// Get key character considering shift/caps lock.
    fn key_char<'a>(&self, base: &'a str) -> &'a str {
        let uppercase = self.modifiers.shift ^ self.modifiers.caps_lock;
        if uppercase {
            match base {
                "a" => "A",
                "b" => "B",
                "c" => "C",
                "d" => "D",
                "e" => "E",
                "f" => "F",
                "g" => "G",
                "h" => "H",
                "i" => "I",
                "j" => "J",
                "k" => "K",
                "l" => "L",
                "m" => "M",
                "n" => "N",
                "o" => "O",
                "p" => "P",
                "q" => "Q",
                "r" => "R",
                "s" => "S",
                "t" => "T",
                "u" => "U",
                "v" => "V",
                "w" => "W",
                "x" => "X",
                "y" => "Y",
                "z" => "Z",
                _ => base,
            }
        } else {
            base
        }
    }

    /// Check if key produces printable character.
    fn is_printable_key(&self, vkey: VirtualKey) -> bool {
        matches!(vkey.0, 32 | 48..=57 | 65..=90 | 186..=192 | 219..=222)
    }

    /// Convert virtual key to character code.
    fn key_to_char(&self, vkey: VirtualKey, shift: bool) -> u32 {
        match vkey.0 {
            32 => 32,                    // Space
            48..=57 if !shift => vkey.0, // 0-9
            48 if shift => ')' as u32,
            49 if shift => '!' as u32,
            50 if shift => '@' as u32,
            51 if shift => '#' as u32,
            52 if shift => '$' as u32,
            53 if shift => '%' as u32,
            54 if shift => '^' as u32,
            55 if shift => '&' as u32,
            56 if shift => '*' as u32,
            57 if shift => '(' as u32,
            65..=90 => {
                if shift ^ self.modifiers.caps_lock {
                    vkey.0 // Uppercase
                } else {
                    vkey.0 + 32 // Lowercase
                }
            }
            _ => 0,
        }
    }
}

impl Default for InputManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Hit tester for layout tree.
pub struct HitTester {
    /// Viewport scroll offset.
    pub scroll_x: i32,
    pub scroll_y: i32,
}

impl HitTester {
    /// Create new hit tester.
    pub fn new() -> Self {
        Self {
            scroll_x: 0,
            scroll_y: 0,
        }
    }

    /// Set scroll offset.
    pub fn set_scroll(&mut self, x: i32, y: i32) {
        self.scroll_x = x;
        self.scroll_y = y;
    }

    /// Perform hit test at a point.
    /// Returns element ID path from root to deepest element at the point.
    pub fn hit_test_point(
        &self,
        x: i32,
        y: i32,
        elements: &[(u64, i32, i32, i32, i32, bool, bool, CursorType)],
    ) -> Option<HitTestResult> {
        // Elements are (id, x, y, width, height, focusable, clickable, cursor)
        // Find deepest element containing the point
        let test_x = x + self.scroll_x;
        let test_y = y + self.scroll_y;

        let mut path = Vec::new();
        let mut result: Option<(u64, bool, bool, CursorType)> = None;

        for &(id, ex, ey, ew, eh, focusable, clickable, cursor) in elements {
            if test_x >= ex && test_x < ex + ew && test_y >= ey && test_y < ey + eh {
                path.push(id);
                result = Some((id, focusable, clickable, cursor));
            }
        }

        result.map(|(element_id, focusable, clickable, cursor)| HitTestResult {
            element_id,
            path,
            focusable,
            clickable,
            cursor,
        })
    }
}

impl Default for HitTester {
    fn default() -> Self {
        Self::new()
    }
}
