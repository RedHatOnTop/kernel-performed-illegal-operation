//! DOM bindings for JavaScript.
//!
//! Provides JavaScript bindings to DOM-like functionality.

use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::error::JsResult;
use crate::interpreter::Interpreter;
use crate::object::{Callable, JsObject, NativeFunction, PropertyDescriptor, PropertyKey};
use crate::value::Value;

/// DOM node types.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeType {
    Element = 1,
    Attribute = 2,
    Text = 3,
    Comment = 8,
    Document = 9,
    DocumentType = 10,
    DocumentFragment = 11,
}

/// A DOM element.
#[derive(Clone, Debug)]
pub struct DomElement {
    /// Tag name.
    pub tag_name: String,
    /// Element ID.
    pub id: Option<String>,
    /// Class name.
    pub class_name: String,
    /// Inner HTML.
    pub inner_html: String,
    /// Inner text.
    pub inner_text: String,
    /// Attributes.
    pub attributes: Vec<(String, String)>,
    /// Children.
    pub children: Vec<DomElement>,
    /// Parent (weak reference by ID).
    pub parent_id: Option<u64>,
    /// Unique ID.
    pub unique_id: u64,
    /// Style.
    pub style: DomStyle,
    /// Event listeners.
    pub event_listeners: Vec<(String, Value)>,
}

impl DomElement {
    /// Create a new element.
    pub fn new(tag_name: &str) -> Self {
        static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);

        DomElement {
            tag_name: tag_name.to_uppercase(),
            id: None,
            class_name: String::new(),
            inner_html: String::new(),
            inner_text: String::new(),
            attributes: Vec::new(),
            children: Vec::new(),
            parent_id: None,
            unique_id: COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst),
            style: DomStyle::new(),
            event_listeners: Vec::new(),
        }
    }

    /// Get attribute.
    pub fn get_attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Set attribute.
    pub fn set_attribute(&mut self, name: &str, value: &str) {
        if let Some(attr) = self.attributes.iter_mut().find(|(n, _)| n == name) {
            attr.1 = value.into();
        } else {
            self.attributes.push((name.into(), value.into()));
        }

        // Update special attributes
        if name == "id" {
            self.id = Some(value.into());
        } else if name == "class" {
            self.class_name = value.into();
        }
    }

    /// Remove attribute.
    pub fn remove_attribute(&mut self, name: &str) {
        self.attributes.retain(|(n, _)| n != name);

        if name == "id" {
            self.id = None;
        } else if name == "class" {
            self.class_name.clear();
        }
    }

    /// Has attribute.
    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes.iter().any(|(n, _)| n == name)
    }

    /// Add child.
    pub fn append_child(&mut self, mut child: DomElement) {
        child.parent_id = Some(self.unique_id);
        self.children.push(child);
    }

    /// Remove child by index.
    pub fn remove_child(&mut self, index: usize) -> Option<DomElement> {
        if index < self.children.len() {
            Some(self.children.remove(index))
        } else {
            None
        }
    }

    /// Add event listener.
    pub fn add_event_listener(&mut self, event: &str, handler: Value) {
        self.event_listeners.push((event.into(), handler));
    }

    /// Remove event listener.
    pub fn remove_event_listener(&mut self, event: &str) {
        self.event_listeners.retain(|(e, _)| e != event);
    }

    /// Convert to JavaScript object.
    pub fn to_js_object(&self) -> JsObject {
        let mut obj = JsObject::new();

        // Node type
        obj.define_property(
            PropertyKey::string("nodeType"),
            PropertyDescriptor::data(
                Value::number(NodeType::Element as i32 as f64),
                false,
                true,
                true,
            ),
        );

        // Tag name
        obj.define_property(
            PropertyKey::string("tagName"),
            PropertyDescriptor::data(Value::string(self.tag_name.clone()), false, true, true),
        );

        // ID
        obj.define_property(
            PropertyKey::string("id"),
            PropertyDescriptor::data(
                self.id
                    .as_ref()
                    .map(|s| Value::string(s.clone()))
                    .unwrap_or(Value::string("")),
                true,
                true,
                true,
            ),
        );

        // Class name
        obj.define_property(
            PropertyKey::string("className"),
            PropertyDescriptor::data(Value::string(self.class_name.clone()), true, true, true),
        );

        // Inner HTML
        obj.define_property(
            PropertyKey::string("innerHTML"),
            PropertyDescriptor::data(Value::string(self.inner_html.clone()), true, true, true),
        );

        // Inner text
        obj.define_property(
            PropertyKey::string("innerText"),
            PropertyDescriptor::data(Value::string(self.inner_text.clone()), true, true, true),
        );

        // Child element count
        obj.define_property(
            PropertyKey::string("childElementCount"),
            PropertyDescriptor::data(Value::number(self.children.len() as f64), false, true, true),
        );

        obj
    }
}

