//! CSS Selectors - Selector parsing and matching

use alloc::string::String;
use alloc::vec::Vec;
use alloc::boxed::Box;
use core::fmt;
use core::cmp::Ordering;

use servo_types::{LocalName, Namespace};

/// A list of selectors (comma-separated in CSS).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SelectorList {
    pub selectors: Vec<Selector>,
}

impl SelectorList {
    /// Create a new empty selector list.
    pub fn new() -> Self {
        SelectorList {
            selectors: Vec::new(),
        }
    }

    /// Create a selector list with one selector.
    pub fn single(selector: Selector) -> Self {
        SelectorList {
            selectors: alloc::vec![selector],
        }
    }

    /// Add a selector to the list.
    pub fn push(&mut self, selector: Selector) {
        self.selectors.push(selector);
    }

    /// Check if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.selectors.is_empty()
    }

    /// Get the highest specificity among all selectors.
    pub fn max_specificity(&self) -> Specificity {
        self.selectors
            .iter()
            .map(|s| s.specificity())
            .max()
            .unwrap_or_default()
    }
}

/// A CSS selector.
#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    /// The components of this selector, in order.
    pub components: Vec<SelectorComponent>,
}

impl Selector {
    /// Create a new empty selector.
    pub fn new() -> Self {
        Selector {
            components: Vec::new(),
        }
    }

    /// Create a universal selector.
    pub fn universal() -> Self {
        Selector {
            components: alloc::vec![SelectorComponent::Universal],
        }
    }

    /// Create a type selector.
    pub fn element(name: &str) -> Self {
        Selector {
            components: alloc::vec![SelectorComponent::Type(LocalName::new(name))],
        }
    }

    /// Create a class selector.
    pub fn class(name: &str) -> Self {
        Selector {
            components: alloc::vec![SelectorComponent::Class(name.into())],
        }
    }

    /// Create an ID selector.
    pub fn id(name: &str) -> Self {
        Selector {
            components: alloc::vec![SelectorComponent::Id(name.into())],
        }
    }

    /// Add a component to this selector.
    pub fn with(mut self, component: SelectorComponent) -> Self {
        self.components.push(component);
        self
    }

    /// Calculate the specificity of this selector.
    pub fn specificity(&self) -> Specificity {
        let mut spec = Specificity::default();
        for component in &self.components {
            match component {
                SelectorComponent::Id(_) => spec.id += 1,
                SelectorComponent::Class(_) 
                | SelectorComponent::Attribute { .. }
                | SelectorComponent::PseudoClass(_) => spec.class += 1,
                SelectorComponent::Type(_) 
                | SelectorComponent::PseudoElement(_) => spec.element += 1,
                SelectorComponent::Universal => {}
                SelectorComponent::Combinator(_) => {}
            }
        }
        spec
    }

    /// Check if this selector is empty.
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

impl Default for Selector {
    fn default() -> Self {
        Self::new()
    }
}

/// A component of a selector.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectorComponent {
    /// Universal selector `*`
    Universal,
    /// Type/element selector (e.g., `div`, `p`)
    Type(LocalName),
    /// Class selector (e.g., `.class-name`)
    Class(String),
    /// ID selector (e.g., `#id-name`)
    Id(String),
    /// Attribute selector (e.g., `[attr]`, `[attr=value]`)
    Attribute {
        name: LocalName,
        namespace: Option<Namespace>,
        operator: AttributeOperator,
        value: Option<String>,
        case_sensitivity: CaseSensitivity,
    },
    /// Pseudo-class (e.g., `:hover`, `:first-child`)
    PseudoClass(PseudoClass),
    /// Pseudo-element (e.g., `::before`, `::after`)
    PseudoElement(PseudoElement),
    /// Combinator between other components
    Combinator(Combinator),
}

/// Attribute selector operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeOperator {
    /// `[attr]` - has attribute
    Exists,
    /// `[attr=value]` - exact match
    Equals,
    /// `[attr~=value]` - whitespace-separated list contains value
    Includes,
    /// `[attr|=value]` - equals or starts with value followed by hyphen
    DashMatch,
    /// `[attr^=value]` - starts with
    Prefix,
    /// `[attr$=value]` - ends with
    Suffix,
    /// `[attr*=value]` - contains
    Substring,
}

/// Case sensitivity for attribute matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaseSensitivity {
    #[default]
    CaseSensitive,
    AsciiCaseInsensitive,
}

/// Selector combinators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    /// Descendant combinator (space)
    Descendant,
    /// Child combinator `>`
    Child,
    /// Next sibling combinator `+`
    NextSibling,
    /// Subsequent sibling combinator `~`
    SubsequentSibling,
}

