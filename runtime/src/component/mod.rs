//! Component Model Core
//!
//! Implements the WebAssembly Component Model canonical ABI, component linker,
//! and component instance management. This bridges WIT interface types to
//! core WASM module types.
//!
//! # Architecture
//!
//! - `canonical.rs` — Canonical ABI lowering (host→WASM) and lifting (WASM→host)
//! - `linker.rs` — Component linker for import resolution and instantiation
//! - `instance.rs` — Component instance with typed call interface

pub mod canonical;
pub mod instance;
pub mod linker;
pub mod wasi_bridge;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// Component value — high-level typed value for component interfaces.
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentValue {
    /// Boolean value.
    Bool(bool),
    /// Unsigned 8-bit integer.
    U8(u8),
    /// Unsigned 16-bit integer.
    U16(u16),
    /// Unsigned 32-bit integer.
    U32(u32),
    /// Unsigned 64-bit integer.
    U64(u64),
    /// Signed 8-bit integer.
    S8(i8),
    /// Signed 16-bit integer.
    S16(i16),
    /// Signed 32-bit integer.
    S32(i32),
    /// Signed 64-bit integer.
    S64(i64),
    /// 32-bit float.
    F32(f32),
    /// 64-bit float.
    F64(f64),
    /// Unicode character.
    Char(char),
    /// UTF-8 string.
    String(String),
    /// List of values.
    List(Vec<ComponentValue>),
    /// Record (named fields).
    Record(Vec<(String, ComponentValue)>),
    /// Variant (discriminant + optional payload).
    Variant {
        discriminant: u32,
        name: String,
        value: Option<Box<ComponentValue>>,
    },
    /// Enum (discriminant only, no payload).
    Enum {
        discriminant: u32,
        name: String,
    },
    /// Flags (bitmask).
    Flags(u32),
    /// Option (None or Some).
    Option(Option<Box<ComponentValue>>),
    /// Result (Ok or Err).
    Result(core::result::Result<Option<Box<ComponentValue>>, Option<Box<ComponentValue>>>),
}

/// Component type descriptor — describes the type of a component value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentType {
    Bool,
    U8,
    U16,
    U32,
    U64,
    S8,
    S16,
    S32,
    S64,
    F32,
    F64,
    Char,
    String,
    List(Box<ComponentType>),
    Record(Vec<(String, ComponentType)>),
    Variant(Vec<(String, Option<ComponentType>)>),
    Enum(Vec<String>),
    Flags(Vec<String>),
    Option(Box<ComponentType>),
    Result {
        ok: Option<Box<ComponentType>>,
        err: Option<Box<ComponentType>>,
    },
}

/// Error type for component operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentError {
    /// Type mismatch during lowering or lifting.
    TypeMismatch(String),
    /// Out-of-bounds memory access during string/list operations.
    MemoryOutOfBounds,
    /// Invalid discriminant value.
    InvalidDiscriminant(u32),
    /// Import not found during linking.
    ImportNotFound(String),
    /// Export not found in component.
    ExportNotFound(String),
    /// Invalid component binary.
    InvalidComponent(String),
    /// Instantiation error.
    InstantiationError(String),
    /// Execution error (trap).
    Trap(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_value_bool() {
        let v = ComponentValue::Bool(true);
        assert_eq!(v, ComponentValue::Bool(true));
    }

    #[test]
    fn test_component_value_string() {
        let v = ComponentValue::String(String::from("hello"));
        if let ComponentValue::String(s) = &v {
            assert_eq!(s, "hello");
        } else {
            panic!("expected String");
        }
    }

    #[test]
    fn test_component_value_list() {
        let v = ComponentValue::List(alloc::vec![
            ComponentValue::U32(1),
            ComponentValue::U32(2),
            ComponentValue::U32(3),
        ]);
        if let ComponentValue::List(items) = &v {
            assert_eq!(items.len(), 3);
        } else {
            panic!("expected List");
        }
    }

    #[test]
    fn test_component_value_record() {
        let v = ComponentValue::Record(alloc::vec![
            (String::from("x"), ComponentValue::S32(10)),
            (String::from("y"), ComponentValue::S32(20)),
        ]);
        if let ComponentValue::Record(fields) = &v {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].0, "x");
        } else {
            panic!("expected Record");
        }
    }

    #[test]
    fn test_component_value_option() {
        let some = ComponentValue::Option(Some(Box::new(ComponentValue::U32(42))));
        let none = ComponentValue::Option(None);
        assert_ne!(some, none);
    }

    #[test]
    fn test_component_value_result() {
        let ok_val = ComponentValue::Result(Ok(Some(Box::new(ComponentValue::U32(42)))));
        let err_val = ComponentValue::Result(Err(Some(Box::new(ComponentValue::String(String::from("error"))))));
        assert_ne!(ok_val, err_val);
    }

    #[test]
    fn test_component_value_variant() {
        let v = ComponentValue::Variant {
            discriminant: 0,
            name: String::from("some-case"),
            value: Some(Box::new(ComponentValue::Bool(true))),
        };
        if let ComponentValue::Variant { discriminant, name, value } = &v {
            assert_eq!(*discriminant, 0);
            assert_eq!(name, "some-case");
            assert!(value.is_some());
        }
    }

    #[test]
    fn test_component_value_enum() {
        let v = ComponentValue::Enum {
            discriminant: 2,
            name: String::from("red"),
        };
        if let ComponentValue::Enum { discriminant, name } = &v {
            assert_eq!(*discriminant, 2);
            assert_eq!(name, "red");
        }
    }

    #[test]
    fn test_component_value_flags() {
        let v = ComponentValue::Flags(0b1010);
        assert_eq!(v, ComponentValue::Flags(10));
    }

    #[test]
    fn test_component_error_display() {
        let e = ComponentError::ImportNotFound(String::from("wasi:io/streams"));
        assert_eq!(
            e,
            ComponentError::ImportNotFound(String::from("wasi:io/streams"))
        );
    }
}