/// DOM style.
#[derive(Clone, Debug, Default)]
pub struct DomStyle {
    /// Style properties.
    pub properties: Vec<(String, String)>,
}

impl DomStyle {
    /// Create a new style.
    pub fn new() -> Self {
        DomStyle {
            properties: Vec::new(),
        }
    }

    /// Get property.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Set property.
    pub fn set(&mut self, name: &str, value: &str) {
        if let Some(prop) = self.properties.iter_mut().find(|(n, _)| n == name) {
            prop.1 = value.into();
        } else {
            self.properties.push((name.into(), value.into()));
        }
    }

    /// Remove property.
    pub fn remove(&mut self, name: &str) {
        self.properties.retain(|(n, _)| n != name);
    }

    /// Convert to CSS string.
    pub fn to_css_string(&self) -> String {
        self.properties
            .iter()
            .map(|(n, v)| alloc::format!("{}: {}", n, v))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// DOM document.
#[derive(Clone, Debug)]
pub struct DomDocument {
    /// Document element.
    pub document_element: Option<DomElement>,
    /// Title.
    pub title: String,
    /// URL.
    pub url: String,
    /// All elements by ID.
    pub elements_by_id: Vec<(String, DomElement)>,
}

impl DomDocument {
    /// Create a new document.
    pub fn new() -> Self {
        DomDocument {
            document_element: None,
            title: String::new(),
            url: String::new(),
            elements_by_id: Vec::new(),
        }
    }

    /// Create an element.
    pub fn create_element(&self, tag_name: &str) -> DomElement {
        DomElement::new(tag_name)
    }

    /// Create text node.
    pub fn create_text_node(&self, text: &str) -> DomElement {
        let mut elem = DomElement::new("#text");
        elem.inner_text = text.into();
        elem
    }

    /// Get element by ID.
    pub fn get_element_by_id(&self, id: &str) -> Option<&DomElement> {
        self.elements_by_id
            .iter()
            .find(|(i, _)| i == id)
            .map(|(_, e)| e)
    }

    /// Register element by ID.
    pub fn register_element(&mut self, element: DomElement) {
        if let Some(id) = &element.id {
            self.elements_by_id.push((id.clone(), element));
        }
    }
}

impl Default for DomDocument {
    fn default() -> Self {
        Self::new()
    }
}

/// Global DOM state.
pub struct DomGlobal {
    /// Current document.
    pub document: Rc<RefCell<DomDocument>>,
    /// Window location.
    pub location: DomLocation,
    /// Window history.
    pub history: DomHistory,
    /// Local storage.
    pub local_storage: DomStorage,
    /// Session storage.
    pub session_storage: DomStorage,
}

impl DomGlobal {
    /// Create a new DOM global.
    pub fn new() -> Self {
        DomGlobal {
            document: Rc::new(RefCell::new(DomDocument::new())),
            location: DomLocation::new(),
            history: DomHistory::new(),
            local_storage: DomStorage::new(),
            session_storage: DomStorage::new(),
        }
    }
}

impl Default for DomGlobal {
    fn default() -> Self {
        Self::new()
    }
}

/// Window location.
#[derive(Clone, Debug, Default)]
pub struct DomLocation {
    /// Full URL.
    pub href: String,
    /// Protocol.
    pub protocol: String,
    /// Hostname.
    pub hostname: String,
    /// Port.
    pub port: String,
    /// Pathname.
    pub pathname: String,
    /// Search.
    pub search: String,
    /// Hash.
    pub hash: String,
}

impl DomLocation {
    /// Create a new location.
    pub fn new() -> Self {
        DomLocation::default()
    }

    /// Set from URL.
    pub fn set_url(&mut self, url: &str) {
        self.href = url.into();
        // Parse URL components (simplified)
        // TODO: Proper URL parsing
    }

    /// Convert to JavaScript object.
    pub fn to_js_object(&self) -> JsObject {
        let mut obj = JsObject::new();

        obj.define_property(
            PropertyKey::string("href"),
            PropertyDescriptor::data(Value::string(self.href.clone()), true, true, true),
        );
        obj.define_property(
            PropertyKey::string("protocol"),
            PropertyDescriptor::data(Value::string(self.protocol.clone()), true, true, true),
        );
        obj.define_property(
            PropertyKey::string("hostname"),
            PropertyDescriptor::data(Value::string(self.hostname.clone()), true, true, true),
        );
        obj.define_property(
            PropertyKey::string("port"),
            PropertyDescriptor::data(Value::string(self.port.clone()), true, true, true),
        );
        obj.define_property(
            PropertyKey::string("pathname"),
            PropertyDescriptor::data(Value::string(self.pathname.clone()), true, true, true),
        );
        obj.define_property(
            PropertyKey::string("search"),
            PropertyDescriptor::data(Value::string(self.search.clone()), true, true, true),
        );
        obj.define_property(
            PropertyKey::string("hash"),
            PropertyDescriptor::data(Value::string(self.hash.clone()), true, true, true),
        );

        obj
    }
}

/// Window history.
#[derive(Clone, Debug)]
pub struct DomHistory {
    /// History entries.
    pub entries: Vec<String>,
    /// Current index.
    pub index: usize,
}

impl DomHistory {
    /// Create a new history.
    pub fn new() -> Self {
        DomHistory {
            entries: Vec::new(),
            index: 0,
        }
    }

    /// Get length.
    pub fn length(&self) -> usize {
        self.entries.len()
    }

    /// Push state.
    pub fn push_state(&mut self, url: String) {
        // Remove forward entries
        self.entries.truncate(self.index + 1);
        self.entries.push(url);
        self.index = self.entries.len() - 1;
    }

    /// Replace state.
    pub fn replace_state(&mut self, url: String) {
        if !self.entries.is_empty() {
            self.entries[self.index] = url;
        } else {
            self.entries.push(url);
        }
    }

    /// Go back.
    pub fn back(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        }
    }

    /// Go forward.
    pub fn forward(&mut self) {
        if self.index < self.entries.len() - 1 {
            self.index += 1;
        }
    }

    /// Go to specific position.
    pub fn go(&mut self, delta: i32) {
        let new_index = self.index as i32 + delta;
        if new_index >= 0 && new_index < self.entries.len() as i32 {
            self.index = new_index as usize;
        }
    }
}

impl Default for DomHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Storage (localStorage/sessionStorage).
#[derive(Clone, Debug)]
pub struct DomStorage {
    /// Storage data.
    pub data: Vec<(String, String)>,
}

impl DomStorage {
    /// Create a new storage.
    pub fn new() -> Self {
        DomStorage { data: Vec::new() }
    }

