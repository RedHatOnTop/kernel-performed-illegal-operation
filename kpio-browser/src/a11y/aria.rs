//! ARIA Attributes
//!
//! WAI-ARIA (Web Accessibility Initiative - Accessible Rich Internet Applications) support.

use alloc::string::String;
use alloc::vec::Vec;

/// ARIA role
pub type AriaRole = super::Role;

/// ARIA property
#[derive(Debug, Clone, PartialEq)]
pub enum AriaProperty {
    /// aria-label
    Label(String),
    /// aria-labelledby
    LabelledBy(Vec<String>),
    /// aria-describedby
    DescribedBy(Vec<String>),
    /// aria-details
    Details(String),
    /// aria-controls
    Controls(Vec<String>),
    /// aria-owns
    Owns(Vec<String>),
    /// aria-flowto
    FlowTo(Vec<String>),
    /// aria-activedescendant
    ActiveDescendant(String),
    /// aria-colcount
    ColCount(i32),
    /// aria-colindex
    ColIndex(i32),
    /// aria-colspan
    ColSpan(i32),
    /// aria-rowcount
    RowCount(i32),
    /// aria-rowindex
    RowIndex(i32),
    /// aria-rowspan
    RowSpan(i32),
    /// aria-level
    Level(u8),
    /// aria-posinset
    PosInSet(u32),
    /// aria-setsize
    SetSize(u32),
    /// aria-valuemax
    ValueMax(f64),
    /// aria-valuemin
    ValueMin(f64),
    /// aria-valuenow
    ValueNow(f64),
    /// aria-valuetext
    ValueText(String),
    /// aria-placeholder
    Placeholder(String),
    /// aria-keyshortcuts
    KeyShortcuts(String),
    /// aria-roledescription
    RoleDescription(String),
    /// aria-orientation
    Orientation(AriaOrientation),
    /// aria-sort
    Sort(AriaSort),
    /// aria-autocomplete
    Autocomplete(AriaAutocomplete),
    /// aria-haspopup
    HasPopup(AriaHasPopup),
    /// aria-live
    Live(AriaLive),
    /// aria-atomic
    Atomic(bool),
    /// aria-relevant
    Relevant(Vec<AriaRelevant>),
    /// aria-dropeffect
    DropEffect(Vec<AriaDropEffect>),
    /// aria-grabbed
    Grabbed(Option<bool>),
    /// aria-errormessage
    ErrorMessage(String),
}

/// ARIA state
#[derive(Debug, Clone, PartialEq)]
pub enum AriaState {
    /// aria-busy
    Busy(bool),
    /// aria-checked
    Checked(AriaTristateValue),
    /// aria-current
    Current(AriaCurrent),
    /// aria-disabled
    Disabled(bool),
    /// aria-expanded
    Expanded(Option<bool>),
    /// aria-hidden
    Hidden(Option<bool>),
    /// aria-invalid
    Invalid(AriaInvalid),
    /// aria-pressed
    Pressed(AriaTristateValue),
    /// aria-selected
    Selected(Option<bool>),
    /// aria-modal
    Modal(bool),
    /// aria-multiline
    Multiline(bool),
    /// aria-multiselectable
    Multiselectable(bool),
    /// aria-readonly
    Readonly(bool),
    /// aria-required
    Required(bool),
}

/// Tristate value
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaTristateValue {
    /// True
    True,
    /// False
    False,
    /// Mixed (indeterminate)
    Mixed,
    /// Undefined (not set)
    Undefined,
}

/// aria-current values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaCurrent {
    /// Not current
    False,
    /// True (generic current)
    True,
    /// Current page
    Page,
    /// Current step
    Step,
    /// Current location
    Location,
    /// Current date
    Date,
    /// Current time
    Time,
}

/// aria-invalid values
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AriaInvalid {
    /// Valid
    False,
    /// Invalid (generic)
    True,
    /// Grammar error
    Grammar,
    /// Spelling error
    Spelling,
}

/// aria-orientation values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaOrientation {
    /// Horizontal
    Horizontal,
    /// Vertical
    Vertical,
    /// Undefined
    Undefined,
}

/// aria-sort values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaSort {
    /// None
    None,
    /// Ascending
    Ascending,
    /// Descending
    Descending,
    /// Other
    Other,
}

/// aria-autocomplete values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaAutocomplete {
    /// None
    None,
    /// Inline
    Inline,
    /// List
    List,
    /// Both
    Both,
}

