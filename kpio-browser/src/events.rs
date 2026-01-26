//! Event handling.
//!
//! DOM event system implementation.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::boxed::Box;

/// Event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    // Mouse events
    Click,
    DblClick,
    MouseDown,
    MouseUp,
    MouseMove,
    MouseEnter,
    MouseLeave,
    MouseOver,
    MouseOut,
    ContextMenu,
    Wheel,
    
    // Keyboard events
    KeyDown,
    KeyUp,
    KeyPress,
    
    // Focus events
    Focus,
    Blur,
    FocusIn,
    FocusOut,
    
    // Form events
    Submit,
    Reset,
    Change,
    Input,
    
    // Document events
    DomContentLoaded,
    Load,
    Unload,
    BeforeUnload,
    
    // Window events
    Resize,
    Scroll,
    
    // Touch events
    TouchStart,
    TouchMove,
    TouchEnd,
    TouchCancel,
    
    // Drag events
    DragStart,
    Drag,
    DragEnd,
    DragEnter,
    DragLeave,
    DragOver,
    Drop,
}

impl EventType {
    /// Get event type name.
    pub fn name(&self) -> &'static str {
        match self {
            EventType::Click => "click",
            EventType::DblClick => "dblclick",
            EventType::MouseDown => "mousedown",
            EventType::MouseUp => "mouseup",
            EventType::MouseMove => "mousemove",
            EventType::MouseEnter => "mouseenter",
            EventType::MouseLeave => "mouseleave",
            EventType::MouseOver => "mouseover",
            EventType::MouseOut => "mouseout",
            EventType::ContextMenu => "contextmenu",
            EventType::Wheel => "wheel",
            EventType::KeyDown => "keydown",
            EventType::KeyUp => "keyup",
            EventType::KeyPress => "keypress",
            EventType::Focus => "focus",
            EventType::Blur => "blur",
            EventType::FocusIn => "focusin",
            EventType::FocusOut => "focusout",
            EventType::Submit => "submit",
            EventType::Reset => "reset",
            EventType::Change => "change",
            EventType::Input => "input",
            EventType::DomContentLoaded => "DOMContentLoaded",
            EventType::Load => "load",
            EventType::Unload => "unload",
            EventType::BeforeUnload => "beforeunload",
            EventType::Resize => "resize",
            EventType::Scroll => "scroll",
            EventType::TouchStart => "touchstart",
            EventType::TouchMove => "touchmove",
            EventType::TouchEnd => "touchend",
            EventType::TouchCancel => "touchcancel",
            EventType::DragStart => "dragstart",
            EventType::Drag => "drag",
            EventType::DragEnd => "dragend",
            EventType::DragEnter => "dragenter",
            EventType::DragLeave => "dragleave",
            EventType::DragOver => "dragover",
            EventType::Drop => "drop",
        }
    }
    
    /// Check if event bubbles.
    pub fn bubbles(&self) -> bool {
        match self {
            EventType::Focus | EventType::Blur | EventType::Load |
            EventType::Unload | EventType::MouseEnter | EventType::MouseLeave => false,
            _ => true,
        }
    }
    
    /// Check if event is cancelable.
    pub fn cancelable(&self) -> bool {
        match self {
            EventType::Load | EventType::Unload | EventType::Scroll |
            EventType::Resize | EventType::DomContentLoaded => false,
            _ => true,
        }
    }
}

/// DOM Event.
#[derive(Debug, Clone)]
pub struct Event {
    /// Event type.
    pub event_type: EventType,
    /// Target element ID (placeholder).
    pub target_id: u64,
    /// Current target element ID.
    pub current_target_id: u64,
    /// Event phase.
    pub phase: EventPhase,
    /// Is default prevented?
    pub default_prevented: bool,
    /// Stop propagation flag.
    pub propagation_stopped: bool,
    /// Stop immediate propagation flag.
    pub immediate_propagation_stopped: bool,
    /// Is trusted (user-initiated)?
    pub is_trusted: bool,
    /// Timestamp.
    pub timestamp: u64,
}

/// Event phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase {
    None = 0,
    Capturing = 1,
    AtTarget = 2,
    Bubbling = 3,
}

impl Event {
    /// Create a new event.
    pub fn new(event_type: EventType, target_id: u64) -> Self {
        Self {
            event_type,
            target_id,
            current_target_id: target_id,
            phase: EventPhase::None,
            default_prevented: false,
            propagation_stopped: false,
            immediate_propagation_stopped: false,
            is_trusted: false,
            timestamp: 0,
        }
    }
    
