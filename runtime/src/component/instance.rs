//! Component Instance — typed call interface for instantiated components.
//!
//! A `ComponentInstance` wraps resolved imports and component exports,
//! providing `call()` that automatically lowers arguments and lifts results
//! through the canonical ABI.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use super::canonical::{self, CoreValue};
use super::linker::{ComponentExport, HostExport};
use super::{ComponentError, ComponentType, ComponentValue};

/// An instantiated component with typed call semantics.
pub struct ComponentInstance {
    /// Resolved host imports: "interface#func" → HostExport
    imports: BTreeMap<String, HostExport>,
    /// Component's own exports.
    exports: Vec<ComponentExport>,
}

impl ComponentInstance {
    /// Create a new component instance with resolved imports and exports.
    pub(crate) fn new(
        imports: BTreeMap<String, HostExport>,
        exports: Vec<ComponentExport>,
    ) -> Self {
        Self { imports, exports }
    }

    /// Call a component export by name with high-level `ComponentValue` args.
    ///
    /// The implementation:
    /// 1. Finds the named export
    /// 2. Validates argument types
    /// 3. Calls the export function directly (MVP: host-defined components)
    /// 4. Returns typed results
    pub fn call(
        &self,
        name: &str,
        args: &[ComponentValue],
    ) -> Result<Vec<ComponentValue>, ComponentError> {
        let export = self
            .exports
            .iter()
            .find(|e| e.name == name)
            .ok_or_else(|| ComponentError::ExportNotFound(String::from(name)))?;

        // Validate argument count
        if args.len() != export.params.len() {
            return Err(ComponentError::TypeMismatch(alloc::format!(
                "expected {} args, got {}",
                export.params.len(),
                args.len()
            )));
        }

        // Validate argument types
        for (i, (arg, expected_ty)) in args.iter().zip(export.params.iter()).enumerate() {
            if !type_matches(arg, expected_ty) {
                return Err(ComponentError::TypeMismatch(alloc::format!(
                    "argument {} type mismatch",
                    i
                )));
            }
        }

        // Execute
        match export.func {
            Some(func) => func(args),
            None => Err(ComponentError::Trap(String::from(
                "export has no implementation",
            ))),
        }
    }

    /// Call a resolved import function (for use by component internals).
    pub fn call_import(
        &self,
        interface: &str,
        func_name: &str,
        args: &[ComponentValue],
    ) -> Result<Vec<ComponentValue>, ComponentError> {
        let key = alloc::format!("{}#{}", interface, func_name);
        let host_export = self
            .imports
            .get(&key)
            .ok_or_else(|| ComponentError::ImportNotFound(key.clone()))?;

        // Validate args
        if args.len() != host_export.params.len() {
            return Err(ComponentError::TypeMismatch(alloc::format!(
                "import {} expected {} args, got {}",
                key,
                host_export.params.len(),
                args.len()
            )));
        }

        (host_export.func)(args)
    }

    /// Get the list of export names.
    pub fn export_names(&self) -> Vec<&str> {
        self.exports.iter().map(|e| e.name.as_str()).collect()
    }

    /// Get the list of import keys ("interface#func").
    pub fn import_keys(&self) -> Vec<&str> {
        self.imports.keys().map(|k| k.as_str()).collect()
    }

    /// Check if an export exists.
    pub fn has_export(&self, name: &str) -> bool {
        self.exports.iter().any(|e| e.name == name)
    }
}

