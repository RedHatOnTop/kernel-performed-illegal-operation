//! DOM Text - Text node implementation

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::node::{Node, NodeData, NodeId};
use crate::Document;

/// Text node methods.
pub trait Text {
    /// Get the text content.
    fn text_data(&self) -> Option<&str>;

    /// Get the text length.
    fn text_length(&self) -> usize;

    /// Check if the text is whitespace only.
    fn is_whitespace_only(&self) -> bool;
}

impl Text for Node {
    fn text_data(&self) -> Option<&str> {
        match &self.data {
            NodeData::Text { content } => Some(content),
            _ => None,
        }
    }

    fn text_length(&self) -> usize {
        self.text_data().map(|t| t.len()).unwrap_or(0)
    }

    fn is_whitespace_only(&self) -> bool {
        self.text_data()
            .map(|t| t.chars().all(|c| c.is_whitespace()))
            .unwrap_or(true)
    }
}

/// Text manipulation methods for Document.
impl Document {
    /// Set the text content of a node (replaces all children).
    pub fn set_text_content(&mut self, node_id: NodeId, text: String) {
        // Remove all children
        let children = self.children(node_id);
        for child_id in children {
            self.remove_child(child_id);
        }

        // Create and append new text node
        let text_id = Document::create_text(self, text);
        self.append_child(node_id, text_id);
    }

    /// Split a text node at the given offset.
    pub fn split_text(&mut self, text_id: NodeId, offset: usize) -> Option<NodeId> {
        let (new_content, parent_id, next_id) = {
            let node = self.get_mut(text_id)?;
            if let NodeData::Text { content } = &mut node.data {
                if offset >= content.len() {
                    return None;
                }
                let new_content: String = content.drain(offset..).collect();
                (new_content, node.parent, node.next_sibling)
            } else {
                return None;
            }
        };

        // Create new text node with the split content
        let new_id = Document::create_text(self, new_content);
        
        // Insert after the original text node
        if let Some(parent_id) = parent_id {
            self.insert_before(parent_id, new_id, next_id);
        }

        Some(new_id)
    }

    /// Normalize text nodes (merge adjacent text nodes).
    pub fn normalize_text(&mut self, parent_id: NodeId) {
        let children = self.children(parent_id);
        let mut to_remove = Vec::new();
        let mut prev_text_id: Option<NodeId> = None;

        for child_id in children {
            let is_text = self.get(child_id).map(|n| n.is_text()).unwrap_or(false);
            
            if is_text {
                if let Some(prev_id) = prev_text_id {
                    // Merge with previous text node
                    let content = self.get(child_id)
                        .and_then(|n| n.text_content())
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    
                    if let Some(prev) = self.get_mut(prev_id) {
                        if let NodeData::Text { content: prev_content } = &mut prev.data {
                            prev_content.push_str(&content);
                        }
                    }
                    
                    to_remove.push(child_id);
                } else {
                    prev_text_id = Some(child_id);
                }
            } else {
                prev_text_id = None;
            }
        }

        // Remove merged nodes
        for id in to_remove {
            self.remove_child(id);
        }
    }
}
