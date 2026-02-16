//! DOM Document - Document node and tree management

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use hashbrown::HashMap;

use servo_types::{LocalName, QualName};

use crate::node::{Attribute, Node, NodeData, NodeId, NodeType};
use kpio_html::tokenizer::Attribute as HtmlAttribute;
use kpio_html::tree_builder::{QuirksMode, TreeSink};

/// A DOM document.
#[derive(Debug)]
pub struct Document {
    /// All nodes in the document.
    nodes: Vec<Node>,
    /// Document quirks mode.
    quirks_mode: QuirksMode,
    /// ID to node mapping.
    id_map: HashMap<String, NodeId>,
}

impl Document {
    /// Create a new empty document.
    pub fn new() -> Self {
        let mut doc = Document {
            nodes: Vec::new(),
            quirks_mode: QuirksMode::NoQuirks,
            id_map: HashMap::new(),
        };

        // Create document node
        doc.nodes.push(Node::new_document(0));
        doc
    }

    /// Get a node by ID.
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// Get a mutable node by ID.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    /// Get the document node.
    pub fn document_node(&self) -> &Node {
        &self.nodes[0]
    }

    /// Get the document element (html).
    pub fn document_element(&self) -> Option<&Node> {
        let doc = &self.nodes[0];
        let mut child_id = doc.first_child;

        while let Some(id) = child_id {
            if let Some(node) = self.get(id) {
                if node.is_element() {
                    return Some(node);
                }
                child_id = node.next_sibling;
            } else {
                break;
            }
        }
        None
    }

    /// Get the document element ID.
    pub fn document_element_id(&self) -> Option<NodeId> {
        self.document_element().map(|n| n.id)
    }

    /// Get the head element.
    pub fn head(&self) -> Option<&Node> {
        self.get_elements_by_tag_name("head")
            .first()
            .and_then(|&id| self.get(id))
    }

    /// Get the body element.
    pub fn body(&self) -> Option<&Node> {
        self.get_elements_by_tag_name("body")
            .first()
            .and_then(|&id| self.get(id))
    }

    /// Create a new element.
    pub fn create_element(&mut self, tag_name: &str) -> NodeId {
        self.create_element_ns(QualName::html(LocalName::new(tag_name)), Vec::new())
    }

    /// Create a new element with namespace.
    pub fn create_element_ns(&mut self, name: QualName, attrs: Vec<Attribute>) -> NodeId {
        let id = self.nodes.len();

        // Extract ID attribute for mapping
        let id_attr = attrs
            .iter()
            .find(|a| a.name.local.as_str() == "id")
            .map(|a| a.value.clone());

        if let Some(ref id_value) = id_attr {
            self.id_map.insert(id_value.clone(), id);
        }

        self.nodes.push(Node::new_element(id, name, attrs));
        id
    }