    /// Get length.
    pub fn length(&self) -> usize {
        self.data.len()
    }

    /// Get item.
    pub fn get_item(&self, key: &str) -> Option<&str> {
        self.data
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Set item.
    pub fn set_item(&mut self, key: &str, value: &str) {
        if let Some(item) = self.data.iter_mut().find(|(k, _)| k == key) {
            item.1 = value.into();
        } else {
            self.data.push((key.into(), value.into()));
        }
    }

    /// Remove item.
    pub fn remove_item(&mut self, key: &str) {
        self.data.retain(|(k, _)| k != key);
    }

    /// Clear all.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Get key by index.
    pub fn key(&self, index: usize) -> Option<&str> {
        self.data.get(index).map(|(k, _)| k.as_str())
    }

    /// Convert to JavaScript object.
    pub fn to_js_object(&self) -> JsObject {
        let mut obj = JsObject::new();

        obj.define_property(
            PropertyKey::string("length"),
            PropertyDescriptor::data(Value::number(self.data.len() as f64), false, true, true),
        );

        // Add methods
        obj.define_property(
            PropertyKey::string("getItem"),
            PropertyDescriptor::data(
                Value::object(JsObject::function(Callable::Native(NativeFunction {
                    name: "getItem".into(),
                    length: 1,
                    func: storage_get_item,
                }))),
                false,
                true,
                true,
            ),
        );

        obj.define_property(
            PropertyKey::string("setItem"),
            PropertyDescriptor::data(
                Value::object(JsObject::function(Callable::Native(NativeFunction {
                    name: "setItem".into(),
                    length: 2,
                    func: storage_set_item,
                }))),
                false,
                true,
                true,
            ),
        );

        obj.define_property(
            PropertyKey::string("removeItem"),
            PropertyDescriptor::data(
                Value::object(JsObject::function(Callable::Native(NativeFunction {
                    name: "removeItem".into(),
                    length: 1,
                    func: storage_remove_item,
                }))),
                false,
                true,
                true,
            ),
        );

        obj.define_property(
            PropertyKey::string("clear"),
            PropertyDescriptor::data(
                Value::object(JsObject::function(Callable::Native(NativeFunction {
                    name: "clear".into(),
                    length: 0,
                    func: storage_clear,
                }))),
                false,
                true,
                true,
            ),
        );

        obj
    }
}

impl Default for DomStorage {
    fn default() -> Self {
        Self::new()
    }
}

// Storage native functions (simplified - would need proper state management)
fn storage_get_item(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let _key = args.first().unwrap_or(&Value::undefined()).to_string()?;
    // In a real implementation, this would access the actual storage
    Ok(Value::null())
}

fn storage_set_item(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let _key = args.first().unwrap_or(&Value::undefined()).to_string()?;
    let _value = args.get(1).unwrap_or(&Value::undefined()).to_string()?;
    // In a real implementation, this would set the actual storage
    Ok(Value::undefined())
}

fn storage_remove_item(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let _key = args.first().unwrap_or(&Value::undefined()).to_string()?;
    // In a real implementation, this would remove from actual storage
    Ok(Value::undefined())
}

fn storage_clear(_this: &Value, _args: &[Value]) -> JsResult<Value> {
    // In a real implementation, this would clear actual storage
    Ok(Value::undefined())
}

/// Initialize DOM bindings in the interpreter.
pub fn init_dom(interp: &mut Interpreter) {
    // Document object
    init_document(interp);

    // Window object
    init_window(interp);
}

fn init_document(interp: &mut Interpreter) {
    let mut doc = JsObject::new();

    // createElement
    doc.define_property(
        PropertyKey::string("createElement"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "createElement".into(),
                length: 1,
                func: document_create_element,
            }))),
            false,
            true,
            true,
        ),
    );

    // createTextNode
    doc.define_property(
        PropertyKey::string("createTextNode"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "createTextNode".into(),
                length: 1,
                func: document_create_text_node,
            }))),
            false,
            true,
            true,
        ),
    );

    // getElementById
    doc.define_property(
        PropertyKey::string("getElementById"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "getElementById".into(),
                length: 1,
                func: document_get_element_by_id,
            }))),
            false,
            true,
            true,
        ),
    );

    // querySelector
    doc.define_property(
        PropertyKey::string("querySelector"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "querySelector".into(),
                length: 1,
                func: document_query_selector,
            }))),
            false,
            true,
            true,
        ),
    );

    // querySelectorAll
    doc.define_property(
        PropertyKey::string("querySelectorAll"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "querySelectorAll".into(),
                length: 1,
                func: document_query_selector_all,
            }))),
            false,
            true,
            true,
        ),
    );

    interp.define_global("document", Value::object(doc));
}