/// aria-haspopup values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaHasPopup {
    /// False
    False,
    /// True (menu)
    True,
    /// Menu
    Menu,
    /// Listbox
    Listbox,
    /// Tree
    Tree,
    /// Grid
    Grid,
    /// Dialog
    Dialog,
}

/// aria-live values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaLive {
    /// Off
    Off,
    /// Polite
    Polite,
    /// Assertive
    Assertive,
}

/// aria-relevant values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaRelevant {
    /// Additions
    Additions,
    /// Removals
    Removals,
    /// Text
    Text,
    /// All
    All,
}

/// aria-dropeffect values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaDropEffect {
    /// Copy
    Copy,
    /// Execute
    Execute,
    /// Link
    Link,
    /// Move
    Move,
    /// None
    None,
    /// Popup
    Popup,
}

/// ARIA attribute collection
#[derive(Debug, Clone, Default)]
pub struct AriaAttributes {
    /// Role
    pub role: Option<AriaRole>,
    /// Properties
    pub properties: Vec<AriaProperty>,
    /// States
    pub states: Vec<AriaState>,
}

impl AriaAttributes {
    /// Create new collection
    pub fn new() -> Self {
        Self::default()
    }

    /// Set role
    pub fn with_role(mut self, role: AriaRole) -> Self {
        self.role = Some(role);
        self
    }

    /// Add property
    pub fn with_property(mut self, prop: AriaProperty) -> Self {
        self.properties.push(prop);
        self
    }

    /// Add state
    pub fn with_state(mut self, state: AriaState) -> Self {
        self.states.push(state);
        self
    }

    /// Get property
    pub fn get_property<F, T>(&self, f: F) -> Option<T>
    where
        F: Fn(&AriaProperty) -> Option<T>,
    {
        for prop in &self.properties {
            if let Some(val) = f(prop) {
                return Some(val);
            }
        }
        None
    }

    /// Check if disabled
    pub fn is_disabled(&self) -> bool {
        self.states
            .iter()
            .any(|s| matches!(s, AriaState::Disabled(true)))
    }

    /// Check if hidden
    pub fn is_hidden(&self) -> bool {
        self.states
            .iter()
            .any(|s| matches!(s, AriaState::Hidden(Some(true))))
    }

    /// Check if expanded
    pub fn is_expanded(&self) -> Option<bool> {
        for state in &self.states {
            if let AriaState::Expanded(val) = state {
                return *val;
            }
        }
        None
    }

    /// Check if checked
    pub fn is_checked(&self) -> AriaTristateValue {
        for state in &self.states {
            if let AriaState::Checked(val) = state {
                return *val;
            }
        }
        AriaTristateValue::Undefined
    }
}

/// Parse ARIA role from string
pub fn parse_role(role: &str) -> Option<AriaRole> {
    match role {
        "button" => Some(super::Role::Button),
        "link" => Some(super::Role::Link),
        "heading" => Some(super::Role::Heading),
        "list" => Some(super::Role::List),
        "listitem" => Some(super::Role::ListItem),
        "textbox" => Some(super::Role::TextField),
        "checkbox" => Some(super::Role::CheckBox),
        "radio" => Some(super::Role::RadioButton),
        "combobox" => Some(super::Role::ComboBox),
        "menu" => Some(super::Role::Menu),
        "menuitem" => Some(super::Role::MenuItem),
        "dialog" => Some(super::Role::Dialog),
        "alert" => Some(super::Role::Alert),
        "alertdialog" => Some(super::Role::Dialog),
        "toolbar" => Some(super::Role::Toolbar),
        "tab" => Some(super::Role::Tab),
        "tabpanel" => Some(super::Role::TabPanel),
        "table" => Some(super::Role::Table),
        "row" => Some(super::Role::Row),
        "cell" => Some(super::Role::Cell),
        "progressbar" => Some(super::Role::ProgressBar),
        "slider" => Some(super::Role::Slider),
        "tree" => Some(super::Role::Tree),
        "treeitem" => Some(super::Role::TreeItem),
        "navigation" => Some(super::Role::Navigation),
        "main" => Some(super::Role::Main),
        "banner" => Some(super::Role::Banner),
        "complementary" => Some(super::Role::Complementary),
        "contentinfo" => Some(super::Role::ContentInfo),
        "search" => Some(super::Role::Search),
        "region" => Some(super::Role::Region),
        "article" => Some(super::Role::Article),
        "document" => Some(super::Role::Document),
        "img" => Some(super::Role::Image),
        "form" => Some(super::Role::Form),
        _ => None,
    }
}
