//! WIT type definitions.
//!
//! Represents the abstract syntax of a parsed WIT document.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// Primitive types
// ---------------------------------------------------------------------------

/// Primitive types that WIT exposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitPrimitive {
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
    Bool,
    Char,
    StringType,
}

// ---------------------------------------------------------------------------
// Type references
// ---------------------------------------------------------------------------

/// A reference to a WIT type – either a primitive or a user-defined name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitTypeRef {
    Primitive(WitPrimitive),
    Named(String),
    List(Box<WitTypeRef>),
    Option(Box<WitTypeRef>),
    Result {
        ok: Option<Box<WitTypeRef>>,
        err: Option<Box<WitTypeRef>>,
    },
    Tuple(Vec<WitTypeRef>),
}

// ---------------------------------------------------------------------------
// Composite types
// ---------------------------------------------------------------------------

/// A named field in a record or function parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitField {
    pub name: String,
    pub ty: WitTypeRef,
}

/// Record type (struct).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitRecord {
    pub name: String,
    pub fields: Vec<WitField>,
}

/// Enum type (C-style named variants without payloads).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitEnum {
    pub name: String,
    pub cases: Vec<String>,
}

/// Flags type (bitfield).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitFlags {
    pub name: String,
    pub flags: Vec<String>,
}

/// Variant case with optional payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitVariantCase {
    pub name: String,
    pub ty: Option<WitTypeRef>,
}

/// Variant type (tagged union).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitVariant {
    pub name: String,
    pub cases: Vec<WitVariantCase>,
}

/// Resource type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitResource {
    pub name: String,
    pub methods: Vec<WitFunction>,
}

/// Type alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitTypeAlias {
    pub name: String,
    pub target: WitTypeRef,
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

/// A function parameter or result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitParam {
    pub name: String,
    pub ty: WitTypeRef,
}

/// Function definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitFunction {
    pub name: String,
    pub params: Vec<WitParam>,
    pub results: Vec<WitParam>,
}

// ---------------------------------------------------------------------------
// Interface / World
// ---------------------------------------------------------------------------

/// Top-level item in an interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitItem {
    Record(WitRecord),
    Enum(WitEnum),
    Flags(WitFlags),
    Variant(WitVariant),
    Resource(WitResource),
    TypeAlias(WitTypeAlias),
    Function(WitFunction),
    Use(WitUse),
}

/// `use` statement that imports types from another interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitUse {
    pub path: String,
    pub names: Vec<String>,
}

/// A WIT interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitInterface {
    pub name: String,
    pub items: Vec<WitItem>,
}

/// A world export/import reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldRef {
    InterfaceName(String),
    InlineInterface(Vec<WitItem>),
}

/// A WIT world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitWorld {
    pub name: String,
    pub imports: Vec<(String, WorldRef)>,
    pub exports: Vec<(String, WorldRef)>,
}

/// A package (optional) header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitPackage {
    pub namespace: String,
    pub name: String,
    pub version: Option<String>,
}

/// Parsed WIT document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitDocument {
    pub package: Option<WitPackage>,
    pub interfaces: Vec<WitInterface>,
    pub worlds: Vec<WitWorld>,
}

impl WitDocument {
    pub fn new() -> Self {
        WitDocument {
            package: None,
            interfaces: Vec::new(),
            worlds: Vec::new(),
        }
    }

    /// Find an interface by name.
    pub fn find_interface(&self, name: &str) -> Option<&WitInterface> {
        self.interfaces.iter().find(|i| i.name == name)
    }

    /// Find a world by name.
    pub fn find_world(&self, name: &str) -> Option<&WitWorld> {
        self.worlds.iter().find(|w| w.name == name)
    }
}