    /// Create a new text node.
    pub fn create_text(&mut self, content: String) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node::new_text(id, content));
        id
    }

    /// Create a new comment node.
    pub fn create_comment(&mut self, content: String) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node::new_comment(id, content));
        id
    }

    /// Append a child to a parent.
    pub fn append_child(&mut self, parent_id: NodeId, child_id: NodeId) {
        // Set child's parent
        if let Some(child) = self.nodes.get_mut(child_id) {
            child.parent = Some(parent_id);
        }

        // Get parent's current last child
        let old_last_child = self.nodes.get(parent_id).and_then(|p| p.last_child);

        // Update old last child's next_sibling
        if let Some(old_last_id) = old_last_child {
            if let Some(old_last) = self.nodes.get_mut(old_last_id) {
                old_last.next_sibling = Some(child_id);
            }
        }

        // Update child's prev_sibling
        if let Some(child) = self.nodes.get_mut(child_id) {
            child.prev_sibling = old_last_child;
        }

        // Update parent
        if let Some(parent) = self.nodes.get_mut(parent_id) {
            if parent.first_child.is_none() {
                parent.first_child = Some(child_id);
            }
            parent.last_child = Some(child_id);
        }
    }

    /// Insert a child before another child.
    pub fn insert_before(
        &mut self,
        parent_id: NodeId,
        new_child_id: NodeId,
        ref_child_id: Option<NodeId>,
    ) {
        if ref_child_id.is_none() {
            self.append_child(parent_id, new_child_id);
            return;
        }

        let ref_id = ref_child_id.unwrap();
        let prev_id = self.nodes.get(ref_id).and_then(|n| n.prev_sibling);

        // Update new child
        if let Some(new_child) = self.nodes.get_mut(new_child_id) {
            new_child.parent = Some(parent_id);
            new_child.prev_sibling = prev_id;
            new_child.next_sibling = Some(ref_id);
        }

        // Update ref child's prev_sibling
        if let Some(ref_child) = self.nodes.get_mut(ref_id) {
            ref_child.prev_sibling = Some(new_child_id);
        }

        // Update previous sibling's next_sibling
        if let Some(prev_id) = prev_id {
            if let Some(prev) = self.nodes.get_mut(prev_id) {
                prev.next_sibling = Some(new_child_id);
            }
        } else {
            // new child is first child
            if let Some(parent) = self.nodes.get_mut(parent_id) {
                parent.first_child = Some(new_child_id);
            }
        }
    }

    /// Remove a child from its parent.
    pub fn remove_child(&mut self, child_id: NodeId) {
        let (parent_id, prev_id, next_id) = {
            let child = match self.nodes.get(child_id) {
                Some(c) => c,
                None => return,
            };
            (child.parent, child.prev_sibling, child.next_sibling)
        };

        // Update previous sibling
        if let Some(prev_id) = prev_id {
            if let Some(prev) = self.nodes.get_mut(prev_id) {
                prev.next_sibling = next_id;
            }
        } else if let Some(parent_id) = parent_id {
            // child was first child
            if let Some(parent) = self.nodes.get_mut(parent_id) {
                parent.first_child = next_id;
            }
        }

        // Update next sibling
        if let Some(next_id) = next_id {
            if let Some(next) = self.nodes.get_mut(next_id) {
                next.prev_sibling = prev_id;
            }
        } else if let Some(parent_id) = parent_id {
            // child was last child
            if let Some(parent) = self.nodes.get_mut(parent_id) {
                parent.last_child = prev_id;
            }
        }

        // Clear child's links
        if let Some(child) = self.nodes.get_mut(child_id) {
            child.parent = None;
            child.prev_sibling = None;
            child.next_sibling = None;
        }
    }

    /// Get children of a node.
    pub fn children(&self, parent_id: NodeId) -> Vec<NodeId> {
        let mut children = Vec::new();
        let mut child_id = self.nodes.get(parent_id).and_then(|p| p.first_child);

        while let Some(id) = child_id {
            children.push(id);
            child_id = self.nodes.get(id).and_then(|n| n.next_sibling);
        }

        children
    }

    /// Get child element nodes.
    pub fn child_elements(&self, parent_id: NodeId) -> Vec<NodeId> {
        self.children(parent_id)
            .into_iter()
            .filter(|&id| self.nodes.get(id).map(|n| n.is_element()).unwrap_or(false))
            .collect()
    }

    /// Get elements by tag name.
    pub fn get_elements_by_tag_name(&self, tag_name: &str) -> Vec<NodeId> {
        self.nodes
            .iter()
            .filter(|n| {
                n.tag_name()
                    .map(|t| t.eq_ignore_ascii_case(tag_name))
                    .unwrap_or(false)
            })
            .map(|n| n.id)
            .collect()
    }

    /// Get element by ID.
    pub fn get_element_by_id(&self, id: &str) -> Option<NodeId> {
        self.id_map.get(id).copied()
    }

    /// Get elements by class name.
    pub fn get_elements_by_class_name(&self, class_name: &str) -> Vec<NodeId> {
        self.nodes
            .iter()
            .filter(|n| n.has_class(class_name))
            .map(|n| n.id)
            .collect()
    }

    /// Get text content of a node (recursive).
    pub fn text_content(&self, node_id: NodeId) -> String {
        let node = match self.get(node_id) {
            Some(n) => n,
            None => return String::new(),
        };

        match &node.data {
            NodeData::Text { content } => content.clone(),
            NodeData::Element { .. } => {
                let mut result = String::new();
                for child_id in self.children(node_id) {
                    result.push_str(&self.text_content(child_id));
                }
                result
            }
            _ => String::new(),
        }
    }

    /// Append or merge text to a node.
    pub fn append_text(&mut self, parent_id: NodeId, text: &str) {
        // Check if last child is text and merge
        if let Some(parent) = self.nodes.get(parent_id) {
            if let Some(last_id) = parent.last_child {
                if let Some(last) = self.nodes.get_mut(last_id) {
                    if let NodeData::Text { content } = &mut last.data {
                        content.push_str(text);
                        return;
                    }
                }
            }
        }

        // Create new text node
        let text_id = self.create_text(text.into());
        self.append_child(parent_id, text_id);
    }

    /// Get quirks mode.
    pub fn quirks_mode(&self) -> QuirksMode {
        self.quirks_mode
    }

    /// Total number of nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if document is empty (only document node).
    pub fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    /// Iterate over all nodes.
    pub fn iter(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter()
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

// Implement TreeSink for Document to integrate with HTML parser
impl TreeSink for Document {
    fn document(&self) -> NodeId {
        0
    }

    fn create_element(&mut self, name: QualName, attrs: Vec<HtmlAttribute>) -> NodeId {
        // Convert HTML attributes to DOM attributes
        let dom_attrs: Vec<Attribute> = attrs
            .into_iter()
            .map(|a| Attribute::new(a.name.as_str(), &a.value))
            .collect();

        self.create_element_ns(name, dom_attrs)
    }

    fn create_text(&mut self, text: String) -> NodeId {
        Document::create_text(self, text)
    }

    fn create_comment(&mut self, text: String) -> NodeId {
        Document::create_comment(self, text)
    }

    fn append(&mut self, parent: NodeId, child: NodeId) {
        self.append_child(parent, child);
    }

    fn append_text(&mut self, parent: NodeId, text: &str) {
        Document::append_text(self, parent, text);
    }

    fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.get(node).and_then(|n| n.parent)
    }

    fn element_name(&self, node: NodeId) -> Option<QualName> {
        self.get(node).and_then(|n| n.element_name().cloned())
    }

    fn set_quirks_mode(&mut self, quirks: QuirksMode) {
        self.quirks_mode = quirks;
    }
}

/// Parse HTML into a Document.
pub fn parse_html(html: &str) -> Document {
    use kpio_html::HtmlParser;

    let parser = HtmlParser::new(Document::new());
    parser.parse(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_document() {
        let doc = Document::new();
        assert_eq!(doc.len(), 1);
        assert!(doc.document_node().is_document());
    }

    #[test]
    fn test_create_elements() {
        let mut doc = Document::new();
        let div_id = doc.create_element("div");
        doc.append_child(0, div_id);

        assert_eq!(doc.len(), 2);
        assert_eq!(doc.get(div_id).unwrap().tag_name(), Some("div"));
    }

    #[test]
    fn test_parse_html() {
        let doc = parse_html("<html><body><p>Hello</p></body></html>");

        let p_elements = doc.get_elements_by_tag_name("p");
        assert_eq!(p_elements.len(), 1);

        let text = doc.text_content(p_elements[0]);
        assert_eq!(text, "Hello");
    }
}
