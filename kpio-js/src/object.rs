//! JavaScript object system.
//!
//! Implements JavaScript objects and their internal slots.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::ast::BlockStmt;
use crate::error::{JsError, JsResult};
use crate::value::{Symbol, Value};

/// Property key (string or symbol).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PropertyKey {
    /// String key.
    String(String),
    /// Symbol key.
    Symbol(Symbol),
    /// Index key (for arrays).
    Index(u32),
}

impl PropertyKey {
    /// Create a string key.
    pub fn string<S: Into<String>>(s: S) -> Self {
        PropertyKey::String(s.into())
    }

    /// Create an index key.
    pub fn index(i: u32) -> Self {
        PropertyKey::Index(i)
    }

    /// Convert to string.
    pub fn to_string(&self) -> String {
        match self {
            PropertyKey::String(s) => s.clone(),
            PropertyKey::Symbol(s) => {
                if let Some(desc) = &s.description {
                    alloc::format!("Symbol({})", desc)
                } else {
                    "Symbol()".into()
                }
            }
            PropertyKey::Index(i) => alloc::format!("{}", i),
        }
    }
}

impl From<&str> for PropertyKey {
    fn from(s: &str) -> Self {
        PropertyKey::String(s.into())
    }
}

impl From<String> for PropertyKey {
    fn from(s: String) -> Self {
        PropertyKey::String(s)
    }
}

impl From<u32> for PropertyKey {
    fn from(i: u32) -> Self {
        PropertyKey::Index(i)
    }
}

/// Property descriptor.
#[derive(Clone, Debug)]
pub struct PropertyDescriptor {
    /// Property value.
    pub value: Option<Value>,
    /// Whether property is writable.
    pub writable: Option<bool>,
    /// Getter function.
    pub get: Option<Value>,
    /// Setter function.
    pub set: Option<Value>,
    /// Whether property is enumerable.
    pub enumerable: Option<bool>,
    /// Whether property is configurable.
    pub configurable: Option<bool>,
}

impl PropertyDescriptor {
    /// Create a data descriptor.
    pub fn data(value: Value, writable: bool, enumerable: bool, configurable: bool) -> Self {
        PropertyDescriptor {
            value: Some(value),
            writable: Some(writable),
            get: None,
            set: None,
            enumerable: Some(enumerable),
            configurable: Some(configurable),
        }
    }

    /// Create an accessor descriptor.
    pub fn accessor(
        get: Option<Value>,
        set: Option<Value>,
        enumerable: bool,
        configurable: bool,
    ) -> Self {
        PropertyDescriptor {
            value: None,
            writable: None,
            get,
            set,
            enumerable: Some(enumerable),
            configurable: Some(configurable),
        }
    }

    /// Check if this is a data descriptor.
    pub fn is_data(&self) -> bool {
        self.value.is_some() || self.writable.is_some()
    }

    /// Check if this is an accessor descriptor.
    pub fn is_accessor(&self) -> bool {
        self.get.is_some() || self.set.is_some()
    }
}

impl Default for PropertyDescriptor {
    fn default() -> Self {
        PropertyDescriptor {
            value: None,
            writable: None,
            get: None,
            set: None,
            enumerable: None,
            configurable: None,
        }
    }
}

/// Property storage.
#[derive(Clone, Debug)]
pub struct Property {
    /// The key.
    pub key: PropertyKey,
    /// The descriptor.
    pub descriptor: PropertyDescriptor,
}

/// Object type classification.
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectKind {
    /// Ordinary object.
    Ordinary,
    /// Array object.
    Array,
    /// Function object.
    Function,
    /// Boolean wrapper.
    Boolean(bool),
    /// Number wrapper.
    Number(f64),
    /// String wrapper.
    String(String),
    /// Symbol wrapper.
    Symbol(Symbol),
    /// BigInt wrapper.
    BigInt(i64),
    /// Date object.
    Date(f64),
    /// RegExp object.
    RegExp { pattern: String, flags: String },
    /// Error object.
    Error { name: String, message: String },
    /// Map object.
    Map,
    /// Set object.
    Set,
    /// WeakMap object.
    WeakMap,
    /// WeakSet object.
    WeakSet,
    /// ArrayBuffer object.
    ArrayBuffer(Vec<u8>),
    /// Promise object.
    Promise,
    /// Proxy object.
    Proxy,
    /// Arguments object.
    Arguments,
}

