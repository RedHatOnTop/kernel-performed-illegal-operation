//! DOM Style - Integration with CSS styling

use alloc::vec::Vec;

use crate::node::{Node, NodeId};
use crate::Document;

use kpio_css::prelude::*;
use kpio_css::computed::ComputedStyle;
use kpio_css::cascade::CascadedValues;
use kpio_css::stylesheet::{Stylesheet, Rule};
use kpio_css::selector::{SelectorList, Selector, SelectorComponent};
use kpio_css::values::LengthContext;

/// A styled DOM node with computed CSS properties.
#[derive(Debug, Clone)]
pub struct StyledNode {
    pub node_id: NodeId,
    pub style: ComputedStyle,
    pub children: Vec<StyledNode>,
}

impl StyledNode {
    /// Create a new styled node.
    pub fn new(node_id: NodeId, style: ComputedStyle) -> Self {
        StyledNode {
            node_id,
            style,
            children: Vec::new(),
        }
    }
}

/// Style resolver for applying CSS to DOM.
pub struct StyleResolver<'a> {
    document: &'a Document,
    stylesheets: Vec<Stylesheet>,
}

impl<'a> StyleResolver<'a> {
    /// Create a new style resolver.
    pub fn new(document: &'a Document) -> Self {
        StyleResolver {
            document,
            stylesheets: Vec::new(),
        }
    }

    /// Add a stylesheet.
    pub fn add_stylesheet(&mut self, stylesheet: Stylesheet) {
        self.stylesheets.push(stylesheet);
    }

    /// Parse and add CSS text.
    pub fn add_css(&mut self, css: &str) {
        if let Ok(stylesheet) = CssParser::new(css).parse_stylesheet() {
            self.stylesheets.push(stylesheet);
        }
    }

    /// Resolve styles for the entire document.
    pub fn resolve(&self) -> Option<StyledNode> {
        let doc_elem_id = self.document.document_element_id()?;
        Some(self.resolve_node(doc_elem_id, None, 0))
    }

    /// Resolve styles for a single node.
    pub fn resolve_node(&self, node_id: NodeId, parent_style: Option<&ComputedStyle>, order: u32) -> StyledNode {
        let node = match self.document.get(node_id) {
            Some(n) => n,
            None => return StyledNode::new(node_id, ComputedStyle::default()),
        };

        // Collect matching rules
        let mut cascaded = CascadedValues::new();
        let mut current_order = order;
        
        for stylesheet in &self.stylesheets {
            for rule in &stylesheet.rules {
                if let Rule::Style(style_rule) = rule {
                    if self.selector_matches(node, &style_rule.selectors) {
                        let specificity = style_rule.selectors.max_specificity();
                        cascaded.apply(
                            &style_rule.declarations,
                            specificity,
                            stylesheet.origin,
                            current_order,
                        );
                        current_order += 1;
                    }
                }
            }
        }

        let computed = ComputedStyle::compute(&cascaded, parent_style, &LengthContext::default());

        // Resolve children
        let mut styled_node = StyledNode::new(node_id, computed.clone());
        
        for child_id in self.document.children(node_id) {
            if let Some(child) = self.document.get(child_id) {
                if child.is_element() {
                    styled_node.children.push(self.resolve_node(child_id, Some(&computed), current_order));
                }
            }
        }

        styled_node
    }

    /// Check if a selector matches a node.
    fn selector_matches(&self, node: &Node, selectors: &SelectorList) -> bool {
        selectors.selectors.iter().any(|selector| self.selector_matches_single(node, selector))
    }

    /// Check if a single selector matches.
    fn selector_matches_single(&self, node: &Node, selector: &Selector) -> bool {
        // Match from right to left (most specific first)
        let components = &selector.components;
        if components.is_empty() {
            return false;
        }

        // Match the last component against the current node
        let last = &components[components.len() - 1];
        if !self.component_matches(node, last) {
            return false;
        }

        // If there's only one component, we're done
        if components.len() == 1 {
            return true;
        }

        // Handle combinators
        // Simplified: only handle descendant combinator
        let mut current_node = node;
        let mut comp_idx = components.len() - 2;
        
        while comp_idx > 0 {
            match &components[comp_idx] {
                SelectorComponent::Combinator(combinator) => {
                    match combinator {
                        kpio_css::selector::Combinator::Descendant => {
                            // Find ancestor that matches
                            if comp_idx == 0 {
                                break;
                            }
                            
                            let target = &components[comp_idx - 1];
                            let mut found = false;
                            
                            let mut parent_id = current_node.parent;
                            while let Some(pid) = parent_id {
                                if let Some(parent) = self.document.get(pid) {
                                    if self.component_matches(parent, target) {
                                        current_node = parent;
                                        found = true;
                                        break;
                                    }
                                    parent_id = parent.parent;
                                } else {
                                    break;
                                }
                            }
                            
                            if !found {
                                return false;
                            }
                            
                            if comp_idx >= 2 {
                                comp_idx -= 2;
                            } else {
                                break;
                            }
                        }
                        kpio_css::selector::Combinator::Child => {
                            // Parent must match
                            if comp_idx == 0 {
                                break;
                            }
                            
                            let target = &components[comp_idx - 1];
                            
                            if let Some(parent_id) = current_node.parent {
                                if let Some(parent) = self.document.get(parent_id) {
                                    if self.component_matches(parent, target) {
                                        current_node = parent;
                                    } else {
                                        return false;
                                    }
                                } else {
                                    return false;
                                }
                            } else {
                                return false;
                            }
                            
                            if comp_idx >= 2 {
                                comp_idx -= 2;
                            } else {
                                break;
                            }
                        }
                        _ => {
                            // Other combinators not fully implemented
                            comp_idx = comp_idx.saturating_sub(1);
                        }
                    }
                }
                _ => {
                    comp_idx = comp_idx.saturating_sub(1);
                }
            }
        }

        true
    }

