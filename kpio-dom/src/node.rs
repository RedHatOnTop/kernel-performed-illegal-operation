//! DOM Node - Base node type

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use servo_types::{LocalName, QualName};

/// Node ID - unique identifier within a document.
pub type NodeId = usize;

/// DOM node types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum NodeType {
    Element = 1,
    Attribute = 2,
    Text = 3,
    CDataSection = 4,
    ProcessingInstruction = 7,
    Comment = 8,
    Document = 9,
    DocumentType = 10,
    DocumentFragment = 11,
}

/// A DOM node.
#[derive(Debug, Clone)]
pub struct Node {
    /// Unique ID of this node.
    pub id: NodeId,
    /// Node type.
    pub node_type: NodeType,
    /// Node data (element, text, etc.)
    pub data: NodeData,
    /// Parent node ID.
    pub parent: Option<NodeId>,
    /// First child node ID.
    pub first_child: Option<NodeId>,
    /// Last child node ID.
    pub last_child: Option<NodeId>,
    /// Previous sibling node ID.
    pub prev_sibling: Option<NodeId>,
    /// Next sibling node ID.
    pub next_sibling: Option<NodeId>,
}

/// Node data union.
#[derive(Debug, Clone)]
pub enum NodeData {
    /// Document node
    Document,
    /// Document type
    DocumentType {
        name: String,
        public_id: String,
        system_id: String,
    },
    /// Element node
    Element {
        name: QualName,
        attrs: Vec<Attribute>,
        /// Element ID attribute value (cached)
        id: Option<String>,
        /// Element class list (cached)
        classes: Vec<String>,
    },
    /// Text node
    Text { content: String },
    /// Comment node
    Comment { content: String },
    /// Processing instruction
    ProcessingInstruction { target: String, data: String },
}

/// An element attribute.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: QualName,
    pub value: String,
}

impl Attribute {
    /// Create a new attribute.
    pub fn new(name: &str, value: &str) -> Self {
        Attribute {
            name: QualName::html(LocalName::new(name)),
            value: value.into(),
        }
    }

    /// Create an attribute with a qualified name.
    pub fn with_name(name: QualName, value: String) -> Self {
        Attribute { name, value }
    }
}

impl Node {
    /// Create a new document node.
    pub fn new_document(id: NodeId) -> Self {
        Node {
            id,
            node_type: NodeType::Document,
            data: NodeData::Document,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
        }
    }

    /// Create a new element node.
    pub fn new_element(id: NodeId, name: QualName, attrs: Vec<Attribute>) -> Self {
        let id_attr = attrs
            .iter()
            .find(|a| a.name.local.as_str() == "id")
            .map(|a| a.value.clone());

        let classes: Vec<String> = attrs
            .iter()
            .find(|a| a.name.local.as_str() == "class")
            .map(|a| a.value.split_whitespace().map(|s| s.into()).collect())
            .unwrap_or_default();

        Node {
            id,
            node_type: NodeType::Element,
            data: NodeData::Element {
                name,
                attrs,
                id: id_attr,
                classes,
            },
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
        }
    }

    /// Create a new text node.
    pub fn new_text(id: NodeId, content: String) -> Self {
        Node {
            id,
            node_type: NodeType::Text,
            data: NodeData::Text { content },
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
        }
    }

    /// Create a new comment node.
    pub fn new_comment(id: NodeId, content: String) -> Self {
        Node {
            id,
            node_type: NodeType::Comment,
            data: NodeData::Comment { content },
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
        }
    }

    /// Create a new doctype node.
    pub fn new_doctype(id: NodeId, name: String, public_id: String, system_id: String) -> Self {
        Node {
            id,
            node_type: NodeType::DocumentType,
            data: NodeData::DocumentType {
                name,
                public_id,
                system_id,
            },
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
        }
    }

    /// Check if this is an element node.
    pub fn is_element(&self) -> bool {
        self.node_type == NodeType::Element
    }

    /// Check if this is a text node.
    pub fn is_text(&self) -> bool {
        self.node_type == NodeType::Text
    }

    /// Check if this is a document node.
    pub fn is_document(&self) -> bool {
        self.node_type == NodeType::Document
    }

    /// Get element name (if element).
    pub fn element_name(&self) -> Option<&QualName> {
        match &self.data {
            NodeData::Element { name, .. } => Some(name),
            _ => None,
        }
    }

    /// Get tag name (local name of element).
    pub fn tag_name(&self) -> Option<&str> {
        self.element_name().map(|n| n.local.as_str())
    }

    /// Get element ID (if element with id attribute).
    pub fn element_id(&self) -> Option<&str> {
        match &self.data {
            NodeData::Element { id: Some(id), .. } => Some(id.as_str()),
            _ => None,
        }
    }

    /// Get element classes (if element).
    pub fn element_classes(&self) -> &[String] {
        match &self.data {
            NodeData::Element { classes, .. } => classes,
            _ => &[],
        }
    }

    /// Get text content (if text node).
    pub fn text_content(&self) -> Option<&str> {
        match &self.data {
            NodeData::Text { content } => Some(content),
            _ => None,
        }
    }

    /// Get attribute value.
    pub fn get_attribute(&self, name: &str) -> Option<&str> {
        match &self.data {
            NodeData::Element { attrs, .. } => attrs
                .iter()
                .find(|a| a.name.local.as_str() == name)
                .map(|a| a.value.as_str()),
            _ => None,
        }
    }

    /// Check if element has a class.
    pub fn has_class(&self, class: &str) -> bool {
        match &self.data {
            NodeData::Element { classes, .. } => classes.iter().any(|c| c == class),
            _ => false,
        }
    }

    /// Check if this is a void element (no closing tag).
    pub fn is_void_element(&self) -> bool {
        if let Some(name) = self.tag_name() {
            matches!(
                name,
                "area"
                    | "base"
                    | "br"
                    | "col"
                    | "embed"
                    | "hr"
                    | "img"
                    | "input"
                    | "link"
                    | "meta"
                    | "param"
                    | "source"
                    | "track"
                    | "wbr"
            )
        } else {
            false
        }
    }

    /// Check if node has children.
    pub fn has_children(&self) -> bool {
        self.first_child.is_some()
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.data {
            NodeData::Document => write!(f, "#document"),
            NodeData::DocumentType { name, .. } => write!(f, "<!DOCTYPE {}>", name),
            NodeData::Element { name, .. } => write!(f, "<{}>", name.local.as_str()),
            NodeData::Text { content } => {
                if content.len() > 20 {
                    write!(f, "\"{}...\"", &content[..20])
                } else {
                    write!(f, "\"{}\"", content)
                }
            }
            NodeData::Comment { content } => write!(f, "<!-- {} -->", content),
            NodeData::ProcessingInstruction { target, data } => {
                write!(f, "<?{} {}?>", target, data)
            }
        }
    }
}