fn init_window(interp: &mut Interpreter) {
    let mut window = JsObject::new();

    // setTimeout
    window.define_property(
        PropertyKey::string("setTimeout"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "setTimeout".into(),
                length: 2,
                func: window_set_timeout,
            }))),
            true,
            true,
            true,
        ),
    );

    // setInterval
    window.define_property(
        PropertyKey::string("setInterval"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "setInterval".into(),
                length: 2,
                func: window_set_interval,
            }))),
            true,
            true,
            true,
        ),
    );

    // clearTimeout
    window.define_property(
        PropertyKey::string("clearTimeout"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "clearTimeout".into(),
                length: 1,
                func: window_clear_timeout,
            }))),
            true,
            true,
            true,
        ),
    );

    // clearInterval
    window.define_property(
        PropertyKey::string("clearInterval"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "clearInterval".into(),
                length: 1,
                func: window_clear_timeout,
            }))),
            true,
            true,
            true,
        ),
    );

    // alert
    window.define_property(
        PropertyKey::string("alert"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "alert".into(),
                length: 1,
                func: window_alert,
            }))),
            true,
            true,
            true,
        ),
    );

    // confirm
    window.define_property(
        PropertyKey::string("confirm"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "confirm".into(),
                length: 1,
                func: window_confirm,
            }))),
            true,
            true,
            true,
        ),
    );

    // prompt
    window.define_property(
        PropertyKey::string("prompt"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "prompt".into(),
                length: 2,
                func: window_prompt,
            }))),
            true,
            true,
            true,
        ),
    );

    // location
    let location = DomLocation::new();
    window.define_property(
        PropertyKey::string("location"),
        PropertyDescriptor::data(Value::object(location.to_js_object()), true, true, true),
    );

    // localStorage
    let storage = DomStorage::new();
    window.define_property(
        PropertyKey::string("localStorage"),
        PropertyDescriptor::data(Value::object(storage.to_js_object()), false, true, true),
    );

    // sessionStorage
    window.define_property(
        PropertyKey::string("sessionStorage"),
        PropertyDescriptor::data(Value::object(storage.to_js_object()), false, true, true),
    );

    interp.define_global("window", Value::object(window));

    // Also define global functions
    interp.define_native_function("setTimeout", 2, window_set_timeout);
    interp.define_native_function("setInterval", 2, window_set_interval);
    interp.define_native_function("clearTimeout", 1, window_clear_timeout);
    interp.define_native_function("clearInterval", 1, window_clear_timeout);
    interp.define_native_function("alert", 1, window_alert);
    interp.define_native_function("confirm", 1, window_confirm);
    interp.define_native_function("prompt", 2, window_prompt);
}

