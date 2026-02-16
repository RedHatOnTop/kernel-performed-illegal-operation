//! DOM Traversal - Tree walking and iteration

use alloc::vec;
use alloc::vec::Vec;

use crate::node::{Node, NodeId, NodeType};
use crate::Document;

/// Filter result for tree walkers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterResult {
    /// Accept the node.
    Accept,
    /// Skip this node, but process children.
    Skip,
    /// Reject this node and all descendants.
    Reject,
}

/// Node filter function type.
pub type NodeFilter = fn(&Node) -> FilterResult;

/// Tree walker for traversing the DOM.
pub struct TreeWalker<'a> {
    document: &'a Document,
    root: NodeId,
    current: NodeId,
    what_to_show: u32,
    filter: Option<NodeFilter>,
}

/// What to show constants (bitmask).
pub mod show {
    pub const ALL: u32 = 0xFFFFFFFF;
    pub const ELEMENT: u32 = 0x1;
    pub const ATTRIBUTE: u32 = 0x2;
    pub const TEXT: u32 = 0x4;
    pub const CDATA_SECTION: u32 = 0x8;
    pub const PROCESSING_INSTRUCTION: u32 = 0x40;
    pub const COMMENT: u32 = 0x80;
    pub const DOCUMENT: u32 = 0x100;
    pub const DOCUMENT_TYPE: u32 = 0x200;
    pub const DOCUMENT_FRAGMENT: u32 = 0x400;
}

impl<'a> TreeWalker<'a> {
    /// Create a new tree walker.
    pub fn new(document: &'a Document, root: NodeId) -> Self {
        TreeWalker {
            document,
            root,
            current: root,
            what_to_show: show::ALL,
            filter: None,
        }
    }

    /// Set what to show filter.
    pub fn with_what_to_show(mut self, what_to_show: u32) -> Self {
        self.what_to_show = what_to_show;
        self
    }

    /// Set custom filter.
    pub fn with_filter(mut self, filter: NodeFilter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Get the root node.
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Get the current node.
    pub fn current_node(&self) -> NodeId {
        self.current
    }

    /// Set the current node.
    pub fn set_current_node(&mut self, node: NodeId) {
        self.current = node;
    }

    /// Move to the first child.
    pub fn first_child(&mut self) -> Option<NodeId> {
        let node = self.document.get(self.current)?;
        let mut child_id = node.first_child;

        while let Some(id) = child_id {
            if self.accept_node(id) == FilterResult::Accept {
                self.current = id;
                return Some(id);
            }
            child_id = self.document.get(id).and_then(|n| n.next_sibling);
        }

        None
    }

    /// Move to the last child.
    pub fn last_child(&mut self) -> Option<NodeId> {
        let node = self.document.get(self.current)?;
        let mut child_id = node.last_child;

        while let Some(id) = child_id {
            if self.accept_node(id) == FilterResult::Accept {
                self.current = id;
                return Some(id);
            }
            child_id = self.document.get(id).and_then(|n| n.prev_sibling);
        }

        None
    }

    /// Move to the next sibling.
    pub fn next_sibling(&mut self) -> Option<NodeId> {
        let mut node = self.document.get(self.current)?;

        while let Some(sibling_id) = node.next_sibling {
            if self.accept_node(sibling_id) == FilterResult::Accept {
                self.current = sibling_id;
                return Some(sibling_id);
            }
            node = self.document.get(sibling_id)?;
        }

        None
    }

    /// Move to the previous sibling.
    pub fn previous_sibling(&mut self) -> Option<NodeId> {
        let mut node = self.document.get(self.current)?;

        while let Some(sibling_id) = node.prev_sibling {
            if self.accept_node(sibling_id) == FilterResult::Accept {
                self.current = sibling_id;
                return Some(sibling_id);
            }
            node = self.document.get(sibling_id)?;
        }

        None
    }

    /// Move to the parent node.
    pub fn parent_node(&mut self) -> Option<NodeId> {
        let node = self.document.get(self.current)?;

        if let Some(parent_id) = node.parent {
            if parent_id != self.root || self.accept_node(parent_id) == FilterResult::Accept {
                self.current = parent_id;
                return Some(parent_id);
            }
        }

        None
    }

    /// Move to the next node (depth-first).
    pub fn next_node(&mut self) -> Option<NodeId> {
        // Try first child
        if let Some(id) = self.first_child() {
            return Some(id);
        }

        // Try next sibling
        if let Some(id) = self.next_sibling() {
            return Some(id);
        }

        // Go up and find next sibling
        while let Some(_parent_id) = self.parent_node() {
            if let Some(id) = self.next_sibling() {
                return Some(id);
            }
        }

        None
    }

    /// Move to the previous node (depth-first, reverse).
    pub fn previous_node(&mut self) -> Option<NodeId> {
        // Try previous sibling's last descendant
        if let Some(sibling_id) = self.previous_sibling() {
            // Go to last descendant
            while self.last_child().is_some() {}
            return Some(self.current);
        }

        // Try parent
        self.parent_node()
    }

    fn accept_node(&self, node_id: NodeId) -> FilterResult {
        let node = match self.document.get(node_id) {
            Some(n) => n,
            None => return FilterResult::Reject,
        };

        // Check what_to_show
        let show_bit = match node.node_type {
            NodeType::Element => show::ELEMENT,
            NodeType::Attribute => show::ATTRIBUTE,
            NodeType::Text => show::TEXT,
            NodeType::CDataSection => show::CDATA_SECTION,
            NodeType::ProcessingInstruction => show::PROCESSING_INSTRUCTION,
            NodeType::Comment => show::COMMENT,
            NodeType::Document => show::DOCUMENT,
            NodeType::DocumentType => show::DOCUMENT_TYPE,
            NodeType::DocumentFragment => show::DOCUMENT_FRAGMENT,
        };

        if self.what_to_show & show_bit == 0 {
            return FilterResult::Skip;
        }

        // Apply custom filter
        if let Some(filter) = self.filter {
            filter(node)
        } else {
            FilterResult::Accept
        }
    }
}

/// Node iterator for simple iteration over a document.
pub struct NodeIterator<'a> {
    document: &'a Document,
    stack: Vec<NodeId>,
    what_to_show: u32,
}

