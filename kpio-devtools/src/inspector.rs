//! DOM Inspector Panel
//!
//! Provides DOM tree viewing, CSS inspection, and live editing capabilities.

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use alloc::boxed::Box;

/// Node ID for DevTools protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub i32);

/// Backend node ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendNodeId(pub i32);

/// Node type as per DevTools protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum NodeType {
    Element = 1,
    Attribute = 2,
    Text = 3,
    CdataSection = 4,
    ProcessingInstruction = 7,
    Comment = 8,
    Document = 9,
    DocumentType = 10,
    DocumentFragment = 11,
}

/// DOM node representation for DevTools.
#[derive(Debug, Clone)]
pub struct DomNode {
    /// Node ID.
    pub node_id: NodeId,
    /// Backend node ID.
    pub backend_node_id: BackendNodeId,
    /// Node type.
    pub node_type: NodeType,
    /// Node name (tag name for elements).
    pub node_name: String,
    /// Local name.
    pub local_name: String,
    /// Node value (text content for text nodes).
    pub node_value: Option<String>,
    /// Child node count.
    pub child_node_count: i32,
    /// Child nodes (may be lazy loaded).
    pub children: Option<Vec<DomNode>>,
    /// Attributes (name-value pairs).
    pub attributes: Vec<String>,
    /// Document URL (for document nodes).
    pub document_url: Option<String>,
    /// Base URL.
    pub base_url: Option<String>,
    /// Frame ID (if applicable).
    pub frame_id: Option<String>,
    /// Content document (for iframes).
    pub content_document: Option<Box<DomNode>>,
    /// Shadow root type.
    pub shadow_root_type: Option<String>,
    /// Shadow roots.
    pub shadow_roots: Option<Vec<DomNode>>,
    /// Pseudo type (::before, ::after, etc.).
    pub pseudo_type: Option<String>,
    /// Pseudo elements.
    pub pseudo_elements: Option<Vec<DomNode>>,
}

impl DomNode {
    /// Create an element node.
    pub fn element(node_id: NodeId, tag_name: &str, attributes: Vec<(&str, &str)>) -> Self {
        let attrs: Vec<String> = attributes
            .into_iter()
            .flat_map(|(k, v)| vec![k.to_string(), v.to_string()])
            .collect();
        
        Self {
            node_id,
            backend_node_id: BackendNodeId(node_id.0),
            node_type: NodeType::Element,
            node_name: tag_name.to_uppercase(),
            local_name: tag_name.to_string(),
            node_value: None,
            child_node_count: 0,
            children: Some(Vec::new()),
            attributes: attrs,
            document_url: None,
            base_url: None,
            frame_id: None,
            content_document: None,
            shadow_root_type: None,
            shadow_roots: None,
            pseudo_type: None,
            pseudo_elements: None,
        }
    }
    
    /// Create a text node.
    pub fn text(node_id: NodeId, value: &str) -> Self {
        Self {
            node_id,
            backend_node_id: BackendNodeId(node_id.0),
            node_type: NodeType::Text,
            node_name: "#text".to_string(),
            local_name: String::new(),
            node_value: Some(value.to_string()),
            child_node_count: 0,
            children: None,
            attributes: Vec::new(),
            document_url: None,
            base_url: None,
            frame_id: None,
            content_document: None,
            shadow_root_type: None,
            shadow_roots: None,
            pseudo_type: None,
            pseudo_elements: None,
        }
    }
    
    /// Create a document node.
    pub fn document(node_id: NodeId, url: &str) -> Self {
        Self {
            node_id,
            backend_node_id: BackendNodeId(node_id.0),
            node_type: NodeType::Document,
            node_name: "#document".to_string(),
            local_name: String::new(),
            node_value: None,
            child_node_count: 0,
            children: Some(Vec::new()),
            attributes: Vec::new(),
            document_url: Some(url.to_string()),
            base_url: Some(url.to_string()),
            frame_id: None,
            content_document: None,
            shadow_root_type: None,
            shadow_roots: None,
            pseudo_type: None,
            pseudo_elements: None,
        }
    }
    