impl fmt::Display for Combinator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Combinator::Descendant => write!(f, " "),
            Combinator::Child => write!(f, " > "),
            Combinator::NextSibling => write!(f, " + "),
            Combinator::SubsequentSibling => write!(f, " ~ "),
        }
    }
}

/// Pseudo-classes.
#[derive(Debug, Clone, PartialEq)]
pub enum PseudoClass {
    // User action
    Hover,
    Active,
    Focus,
    FocusVisible,
    FocusWithin,
    Visited,
    Link,
    AnyLink,

    // Input states
    Enabled,
    Disabled,
    Checked,
    Indeterminate,
    Required,
    Optional,
    Valid,
    Invalid,
    ReadOnly,
    ReadWrite,
    PlaceholderShown,
    Default,
    InRange,
    OutOfRange,

    // Tree-structural
    Root,
    Empty,
    FirstChild,
    LastChild,
    OnlyChild,
    FirstOfType,
    LastOfType,
    OnlyOfType,
    NthChild(NthExpr),
    NthLastChild(NthExpr),
    NthOfType(NthExpr),
    NthLastOfType(NthExpr),

    // Logical
    Not(Box<SelectorList>),
    Is(Box<SelectorList>),
    Where(Box<SelectorList>),
    Has(Box<SelectorList>),

    // Other
    Lang(String),
    Dir(Direction),
    Target,
    Scope,
}

/// The :dir() pseudo-class direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Ltr,
    Rtl,
}

/// An+B expression for :nth-* selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NthExpr {
    pub a: i32,
    pub b: i32,
}

impl NthExpr {
    /// Create a new An+B expression.
    pub const fn new(a: i32, b: i32) -> Self {
        NthExpr { a, b }
    }

    /// `:nth-child(odd)` = 2n+1
    pub const ODD: NthExpr = NthExpr { a: 2, b: 1 };

    /// `:nth-child(even)` = 2n
    pub const EVEN: NthExpr = NthExpr { a: 2, b: 0 };

    /// Check if an index matches this expression.
    /// Index is 1-based.
    pub fn matches(&self, index: i32) -> bool {
        if self.a == 0 {
            index == self.b
        } else {
            let n = index - self.b;
            n % self.a == 0 && n / self.a >= 0
        }
    }
}

/// Pseudo-elements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PseudoElement {
    Before,
    After,
    FirstLine,
    FirstLetter,
    Selection,
    Placeholder,
    Marker,
    Backdrop,
}

impl fmt::Display for PseudoElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PseudoElement::Before => write!(f, "::before"),
            PseudoElement::After => write!(f, "::after"),
            PseudoElement::FirstLine => write!(f, "::first-line"),
            PseudoElement::FirstLetter => write!(f, "::first-letter"),
            PseudoElement::Selection => write!(f, "::selection"),
            PseudoElement::Placeholder => write!(f, "::placeholder"),
            PseudoElement::Marker => write!(f, "::marker"),
            PseudoElement::Backdrop => write!(f, "::backdrop"),
        }
    }
}

/// Selector specificity.
///
/// Specificity is calculated as (a, b, c) where:
/// - a = number of ID selectors
/// - b = number of class selectors, attribute selectors, and pseudo-classes
/// - c = number of type selectors and pseudo-elements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Specificity {
    pub id: u16,
    pub class: u16,
    pub element: u16,
}

impl Specificity {
    /// Create a new specificity.
    pub const fn new(id: u16, class: u16, element: u16) -> Self {
        Specificity { id, class, element }
    }

    /// Convert to a single value for comparison.
    pub fn to_u32(&self) -> u32 {
        ((self.id as u32) << 16) | ((self.class as u32) << 8) | (self.element as u32)
    }

    /// Inline style specificity (highest).
    pub const INLINE: Specificity = Specificity {
        id: 1000,
        class: 0,
        element: 0,
    };
}

impl PartialOrd for Specificity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Specificity {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id
            .cmp(&other.id)
            .then(self.class.cmp(&other.class))
            .then(self.element.cmp(&other.element))
    }
}

impl fmt::Display for Specificity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.id, self.class, self.element)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specificity_comparison() {
        let id = Specificity::new(1, 0, 0);
        let class = Specificity::new(0, 1, 0);
        let element = Specificity::new(0, 0, 1);

        assert!(id > class);
        assert!(class > element);
    }

    #[test]
    fn test_nth_expr() {
        assert!(NthExpr::ODD.matches(1));
        assert!(NthExpr::ODD.matches(3));
        assert!(!NthExpr::ODD.matches(2));

        assert!(NthExpr::EVEN.matches(2));
        assert!(NthExpr::EVEN.matches(4));
        assert!(!NthExpr::EVEN.matches(1));
    }
}
