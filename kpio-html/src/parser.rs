//! HTML Parser - High-level parsing API

use alloc::string::String;
use alloc::vec::Vec;

use crate::tokenizer::{Attribute, Token, Tokenizer};
use crate::tree_builder::{NodeId, QuirksMode, TreeBuilder, TreeSink};
use servo_types::QualName;

/// HTML parse error.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    /// Unexpected token
    UnexpectedToken(String),
    /// Unexpected end of file
    UnexpectedEof,
    /// Invalid nesting
    InvalidNesting(String),
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ParseError::UnexpectedToken(t) => write!(f, "Unexpected token: {}", t),
            ParseError::UnexpectedEof => write!(f, "Unexpected end of file"),
            ParseError::InvalidNesting(s) => write!(f, "Invalid nesting: {}", s),
        }
    }
}

/// High-level HTML parser.
pub struct HtmlParser<S: TreeSink> {
    tree_builder: TreeBuilder<S>,
}

impl<S: TreeSink> HtmlParser<S> {
    /// Create a new HTML parser.
    pub fn new(sink: S) -> Self {
        HtmlParser {
            tree_builder: TreeBuilder::new(sink),
        }
    }

    /// Parse an HTML string.
    pub fn parse(mut self, html: &str) -> S {
        let tokenizer = Tokenizer::new(html);

        for token in tokenizer {
            self.tree_builder.process_token(token);
        }

        // Process EOF
        self.tree_builder.process_token(Token::Eof);

        self.tree_builder.into_sink()
    }

    /// Parse an HTML fragment.
    pub fn parse_fragment(mut self, html: &str, context: &str) -> S {
        // Simplified fragment parsing
        self.parse(html)
    }

    /// Get a reference to the sink.
    pub fn sink(&self) -> &S {
        self.tree_builder.sink()
    }

    /// Get a mutable reference to the sink.
    pub fn sink_mut(&mut self) -> &mut S {
        self.tree_builder.sink_mut()
    }
}

/// A simple DOM implementation for testing.
#[derive(Debug, Clone, Default)]
pub struct SimpleDocument {
    nodes: Vec<SimpleNode>,
}

/// A simple DOM node.
#[derive(Debug, Clone)]
pub enum SimpleNode {
    Document,
    Element {
        name: QualName,
        attrs: Vec<Attribute>,
        children: Vec<NodeId>,
        parent: Option<NodeId>,
    },
    Text {
        content: String,
        parent: Option<NodeId>,
    },
    Comment {
        content: String,
        parent: Option<NodeId>,
    },
}

impl SimpleDocument {
    /// Create a new empty document.
    pub fn new() -> Self {
        let mut doc = SimpleDocument { nodes: Vec::new() };
        doc.nodes.push(SimpleNode::Document);
        doc
    }

    /// Get a node by ID.
    pub fn get(&self, id: NodeId) -> Option<&SimpleNode> {
        self.nodes.get(id)
    }

    /// Get a mutable node by ID.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut SimpleNode> {
        self.nodes.get_mut(id)
    }

    /// Get the document element (html).
    pub fn document_element(&self) -> Option<NodeId> {
        if let Some(SimpleNode::Document) = self.nodes.get(0) {
            // Find the first element child
            for (i, node) in self.nodes.iter().enumerate() {
                if let SimpleNode::Element {
                    parent: Some(0), ..
                } = node
                {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Get children of a node.
    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        match self.get(id) {
            Some(SimpleNode::Document) => self
                .nodes
                .iter()
                .enumerate()
                .filter_map(|(i, n)| match n {
                    SimpleNode::Element {
                        parent: Some(0), ..
                    } => Some(i),
                    SimpleNode::Text {
                        parent: Some(0), ..
                    } => Some(i),
                    SimpleNode::Comment {
                        parent: Some(0), ..
                    } => Some(i),
                    _ => None,
                })
                .collect(),
            Some(SimpleNode::Element { children, .. }) => children.clone(),
            _ => Vec::new(),
        }
    }

    /// Get element by tag name (first match).
    pub fn get_elements_by_tag_name(&self, name: &str) -> Vec<NodeId> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                if let SimpleNode::Element { name: n, .. } = n {
                    if n.local.as_str() == name {
                        return Some(i);
                    }
                }
                None
            })
            .collect()
    }

    /// Get text content of a node.
    pub fn text_content(&self, id: NodeId) -> String {
        match self.get(id) {
            Some(SimpleNode::Text { content, .. }) => content.clone(),
            Some(SimpleNode::Element { children, .. }) => children
                .iter()
                .map(|&c| self.text_content(c))
                .collect::<Vec<_>>()
                .join(""),
            _ => String::new(),
        }
    }
}