/// A JavaScript object.
#[derive(Clone, Debug)]
pub struct JsObject {
    /// Object kind.
    kind: ObjectKind,
    /// Properties.
    properties: Vec<Property>,
    /// Prototype.
    prototype: Option<Rc<RefCell<JsObject>>>,
    /// Extensible flag.
    extensible: bool,
    /// Call internal method (for functions).
    callable: Option<Callable>,
    /// Construct internal method (for constructors).
    constructable: bool,
    /// Array elements (for array-like objects).
    elements: Vec<Option<Value>>,
}

impl JsObject {
    /// Create a new ordinary object.
    pub fn new() -> Self {
        JsObject {
            kind: ObjectKind::Ordinary,
            properties: Vec::new(),
            prototype: None,
            extensible: true,
            callable: None,
            constructable: false,
            elements: Vec::new(),
        }
    }

    /// Create an array object.
    pub fn array(elements: Vec<Option<Value>>) -> Self {
        let len = elements.len();
        let mut obj = JsObject {
            kind: ObjectKind::Array,
            properties: Vec::new(),
            prototype: None,
            extensible: true,
            callable: None,
            constructable: false,
            elements,
        };

        obj.define_property(
            PropertyKey::string("length"),
            PropertyDescriptor::data(Value::number(len as f64), true, false, false),
        );

        obj
    }

    /// Create a function object.
    pub fn function(callable: Callable) -> Self {
        let name = callable.name();
        let length = callable.length();

        let mut obj = JsObject {
            kind: ObjectKind::Function,
            properties: Vec::new(),
            prototype: None,
            extensible: true,
            callable: Some(callable),
            constructable: true,
            elements: Vec::new(),
        };

        obj.define_property(
            PropertyKey::string("name"),
            PropertyDescriptor::data(Value::string(name), false, false, true),
        );
        obj.define_property(
            PropertyKey::string("length"),
            PropertyDescriptor::data(Value::number(length as f64), false, false, true),
        );

        obj
    }

    /// Create a boolean wrapper object.
    pub fn boolean_object(value: bool) -> Self {
        JsObject {
            kind: ObjectKind::Boolean(value),
            properties: Vec::new(),
            prototype: None,
            extensible: true,
            callable: None,
            constructable: false,
            elements: Vec::new(),
        }
    }

    /// Create a number wrapper object.
    pub fn number_object(value: f64) -> Self {
        JsObject {
            kind: ObjectKind::Number(value),
            properties: Vec::new(),
            prototype: None,
            extensible: true,
            callable: None,
            constructable: false,
            elements: Vec::new(),
        }
    }

    /// Create a string wrapper object.
    pub fn string_object(value: String) -> Self {
        let len = value.len();
        let mut obj = JsObject {
            kind: ObjectKind::String(value),
            properties: Vec::new(),
            prototype: None,
            extensible: true,
            callable: None,
            constructable: false,
            elements: Vec::new(),
        };

        obj.define_property(
            PropertyKey::string("length"),
            PropertyDescriptor::data(Value::number(len as f64), false, false, false),
        );

        obj
    }

    /// Create a symbol wrapper object.
    pub fn symbol_object(value: Symbol) -> Self {
        JsObject {
            kind: ObjectKind::Symbol(value),
            properties: Vec::new(),
            prototype: None,
            extensible: true,
            callable: None,
            constructable: false,
            elements: Vec::new(),
        }
    }

    /// Create a BigInt wrapper object.
    pub fn bigint_object(value: i64) -> Self {
        JsObject {
            kind: ObjectKind::BigInt(value),
            properties: Vec::new(),
            prototype: None,
            extensible: true,
            callable: None,
            constructable: false,
            elements: Vec::new(),
        }
    }

    /// Create an error object.
    pub fn error(name: String, message: String) -> Self {
        let mut obj = JsObject {
            kind: ObjectKind::Error {
                name: name.clone(),
                message: message.clone(),
            },
            properties: Vec::new(),
            prototype: None,
            extensible: true,
            callable: None,
            constructable: false,
            elements: Vec::new(),
        };

        obj.define_property(
            PropertyKey::string("name"),
            PropertyDescriptor::data(Value::string(name), true, false, true),
        );
        obj.define_property(
            PropertyKey::string("message"),
            PropertyDescriptor::data(Value::string(message), true, false, true),
        );

        obj
    }

