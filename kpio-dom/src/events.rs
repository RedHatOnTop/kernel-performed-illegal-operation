//! DOM Event System
//!
//! This module implements the W3C DOM Events specification including
//! event bubbling, capturing, and common event types.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::node::NodeId;

/// Event phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase {
    /// Event is not being dispatched.
    None = 0,
    /// Event is propagating through target's ancestors (capture phase).
    Capturing = 1,
    /// Event has arrived at the event target.
    AtTarget = 2,
    /// Event is propagating back through target's ancestors (bubble phase).
    Bubbling = 3,
}

/// Event type categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    Input,
    Change,
    Submit,
    Reset,

    // Clipboard events
    Cut,
    Copy,
    Paste,

    // Drag events
    DragStart,
    Drag,
    DragEnd,
    DragEnter,
    DragOver,
    DragLeave,
    Drop,

    // Touch events
    TouchStart,
    TouchMove,
    TouchEnd,
    TouchCancel,

    // Document events
    DOMContentLoaded,
    Load,
    Unload,
    BeforeUnload,
    Resize,
    Scroll,

    // Animation events
    AnimationStart,
    AnimationEnd,
    AnimationIteration,
    TransitionEnd,

    // Custom event
    Custom(u32),
}

impl EventType {
    /// Get event type name.
    pub fn as_str(&self) -> &'static str {
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
            EventType::Input => "input",
            EventType::Change => "change",
            EventType::Submit => "submit",
            EventType::Reset => "reset",
            EventType::Cut => "cut",
            EventType::Copy => "copy",
            EventType::Paste => "paste",
            EventType::DragStart => "dragstart",
            EventType::Drag => "drag",
            EventType::DragEnd => "dragend",
            EventType::DragEnter => "dragenter",
            EventType::DragOver => "dragover",
            EventType::DragLeave => "dragleave",
            EventType::Drop => "drop",
            EventType::TouchStart => "touchstart",
            EventType::TouchMove => "touchmove",
            EventType::TouchEnd => "touchend",
            EventType::TouchCancel => "touchcancel",
            EventType::DOMContentLoaded => "DOMContentLoaded",
            EventType::Load => "load",
            EventType::Unload => "unload",
            EventType::BeforeUnload => "beforeunload",
            EventType::Resize => "resize",
            EventType::Scroll => "scroll",
            EventType::AnimationStart => "animationstart",
            EventType::AnimationEnd => "animationend",
            EventType::AnimationIteration => "animationiteration",
            EventType::TransitionEnd => "transitionend",
            EventType::Custom(_) => "custom",
        }
    }

    /// Parse event type from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "click" => Some(EventType::Click),
            "dblclick" => Some(EventType::DblClick),
            "mousedown" => Some(EventType::MouseDown),
            "mouseup" => Some(EventType::MouseUp),
            "mousemove" => Some(EventType::MouseMove),
            "mouseenter" => Some(EventType::MouseEnter),
            "mouseleave" => Some(EventType::MouseLeave),
            "mouseover" => Some(EventType::MouseOver),
            "mouseout" => Some(EventType::MouseOut),
            "contextmenu" => Some(EventType::ContextMenu),
            "wheel" => Some(EventType::Wheel),
            "keydown" => Some(EventType::KeyDown),
            "keyup" => Some(EventType::KeyUp),
            "keypress" => Some(EventType::KeyPress),
            "focus" => Some(EventType::Focus),
            "blur" => Some(EventType::Blur),
            "focusin" => Some(EventType::FocusIn),
            "focusout" => Some(EventType::FocusOut),
            "input" => Some(EventType::Input),
            "change" => Some(EventType::Change),
            "submit" => Some(EventType::Submit),
            "reset" => Some(EventType::Reset),
            "cut" => Some(EventType::Cut),
            "copy" => Some(EventType::Copy),
            "paste" => Some(EventType::Paste),
            "dragstart" => Some(EventType::DragStart),
            "drag" => Some(EventType::Drag),
            "dragend" => Some(EventType::DragEnd),
            "dragenter" => Some(EventType::DragEnter),
            "dragover" => Some(EventType::DragOver),
            "dragleave" => Some(EventType::DragLeave),
            "drop" => Some(EventType::Drop),
            "touchstart" => Some(EventType::TouchStart),
            "touchmove" => Some(EventType::TouchMove),
            "touchend" => Some(EventType::TouchEnd),
            "touchcancel" => Some(EventType::TouchCancel),
            "DOMContentLoaded" => Some(EventType::DOMContentLoaded),
            "load" => Some(EventType::Load),
            "unload" => Some(EventType::Unload),
            "beforeunload" => Some(EventType::BeforeUnload),
            "resize" => Some(EventType::Resize),
            "scroll" => Some(EventType::Scroll),
            "animationstart" => Some(EventType::AnimationStart),
            "animationend" => Some(EventType::AnimationEnd),
            "animationiteration" => Some(EventType::AnimationIteration),
            "transitionend" => Some(EventType::TransitionEnd),
            _ => None,
        }
    }

    /// Check if event bubbles by default.
    pub fn bubbles(&self) -> bool {
        match self {
            EventType::Focus
            | EventType::Blur
            | EventType::Load
            | EventType::Unload
            | EventType::MouseEnter
            | EventType::MouseLeave
            | EventType::Scroll => false,
            _ => true,
        }
    }

    /// Check if event is cancelable.
    pub fn cancelable(&self) -> bool {
        match self {
            EventType::Load
            | EventType::Unload
            | EventType::Blur
            | EventType::Focus
            | EventType::Scroll => false,
            _ => true,
        }
    }
}

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left = 0,
    Middle = 1,
    Right = 2,
    Back = 3,
    Forward = 4,
}

