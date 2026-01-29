//! Accessibility Module (a11y)
//!
//! Screen reader support, keyboard navigation, and visual accessibility.

pub mod screen_reader;
pub mod focus;
pub mod aria;
pub mod visual;

pub use screen_reader::*;
pub use focus::*;
pub use aria::*;
pub use visual::*;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

/// Accessibility error
#[derive(Debug, Clone)]
pub enum A11yError {
    /// Node not found
    NodeNotFound(u64),
    /// Invalid operation
    InvalidOperation(String),
    /// Screen reader unavailable
    ScreenReaderUnavailable,
}

/// Accessibility node role
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Generic container
    Generic,
    /// Document
    Document,
    /// Article
    Article,
    /// Button
    Button,
    /// Link
    Link,
    /// Heading
    Heading,
    /// List
    List,
    /// ListItem
    ListItem,
    /// Image
    Image,
    /// Form
    Form,
    /// TextField
    TextField,
    /// CheckBox
    CheckBox,
    /// RadioButton
    RadioButton,
    /// ComboBox
    ComboBox,
    /// Menu
    Menu,
    /// MenuItem
    MenuItem,
    /// Dialog
    Dialog,
    /// Alert
    Alert,
    /// Toolbar
    Toolbar,
    /// Tab
    Tab,
    /// TabPanel
    TabPanel,
    /// Table
    Table,
    /// Row
    Row,
    /// Cell
    Cell,
    /// ProgressBar
    ProgressBar,
    /// Slider
    Slider,
    /// Tree
    Tree,
    /// TreeItem
    TreeItem,
    /// Region
    Region,
    /// Banner
    Banner,
    /// Navigation
    Navigation,
    /// Main
    Main,
    /// Complementary
    Complementary,
    /// ContentInfo
    ContentInfo,
    /// Search
    Search,
}

/// Accessibility state
#[derive(Debug, Clone, Copy, Default)]
pub struct A11yState {
    /// Is focusable
    pub focusable: bool,
    /// Is focused
    pub focused: bool,
    /// Is disabled
    pub disabled: bool,
    /// Is selected
    pub selected: bool,
    /// Is checked
    pub checked: Option<bool>,
    /// Is expanded
    pub expanded: Option<bool>,
    /// Is pressed
    pub pressed: Option<bool>,
    /// Is required
    pub required: bool,
    /// Is invalid
    pub invalid: bool,
    /// Is busy
    pub busy: bool,
    /// Is hidden
    pub hidden: bool,
}

/// Accessibility node
#[derive(Debug, Clone)]
pub struct A11yNode {
    /// Unique ID
    pub id: u64,
    /// Role
    pub role: Role,
    /// Name (accessible name)
    pub name: String,
    /// Description
    pub description: String,
    /// Value (for inputs)
    pub value: String,
    /// State
    pub state: A11yState,
    /// Level (for headings)
    pub level: Option<u8>,
    /// Position in set
    pub pos_in_set: Option<u32>,
    /// Set size
    pub set_size: Option<u32>,
    /// Parent ID
    pub parent: Option<u64>,
    /// Child IDs
    pub children: Vec<u64>,
    /// Labelled by
    pub labelled_by: Vec<u64>,
    /// Described by
    pub described_by: Vec<u64>,
    /// Controls
    pub controls: Vec<u64>,
    /// Bounding box
    pub bounds: A11yBounds,
}

/// Bounding box
#[derive(Debug, Clone, Copy, Default)]
pub struct A11yBounds {
    /// X position
    pub x: i32,
    /// Y position
    pub y: i32,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
}

impl A11yNode {
    /// Create new node
    pub fn new(id: u64, role: Role) -> Self {
        Self {
            id,
            role,
            name: String::new(),
            description: String::new(),
            value: String::new(),
            state: A11yState::default(),
            level: None,
            pos_in_set: None,
            set_size: None,
            parent: None,
            children: Vec::new(),
            labelled_by: Vec::new(),
            described_by: Vec::new(),
            controls: Vec::new(),
            bounds: A11yBounds::default(),
        }
    }

    /// Set name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set focusable
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.state.focusable = focusable;
        self
    }

    /// Add child
    pub fn with_child(mut self, child_id: u64) -> Self {
        self.children.push(child_id);
        self
    }
}

/// Accessibility tree
pub struct A11yTree {
    /// Root node ID
    root: Option<u64>,
    /// All nodes
    nodes: alloc::collections::BTreeMap<u64, A11yNode>,
    /// Next ID
    next_id: u64,
    /// Focus ID
    focus: Option<u64>,
}

impl A11yTree {
    /// Create new tree
    pub fn new() -> Self {
        Self {
            root: None,
            nodes: alloc::collections::BTreeMap::new(),
            next_id: 1,
            focus: None,
        }
    }