/// Check if a `ComponentValue` is compatible with a `ComponentType`.
fn type_matches(value: &ComponentValue, ty: &ComponentType) -> bool {
    match (value, ty) {
        (ComponentValue::Bool(_), ComponentType::Bool) => true,
        (ComponentValue::U8(_), ComponentType::U8) => true,
        (ComponentValue::U16(_), ComponentType::U16) => true,
        (ComponentValue::U32(_), ComponentType::U32) => true,
        (ComponentValue::U64(_), ComponentType::U64) => true,
        (ComponentValue::S8(_), ComponentType::S8) => true,
        (ComponentValue::S16(_), ComponentType::S16) => true,
        (ComponentValue::S32(_), ComponentType::S32) => true,
        (ComponentValue::S64(_), ComponentType::S64) => true,
        (ComponentValue::F32(_), ComponentType::F32) => true,
        (ComponentValue::F64(_), ComponentType::F64) => true,
        (ComponentValue::Char(_), ComponentType::Char) => true,
        (ComponentValue::String(_), ComponentType::String) => true,
        (ComponentValue::List(_), ComponentType::List(_)) => true,
        (ComponentValue::Record(_), ComponentType::Record(_)) => true,
        (ComponentValue::Variant { .. }, ComponentType::Variant(_)) => true,
        (ComponentValue::Enum { .. }, ComponentType::Enum(_)) => true,
        (ComponentValue::Flags(_), ComponentType::Flags(_)) => true,
        (ComponentValue::Option(_), ComponentType::Option(_)) => true,
        (ComponentValue::Result(_), ComponentType::Result { .. }) => true,
        _ => false,
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;

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

    fn greet_fn(args: &[ComponentValue]) -> Result<Vec<ComponentValue>, ComponentError> {
        let name = match &args[0] {
            ComponentValue::String(s) => s.clone(),
            _ => return Err(ComponentError::TypeMismatch(String::from("expected string"))),
        };
        Ok(alloc::vec![ComponentValue::String(alloc::format!(
            "Hello, {}!",
            name
        ))])
    }

    fn make_instance() -> ComponentInstance {
        let mut imports = BTreeMap::new();
        imports.insert(
            String::from("test:math/ops#add"),
            HostExport {
                name: String::from("add"),
                params: alloc::vec![ComponentType::S32, ComponentType::S32],
                results: alloc::vec![ComponentType::S32],
                func: add_fn,
            },
        );

        let exports = alloc::vec![
            ComponentExport {
                name: String::from("compute"),
                params: alloc::vec![ComponentType::S32, ComponentType::S32],
                results: alloc::vec![ComponentType::S32],
                func: Some(add_fn),
            },
            ComponentExport {
                name: String::from("greet"),
                params: alloc::vec![ComponentType::String],
                results: alloc::vec![ComponentType::String],
                func: Some(greet_fn),
            },
        ];

        ComponentInstance::new(imports, exports)
    }

    #[test]
    fn test_instance_call_export() {
        let inst = make_instance();
        let result = inst
            .call(
                "compute",
                &[ComponentValue::S32(10), ComponentValue::S32(20)],
            )
            .unwrap();
        assert_eq!(result, alloc::vec![ComponentValue::S32(30)]);
    }

    #[test]
    fn test_instance_call_greet() {
        let inst = make_instance();
        let result = inst
            .call("greet", &[ComponentValue::String(String::from("World"))])
            .unwrap();
        assert_eq!(
            result,
            alloc::vec![ComponentValue::String(String::from("Hello, World!"))]
        );
    }

    #[test]
    fn test_instance_call_import() {
        let inst = make_instance();
        let result = inst
            .call_import(
                "test:math/ops",
                "add",
                &[ComponentValue::S32(3), ComponentValue::S32(4)],
            )
            .unwrap();
        assert_eq!(result, alloc::vec![ComponentValue::S32(7)]);
    }

    #[test]
    fn test_instance_export_not_found() {
        let inst = make_instance();
        let result = inst.call("nonexistent", &[]);
        assert!(matches!(result, Err(ComponentError::ExportNotFound(_))));
    }

    #[test]
    fn test_instance_arg_count_mismatch() {
        let inst = make_instance();
        let result = inst.call("compute", &[ComponentValue::S32(10)]);
        assert!(matches!(result, Err(ComponentError::TypeMismatch(_))));
    }

    #[test]
    fn test_instance_arg_type_mismatch() {
        let inst = make_instance();
        let result = inst.call(
            "compute",
            &[ComponentValue::Bool(true), ComponentValue::S32(10)],
        );
        assert!(matches!(result, Err(ComponentError::TypeMismatch(_))));
    }

    #[test]
    fn test_instance_export_names() {
        let inst = make_instance();
        let names = inst.export_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"compute"));
        assert!(names.contains(&"greet"));
    }

    #[test]
    fn test_instance_has_export() {
        let inst = make_instance();
        assert!(inst.has_export("compute"));
        assert!(inst.has_export("greet"));
        assert!(!inst.has_export("missing"));
    }

    #[test]
    fn test_instance_import_not_found() {
        let inst = make_instance();
        let result = inst.call_import("missing:iface/here", "func", &[]);
        assert!(matches!(result, Err(ComponentError::ImportNotFound(_))));
    }

    #[test]
    fn test_type_matches_all_scalars() {
        assert!(type_matches(&ComponentValue::Bool(true), &ComponentType::Bool));
        assert!(type_matches(&ComponentValue::U8(0), &ComponentType::U8));
        assert!(type_matches(&ComponentValue::U16(0), &ComponentType::U16));
        assert!(type_matches(&ComponentValue::U32(0), &ComponentType::U32));
        assert!(type_matches(&ComponentValue::U64(0), &ComponentType::U64));
        assert!(type_matches(&ComponentValue::S8(0), &ComponentType::S8));
        assert!(type_matches(&ComponentValue::S16(0), &ComponentType::S16));
        assert!(type_matches(&ComponentValue::S32(0), &ComponentType::S32));
        assert!(type_matches(&ComponentValue::S64(0), &ComponentType::S64));
        assert!(type_matches(&ComponentValue::F32(0.0), &ComponentType::F32));
        assert!(type_matches(&ComponentValue::F64(0.0), &ComponentType::F64));
        assert!(type_matches(&ComponentValue::Char('a'), &ComponentType::Char));
    }

    #[test]
    fn test_type_mismatch() {
        assert!(!type_matches(&ComponentValue::Bool(true), &ComponentType::U32));
        assert!(!type_matches(&ComponentValue::S32(0), &ComponentType::F32));
        assert!(!type_matches(
            &ComponentValue::String(String::from("x")),
            &ComponentType::U32
        ));
    }

    #[test]
    fn test_instance_no_impl_export() {
        let exports = alloc::vec![ComponentExport {
            name: String::from("stub"),
            params: alloc::vec![],
            results: alloc::vec![],
            func: None,
        }];
        let inst = ComponentInstance::new(BTreeMap::new(), exports);
        let result = inst.call("stub", &[]);
        assert!(matches!(result, Err(ComponentError::Trap(_))));
    }

    // ── Integration: linker → instance → call ───────────────────────

    #[test]
    fn test_full_linker_to_instance_call() {
        use super::super::linker::{ComponentLinker, InterfaceInstance};

        let mut linker = ComponentLinker::new();
        linker
            .define_instance(InterfaceInstance {
                name: String::from("test:math/ops"),
                exports: alloc::vec![HostExport {
                    name: String::from("add"),
                    params: alloc::vec![ComponentType::S32, ComponentType::S32],
                    results: alloc::vec![ComponentType::S32],
                    func: add_fn,
                }],
            })
            .unwrap();

        let exports = alloc::vec![ComponentExport {
            name: String::from("run"),
            params: alloc::vec![ComponentType::S32, ComponentType::S32],
            results: alloc::vec![ComponentType::S32],
            func: Some(add_fn),
        }];

        let instance = linker.instantiate(&["test:math/ops"], exports).unwrap();

        // Call export
        let result = instance
            .call("run", &[ComponentValue::S32(5), ComponentValue::S32(7)])
            .unwrap();
        assert_eq!(result, alloc::vec![ComponentValue::S32(12)]);

        // Call import
        let result = instance
            .call_import(
                "test:math/ops",
                "add",
                &[ComponentValue::S32(100), ComponentValue::S32(200)],
            )
            .unwrap();
        assert_eq!(result, alloc::vec![ComponentValue::S32(300)]);
    }
}