/// Modifier keys.
#[derive(Debug, Clone, Copy, Default)]
pub struct ModifierKeys {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

/// Base event data.
#[derive(Debug, Clone)]
pub struct Event {
    /// Event type.
    pub event_type: EventType,
    /// Target element.
    pub target: NodeId,
    /// Current target (changes during propagation).
    pub current_target: Option<NodeId>,
    /// Event phase.
    pub phase: EventPhase,
    /// Whether event bubbles.
    pub bubbles: bool,
    /// Whether event is cancelable.
    pub cancelable: bool,
    /// Whether default action was prevented.
    pub default_prevented: bool,
    /// Whether propagation was stopped.
    pub propagation_stopped: bool,
    /// Whether immediate propagation was stopped.
    pub immediate_propagation_stopped: bool,
    /// Whether this is a trusted event.
    pub is_trusted: bool,
    /// Timestamp.
    pub timestamp: u64,
    /// Event-specific data.
    pub data: EventData,
}

impl Event {
    /// Create a new event.
    pub fn new(event_type: EventType, target: NodeId) -> Self {
        Self {
            event_type,
            target,
            current_target: None,
            phase: EventPhase::None,
            bubbles: event_type.bubbles(),
            cancelable: event_type.cancelable(),
            default_prevented: false,
            propagation_stopped: false,
            immediate_propagation_stopped: false,
            is_trusted: true,
            timestamp: 0,
            data: EventData::None,
        }
    }

    /// Create mouse event.
    pub fn mouse(event_type: EventType, target: NodeId, data: MouseEventData) -> Self {
        let mut event = Self::new(event_type, target);
        event.data = EventData::Mouse(data);
        event
    }

    /// Create keyboard event.
    pub fn keyboard(event_type: EventType, target: NodeId, data: KeyboardEventData) -> Self {
        let mut event = Self::new(event_type, target);
        event.data = EventData::Keyboard(data);
        event
    }