    /// Add a child node.
    pub fn add_child(&mut self, child: DomNode) {
        if let Some(ref mut children) = self.children {
            children.push(child);
            self.child_node_count = children.len() as i32;
        }
    }
    
    /// Set attribute.
    pub fn set_attribute(&mut self, name: &str, value: &str) {
        // Find and update existing attribute
        let mut found = false;
        let mut i = 0;
        while i + 1 < self.attributes.len() {
            if self.attributes[i] == name {
                self.attributes[i + 1] = value.to_string();
                found = true;
                break;
            }
            i += 2;
        }
        
        if !found {
            self.attributes.push(name.to_string());
            self.attributes.push(value.to_string());
        }
    }
    
    /// Remove attribute.
    pub fn remove_attribute(&mut self, name: &str) {
        let mut i = 0;
        while i + 1 < self.attributes.len() {
            if self.attributes[i] == name {
                self.attributes.remove(i);
                self.attributes.remove(i);
                break;
            }
            i += 2;
        }
    }
    
    /// Get attribute value.
    pub fn get_attribute(&self, name: &str) -> Option<&str> {
        let mut i = 0;
        while i + 1 < self.attributes.len() {
            if self.attributes[i] == name {
                return Some(&self.attributes[i + 1]);
            }
            i += 2;
        }
        None
    }
}

/// CSS style information.
#[derive(Debug, Clone)]
pub struct CssStyle {
    /// Style sheet ID.
    pub style_sheet_id: Option<String>,
    /// CSS properties.
    pub css_properties: Vec<CssProperty>,
    /// Short-hand entries.
    pub shorthand_entries: Vec<ShorthandEntry>,
    /// CSS text.
    pub css_text: Option<String>,
    /// Range in the style sheet.
    pub range: Option<SourceRange>,
}

impl CssStyle {
    /// Create a new CSS style.
    pub fn new() -> Self {
        Self {
            style_sheet_id: None,
            css_properties: Vec::new(),
            shorthand_entries: Vec::new(),
            css_text: None,
            range: None,
        }
    }
    
    /// Add a property.
    pub fn add_property(&mut self, name: &str, value: &str) {
        self.css_properties.push(CssProperty {
            name: name.to_string(),
            value: value.to_string(),
            important: false,
            implicit: false,
            text: None,
            parsed_ok: true,
            disabled: false,
            range: None,
        });
    }
}

impl Default for CssStyle {
    fn default() -> Self {
        Self::new()
    }
}

/// CSS property.
#[derive(Debug, Clone)]
pub struct CssProperty {
    /// Property name.
    pub name: String,
    /// Property value.
    pub value: String,
    /// Whether the property has !important.
    pub important: bool,
    /// Whether the property is implicit.
    pub implicit: bool,
    /// The full property text.
    pub text: Option<String>,
    /// Whether the property was parsed correctly.
    pub parsed_ok: bool,
    /// Whether the property is disabled.
    pub disabled: bool,
    /// Range in the style sheet.
    pub range: Option<SourceRange>,
}

/// Shorthand entry.
#[derive(Debug, Clone)]
pub struct ShorthandEntry {
    /// Shorthand name.
    pub name: String,
    /// Shorthand value.
    pub value: String,
    /// Whether it has !important.
    pub important: bool,
}

/// Source range in a style sheet.
#[derive(Debug, Clone, Copy)]
pub struct SourceRange {
    /// Start line.
    pub start_line: i32,
    /// Start column.
    pub start_column: i32,
    /// End line.
    pub end_line: i32,
    /// End column.
    pub end_column: i32,
}

/// Matched CSS rules for an element.
#[derive(Debug, Clone)]
pub struct MatchedStyles {
    /// Inline style.
    pub inline_style: Option<CssStyle>,
    /// Attribute style (from style attribute).
    pub attributes_style: Option<CssStyle>,
    /// Matched CSS rules.
    pub matched_css_rules: Vec<RuleMatch>,
    /// Pseudo elements.
    pub pseudo_elements: Vec<PseudoElementMatches>,
    /// Inherited styles.
    pub inherited: Vec<InheritedStyleEntry>,
    /// CSS keyframes animations.
    pub css_keyframes_rules: Vec<CssKeyframesRule>,
}