// Document methods
fn document_create_element(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let tag = args.first().unwrap_or(&Value::undefined()).to_string()?;
    let elem = DomElement::new(&tag);
    Ok(Value::object(elem.to_js_object()))
}

fn document_create_text_node(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let text = args.first().unwrap_or(&Value::undefined()).to_string()?;
    let mut obj = JsObject::new();
    obj.define_property(
        PropertyKey::string("nodeType"),
        PropertyDescriptor::data(
            Value::number(NodeType::Text as i32 as f64),
            false,
            true,
            true,
        ),
    );
    obj.define_property(
        PropertyKey::string("textContent"),
        PropertyDescriptor::data(Value::string(text), true, true, true),
    );
    Ok(Value::object(obj))
}

fn document_get_element_by_id(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let _id = args.first().unwrap_or(&Value::undefined()).to_string()?;
    // In a real implementation, this would search the DOM
    Ok(Value::null())
}

fn document_query_selector(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let _selector = args.first().unwrap_or(&Value::undefined()).to_string()?;
    // In a real implementation, this would query the DOM
    Ok(Value::null())
}

fn document_query_selector_all(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let _selector = args.first().unwrap_or(&Value::undefined()).to_string()?;
    // Return empty NodeList
    Ok(Value::object(JsObject::array(Vec::new())))
}

// Window methods
fn window_set_timeout(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let _callback = args.first().cloned().unwrap_or(Value::undefined());
    let _delay = args.get(1).unwrap_or(&Value::number(0.0)).to_number()?;

    // Return timer ID
    static TIMER_ID: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);
    let id = TIMER_ID.fetch_add(1, core::sync::atomic::Ordering::SeqCst);

    Ok(Value::number(id as f64))
}

fn window_set_interval(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let _callback = args.first().cloned().unwrap_or(Value::undefined());
    let _delay = args.get(1).unwrap_or(&Value::number(0.0)).to_number()?;

    static TIMER_ID: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);
    let id = TIMER_ID.fetch_add(1, core::sync::atomic::Ordering::SeqCst);

    Ok(Value::number(id as f64))
}

fn window_clear_timeout(_this: &Value, _args: &[Value]) -> JsResult<Value> {
    // In a real implementation, this would clear the timer
    Ok(Value::undefined())
}

fn window_alert(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let _message = args.first().unwrap_or(&Value::undefined()).to_string()?;
    // In a real implementation, this would show an alert dialog
    Ok(Value::undefined())
}

fn window_confirm(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let _message = args.first().unwrap_or(&Value::undefined()).to_string()?;
    // In a real implementation, this would show a confirm dialog
    Ok(Value::boolean(true))
}

fn window_prompt(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let _message = args.first().unwrap_or(&Value::undefined()).to_string()?;
    let default_value = args
        .get(1)
        .map(|v| v.to_string())
        .transpose()?
        .unwrap_or_default();

    // In a real implementation, this would show a prompt dialog
    Ok(Value::string(default_value))
}
