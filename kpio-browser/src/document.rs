//! DOM Document representation.
//!
//! Wraps the parsed HTML and provides DOM-like access.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::rc::Rc;
use core::cell::RefCell;

use kpio_css::Stylesheet;

/// Document object.
pub struct Document {
    /// Document title.
    title: String,
    /// Document URL.
    url: String,
    /// Root node.
    root: Option<Rc<RefCell<DocumentNode>>>,
    /// All stylesheets.
    stylesheets: Vec<Stylesheet>,
    /// Raw HTML content.
    html_content: String,
}

/// Node kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Document,
    Element,
    Text,
    Comment,
    DocType,
}

impl Default for NodeKind {
    fn default() -> Self {
        NodeKind::Element
    }
}

/// Document node - wrapper around HTML node.
#[derive(Default)]
pub struct DocumentNode {
    /// Node kind.
    pub kind: NodeKind,
    /// Tag name (for elements).
    pub tag_name: Option<String>,
    /// Node ID.
    pub id: Option<String>,
    /// Node classes.
    pub classes: Vec<String>,
    /// Node attributes.
    pub attributes: Vec<(String, String)>,
    /// Child nodes.
    pub children: Vec<Rc<RefCell<DocumentNode>>>,
    /// Parent node (weak reference to avoid cycles).
    pub parent: Option<Rc<RefCell<DocumentNode>>>,
    /// Text content (for text nodes).
    pub text_content: Option<String>,
    /// Computed styles.
    pub computed_styles: ComputedStyles,
}

/// Computed styles for a node.
#[derive(Debug, Clone, Default)]
pub struct ComputedStyles {
    pub display: DisplayValue,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub margin_top: f32,
    pub margin_right: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,
    pub color: Color,
    pub background_color: Color,
    pub font_size: f32,
    pub font_family: String,
    pub font_weight: u16,
}

/// Display value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayValue {
    #[default]
    Block,
    Inline,
    InlineBlock,
    Flex,
    Grid,
    None,
}

/// Color.
#[derive(Debug, Clone, Copy, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
    
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);
}

impl Document {
    /// Create a new empty document.
    pub fn new(url: &str) -> Self {
        Self {
            title: String::new(),
            url: url.into(),
            root: None,
            stylesheets: Vec::new(),
            html_content: String::new(),
        }
    }
    
    /// Parse HTML and create document.
    pub fn from_html(html: &str, url: &str) -> Self {
        let mut doc = Self::new(url);
        doc.parse_html(html);
        doc
    }
    
    /// Parse HTML into this document.
    pub fn parse_html(&mut self, html: &str) {
        self.html_content = html.into();
        
        // Simple HTML parsing - just extract basic structure
        let root = self.simple_parse(html);
        self.root = Some(root);
        
        // Extract title
        if let Some(root) = &self.root {
            self.title = Self::find_title(&root.borrow());
        }
    }
    
    /// Simple HTML parser for basic structure.
    fn simple_parse(&self, html: &str) -> Rc<RefCell<DocumentNode>> {
        let root = Rc::new(RefCell::new(DocumentNode {
            kind: NodeKind::Document,
            tag_name: None,
            ..Default::default()
        }));
        
        // Very basic parser - find tags and text
        let mut current = root.clone();
        let mut chars = html.chars().peekable();
        let mut text_buffer = String::new();
        
        while let Some(c) = chars.next() {
            if c == '<' {
                // Flush text buffer
                if !text_buffer.trim().is_empty() {
                    let text_node = Rc::new(RefCell::new(DocumentNode {
                        kind: NodeKind::Text,
                        text_content: Some(text_buffer.clone()),
                        parent: Some(current.clone()),
                        ..Default::default()
                    }));
                    current.borrow_mut().children.push(text_node);
                }
                text_buffer.clear();
                
                // Parse tag
                let mut tag = String::new();
                let mut is_closing = false;
                let mut is_self_closing = false;
                
                if chars.peek() == Some(&'/') {
                    chars.next();
                    is_closing = true;
                }
                
                if chars.peek() == Some(&'!') {
                    // Comment or doctype - skip
                    while let Some(tc) = chars.next() {
                        if tc == '>' {
                            break;
                        }
                    }
                    continue;
                }
                
                // Read tag name
                while let Some(&tc) = chars.peek() {
                    if tc == '>' || tc == ' ' || tc == '/' {
                        break;
                    }
                    tag.push(chars.next().unwrap());
                }
                
                // Skip attributes and find end
                while let Some(tc) = chars.next() {
                    if tc == '/' {
                        is_self_closing = true;
                    }
                    if tc == '>' {
                        break;
                    }
                }
                
                let tag_lower = tag.to_lowercase();
                
                if is_closing {
                    // Move up
                    let parent = current.borrow().parent.clone();
                    if let Some(p) = parent {
                        current = p;
                    }
                } else if is_self_closing || Self::is_void_element(&tag_lower) {
                    // Create self-closing element
                    let elem = Rc::new(RefCell::new(DocumentNode {
                        kind: NodeKind::Element,
                        tag_name: Some(tag_lower),
                        parent: Some(current.clone()),
                        ..Default::default()
                    }));
                    current.borrow_mut().children.push(elem);
                } else {
                    // Create element and move into it
                    let elem = Rc::new(RefCell::new(DocumentNode {
                        kind: NodeKind::Element,
                        tag_name: Some(tag_lower),
                        parent: Some(current.clone()),
                        ..Default::default()
                    }));
                    current.borrow_mut().children.push(elem.clone());
                    current = elem;
                }
            } else {
                text_buffer.push(c);
            }
        }
        
        root
    }
    
