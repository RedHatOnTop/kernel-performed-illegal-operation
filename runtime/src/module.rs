//! WASM module loading and validation.
//!
//! This module handles parsing and validation of WASM binaries.
//! All WASM sections are parsed into structured data that can be
//! consumed by the interpreter engine and JIT compiler.

use alloc::string::String;
use alloc::vec::Vec;

use crate::opcodes::Instruction;
use crate::parser::{ModuleValidator, ParseError, WasmParser};
use crate::RuntimeError;

/// A parsed WASM module containing all sections.
#[derive(Debug, Clone)]
pub struct Module {
    /// Type section: function signatures.
    pub types: Vec<FunctionType>,
    /// Import section: external imports.
    pub imports: Vec<Import>,
    /// Function section: type index per function body.
    pub functions: Vec<u32>,
    /// Table section: table definitions.
    pub tables: Vec<TableType>,
    /// Memory section: memory definitions.
    pub memories: Vec<MemoryType>,
    /// Global section: global variable definitions.
    pub globals: Vec<Global>,
    /// Export section: exported items.
    pub exports: Vec<Export>,
    /// Start section: optional start function index.
    pub start: Option<u32>,
    /// Element section: table initialization segments.
    pub elements: Vec<Element>,
    /// Code section: function bodies.
    pub code: Vec<FunctionBody>,
    /// Data section: memory initialization segments.
    pub data: Vec<DataSegment>,
    /// Module name (from custom "name" section).
    pub name: Option<String>,
    /// Data count section (for validation).
    pub data_count: Option<u32>,
}

impl Module {
    /// Create an empty module (useful for testing).
    pub fn empty() -> Self {
        Self {
            types: Vec::new(),
            imports: Vec::new(),
            functions: Vec::new(),
            tables: Vec::new(),
            memories: Vec::new(),
            globals: Vec::new(),
            exports: Vec::new(),
            start: None,
            elements: Vec::new(),
            code: Vec::new(),
            data: Vec::new(),
            name: None,
            data_count: None,
        }
    }

    /// Create a module from WASM bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, RuntimeError> {
        let module = WasmParser::parse(bytes)
            .map_err(|e| RuntimeError::InvalidBinary(alloc::format!("{}", e)))?;

        // Validate the parsed module
        module.validate_structure()?;

        Ok(module)
    }

    /// Validate WASM bytes without creating a module.
    pub fn validate(bytes: &[u8]) -> Result<(), RuntimeError> {
        // Check minimum size
        if bytes.len() < 8 {
            return Err(RuntimeError::InvalidBinary("Binary too small".into()));
        }

        // Check magic number
        if &bytes[0..4] != b"\0asm" {
            return Err(RuntimeError::InvalidBinary("Invalid magic number".into()));
        }

        // Check version
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if version != 1 {
            return Err(RuntimeError::InvalidBinary(alloc::format!(
                "Unsupported WASM version: {}",
                version
            )));
        }

        // Full parse + validate
        let module = WasmParser::parse(bytes)
            .map_err(|e| RuntimeError::InvalidBinary(alloc::format!("{}", e)))?;
        module.validate_structure()?;

        Ok(())
    }

    /// Validate structural correctness of the parsed module.
    pub fn validate_structure(&self) -> Result<(), RuntimeError> {
        ModuleValidator::validate(self)
            .map_err(|e| RuntimeError::InvalidBinary(alloc::format!("Validation: {}", e)))
    }

    /// Get the module name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Get exported functions.
    pub fn exports(&self) -> &[Export] {
        &self.exports
    }

    /// Get imported functions.
    pub fn imports(&self) -> &[Import] {
        &self.imports
    }

    /// Get the primary memory definition (if any).
    pub fn memory(&self) -> Option<&MemoryType> {
        // Check imports first for an imported memory
        for import in &self.imports {
            if let ImportKind::Memory(ref mem) = import.kind {
                return Some(mem);
            }
        }
        self.memories.first()
    }

    /// Find an export by name.
    pub fn find_export(&self, name: &str) -> Option<&Export> {
        self.exports.iter().find(|e| e.name == name)
    }

    /// Get the function type for a function index (including imports).
    pub fn function_type(&self, func_idx: u32) -> Option<&FunctionType> {
        let import_func_count = self.import_function_count();
        if (func_idx as usize) < import_func_count {
            // It's an imported function
            let mut idx = 0;
            for import in &self.imports {
                if let ImportKind::Function(type_idx) = import.kind {
                    if idx == func_idx as usize {
                        return self.types.get(type_idx as usize);
                    }
                    idx += 1;
                }
            }
            None
        } else {
            // It's a local function
            let local_idx = func_idx as usize - import_func_count;
            let type_idx = self.functions.get(local_idx)?;
            self.types.get(*type_idx as usize)
        }
    }

    /// Count the number of imported functions.
    pub fn import_function_count(&self) -> usize {
        self.imports
            .iter()
            .filter(|i| matches!(i.kind, ImportKind::Function(_)))
            .count()
    }

    /// Count total functions (imports + local).
    pub fn total_function_count(&self) -> usize {
        self.import_function_count() + self.functions.len()
    }
}