/// Rule match.
#[derive(Debug, Clone)]
pub struct RuleMatch {
    /// The CSS rule.
    pub rule: CssRule,
    /// Matching selectors indices.
    pub matching_selectors: Vec<i32>,
}

/// CSS rule.
#[derive(Debug, Clone)]
pub struct CssRule {
    /// Style sheet ID.
    pub style_sheet_id: Option<String>,
    /// Selector list.
    pub selector_list: SelectorList,
    /// Origin.
    pub origin: StyleSheetOrigin,
    /// Style declaration.
    pub style: CssStyle,
    /// Media list (for @media rules).
    pub media: Option<Vec<CssMedia>>,
}

/// Selector list.
#[derive(Debug, Clone)]
pub struct SelectorList {
    /// Selectors.
    pub selectors: Vec<CssSelector>,
    /// Combined selector text.
    pub text: String,
}

/// CSS selector.
#[derive(Debug, Clone)]
pub struct CssSelector {
    /// Selector text.
    pub value: String,
    /// Specificity.
    pub specificity: Option<[i32; 4]>,
}

/// Style sheet origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleSheetOrigin {
    /// User agent styles.
    UserAgent,
    /// Injected styles.
    Injected,
    /// User styles.
    User,
    /// Author styles.
    Author,
}

/// CSS media query.
#[derive(Debug, Clone)]
pub struct CssMedia {
    /// Media query text.
    pub text: String,
    /// Source.
    pub source: MediaSource,
}

/// Media source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaSource {
    /// Media attribute.
    MediaRule,
    /// Import rule.
    ImportRule,
    /// Link tag.
    LinkedSheet,
    /// Inline style.
    InlineSheet,
}

/// Pseudo element matches.
#[derive(Debug, Clone)]
pub struct PseudoElementMatches {
    /// Pseudo type.
    pub pseudo_type: String,
    /// Matched rules.
    pub matches: Vec<RuleMatch>,
}

/// Inherited style entry.
#[derive(Debug, Clone)]
pub struct InheritedStyleEntry {
    /// Inline style of the ancestor.
    pub inline_style: Option<CssStyle>,
    /// Matched CSS rules.
    pub matched_css_rules: Vec<RuleMatch>,
}

/// CSS keyframes rule.
#[derive(Debug, Clone)]
pub struct CssKeyframesRule {
    /// Animation name.
    pub animation_name: String,
    /// Keyframes.
    pub keyframes: Vec<CssKeyframeRule>,
}

/// CSS keyframe rule.
#[derive(Debug, Clone)]
pub struct CssKeyframeRule {
    /// Key text (e.g., "0%", "50%", "100%").
    pub key_text: String,
    /// Style.
    pub style: CssStyle,
}

/// Box model information.
#[derive(Debug, Clone)]
pub struct BoxModel {
    /// Content box.
    pub content: Quad,
    /// Padding box.
    pub padding: Quad,
    /// Border box.
    pub border: Quad,
    /// Margin box.
    pub margin: Quad,
    /// Width.
    pub width: i32,
    /// Height.
    pub height: i32,
    /// Shape outside.
    pub shape_outside: Option<ShapeOutsideInfo>,
}

/// Quad (four points).
pub type Quad = [f64; 8];

/// Shape outside info.
#[derive(Debug, Clone)]
pub struct ShapeOutsideInfo {
    /// Bounds.
    pub bounds: Quad,
    /// Shape.
    pub shape: Vec<String>,
    /// Margin shape.
    pub margin_shape: Vec<String>,
}

/// DOM Inspector.
pub struct DomInspector {
    /// Node ID counter.
    next_node_id: i32,
    /// Node map.
    nodes: BTreeMap<i32, DomNode>,
    /// Document root.
    document: Option<DomNode>,
}