    /// Get the object kind.
    pub fn kind(&self) -> &ObjectKind {
        &self.kind
    }

    /// Check if object is callable.
    pub fn is_callable(&self) -> bool {
        self.callable.is_some()
    }

    /// Check if object is constructable.
    pub fn is_constructable(&self) -> bool {
        self.constructable
    }

    /// Check if object is an array.
    pub fn is_array(&self) -> bool {
        matches!(self.kind, ObjectKind::Array)
    }

    /// Get the callable.
    pub fn callable(&self) -> Option<&Callable> {
        self.callable.as_ref()
    }

    /// Set the prototype.
    pub fn set_prototype(&mut self, proto: Option<Rc<RefCell<JsObject>>>) {
        self.prototype = proto;
    }

    /// Get the prototype.
    pub fn prototype(&self) -> Option<&Rc<RefCell<JsObject>>> {
        self.prototype.as_ref()
    }

    /// Get a property.
    pub fn get(&self, key: &PropertyKey) -> JsResult<Value> {
        // Check array elements first
        if let PropertyKey::Index(i) = key {
            if let Some(Some(v)) = self.elements.get(*i as usize) {
                return Ok(v.clone());
            }
        }

        // Check own properties
        for prop in &self.properties {
            if &prop.key == key {
                if let Some(ref v) = prop.descriptor.value {
                    return Ok(v.clone());
                }
                // TODO: Call getter
                return Ok(Value::undefined());
            }
        }

        // Check prototype chain
        if let Some(proto) = &self.prototype {
            return proto.borrow().get(key);
        }

        Ok(Value::undefined())
    }

    /// Set a property.
    pub fn set(&mut self, key: PropertyKey, value: Value) -> JsResult<()> {
        // Handle array elements
        if let PropertyKey::Index(i) = key {
            let i = i as usize;
            if self.is_array() {
                // Extend elements if needed
                while self.elements.len() <= i {
                    self.elements.push(None);
                }
                self.elements[i] = Some(value.clone());

                // Update length if needed
                let new_len = i + 1;
                self.define_property(
                    PropertyKey::string("length"),
                    PropertyDescriptor::data(Value::number(new_len as f64), true, false, false),
                );
                return Ok(());
            }
        }

        // Check for existing property
        for prop in &mut self.properties {
            if prop.key == key {
                if prop.descriptor.writable == Some(false) {
                    return Err(JsError::type_error("Cannot assign to read-only property"));
                }
                prop.descriptor.value = Some(value);
                return Ok(());
            }
        }

        // Add new property
        if !self.extensible {
            return Err(JsError::type_error(
                "Cannot add property to non-extensible object",
            ));
        }

        self.properties.push(Property {
            key,
            descriptor: PropertyDescriptor::data(value, true, true, true),
        });

        Ok(())
    }

    /// Define a property.
    pub fn define_property(&mut self, key: PropertyKey, descriptor: PropertyDescriptor) {
        // Check for existing property
        for prop in &mut self.properties {
            if prop.key == key {
                // Update existing
                if let Some(v) = descriptor.value {
                    prop.descriptor.value = Some(v);
                }
                if let Some(w) = descriptor.writable {
                    prop.descriptor.writable = Some(w);
                }
                if let Some(e) = descriptor.enumerable {
                    prop.descriptor.enumerable = Some(e);
                }
                if let Some(c) = descriptor.configurable {
                    prop.descriptor.configurable = Some(c);
                }
                if descriptor.get.is_some() {
                    prop.descriptor.get = descriptor.get;
                }
                if descriptor.set.is_some() {
                    prop.descriptor.set = descriptor.set;
                }
                return;
            }
        }

        // Add new property
        self.properties.push(Property { key, descriptor });
    }

    /// Check if object has own property.
    pub fn has_own_property(&self, key: &PropertyKey) -> bool {
        // Check array elements
        if let PropertyKey::Index(i) = key {
            if let Some(Some(_)) = self.elements.get(*i as usize) {
                return true;
            }
        }

        self.properties.iter().any(|p| &p.key == key)
    }

    /// Check if property exists (including prototype chain).
    pub fn has(&self, key: &PropertyKey) -> bool {
        if self.has_own_property(key) {
            return true;
        }

        if let Some(proto) = &self.prototype {
            return proto.borrow().has(key);
        }

        false
    }