    /// Check if a component matches a node.
    fn component_matches(&self, node: &Node, component: &SelectorComponent) -> bool {
        match component {
            SelectorComponent::Universal => true,
            
            SelectorComponent::Type(name) => {
                node.tag_name()
                    .map(|t| t.eq_ignore_ascii_case(name.as_str()))
                    .unwrap_or(false)
            }
            
            SelectorComponent::Class(class) => {
                node.has_class(class)
            }
            
            SelectorComponent::Id(id) => {
                node.element_id() == Some(id.as_str())
            }
            
            SelectorComponent::Attribute { name, operator, value, .. } => {
                if let Some(attr_value) = node.get_attribute(name.as_str()) {
                    if let Some(expected) = value {
                        match operator {
                            kpio_css::selector::AttributeOperator::Exists => true,
                            kpio_css::selector::AttributeOperator::Equals => {
                                attr_value == expected.as_str()
                            }
                            kpio_css::selector::AttributeOperator::Includes => {
                                attr_value.split_whitespace()
                                    .any(|w| w == expected.as_str())
                            }
                            kpio_css::selector::AttributeOperator::DashMatch => {
                                attr_value == expected.as_str() ||
                                (attr_value.starts_with(expected.as_str()) &&
                                 attr_value.chars().nth(expected.len()) == Some('-'))
                            }
                            kpio_css::selector::AttributeOperator::Prefix => {
                                attr_value.starts_with(expected.as_str())
                            }
                            kpio_css::selector::AttributeOperator::Suffix => {
                                attr_value.ends_with(expected.as_str())
                            }
                            kpio_css::selector::AttributeOperator::Substring => {
                                attr_value.contains(expected.as_str())
                            }
                        }
                    } else {
                        // Just checking for presence
                        matches!(operator, kpio_css::selector::AttributeOperator::Exists)
                    }
                } else {
                    false
                }
            }
            
            SelectorComponent::PseudoClass(pseudo) => {
                match pseudo {
                    kpio_css::selector::PseudoClass::FirstChild => {
                        if let Some(parent_id) = node.parent {
                            let children = self.document.child_elements(parent_id);
                            children.first() == Some(&node.id)
                        } else {
                            false
                        }
                    }
                    kpio_css::selector::PseudoClass::LastChild => {
                        if let Some(parent_id) = node.parent {
                            let children = self.document.child_elements(parent_id);
                            children.last() == Some(&node.id)
                        } else {
                            false
                        }
                    }
                    kpio_css::selector::PseudoClass::OnlyChild => {
                        if let Some(parent_id) = node.parent {
                            let children = self.document.child_elements(parent_id);
                            children.len() == 1 && children.first() == Some(&node.id)
                        } else {
                            false
                        }
                    }
                    kpio_css::selector::PseudoClass::Root => {
                        node.parent.map(|p| p == 0).unwrap_or(false)
                    }
                    kpio_css::selector::PseudoClass::Empty => {
                        !node.has_children()
                    }
                    // Other pseudo-classes need more context
                    _ => false,
                }
            }
            
            _ => false,
        }
    }
}

/// Extension methods for Document.
impl Document {
    /// Create a style resolver for this document.
    pub fn create_style_resolver(&self) -> StyleResolver {
        StyleResolver::new(self)
    }

    /// Apply inline styles to an element.
    pub fn get_inline_style(&self, node_id: NodeId) -> Option<kpio_css::properties::DeclarationBlock> {
        let node = self.get(node_id)?;
        let style_attr = node.get_attribute("style")?;
        
        CssParser::new(style_attr)
            .parse_declaration_block()
            .ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::parse_html;

    #[test]
    fn test_style_resolver() {
        let doc = parse_html("<html><body><div class='test'>Hello</div></body></html>");
        let mut resolver = doc.create_style_resolver();
        
        resolver.add_css(".test { color: red; }");
        
        let styled = resolver.resolve();
        assert!(styled.is_some());
    }
}