impl TreeSink for SimpleDocument {
    fn document(&self) -> NodeId {
        0
    }

    fn create_element(&mut self, name: QualName, attrs: Vec<Attribute>) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(SimpleNode::Element {
            name,
            attrs,
            children: Vec::new(),
            parent: None,
        });
        id
    }

    fn create_text(&mut self, text: String) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(SimpleNode::Text {
            content: text,
            parent: None,
        });
        id
    }

    fn create_comment(&mut self, text: String) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(SimpleNode::Comment {
            content: text,
            parent: None,
        });
        id
    }

    fn append(&mut self, parent: NodeId, child: NodeId) {
        // Set parent
        match self.nodes.get_mut(child) {
            Some(SimpleNode::Element { parent: p, .. }) => *p = Some(parent),
            Some(SimpleNode::Text { parent: p, .. }) => *p = Some(parent),
            Some(SimpleNode::Comment { parent: p, .. }) => *p = Some(parent),
            _ => {}
        }

        // Add to children
        if let Some(SimpleNode::Element { children, .. }) = self.nodes.get_mut(parent) {
            children.push(child);
        }
    }

    fn append_text(&mut self, parent: NodeId, text: &str) {
        // Check if last child is text and merge
        if let Some(SimpleNode::Element { children, .. }) = self.nodes.get(parent) {
            if let Some(&last_child) = children.last() {
                if let Some(SimpleNode::Text { content, .. }) = self.nodes.get_mut(last_child) {
                    content.push_str(text);
                    return;
                }
            }
        }

        // Create new text node
        let text_node = self.create_text(text.into());
        self.append(parent, text_node);
    }

    fn parent(&self, node: NodeId) -> Option<NodeId> {
        match self.get(node) {
            Some(SimpleNode::Element { parent, .. }) => *parent,
            Some(SimpleNode::Text { parent, .. }) => *parent,
            Some(SimpleNode::Comment { parent, .. }) => *parent,
            _ => None,
        }
    }

    fn element_name(&self, node: NodeId) -> Option<QualName> {
        match self.get(node) {
            Some(SimpleNode::Element { name, .. }) => Some(name.clone()),
            _ => None,
        }
    }

    fn set_quirks_mode(&mut self, _quirks: QuirksMode) {
        // Simplified: ignore quirks mode
    }
}

/// Parse HTML into a SimpleDocument.
pub fn parse_html(html: &str) -> SimpleDocument {
    let parser = HtmlParser::new(SimpleDocument::new());
    parser.parse(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let doc = parse_html("<html><head></head><body><p>Hello</p></body></html>");

        let html_elements = doc.get_elements_by_tag_name("html");
        assert_eq!(html_elements.len(), 1);

        let p_elements = doc.get_elements_by_tag_name("p");
        assert_eq!(p_elements.len(), 1);

        let text = doc.text_content(p_elements[0]);
        assert_eq!(text, "Hello");
    }

    #[test]
    fn test_parse_implicit_tags() {
        let doc = parse_html("<p>Hello</p>");

        // Should have html, head, body inserted
        assert!(!doc.get_elements_by_tag_name("html").is_empty());
        assert!(!doc.get_elements_by_tag_name("head").is_empty());
        assert!(!doc.get_elements_by_tag_name("body").is_empty());
    }
}