    /// Delete a property.
    pub fn delete(&mut self, key: &PropertyKey) -> bool {
        // Check array elements
        if let PropertyKey::Index(i) = key {
            if (*i as usize) < self.elements.len() {
                self.elements[*i as usize] = None;
                return true;
            }
        }

        if let Some(pos) = self.properties.iter().position(|p| &p.key == key) {
            let prop = &self.properties[pos];
            if prop.descriptor.configurable == Some(false) {
                return false;
            }
            self.properties.remove(pos);
            return true;
        }

        true
    }

    /// Get own property keys.
    pub fn own_keys(&self) -> Vec<PropertyKey> {
        let mut keys = Vec::new();

        // Integer indices first
        for i in 0..self.elements.len() {
            if self.elements[i].is_some() {
                keys.push(PropertyKey::Index(i as u32));
            }
        }

        // String keys
        for prop in &self.properties {
            if !matches!(prop.key, PropertyKey::Index(_)) {
                keys.push(prop.key.clone());
            }
        }

        keys
    }

    /// Get own enumerable property keys.
    pub fn own_enumerable_keys(&self) -> Vec<PropertyKey> {
        let mut keys = Vec::new();

        // Integer indices first
        for i in 0..self.elements.len() {
            if self.elements[i].is_some() {
                keys.push(PropertyKey::Index(i as u32));
            }
        }

        // String keys
        for prop in &self.properties {
            if prop.descriptor.enumerable == Some(true) {
                if !matches!(prop.key, PropertyKey::Index(_)) {
                    keys.push(prop.key.clone());
                }
            }
        }

        keys
    }

    /// Prevent extensions.
    pub fn prevent_extensions(&mut self) {
        self.extensible = false;
    }

    /// Check if extensible.
    pub fn is_extensible(&self) -> bool {
        self.extensible
    }

    /// Get array length.
    pub fn array_length(&self) -> usize {
        if self.is_array() {
            self.elements.len()
        } else {
            0
        }
    }

    /// Push to array.
    pub fn array_push(&mut self, value: Value) {
        if self.is_array() {
            self.elements.push(Some(value));
            let len = self.elements.len();
            self.define_property(
                PropertyKey::string("length"),
                PropertyDescriptor::data(Value::number(len as f64), true, false, false),
            );
        }
    }

    /// Pop from array.
    pub fn array_pop(&mut self) -> Option<Value> {
        if self.is_array() && !self.elements.is_empty() {
            let value = self.elements.pop().flatten();
            let len = self.elements.len();
            self.define_property(
                PropertyKey::string("length"),
                PropertyDescriptor::data(Value::number(len as f64), true, false, false),
            );
            value
        } else {
            None
        }
    }
}

impl Default for JsObject {
    fn default() -> Self {
        Self::new()
    }
}

/// Callable function type.
#[derive(Clone, Debug)]
pub enum Callable {
    /// Native function.
    Native(NativeFunction),
    /// User-defined function.
    UserDefined(UserFunction),
    /// Bound function.
    Bound(BoundFunction),
}

impl Callable {
    /// Get function name.
    pub fn name(&self) -> String {
        match self {
            Callable::Native(f) => f.name.clone(),
            Callable::UserDefined(f) => f.name.clone().unwrap_or_default(),
            Callable::Bound(f) => alloc::format!("bound {}", f.target.name()),
        }
    }

    /// Get function length (parameter count).
    pub fn length(&self) -> usize {
        match self {
            Callable::Native(f) => f.length,
            Callable::UserDefined(f) => f.params.len(),
            Callable::Bound(f) => {
                let target_len = f.target.length();
                target_len.saturating_sub(f.bound_args.len())
            }
        }
    }
}

/// Native function.
#[derive(Clone)]
pub struct NativeFunction {
    /// Function name.
    pub name: String,
    /// Function length.
    pub length: usize,
    /// Function pointer.
    pub func: fn(&Value, &[Value]) -> JsResult<Value>,
}

impl core::fmt::Debug for NativeFunction {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NativeFunction")
            .field("name", &self.name)
            .field("length", &self.length)
            .finish()
    }
}

/// User-defined function.
#[derive(Clone, Debug)]
pub struct UserFunction {
    /// Function name.
    pub name: Option<String>,
    /// Parameters.
    pub params: Vec<String>,
    /// Function body.
    pub body: BlockStmt,
    /// Captured environment.
    pub environment: Rc<RefCell<Environment>>,
    /// Is async.
    pub is_async: bool,
    /// Is generator.
    pub is_generator: bool,
}