impl<'a> NodeIterator<'a> {
    /// Create a new node iterator.
    pub fn new(document: &'a Document, root: NodeId) -> Self {
        NodeIterator {
            document,
            stack: vec![root],
            what_to_show: show::ALL,
        }
    }

    /// Set what to show filter.
    pub fn with_what_to_show(mut self, what_to_show: u32) -> Self {
        self.what_to_show = what_to_show;
        self
    }

    /// Create an iterator for elements only.
    pub fn elements(document: &'a Document, root: NodeId) -> Self {
        NodeIterator {
            document,
            stack: vec![root],
            what_to_show: show::ELEMENT,
        }
    }
}

impl<'a> Iterator for NodeIterator<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(node_id) = self.stack.pop() {
            let node = self.document.get(node_id)?;

            // Add children in reverse order (so first child is processed first)
            let mut child_id = node.last_child;
            while let Some(id) = child_id {
                self.stack.push(id);
                child_id = self.document.get(id).and_then(|n| n.prev_sibling);
            }

            // Check if this node matches the filter
            let show_bit = match node.node_type {
                NodeType::Element => show::ELEMENT,
                NodeType::Attribute => show::ATTRIBUTE,
                NodeType::Text => show::TEXT,
                NodeType::CDataSection => show::CDATA_SECTION,
                NodeType::ProcessingInstruction => show::PROCESSING_INSTRUCTION,
                NodeType::Comment => show::COMMENT,
                NodeType::Document => show::DOCUMENT,
                NodeType::DocumentType => show::DOCUMENT_TYPE,
                NodeType::DocumentFragment => show::DOCUMENT_FRAGMENT,
            };

            if self.what_to_show & show_bit != 0 {
                return Some(node);
            }
        }

        None
    }
}

/// Extension methods for Document.
impl Document {
    /// Create a tree walker for this document.
    pub fn create_tree_walker(&self, root: NodeId) -> TreeWalker {
        TreeWalker::new(self, root)
    }

    /// Create a node iterator for this document.
    pub fn create_node_iterator(&self, root: NodeId) -> NodeIterator {
        NodeIterator::new(self, root)
    }

    /// Get all descendants of a node.
    pub fn descendants(&self, root: NodeId) -> Vec<NodeId> {
        NodeIterator::new(self, root)
            .filter(|n| n.id != root)
            .map(|n| n.id)
            .collect()
    }

    /// Get all element descendants of a node.
    pub fn element_descendants(&self, root: NodeId) -> Vec<NodeId> {
        NodeIterator::elements(self, root)
            .filter(|n| n.id != root)
            .map(|n| n.id)
            .collect()
    }

    /// Get ancestors of a node (from parent to root).
    pub fn ancestors(&self, node_id: NodeId) -> Vec<NodeId> {
        let mut ancestors = Vec::new();
        let mut current = self.get(node_id).and_then(|n| n.parent);

        while let Some(id) = current {
            ancestors.push(id);
            current = self.get(id).and_then(|n| n.parent);
        }

        ancestors
    }

    /// Check if node is a descendant of another.
    pub fn is_descendant_of(&self, node_id: NodeId, ancestor_id: NodeId) -> bool {
        self.ancestors(node_id).contains(&ancestor_id)
    }

    /// Get the depth of a node (distance from root).
    pub fn depth(&self, node_id: NodeId) -> usize {
        self.ancestors(node_id).len()
    }
}