impl DomInspector {
    /// Create a new DOM inspector.
    pub fn new() -> Self {
        Self {
            next_node_id: 1,
            nodes: BTreeMap::new(),
            document: None,
        }
    }
    
    /// Generate a new node ID.
    pub fn new_node_id(&mut self) -> NodeId {
        let id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        id
    }
    
    /// Set the document.
    pub fn set_document(&mut self, document: DomNode) {
        self.document = Some(document);
    }
    
    /// Get the document.
    pub fn get_document(&self) -> Option<&DomNode> {
        self.document.as_ref()
    }
    
    /// Get node by ID.
    pub fn get_node(&self, node_id: NodeId) -> Option<&DomNode> {
        self.nodes.get(&node_id.0)
    }
    
    /// Register a node.
    pub fn register_node(&mut self, node: DomNode) {
        self.nodes.insert(node.node_id.0, node);
    }
    
    /// Query selector (simplified).
    pub fn query_selector(&self, _node_id: NodeId, selector: &str) -> Option<NodeId> {
        // Simplified implementation - would use CSS selector matching
        if let Some(ref doc) = self.document {
            if let Some(ref children) = doc.children {
                for child in children {
                    if child.local_name == selector {
                        return Some(child.node_id);
                    }
                }
            }
        }
        None
    }
    
    /// Query selector all.
    pub fn query_selector_all(&self, _node_id: NodeId, _selector: &str) -> Vec<NodeId> {
        // Simplified implementation
        Vec::new()
    }
    
    /// Get outer HTML.
    pub fn get_outer_html(&self, node_id: NodeId) -> Option<String> {
        self.get_node(node_id).map(|node| {
            self.serialize_node(node)
        })
    }
    
    /// Serialize node to HTML.
    fn serialize_node(&self, node: &DomNode) -> String {
        match node.node_type {
            NodeType::Element => {
                let mut html = String::new();
                html.push('<');
                html.push_str(&node.local_name);
                
                // Attributes
                let mut i = 0;
                while i + 1 < node.attributes.len() {
                    html.push(' ');
                    html.push_str(&node.attributes[i]);
                    html.push_str("=\"");
                    html.push_str(&node.attributes[i + 1]);
                    html.push('"');
                    i += 2;
                }
                
                html.push('>');
                
                // Children
                if let Some(ref children) = node.children {
                    for child in children {
                        html.push_str(&self.serialize_node(child));
                    }
                }
                
                html.push_str("</");
                html.push_str(&node.local_name);
                html.push('>');
                
                html
            }
            NodeType::Text => {
                node.node_value.clone().unwrap_or_default()
            }
            NodeType::Comment => {
                let mut html = String::from("<!--");
                if let Some(ref value) = node.node_value {
                    html.push_str(value);
                }
                html.push_str("-->");
                html
            }
            _ => String::new(),
        }
    }
    
    /// Set outer HTML.
    pub fn set_outer_html(&mut self, _node_id: NodeId, _html: &str) -> Result<(), &'static str> {
        // Would parse HTML and replace node
        Ok(())
    }
    
    /// Get computed style.
    pub fn get_computed_style(&self, _node_id: NodeId) -> Vec<ComputedStyleProperty> {
        // Would compute cascaded styles
        Vec::new()
    }
    
    /// Get matched styles.
    pub fn get_matched_styles(&self, _node_id: NodeId) -> MatchedStyles {
        MatchedStyles {
            inline_style: None,
            attributes_style: None,
            matched_css_rules: Vec::new(),
            pseudo_elements: Vec::new(),
            inherited: Vec::new(),
            css_keyframes_rules: Vec::new(),
        }
    }
    
    /// Get box model.
    pub fn get_box_model(&self, _node_id: NodeId) -> Option<BoxModel> {
        // Would compute layout boxes
        None
    }
    
    /// Highlight node.
    pub fn highlight_node(&self, _node_id: NodeId, _config: &HighlightConfig) {
        // Would draw overlay on the node
    }
    
    /// Hide highlight.
    pub fn hide_highlight(&self) {
        // Would remove overlay
    }
}

