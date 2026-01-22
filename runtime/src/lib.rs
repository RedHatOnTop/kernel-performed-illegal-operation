//! KPIO WASM Runtime
//!
//! This crate provides the WebAssembly runtime for the KPIO operating system.
//! It uses Wasmtime with Cranelift JIT compilation for executing WASM modules.
//!
//! # Architecture
//!
//! The runtime is organized into the following modules:
//!
//! - `engine`: Core Wasmtime engine configuration
//! - `module`: WASM module loading and validation
//! - `instance`: Module instantiation and execution
//! - `wasi`: WASI Preview 2 implementation
//! - `host`: Host function bindings for kernel services
//! - `memory`: Linear memory management
//! - `sandbox`: Security sandbox implementation

#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;

pub mod engine;
pub mod module;
pub mod instance;
pub mod wasi;
pub mod host;
pub mod memory;
pub mod sandbox;

use alloc::string::String;
use alloc::vec::Vec;

/// Runtime error types.
#[derive(Debug, Clone)]
pub enum RuntimeError {
    /// Failed to compile WASM module.
    CompilationError(String),
    /// Failed to instantiate module.
    InstantiationError(String),
    /// Failed to execute function.
    ExecutionError(String),
    /// Memory access violation.
    MemoryError(String),
    /// WASI error.
    WasiError(String),
    /// Invalid WASM binary.
    InvalidBinary(String),
    /// Resource limit exceeded.
    ResourceLimit(String),
    /// Permission denied.
    PermissionDenied(String),
}

/// Runtime configuration.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Maximum memory per instance (in pages, 64KB each).
    pub max_memory_pages: u32,
    /// Maximum table size.
    pub max_table_size: u32,
    /// Enable SIMD instructions.
    pub enable_simd: bool,
    /// Enable multi-threading.
    pub enable_threads: bool,
    /// Enable reference types.
    pub enable_reference_types: bool,
    /// Enable bulk memory operations.
    pub enable_bulk_memory: bool,
    /// Stack size for WASM execution.
    pub stack_size: usize,
    /// Enable fuel-based execution limiting.
    pub enable_fuel: bool,
    /// Initial fuel amount.
    pub initial_fuel: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        RuntimeConfig {
            max_memory_pages: 256, // 16 MB
            max_table_size: 10000,
            enable_simd: true,
            enable_threads: false,
            enable_reference_types: true,
            enable_bulk_memory: true,
            stack_size: 1024 * 1024, // 1 MB
            enable_fuel: true,
            initial_fuel: 1_000_000,
        }
    }
}

/// Initialize the WASM runtime.
pub fn init() -> Result<(), RuntimeError> {
    engine::init()?;
    Ok(())
}

/// Execute a WASM module.
pub fn execute(
    wasm_bytes: &[u8],
    entry_point: &str,
    args: &[u8],
) -> Result<Vec<u8>, RuntimeError> {
    let module = module::Module::from_bytes(wasm_bytes)?;
    let instance = instance::Instance::new(&module)?;
    instance.call(entry_point, args)
}

/// Load and validate a WASM module without executing.
pub fn validate(wasm_bytes: &[u8]) -> Result<(), RuntimeError> {
    module::Module::validate(wasm_bytes)
}