/// Bound function.
#[derive(Clone, Debug)]
pub struct BoundFunction {
    /// Target function.
    pub target: Box<Callable>,
    /// Bound this value.
    pub bound_this: Value,
    /// Bound arguments.
    pub bound_args: Vec<Value>,
}

/// Environment record.
#[derive(Clone, Debug)]
pub struct Environment {
    /// Variable bindings.
    bindings: Vec<(String, Binding)>,
    /// Outer environment.
    outer: Option<Rc<RefCell<Environment>>>,
    /// This binding.
    this_binding: Option<Value>,
}

/// Variable binding.
#[derive(Clone, Debug)]
pub struct Binding {
    /// Value.
    pub value: Value,
    /// Is mutable.
    pub mutable: bool,
    /// Is initialized.
    pub initialized: bool,
}

impl Environment {
    /// Create a new global environment.
    pub fn global() -> Self {
        Environment {
            bindings: Vec::new(),
            outer: None,
            this_binding: Some(Value::undefined()),
        }
    }

    /// Create a child environment.
    pub fn child(outer: Rc<RefCell<Environment>>) -> Self {
        Environment {
            bindings: Vec::new(),
            outer: Some(outer),
            this_binding: None,
        }
    }

    /// Create a function environment.
    pub fn function(outer: Rc<RefCell<Environment>>, this_value: Value) -> Self {
        Environment {
            bindings: Vec::new(),
            outer: Some(outer),
            this_binding: Some(this_value),
        }
    }

    /// Declare a variable.
    pub fn declare(&mut self, name: String, mutable: bool) -> JsResult<()> {
        // Check for duplicate in this environment
        for (existing_name, _) in &self.bindings {
            if existing_name == &name {
                return Err(JsError::syntax(alloc::format!(
                    "Identifier '{}' has already been declared",
                    name
                )));
            }
        }

        self.bindings.push((
            name,
            Binding {
                value: Value::undefined(),
                mutable,
                initialized: false,
            },
        ));

        Ok(())
    }

    /// Initialize a variable.
    pub fn initialize(&mut self, name: &str, value: Value) -> JsResult<()> {
        for (n, binding) in &mut self.bindings {
            if n == name {
                binding.value = value;
                binding.initialized = true;
                return Ok(());
            }
        }

        // Var hoisting - create if not exists
        self.bindings.push((
            name.to_string(),
            Binding {
                value,
                mutable: true,
                initialized: true,
            },
        ));

        Ok(())
    }

    /// Get a variable.
    pub fn get(&self, name: &str) -> JsResult<Value> {
        for (n, binding) in &self.bindings {
            if n == name {
                if !binding.initialized {
                    return Err(JsError::reference(alloc::format!(
                        "Cannot access '{}' before initialization",
                        name
                    )));
                }
                return Ok(binding.value.clone());
            }
        }

        if let Some(outer) = &self.outer {
            return outer.borrow().get(name);
        }

        Err(JsError::reference(alloc::format!(
            "{} is not defined",
            name
        )))
    }

    /// Set a variable.
    pub fn set(&mut self, name: &str, value: Value) -> JsResult<()> {
        for (n, binding) in &mut self.bindings {
            if n == name {
                if !binding.mutable {
                    return Err(JsError::type_error(alloc::format!(
                        "Assignment to constant variable '{}'",
                        name
                    )));
                }
                binding.value = value;
                binding.initialized = true;
                return Ok(());
            }
        }

        if let Some(outer) = &self.outer {
            return outer.borrow_mut().set(name, value);
        }

        // Create in global scope (sloppy mode)
        self.bindings.push((
            name.to_string(),
            Binding {
                value,
                mutable: true,
                initialized: true,
            },
        ));

        Ok(())
    }

    /// Get this value.
    pub fn get_this(&self) -> Value {
        if let Some(this) = &self.this_binding {
            return this.clone();
        }

        if let Some(outer) = &self.outer {
            return outer.borrow().get_this();
        }

        Value::undefined()
    }

    /// Set this value.
    pub fn set_this(&mut self, value: Value) {
        self.this_binding = Some(value);
    }

    /// Check if variable exists.
    pub fn has(&self, name: &str) -> bool {
        for (n, _) in &self.bindings {
            if n == name {
                return true;
            }
        }

        if let Some(outer) = &self.outer {
            return outer.borrow().has(name);
        }

        false
    }
}