    /// Add node
    pub fn add_node(&mut self, mut node: A11yNode) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        node.id = id;

        if self.root.is_none() {
            self.root = Some(id);
        }

        self.nodes.insert(id, node);
        id
    }

    /// Get node
    pub fn get_node(&self, id: u64) -> Option<&A11yNode> {
        self.nodes.get(&id)
    }

    /// Get node mut
    pub fn get_node_mut(&mut self, id: u64) -> Option<&mut A11yNode> {
        self.nodes.get_mut(&id)
    }

    /// Remove node
    pub fn remove_node(&mut self, id: u64) -> Option<A11yNode> {
        self.nodes.remove(&id)
    }

    /// Get root
    pub fn root(&self) -> Option<&A11yNode> {
        self.root.and_then(|id| self.nodes.get(&id))
    }

    /// Get focused node
    pub fn focused(&self) -> Option<&A11yNode> {
        self.focus.and_then(|id| self.nodes.get(&id))
    }

    /// Set focus
    pub fn set_focus(&mut self, id: u64) -> Result<(), A11yError> {
        if let Some(node) = self.nodes.get_mut(&id) {
            if !node.state.focusable {
                return Err(A11yError::InvalidOperation(
                    "Node is not focusable".into()
                ));
            }
            node.state.focused = true;
        } else {
            return Err(A11yError::NodeNotFound(id));
        }

        // Remove focus from previous
        if let Some(old_focus) = self.focus {
            if old_focus != id {
                if let Some(old_node) = self.nodes.get_mut(&old_focus) {
                    old_node.state.focused = false;
                }
            }
        }

        self.focus = Some(id);
        Ok(())
    }

    /// Clear focus
    pub fn clear_focus(&mut self) {
        if let Some(id) = self.focus.take() {
            if let Some(node) = self.nodes.get_mut(&id) {
                node.state.focused = false;
            }
        }
    }

    /// Find nodes by role
    pub fn find_by_role(&self, role: Role) -> Vec<&A11yNode> {
        self.nodes.values()
            .filter(|n| n.role == role)
            .collect()
    }

    /// Clear tree
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.root = None;
        self.focus = None;
        self.next_id = 1;
    }
}

impl Default for A11yTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Accessibility manager
pub struct A11yManager {
    /// Tree
    tree: A11yTree,
    /// Screen reader enabled
    screen_reader_enabled: bool,
    /// High contrast mode
    high_contrast: bool,
    /// Reduced motion
    reduced_motion: bool,
    /// Focus visible
    focus_visible: bool,
    /// Keyboard navigation mode
    keyboard_navigation: bool,
}

impl A11yManager {
    /// Create new manager
    pub fn new() -> Self {
        Self {
            tree: A11yTree::new(),
            screen_reader_enabled: false,
            high_contrast: false,
            reduced_motion: false,
            focus_visible: true,
            keyboard_navigation: false,
        }
    }

    /// Get tree
    pub fn tree(&self) -> &A11yTree {
        &self.tree
    }

    /// Get tree mut
    pub fn tree_mut(&mut self) -> &mut A11yTree {
        &mut self.tree
    }

    /// Enable screen reader
    pub fn set_screen_reader(&mut self, enabled: bool) {
        self.screen_reader_enabled = enabled;
    }

    /// Is screen reader enabled
    pub fn screen_reader_enabled(&self) -> bool {
        self.screen_reader_enabled
    }

    /// Set high contrast
    pub fn set_high_contrast(&mut self, enabled: bool) {
        self.high_contrast = enabled;
    }

    /// Is high contrast enabled
    pub fn high_contrast(&self) -> bool {
        self.high_contrast
    }

    /// Set reduced motion
    pub fn set_reduced_motion(&mut self, enabled: bool) {
        self.reduced_motion = enabled;
    }

    /// Is reduced motion enabled
    pub fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }

    /// Enter keyboard navigation mode
    pub fn enter_keyboard_mode(&mut self) {
        self.keyboard_navigation = true;
        self.focus_visible = true;
    }

    /// Exit keyboard navigation mode
    pub fn exit_keyboard_mode(&mut self) {
        self.keyboard_navigation = false;
    }

    /// Is in keyboard mode
    pub fn is_keyboard_mode(&self) -> bool {
        self.keyboard_navigation
    }
}

impl Default for A11yManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global accessibility manager
pub static A11Y_MANAGER: RwLock<A11yManager> = RwLock::new(A11yManager {
    tree: A11yTree {
        root: None,
        nodes: alloc::collections::BTreeMap::new(),
        next_id: 1,
        focus: None,
    },
    screen_reader_enabled: false,
    high_contrast: false,
    reduced_motion: false,
    focus_visible: true,
    keyboard_navigation: false,
});
