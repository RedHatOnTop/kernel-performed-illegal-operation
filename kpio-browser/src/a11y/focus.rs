//! Focus Management
//!
//! Keyboard navigation and focus handling.

use alloc::vec::Vec;

/// Focus direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    /// Forward (Tab)
    Forward,
    /// Backward (Shift+Tab)
    Backward,
    /// Up (Arrow Up)
    Up,
    /// Down (Arrow Down)
    Down,
    /// Left (Arrow Left)
    Left,
    /// Right (Arrow Right)
    Right,
    /// First
    First,
    /// Last
    Last,
}

/// Focus mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusMode {
    /// Normal document navigation
    Normal,
    /// Focus trap (modal dialogs)
    Trap,
    /// Virtual cursor (screen reader)
    Virtual,
}

impl Default for FocusMode {
    fn default() -> Self {
        Self::Normal
    }
}

/// Focus ring style
#[derive(Debug, Clone)]
pub struct FocusRingStyle {
    /// Color
    pub color: u32,
    /// Width in pixels
    pub width: u32,
    /// Offset from element
    pub offset: i32,
    /// Border radius
    pub radius: u32,
    /// Use outline (true) or box-shadow (false)
    pub outline: bool,
}

impl Default for FocusRingStyle {
    fn default() -> Self {
        Self {
            color: 0xFF0066FF, // Blue
            width: 2,
            offset: 2,
            radius: 3,
            outline: true,
        }
    }
}

/// Focus manager
pub struct FocusManager {
    /// Current focus ID
    current: Option<u64>,
    /// Focus history
    history: Vec<u64>,
    /// Focus mode
    mode: FocusMode,
    /// Focus trap scope
    trap_scope: Option<u64>,
    /// Tab index order
    tab_order: Vec<u64>,
    /// Focus ring style
    ring_style: FocusRingStyle,
    /// Focus visible
    visible: bool,
    /// Keyboard active
    keyboard_active: bool,
}

impl FocusManager {
    /// Create new focus manager
    pub const fn new() -> Self {
        Self {
            current: None,
            history: Vec::new(),
            mode: FocusMode::Normal,
            trap_scope: None,
            tab_order: Vec::new(),
            ring_style: FocusRingStyle {
                color: 0xFF0066FF,
                width: 2,
                offset: 2,
                radius: 3,
                outline: true,
            },
            visible: true,
            keyboard_active: false,
        }
    }

    /// Get current focus
    pub fn current(&self) -> Option<u64> {
        self.current
    }

    /// Set focus
    pub fn set_focus(&mut self, node_id: u64) {
        if let Some(current) = self.current {
            self.history.push(current);
        }
        self.current = Some(node_id);
    }

    /// Clear focus
    pub fn clear_focus(&mut self) {
        self.current = None;
    }

    /// Restore previous focus
    pub fn restore_focus(&mut self) -> Option<u64> {
        if let Some(previous) = self.history.pop() {
            self.current = Some(previous);
            Some(previous)
        } else {
            None
        }
    }

    /// Move focus in direction
    pub fn move_focus(&mut self, direction: FocusDirection) -> Option<u64> {
        if self.tab_order.is_empty() {
            return None;
        }

        let current_idx = self
            .current
            .and_then(|id| self.tab_order.iter().position(|&i| i == id));

        let new_idx = match direction {
            FocusDirection::Forward => current_idx
                .map(|i| (i + 1) % self.tab_order.len())
                .unwrap_or(0),
            FocusDirection::Backward => current_idx
                .map(|i| {
                    if i == 0 {
                        self.tab_order.len() - 1
                    } else {
                        i - 1
                    }
                })
                .unwrap_or(self.tab_order.len() - 1),
            FocusDirection::First => 0,
            FocusDirection::Last => self.tab_order.len() - 1,
            _ => return self.current,
        };

        // Handle focus trap
        if self.mode == FocusMode::Trap {
            if let Some(scope) = self.trap_scope {
                // Would filter tab_order to scope
                let _ = scope;
            }
        }

        let new_id = self.tab_order[new_idx];
        self.set_focus(new_id);
        Some(new_id)
    }

    /// Set tab order
    pub fn set_tab_order(&mut self, order: Vec<u64>) {
        self.tab_order = order;
    }

