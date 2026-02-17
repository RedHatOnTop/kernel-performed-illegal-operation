//! Component Linker — resolves imports and instantiates components.
//!
//! The `ComponentLinker` lets callers register named interface instances
//! (collections of typed host functions) and then instantiate a component
//! with those imports resolved.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use super::canonical::CoreValue;
use super::instance::ComponentInstance;
use super::{ComponentError, ComponentType, ComponentValue};

/// Signature of a host function callable through the component model.
pub type ComponentHostFn = fn(&[ComponentValue]) -> Result<Vec<ComponentValue>, ComponentError>;

/// Definition of a single host function export.
#[derive(Clone)]
pub struct HostExport {
    /// Function name.
    pub name: String,
    /// Parameter types.
    pub params: Vec<ComponentType>,
    /// Result types.
    pub results: Vec<ComponentType>,
    /// Implementation.
    pub func: ComponentHostFn,
}

/// An interface instance holding named exports.
#[derive(Clone)]
pub struct InterfaceInstance {
    /// Interface name (e.g., "wasi:io/streams@0.2.0").
    pub name: String,
    /// Exported functions.
    pub exports: Vec<HostExport>,
}

/// Component linker — collects interface definitions and instantiates
/// components with resolved imports.
pub struct ComponentLinker {
    /// Registered interface instances keyed by interface name.
    instances: BTreeMap<String, InterfaceInstance>,
}

impl ComponentLinker {
    /// Create a new empty linker.
    pub fn new() -> Self {
        Self {
            instances: BTreeMap::new(),
        }
    }

    /// Register an interface instance under the given name.
    ///
    /// Returns an error if an instance with the same name is already registered.
    pub fn define_instance(&mut self, instance: InterfaceInstance) -> Result<(), ComponentError> {
        if self.instances.contains_key(&instance.name) {
            return Err(ComponentError::InstantiationError(alloc::format!(
                "interface '{}' already defined",
                instance.name
            )));
        }
        self.instances.insert(instance.name.clone(), instance);
        Ok(())
    }

    /// Check whether a named interface is already defined.
    pub fn has_instance(&self, name: &str) -> bool {
        self.instances.contains_key(name)
    }

    /// Get all registered interface names.
    pub fn interface_names(&self) -> Vec<&str> {
        self.instances.keys().map(|s| s.as_str()).collect()
    }

    /// Resolve a host function by interface name and function name.
    pub fn resolve(
        &self,
        interface: &str,
        func_name: &str,
    ) -> Result<&HostExport, ComponentError> {
        let inst = self
            .instances
            .get(interface)
            .ok_or_else(|| ComponentError::ImportNotFound(String::from(interface)))?;
        inst.exports
            .iter()
            .find(|e| e.name == func_name)
            .ok_or_else(|| {
                ComponentError::ImportNotFound(alloc::format!("{}#{}", interface, func_name))
            })
    }

    /// Instantiate a component.
    ///
    /// In the MVP, this creates a `ComponentInstance` that wraps the
    /// linker's interface definitions and provides typed call semantics.
    ///
    /// `imports_needed` declares which interfaces the component requires.
    /// All must be satisfied by previously defined instances.
    pub fn instantiate(
        &self,
        imports_needed: &[&str],
        exports: Vec<ComponentExport>,
    ) -> Result<ComponentInstance, ComponentError> {
        // Validate all imports are satisfied.
        for &import in imports_needed {
            if !self.instances.contains_key(import) {
                return Err(ComponentError::ImportNotFound(String::from(import)));
            }
        }

        // Collect resolved host functions.
        let mut resolved = BTreeMap::new();
        for &import in imports_needed {
            let inst = &self.instances[import];
            for export in &inst.exports {
                let key = alloc::format!("{}#{}", import, export.name);
                resolved.insert(key, export.clone());
            }
        }

        Ok(ComponentInstance::new(resolved, exports))
    }
}

impl Default for ComponentLinker {
    fn default() -> Self {
        Self::new()
    }
}