    /// Prevent default action.
    pub fn prevent_default(&mut self) {
        if self.event_type.cancelable() {
            self.default_prevented = true;
        }
    }
    
    /// Stop propagation.
    pub fn stop_propagation(&mut self) {
        self.propagation_stopped = true;
    }
    
    /// Stop immediate propagation.
    pub fn stop_immediate_propagation(&mut self) {
        self.propagation_stopped = true;
        self.immediate_propagation_stopped = true;
    }
}

/// Mouse event.
#[derive(Debug, Clone)]
pub struct MouseEvent {
    /// Base event.
    pub event: Event,
    /// Client X coordinate.
    pub client_x: i32,
    /// Client Y coordinate.
    pub client_y: i32,
    /// Screen X coordinate.
    pub screen_x: i32,
    /// Screen Y coordinate.
    pub screen_y: i32,
    /// Page X coordinate.
    pub page_x: i32,
    /// Page Y coordinate.
    pub page_y: i32,
    /// Button pressed (0=left, 1=middle, 2=right).
    pub button: u8,
    /// Buttons pressed mask.
    pub buttons: u8,
    /// Ctrl key pressed.
    pub ctrl_key: bool,
    /// Shift key pressed.
    pub shift_key: bool,
    /// Alt key pressed.
    pub alt_key: bool,
    /// Meta key pressed.
    pub meta_key: bool,
}

impl MouseEvent {
    /// Create a mouse event.
    pub fn new(event_type: EventType, x: i32, y: i32, button: u8) -> Self {
        Self {
            event: Event::new(event_type, 0),
            client_x: x,
            client_y: y,
            screen_x: x,
            screen_y: y,
            page_x: x,
            page_y: y,
            button,
            buttons: 1 << button,
            ctrl_key: false,
            shift_key: false,
            alt_key: false,
            meta_key: false,
        }
    }
}

/// Keyboard event.
#[derive(Debug, Clone)]
pub struct KeyboardEvent {
    /// Base event.
    pub event: Event,
    /// Key code.
    pub key_code: u32,
    /// Character code.
    pub char_code: u32,
    /// Key string.
    pub key: String,
    /// Code string.
    pub code: String,
    /// Ctrl key pressed.
    pub ctrl_key: bool,
    /// Shift key pressed.
    pub shift_key: bool,
    /// Alt key pressed.
    pub alt_key: bool,
    /// Meta key pressed.
    pub meta_key: bool,
    /// Is repeat?
    pub repeat: bool,
}

impl KeyboardEvent {
    /// Create a keyboard event.
    pub fn new(event_type: EventType, key: &str, code: &str, key_code: u32) -> Self {
        Self {
            event: Event::new(event_type, 0),
            key_code,
            char_code: 0,
            key: key.into(),
            code: code.into(),
            ctrl_key: false,
            shift_key: false,
            alt_key: false,
            meta_key: false,
            repeat: false,
        }
    }
}

/// Event listener.
pub struct EventListener {
    /// Event type.
    pub event_type: EventType,
    /// Callback function (placeholder - would be JS function).
    pub callback_id: u64,
    /// Use capture phase?
    pub capture: bool,
    /// Once only?
    pub once: bool,
    /// Passive?
    pub passive: bool,
}

/// Event target trait.
pub trait EventTarget {
    /// Add event listener.
    fn add_event_listener(&mut self, listener: EventListener);
    
    /// Remove event listener.
    fn remove_event_listener(&mut self, event_type: EventType, callback_id: u64);
    
    /// Dispatch event.
    fn dispatch_event(&mut self, event: &mut Event) -> bool;
}

/// Event queue.
pub struct EventQueue {
    /// Pending events.
    events: Vec<QueuedEvent>,
}

/// A queued event.
struct QueuedEvent {
    event: Event,
    target_id: u64,
}

impl EventQueue {
    /// Create a new event queue.
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
        }
    }
    
    /// Queue an event.
    pub fn queue(&mut self, event: Event, target_id: u64) {
        self.events.push(QueuedEvent { event, target_id });
    }
    
    /// Process all queued events.
    pub fn process_all(&mut self) -> Vec<(Event, u64)> {
        let events = core::mem::take(&mut self.events);
        events.into_iter().map(|e| (e.event, e.target_id)).collect()
    }
    
    /// Check if queue is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}
