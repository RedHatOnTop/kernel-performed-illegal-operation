//! WASM module loading and validation.
//!
//! This module handles parsing and validation of WASM binaries.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::RuntimeError;

/// A compiled WASM module.
#[derive(Clone)]
pub struct Module {
    /// Module name (if provided).
    name: Option<String>,
    
    /// Compiled code.
    code: Vec<u8>,
    
    /// Exported functions.
    exports: Vec<Export>,
    
    /// Imported functions.
    imports: Vec<Import>,
    
    /// Memory requirements.
    memory: Option<MemoryType>,
}

impl Module {
    /// Create a module from WASM bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, RuntimeError> {
        Self::validate(bytes)?;
        
        // Parse the WASM binary
        let module = Self::parse(bytes)?;
        
        Ok(module)
    }
    
    /// Validate WASM bytes without creating a module.
    pub fn validate(bytes: &[u8]) -> Result<(), RuntimeError> {
        // Check WASM magic number
        if bytes.len() < 8 {
            return Err(RuntimeError::InvalidBinary("Binary too small".into()));
        }
        
        if &bytes[0..4] != b"\0asm" {
            return Err(RuntimeError::InvalidBinary("Invalid magic number".into()));
        }
        
        // Check version (should be 1)
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if version != 1 {
            return Err(RuntimeError::InvalidBinary(
                alloc::format!("Unsupported WASM version: {}", version)
            ));
        }
        
        Ok(())
    }
    
    /// Parse a WASM binary.
    fn parse(bytes: &[u8]) -> Result<Self, RuntimeError> {
        // Simplified parsing - real implementation would use wasmparser
        Ok(Module {
            name: None,
            code: bytes.to_vec(),
            exports: Vec::new(),
            imports: Vec::new(),
            memory: None,
        })
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
    
    /// Get memory requirements.
    pub fn memory(&self) -> Option<&MemoryType> {
        self.memory.as_ref()
    }
    
    /// Get the compiled code.
    pub fn code(&self) -> &[u8] {
        &self.code
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
    /// Imported function.
    Function(FunctionType),
    /// Imported table.
    Table(TableType),
    /// Imported memory.
    Memory(MemoryType),
    /// Imported global.
    Global(GlobalType),
}

/// Function type (signature).
#[derive(Debug, Clone)]
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

/// Global type.
#[derive(Debug, Clone)]
pub struct GlobalType {
    /// Value type.
    pub value_type: ValueType,
    /// Is mutable.
    pub mutable: bool,
}
