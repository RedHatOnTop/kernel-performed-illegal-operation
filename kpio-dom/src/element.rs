//! DOM Element - Element node implementation

use alloc::string::String;
use alloc::vec::Vec;

use servo_types::namespace::HTML_NAMESPACE;
use servo_types::{LocalName, Namespace, QualName};

use crate::node::{Attribute, Node, NodeData, NodeId};
use crate::Document;

/// Element-specific data.
#[derive(Debug, Clone)]
pub struct ElementData {
    pub tag_name: QualName,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: Vec<Attribute>,
}

impl ElementData {
    /// Create element data from a QualName and attributes.
    pub fn new(tag_name: QualName, attributes: Vec<Attribute>) -> Self {
        let id = attributes
            .iter()
            .find(|a| a.name.local.as_str() == "id")
            .map(|a| a.value.clone());

        let classes: Vec<String> = attributes
            .iter()
            .find(|a| a.name.local.as_str() == "class")
            .map(|a| a.value.split_whitespace().map(|s| s.into()).collect())
            .unwrap_or_default();

        ElementData {
            tag_name,
            id,
            classes,
            attributes,
        }
    }

    /// Create element data for an HTML element.
    pub fn html(local_name: &str) -> Self {
        ElementData::new(QualName::html(LocalName::new(local_name)), Vec::new())
    }

    /// Get attribute value.
    pub fn get_attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|a| a.name.local.as_str() == name)
            .map(|a| a.value.as_str())
    }

    /// Set attribute value.
    pub fn set_attribute(&mut self, name: &str, value: String) {
        // Update cached id/classes
        if name == "id" {
            self.id = Some(value.clone());
        } else if name == "class" {
            self.classes = value.split_whitespace().map(|s| s.into()).collect();
        }

        // Update or add attribute
        if let Some(attr) = self
            .attributes
            .iter_mut()
            .find(|a| a.name.local.as_str() == name)
        {
            attr.value = value;
        } else {
            self.attributes.push(Attribute::new(name, &value));
        }
    }

    /// Remove attribute.
    pub fn remove_attribute(&mut self, name: &str) {
        if name == "id" {
            self.id = None;
        } else if name == "class" {
            self.classes.clear();
        }
        self.attributes.retain(|a| a.name.local.as_str() != name);
    }

    /// Check if has attribute.
    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes
            .iter()
            .any(|a| a.name.local.as_str() == name)
    }

    /// Get local tag name.
    pub fn local_name(&self) -> &str {
        self.tag_name.local.as_str()
    }

    /// Check if element matches a tag name.
    pub fn matches_tag(&self, tag: &str) -> bool {
        self.tag_name.local.as_str().eq_ignore_ascii_case(tag)
    }

    /// Check if element has a class.
    pub fn has_class(&self, class: &str) -> bool {
        self.classes.iter().any(|c| c == class)
    }

    /// Add a class.
    pub fn add_class(&mut self, class: &str) {
        if !self.has_class(class) {
            self.classes.push(class.into());
            self.update_class_attribute();
        }
    }

    /// Remove a class.
    pub fn remove_class(&mut self, class: &str) {
        self.classes.retain(|c| c != class);
        self.update_class_attribute();
    }

    /// Toggle a class.
    pub fn toggle_class(&mut self, class: &str) -> bool {
        if self.has_class(class) {
            self.remove_class(class);
            false
        } else {
            self.add_class(class);
            true
        }
    }

    fn update_class_attribute(&mut self) {
        let class_str = self.classes.join(" ");
        if class_str.is_empty() {
            self.remove_attribute("class");
        } else {
            if let Some(attr) = self
                .attributes
                .iter_mut()
                .find(|a| a.name.local.as_str() == "class")
            {
                attr.value = class_str;
            } else {
                self.attributes.push(Attribute::new("class", &class_str));
            }
        }
    }
}

/// Element trait for accessing element functionality on nodes.
pub trait Element {
    /// Get the tag name.
    fn tag_name(&self) -> Option<&str>;

    /// Get the ID.
    fn id(&self) -> Option<&str>;

    /// Get the class list.
    fn class_list(&self) -> &[String];

    /// Get an attribute value.
    fn get_attribute(&self, name: &str) -> Option<&str>;

    /// Check if has a class.
    fn has_class(&self, class: &str) -> bool;

    /// Check if this is an HTML element with the given tag name.
    fn is_html_element(&self, tag: &str) -> bool;
}

impl Element for Node {
    fn tag_name(&self) -> Option<&str> {
        self.tag_name()
    }

    fn id(&self) -> Option<&str> {
        self.element_id()
    }

    fn class_list(&self) -> &[String] {
        self.element_classes()
    }

    fn get_attribute(&self, name: &str) -> Option<&str> {
        Node::get_attribute(self, name)
    }

    fn has_class(&self, class: &str) -> bool {
        Node::has_class(self, class)
    }

    fn is_html_element(&self, tag: &str) -> bool {
        if let Some(name) = self.element_name() {
            name.ns == Namespace::new(HTML_NAMESPACE)
                && name.local.as_str().eq_ignore_ascii_case(tag)
        } else {
            false
        }
    }
}