    /// Check if element is void (self-closing).
    fn is_void_element(tag: &str) -> bool {
        matches!(tag, "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | 
                 "input" | "link" | "meta" | "param" | "source" | "track" | "wbr")
    }
    
    /// Find document title.
    fn find_title(node: &DocumentNode) -> String {
        if node.tag_name.as_deref() == Some("title") {
            if let Some(text) = &node.text_content {
                return text.clone();
            }
            // Check first child
            if let Some(child) = node.children.first() {
                if let Some(text) = &child.borrow().text_content {
                    return text.clone();
                }
            }
        }
        
        // Recurse
        for child in &node.children {
            let title = Self::find_title(&child.borrow());
            if !title.is_empty() {
                return title;
            }
        }
        
        String::new()
    }
    
    /// Apply styles to nodes.
    pub fn compute_styles(&mut self) {
        if let Some(root) = self.root.clone() {
            self.compute_styles_recursive(&root);
        }
    }
    
    fn compute_styles_recursive(&self, node: &Rc<RefCell<DocumentNode>>) {
        {
            let mut node_ref = node.borrow_mut();
            
            // Apply default styles based on tag
            self.apply_default_styles(&mut node_ref);
        }
        
        // Recurse
        let children = node.borrow().children.clone();
        for child in children {
            self.compute_styles_recursive(&child);
        }
    }
    
    fn apply_default_styles(&self, node: &mut DocumentNode) {
        match node.tag_name.as_deref() {
            Some("div") | Some("p") | Some("header") | Some("footer") |
            Some("main") | Some("section") | Some("article") | Some("nav") => {
                node.computed_styles.display = DisplayValue::Block;
            }
            Some("h1") => {
                node.computed_styles.display = DisplayValue::Block;
                node.computed_styles.font_size = 32.0;
                node.computed_styles.font_weight = 700;
            }
            Some("h2") => {
                node.computed_styles.display = DisplayValue::Block;
                node.computed_styles.font_size = 24.0;
                node.computed_styles.font_weight = 700;
            }
            Some("h3") => {
                node.computed_styles.display = DisplayValue::Block;
                node.computed_styles.font_size = 18.0;
                node.computed_styles.font_weight = 700;
            }
            Some("h4") | Some("h5") | Some("h6") => {
                node.computed_styles.display = DisplayValue::Block;
                node.computed_styles.font_size = 16.0;
                node.computed_styles.font_weight = 700;
            }
            Some("span") | Some("a") | Some("strong") | Some("em") | Some("b") | Some("i") => {
                node.computed_styles.display = DisplayValue::Inline;
            }
            _ => {}
        }
    }
    
    /// Get document title.
    pub fn title(&self) -> &str {
        &self.title
    }
    
    /// Get document URL.
    pub fn url(&self) -> &str {
        &self.url
    }
    
    /// Get root node.
    pub fn root(&self) -> Option<&Rc<RefCell<DocumentNode>>> {
        self.root.as_ref()
    }
    
    /// Get element by ID.
    pub fn get_element_by_id(&self, id: &str) -> Option<Rc<RefCell<DocumentNode>>> {
        self.root.as_ref().and_then(|root| Self::find_by_id_recursive(&root.borrow(), id))
    }
    
    fn find_by_id_recursive(node: &DocumentNode, id: &str) -> Option<Rc<RefCell<DocumentNode>>> {
        for child in &node.children {
            if child.borrow().id.as_deref() == Some(id) {
                return Some(child.clone());
            }
            if let Some(found) = Self::find_by_id_recursive(&child.borrow(), id) {
                return Some(found);
            }
        }
        None
    }
    
    /// Query selector.
    pub fn query_selector(&self, _selector: &str) -> Option<Rc<RefCell<DocumentNode>>> {
        // Simplified - would need CSS selector parsing
        None
    }
    
    /// Query selector all.
    pub fn query_selector_all(&self, _selector: &str) -> Vec<Rc<RefCell<DocumentNode>>> {
        // Simplified - would need CSS selector parsing
        Vec::new()
    }
}