/// Describes an exported function from a component.
#[derive(Debug, Clone)]
pub struct ComponentExport {
    /// Export name.
    pub name: String,
    /// Parameter types.
    pub params: Vec<ComponentType>,
    /// Result types.
    pub results: Vec<ComponentType>,
    /// Implementation: given lowered args, returns lowered results.
    /// For host-defined components (testing), this is a direct fn pointer.
    pub func: Option<ComponentHostFn>,
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;

    fn dummy_fn(_args: &[ComponentValue]) -> Result<Vec<ComponentValue>, ComponentError> {
        Ok(alloc::vec![ComponentValue::U32(42)])
    }

    fn add_fn(args: &[ComponentValue]) -> Result<Vec<ComponentValue>, ComponentError> {
        let a = match &args[0] {
            ComponentValue::S32(v) => *v,
            _ => return Err(ComponentError::TypeMismatch(String::from("expected s32"))),
        };
        let b = match &args[1] {
            ComponentValue::S32(v) => *v,
            _ => return Err(ComponentError::TypeMismatch(String::from("expected s32"))),
        };
        Ok(alloc::vec![ComponentValue::S32(a + b)])
    }

    fn make_test_interface() -> InterfaceInstance {
        InterfaceInstance {
            name: String::from("test:math/ops"),
            exports: alloc::vec![
                HostExport {
                    name: String::from("get-answer"),
                    params: alloc::vec![],
                    results: alloc::vec![ComponentType::U32],
                    func: dummy_fn,
                },
                HostExport {
                    name: String::from("add"),
                    params: alloc::vec![ComponentType::S32, ComponentType::S32],
                    results: alloc::vec![ComponentType::S32],
                    func: add_fn,
                },
            ],
        }
    }

    #[test]
    fn test_linker_define_instance() {
        let mut linker = ComponentLinker::new();
        let iface = make_test_interface();
        linker.define_instance(iface).unwrap();
        assert!(linker.has_instance("test:math/ops"));
        assert!(!linker.has_instance("test:other/ops"));
    }

    #[test]
    fn test_linker_duplicate_instance_error() {
        let mut linker = ComponentLinker::new();
        let iface = make_test_interface();
        linker.define_instance(iface.clone()).unwrap();
        let result = linker.define_instance(iface);
        assert!(result.is_err());
    }

    #[test]
    fn test_linker_resolve_success() {
        let mut linker = ComponentLinker::new();
        linker.define_instance(make_test_interface()).unwrap();
        let export = linker.resolve("test:math/ops", "get-answer").unwrap();
        assert_eq!(export.name, "get-answer");
    }

    #[test]
    fn test_linker_resolve_missing_interface() {
        let linker = ComponentLinker::new();
        let result = linker.resolve("not:here/missing", "foo");
        assert!(matches!(result, Err(ComponentError::ImportNotFound(_))));
    }

    #[test]
    fn test_linker_resolve_missing_function() {
        let mut linker = ComponentLinker::new();
        linker.define_instance(make_test_interface()).unwrap();
        let result = linker.resolve("test:math/ops", "nonexistent");
        assert!(matches!(result, Err(ComponentError::ImportNotFound(_))));
    }

    #[test]
    fn test_linker_interface_names() {
        let mut linker = ComponentLinker::new();
        linker.define_instance(make_test_interface()).unwrap();
        let names = linker.interface_names();
        assert_eq!(names, alloc::vec!["test:math/ops"]);
    }

    #[test]
    fn test_linker_instantiate_success() {
        let mut linker = ComponentLinker::new();
        linker.define_instance(make_test_interface()).unwrap();
        let exports = alloc::vec![ComponentExport {
            name: String::from("run"),
            params: alloc::vec![],
            results: alloc::vec![ComponentType::U32],
            func: Some(dummy_fn),
        }];
        let instance = linker.instantiate(&["test:math/ops"], exports);
        assert!(instance.is_ok());
    }

    #[test]
    fn test_linker_instantiate_missing_import() {
        let linker = ComponentLinker::new();
        let result = linker.instantiate(&["missing:iface/here"], alloc::vec![]);
        assert!(matches!(result, Err(ComponentError::ImportNotFound(_))));
    }
}