/// An exported function or value.
#[derive(Debug, Clone)]
pub struct Export {
    /// Export name.
    pub name: String,
    /// Export kind.
    pub kind: ExportKind,
    /// Index in the respective section.
    pub index: u32,
}

/// Export kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    /// Exported function.
    Function,
    /// Exported table.
    Table,
    /// Exported memory.
    Memory,
    /// Exported global.
    Global,
}

/// An imported function or value.
#[derive(Debug, Clone)]
pub struct Import {
    /// Module name.
    pub module: String,
    /// Import name.
    pub name: String,
    /// Import kind.
    pub kind: ImportKind,
}

/// Import kinds.
#[derive(Debug, Clone)]
pub enum ImportKind {
    /// Imported function (type index).
    Function(u32),
    /// Imported table.
    Table(TableType),
    /// Imported memory.
    Memory(MemoryType),
    /// Imported global.
    Global(GlobalType),
}

/// Function type (signature).
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionType {
    /// Parameter types.
    pub params: Vec<ValueType>,
    /// Return types.
    pub results: Vec<ValueType>,
}

/// Value types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    I32,
    I64,
    F32,
    F64,
    V128,
    FuncRef,
    ExternRef,
}

/// Table type.
#[derive(Debug, Clone)]
pub struct TableType {
    /// Element type.
    pub element_type: ValueType,
    /// Minimum size.
    pub min: u32,
    /// Maximum size (if specified).
    pub max: Option<u32>,
}

/// Memory type.
#[derive(Debug, Clone)]
pub struct MemoryType {
    /// Minimum pages (64KB each).
    pub min: u32,
    /// Maximum pages (if specified).
    pub max: Option<u32>,
    /// Is shared memory (for threads).
    pub shared: bool,
}

/// Global variable definition (type + init expression).
#[derive(Debug, Clone)]
pub struct Global {
    /// Global type.
    pub global_type: GlobalType,
    /// Initialization expression.
    pub init_expr: Vec<Instruction>,
}

/// Global type.
#[derive(Debug, Clone)]
pub struct GlobalType {
    /// Value type.
    pub value_type: ValueType,
    /// Is mutable.
    pub mutable: bool,
}

/// Element segment for table initialization.
#[derive(Debug, Clone)]
pub struct Element {
    /// Table index (0 for MVP).
    pub table_idx: u32,
    /// Offset expression (empty if passive).
    pub offset_expr: Vec<Instruction>,
    /// Function indices.
    pub func_indices: Vec<u32>,
    /// Whether this is a passive segment.
    pub passive: bool,
}

/// Function body (locals + instructions).
#[derive(Debug, Clone)]
pub struct FunctionBody {
    /// Local variable declarations: (count, type).
    pub locals: Vec<(u32, ValueType)>,
    /// Decoded instructions.
    pub instructions: Vec<Instruction>,
    /// Raw byte representation (for JIT).
    pub raw_bytes: Vec<u8>,
}

impl FunctionBody {
    /// Get the total number of local variables.
    pub fn local_count(&self) -> u32 {
        self.locals.iter().map(|(count, _)| count).sum()
    }

    /// Get the type of a local variable by index.
    pub fn local_type(&self, idx: u32) -> Option<ValueType> {
        let mut offset = 0u32;
        for &(count, vtype) in &self.locals {
            if idx < offset + count {
                return Some(vtype);
            }
            offset += count;
        }
        None
    }
}

/// Data segment for memory initialization.
#[derive(Debug, Clone)]
pub struct DataSegment {
    /// Memory index (0 for MVP).
    pub memory_idx: u32,
    /// Offset expression (empty if passive).
    pub offset_expr: Vec<Instruction>,
    /// Segment data bytes.
    pub data: Vec<u8>,
    /// Whether this is a passive segment.
    pub passive: bool,
}