    /// Prevent default action.
    pub fn prevent_default(&mut self) {
        if self.cancelable {
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

/// Event-specific data.
#[derive(Debug, Clone)]
pub enum EventData {
    /// No additional data.
    None,
    /// Mouse event data.
    Mouse(MouseEventData),
    /// Keyboard event data.
    Keyboard(KeyboardEventData),
    /// Wheel event data.
    Wheel(WheelEventData),
    /// Touch event data.
    Touch(TouchEventData),
    /// Focus event data.
    Focus(FocusEventData),
    /// Input event data.
    Input(InputEventData),
    /// Drag event data.
    Drag(DragEventData),
    /// Custom data.
    Custom(Vec<u8>),
}

/// Mouse event data.
#[derive(Debug, Clone, Default)]
pub struct MouseEventData {
    /// X coordinate relative to viewport.
    pub client_x: i32,
    /// Y coordinate relative to viewport.
    pub client_y: i32,
    /// X coordinate relative to screen.
    pub screen_x: i32,
    /// Y coordinate relative to screen.
    pub screen_y: i32,
    /// X coordinate relative to target element.
    pub offset_x: i32,
    /// Y coordinate relative to target element.
    pub offset_y: i32,
    /// X coordinate relative to page.
    pub page_x: i32,
    /// Y coordinate relative to page.
    pub page_y: i32,
    /// Mouse button.
    pub button: u8,
    /// Buttons currently pressed.
    pub buttons: u16,
    /// Modifier keys.
    pub modifiers: ModifierKeys,
    /// Related target (for enter/leave events).
    pub related_target: Option<NodeId>,
}

/// Keyboard event data.
#[derive(Debug, Clone, Default)]
pub struct KeyboardEventData {
    /// Key value (e.g., "a", "Enter", "Escape").
    pub key: String,
    /// Key code (e.g., "KeyA", "Enter").
    pub code: String,
    /// Location of key.
    pub location: u32,
    /// Whether key is held down (repeat).
    pub repeat: bool,
    /// Modifier keys.
    pub modifiers: ModifierKeys,
}

/// Wheel event data.
#[derive(Debug, Clone, Default)]
pub struct WheelEventData {
    /// Horizontal scroll amount.
    pub delta_x: f64,
    /// Vertical scroll amount.
    pub delta_y: f64,
    /// Z-axis scroll amount.
    pub delta_z: f64,
    /// Delta mode (0=pixels, 1=lines, 2=pages).
    pub delta_mode: u32,
    /// Mouse data.
    pub mouse: MouseEventData,
}

/// Touch point.
#[derive(Debug, Clone, Default)]
pub struct TouchPoint {
    pub identifier: i32,
    pub client_x: i32,
    pub client_y: i32,
    pub screen_x: i32,
    pub screen_y: i32,
    pub page_x: i32,
    pub page_y: i32,
    pub radius_x: f32,
    pub radius_y: f32,
    pub rotation_angle: f32,
    pub force: f32,
    pub target: Option<NodeId>,
}

/// Touch event data.
#[derive(Debug, Clone, Default)]
pub struct TouchEventData {
    pub touches: Vec<TouchPoint>,
    pub target_touches: Vec<TouchPoint>,
    pub changed_touches: Vec<TouchPoint>,
    pub modifiers: ModifierKeys,
}

/// Focus event data.
#[derive(Debug, Clone, Default)]
pub struct FocusEventData {
    pub related_target: Option<NodeId>,
}

/// Input event data.
#[derive(Debug, Clone, Default)]
pub struct InputEventData {
    pub data: Option<String>,
    pub input_type: String,
    pub is_composing: bool,
}

/// Drag event data.
#[derive(Debug, Clone, Default)]
pub struct DragEventData {
    pub mouse: MouseEventData,
    pub data_transfer_types: Vec<String>,
}

/// Event listener options.
#[derive(Debug, Clone, Default)]
pub struct ListenerOptions {
    /// Listen during capture phase.
    pub capture: bool,
    /// Remove listener after first invocation.
    pub once: bool,
    /// Indicate passive listener (won't call preventDefault).
    pub passive: bool,
}

/// Event handler callback type.
pub type EventHandler = Box<dyn Fn(&mut Event) + Send + Sync>;

/// Event listener entry.
struct EventListener {
    /// Handler function.
    handler: EventHandler,
    /// Listener options.
    options: ListenerOptions,
    /// Unique ID.
    id: u64,
}

/// Event target with listener management.
pub struct EventTarget {
    /// Listeners by event type.
    listeners: BTreeMap<String, Vec<EventListener>>,
    /// Next listener ID.
    next_id: u64,
}

impl EventTarget {
    /// Create new event target.
    pub fn new() -> Self {
        Self {
            listeners: BTreeMap::new(),
            next_id: 1,
        }
    }

    /// Add event listener.
    pub fn add_event_listener(
        &mut self,
        event_type: &str,
        handler: EventHandler,
        options: ListenerOptions,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let listener = EventListener {
            handler,
            options,
            id,
        };

        self.listeners
            .entry(event_type.to_string())
            .or_insert_with(Vec::new)
            .push(listener);

        id
    }

    /// Remove event listener by ID.
    pub fn remove_event_listener(&mut self, event_type: &str, id: u64) -> bool {
        if let Some(listeners) = self.listeners.get_mut(event_type) {
            if let Some(pos) = listeners.iter().position(|l| l.id == id) {
                listeners.remove(pos);
                return true;
            }
        }
        false
    }

    /// Dispatch event to this target's listeners.
    pub fn dispatch_to_listeners(&mut self, event: &mut Event, capture: bool) {
        let type_str = event.event_type.as_str();

        if let Some(listeners) = self.listeners.get_mut(type_str) {
            let mut to_remove = Vec::new();

            for listener in listeners.iter() {
                // Skip if wrong phase
                if listener.options.capture != capture {
                    continue;
                }

                // Call handler
                (listener.handler)(event);

                // Mark for removal if once
                if listener.options.once {
                    to_remove.push(listener.id);
                }

                // Stop if immediate propagation stopped
                if event.immediate_propagation_stopped {
                    break;
                }
            }

            // Remove once listeners
            listeners.retain(|l| !to_remove.contains(&l.id));
        }
    }

    /// Check if has listeners for event type.
    pub fn has_listeners(&self, event_type: &str) -> bool {
        self.listeners
            .get(event_type)
            .map(|l| !l.is_empty())
            .unwrap_or(false)
    }
}

impl Default for EventTarget {
    fn default() -> Self {
        Self::new()
    }
}

/// Event dispatcher for the DOM.
pub struct EventDispatcher {
    /// Event targets by node ID.
    targets: BTreeMap<NodeId, EventTarget>,
}

impl EventDispatcher {
    /// Create new dispatcher.
    pub fn new() -> Self {
        Self {
            targets: BTreeMap::new(),
        }
    }

    /// Get or create event target for node.
    pub fn get_target(&mut self, node_id: NodeId) -> &mut EventTarget {
        self.targets.entry(node_id).or_insert_with(EventTarget::new)
    }

    /// Dispatch event with propagation.
    pub fn dispatch(&mut self, mut event: Event, path: &[NodeId]) {
        if path.is_empty() {
            return;
        }

        // Capture phase (from root to target)
        event.phase = EventPhase::Capturing;
        for &node_id in path.iter().rev().skip(1) {
            if event.propagation_stopped {
                break;
            }
            event.current_target = Some(node_id);
            if let Some(target) = self.targets.get_mut(&node_id) {
                target.dispatch_to_listeners(&mut event, true);
            }
        }

        // At target phase
        if !event.propagation_stopped {
            event.phase = EventPhase::AtTarget;
            event.current_target = Some(event.target);
            if let Some(target) = self.targets.get_mut(&event.target) {
                target.dispatch_to_listeners(&mut event, true);
                target.dispatch_to_listeners(&mut event, false);
            }
        }

        // Bubble phase (from target to root)
        if event.bubbles && !event.propagation_stopped {
            event.phase = EventPhase::Bubbling;
            for &node_id in path.iter().skip(1) {
                if event.propagation_stopped {
                    break;
                }
                event.current_target = Some(node_id);
                if let Some(target) = self.targets.get_mut(&node_id) {
                    target.dispatch_to_listeners(&mut event, false);
                }
            }
        }

        event.phase = EventPhase::None;
    }

    /// Remove all listeners for a node.
    pub fn remove_node(&mut self, node_id: NodeId) {
        self.targets.remove(&node_id);
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