    /// Update tab order (add element)
    pub fn add_to_tab_order(&mut self, node_id: u64, index: Option<usize>) {
        if let Some(idx) = index {
            if idx <= self.tab_order.len() {
                self.tab_order.insert(idx, node_id);
            } else {
                self.tab_order.push(node_id);
            }
        } else {
            self.tab_order.push(node_id);
        }
    }

    /// Remove from tab order
    pub fn remove_from_tab_order(&mut self, node_id: u64) {
        self.tab_order.retain(|&id| id != node_id);
        if self.current == Some(node_id) {
            self.current = None;
        }
    }

    /// Enter focus trap
    pub fn enter_trap(&mut self, scope: u64) {
        self.mode = FocusMode::Trap;
        self.trap_scope = Some(scope);
    }

    /// Exit focus trap
    pub fn exit_trap(&mut self) {
        self.mode = FocusMode::Normal;
        self.trap_scope = None;
        self.restore_focus();
    }

    /// Set focus ring style
    pub fn set_ring_style(&mut self, style: FocusRingStyle) {
        self.ring_style = style;
    }

    /// Get focus ring style
    pub fn ring_style(&self) -> &FocusRingStyle {
        &self.ring_style
    }

    /// Set focus visible
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Is focus visible
    pub fn is_visible(&self) -> bool {
        self.visible && self.keyboard_active
    }

    /// Keyboard event occurred
    pub fn on_keyboard(&mut self) {
        self.keyboard_active = true;
        self.visible = true;
    }

    /// Mouse event occurred
    pub fn on_mouse(&mut self) {
        self.keyboard_active = false;
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Skip link
#[derive(Debug, Clone)]
pub struct SkipLink {
    /// Target ID
    pub target: u64,
    /// Text
    pub text: &'static str,
}

/// Standard skip links
impl SkipLink {
    /// Skip to main content
    pub const MAIN: &'static str = "Skip to main content";
    /// Skip to navigation
    pub const NAV: &'static str = "Skip to navigation";
    /// Skip to search
    pub const SEARCH: &'static str = "Skip to search";
    /// Skip to footer
    pub const FOOTER: &'static str = "Skip to footer";
}

/// Roving tabindex manager
pub struct RovingTabindex {
    /// Group ID
    group_id: u64,
    /// Members
    members: Vec<u64>,
    /// Current active
    active: usize,
    /// Wrap around
    wrap: bool,
    /// Orientation
    orientation: Orientation,
}

/// Roving orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Horizontal (left/right)
    Horizontal,
    /// Vertical (up/down)
    Vertical,
    /// Both
    Both,
}

impl RovingTabindex {
    /// Create new roving tabindex
    pub fn new(group_id: u64, orientation: Orientation) -> Self {
        Self {
            group_id,
            members: Vec::new(),
            active: 0,
            wrap: true,
            orientation,
        }
    }

    /// Add member
    pub fn add_member(&mut self, node_id: u64) {
        self.members.push(node_id);
    }

    /// Get active member
    pub fn active(&self) -> Option<u64> {
        self.members.get(self.active).copied()
    }

    /// Move to next
    pub fn next(&mut self) -> Option<u64> {
        if self.members.is_empty() {
            return None;
        }

        if self.active + 1 < self.members.len() {
            self.active += 1;
        } else if self.wrap {
            self.active = 0;
        }

        self.active()
    }

    /// Move to previous
    pub fn previous(&mut self) -> Option<u64> {
        if self.members.is_empty() {
            return None;
        }

        if self.active > 0 {
            self.active -= 1;
        } else if self.wrap {
            self.active = self.members.len() - 1;
        }

        self.active()
    }

    /// Handle key
    pub fn handle_key(&mut self, direction: FocusDirection) -> Option<u64> {
        match self.orientation {
            Orientation::Horizontal => match direction {
                FocusDirection::Right => self.next(),
                FocusDirection::Left => self.previous(),
                _ => None,
            },
            Orientation::Vertical => match direction {
                FocusDirection::Down => self.next(),
                FocusDirection::Up => self.previous(),
                _ => None,
            },
            Orientation::Both => match direction {
                FocusDirection::Right | FocusDirection::Down => self.next(),
                FocusDirection::Left | FocusDirection::Up => self.previous(),
                _ => None,
            },
        }
    }
}
