//! WASM Runtime for KPIO Kernel
//!
//! This module provides a WebAssembly interpreter using wasmi,
//! allowing execution of WASM modules directly in the kernel.

mod engine;
mod host;

pub use engine::{WasmEngine, WasmInstance, WasmModule};
pub use host::HostFunctions;

use alloc::string::String;

/// WASM runtime error types.
#[derive(Debug)]
pub enum WasmError {
    /// Failed to parse WASM module.
    ParseError(String),
    /// Failed to compile module.
    CompilationError(String),
    /// Failed to instantiate module.
    InstantiationError(String),
    /// Failed to execute function.
    ExecutionError(String),
    /// Function not found.
    FunctionNotFound(String),
    /// Memory access error.
    MemoryError(String),
    /// Resource limit exceeded.
    ResourceLimit(String),
}

impl core::fmt::Display for WasmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            WasmError::ParseError(s) => write!(f, "Parse error: {}", s),
            WasmError::CompilationError(s) => write!(f, "Compilation error: {}", s),
            WasmError::InstantiationError(s) => write!(f, "Instantiation error: {}", s),
            WasmError::ExecutionError(s) => write!(f, "Execution error: {}", s),
            WasmError::FunctionNotFound(s) => write!(f, "Function not found: {}", s),
            WasmError::MemoryError(s) => write!(f, "Memory error: {}", s),
            WasmError::ResourceLimit(s) => write!(f, "Resource limit: {}", s),
        }
    }
}

/// Initialize the WASM runtime.
pub fn init() {
    crate::serial_println!("[WASM] Initializing WASM runtime (wasmi interpreter)...");
    engine::init();
    crate::serial_println!("[WASM] Runtime initialized");
}

/// Execute a simple test to verify WASM runtime works.
pub fn test_runtime() -> Result<(), WasmError> {
    crate::serial_println!("[WASM] Running runtime test...");

    // Minimal WASM module that exports an "add" function: (i32, i32) -> i32
    // This is hand-crafted minimal WASM bytecode
    //
    // (module
    //   (func $add (export "add") (param i32 i32) (result i32)
    //     local.get 0
    //     local.get 1
    //     i32.add))
    let wasm_add: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, // magic: \0asm
        0x01, 0x00, 0x00, 0x00, // version: 1
        // Type section (1)
        0x01, 0x07, // section id=1, size=7
        0x01, // 1 type
        0x60, // func
        0x02, 0x7f, 0x7f, // 2 params: i32, i32
        0x01, 0x7f, // 1 result: i32
        // Function section (3)
        0x03, 0x02, // section id=3, size=2
        0x01, // 1 function
        0x00, // type index 0
        // Export section (7)
        0x07, 0x07, // section id=7, size=7
        0x01, // 1 export
        0x03, b'a', b'd', b'd', // name: "add"
        0x00, // kind: func
        0x00, // func index 0
        // Code section (10)
        0x0a, 0x09, // section id=10, size=9
        0x01, // 1 function body
        0x07, // body size=7
        0x00, // 0 locals
        0x20, 0x00, // local.get 0
        0x20, 0x01, // local.get 1
        0x6a, // i32.add
        0x0b, // end
    ];

    // Load and execute
    let engine = WasmEngine::new()?;
    let module = engine.load_module(wasm_add)?;
    let mut instance = engine.instantiate(&module)?;

    // Call add(2, 3) and expect 5
    let result = instance.call_i32_i32_ret_i32("add", 2, 3)?;

    if result == 5 {
        crate::serial_println!("[WASM] Test passed: add(2, 3) = {}", result);
        Ok(())
    } else {
        Err(WasmError::ExecutionError(alloc::format!(
            "Expected 5, got {}",
            result
        )))
    }
}