impl Default for DomInspector {
    fn default() -> Self {
        Self::new()
    }
}

/// Computed style property.
#[derive(Debug, Clone)]
pub struct ComputedStyleProperty {
    /// Property name.
    pub name: String,
    /// Property value.
    pub value: String,
}

/// Highlight configuration.
#[derive(Debug, Clone, Default)]
pub struct HighlightConfig {
    /// Show info tooltip.
    pub show_info: bool,
    /// Show styles in tooltip.
    pub show_styles: bool,
    /// Show rulers.
    pub show_rulers: bool,
    /// Show accessibility info.
    pub show_accessibility_info: bool,
    /// Content color (RGBA).
    pub content_color: Option<Rgba>,
    /// Padding color.
    pub padding_color: Option<Rgba>,
    /// Border color.
    pub border_color: Option<Rgba>,
    /// Margin color.
    pub margin_color: Option<Rgba>,
}

/// RGBA color.
#[derive(Debug, Clone, Copy)]
pub struct Rgba {
    /// Red (0-255).
    pub r: u8,
    /// Green (0-255).
    pub g: u8,
    /// Blue (0-255).
    pub b: u8,
    /// Alpha (0.0-1.0).
    pub a: f64,
}

impl Rgba {
    /// Create a new RGBA color.
    pub fn new(r: u8, g: u8, b: u8, a: f64) -> Self {
        Self { r, g, b, a }
    }
    
    /// Content highlight color (light blue).
    pub fn content() -> Self {
        Self::new(111, 168, 220, 0.66)
    }
    
    /// Padding highlight color (light green).
    pub fn padding() -> Self {
        Self::new(147, 196, 125, 0.55)
    }
    
    /// Border highlight color (yellow).
    pub fn border() -> Self {
        Self::new(255, 229, 153, 0.66)
    }
    
    /// Margin highlight color (orange).
    pub fn margin() -> Self {
        Self::new(246, 178, 107, 0.66)
    }
}

/// Accessibility tree node.
#[derive(Debug, Clone)]
pub struct AXNode {
    /// Node ID.
    pub node_id: String,
    /// Ignored for accessibility.
    pub ignored: bool,
    /// Ignored reasons.
    pub ignored_reasons: Vec<AXProperty>,
    /// Role.
    pub role: Option<AXValue>,
    /// Name.
    pub name: Option<AXValue>,
    /// Description.
    pub description: Option<AXValue>,
    /// Value.
    pub value: Option<AXValue>,
    /// Properties.
    pub properties: Vec<AXProperty>,
    /// Child IDs.
    pub child_ids: Vec<String>,
    /// Backend DOM node ID.
    pub backend_dom_node_id: Option<BackendNodeId>,
}

/// Accessibility value.
#[derive(Debug, Clone)]
pub struct AXValue {
    /// Value type.
    pub value_type: String,
    /// Value.
    pub value: Option<String>,
}

/// Accessibility property.
#[derive(Debug, Clone)]
pub struct AXProperty {
    /// Property name.
    pub name: String,
    /// Property value.
    pub value: AXValue,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dom_node_creation() {
        let node = DomNode::element(NodeId(1), "div", vec![("class", "container")]);
        assert_eq!(node.node_name, "DIV");
        assert_eq!(node.local_name, "div");
        assert_eq!(node.get_attribute("class"), Some("container"));
    }
    
    #[test]
    fn test_dom_inspector() {
        let mut inspector = DomInspector::new();
        let node_id = inspector.new_node_id();
        assert_eq!(node_id.0, 1);
        
        let node_id2 = inspector.new_node_id();
        assert_eq!(node_id2.0, 2);
    }
    
    #[test]
    fn test_css_style() {
        let mut style = CssStyle::new();
        style.add_property("color", "red");
        style.add_property("font-size", "16px");
        assert_eq!(style.css_properties.len(), 2);
    }
}
