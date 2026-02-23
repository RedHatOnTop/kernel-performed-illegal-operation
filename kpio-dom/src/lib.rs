//! KPIO DOM - Document Object Model for KPIO OS
//!
//! This crate provides a DOM implementation for the KPIO browser engine.
//! It implements a subset of the DOM specification suitable for no_std environments.

#![no_std]

extern crate alloc;

pub mod document;
pub mod element;
pub mod events;
pub mod node;
pub mod style;
pub mod text;
pub mod traversal;

pub use document::Document;
pub use element::{Element, ElementData};
pub use events::{Event, EventDispatcher, EventPhase, EventTarget, EventType};
pub use node::{Node, NodeId, NodeType};
pub use style::StyledNode;
pub use text::Text;
pub use traversal::{NodeIterator, TreeWalker};

/// Prelude for common imports
pub mod prelude {
    pub use crate::{Document, Element, ElementData, Node, NodeId, NodeType, Text};
    pub use crate::{NodeIterator, StyledNode, TreeWalker};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::parse_html;
    use alloc::string::String;
    use alloc::vec::Vec;

    /// Test 1: Basic HTML parsing
    #[test]
    fn test_basic_html_parsing() {
        let html = "<html><head><title>Test</title></head><body><p>Hello!</p></body></html>";
        let doc = parse_html(html);

        // Document should have nodes
        assert!(doc.len() > 0, "Document should have nodes");

        // Should find the <p> element
        let p_elements = doc.get_elements_by_tag_name("p");
        assert_eq!(p_elements.len(), 1, "Should have one <p> element");
    }

    /// Test 2: HTML with attributes
    #[test]
    fn test_html_with_attributes() {
        let html = r#"<div id="main" class="container"><span data-value="42">Text</span></div>"#;
        let doc = parse_html(html);

        let divs = doc.get_elements_by_tag_name("div");
        assert_eq!(divs.len(), 1, "Should have one <div> element");

        if let Some(div_id) = divs.first() {
            let id = doc.get_attribute(*div_id, "id");
            assert_eq!(id.as_deref(), Some("main"), "id attribute should be 'main'");

            let class = doc.get_attribute(*div_id, "class");
            assert_eq!(
                class.as_deref(),
                Some("container"),
                "class should be 'container'"
            );
        }
    }

    /// Test 3: Nested structure
    #[test]
    fn test_nested_structure() {
        let html = "<ul><li>A</li><li>B</li><li>C</li></ul>";
        let doc = parse_html(html);

        let li_elements = doc.get_elements_by_tag_name("li");
        assert_eq!(li_elements.len(), 3, "Should have 3 <li> elements");
    }

    /// Test 4: DOM traversal
    #[test]
    fn test_dom_traversal() {
        let html = "<div><p>First</p><p>Second</p></div>";
        let doc = parse_html(html);

        let root = doc.root();
        let walker = doc.create_tree_walker(root);

        // Walk should find nodes
        let mut count = 0;
        for _node in walker {
            count += 1;
        }
        assert!(count > 0, "Walker should traverse nodes");
    }

    /// Test 5: Document structure
    #[test]
    fn test_document_structure() {
        let html = "<html><body><main><article><h1>Title</h1><p>Content</p></article></main></body></html>";
        let doc = parse_html(html);

        // Check all expected elements
        assert!(!doc.get_elements_by_tag_name("html").is_empty());
        assert!(!doc.get_elements_by_tag_name("body").is_empty());
        assert!(!doc.get_elements_by_tag_name("main").is_empty());
        assert!(!doc.get_elements_by_tag_name("article").is_empty());
        assert!(!doc.get_elements_by_tag_name("h1").is_empty());
        assert!(!doc.get_elements_by_tag_name("p").is_empty());
    }
}
