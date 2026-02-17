//! KPIO WASM Runtime
//!
//! This crate provides a lightweight WebAssembly runtime for the KPIO operating system.
//! The execution path consists of a validating parser, a stack-based interpreter,
//! and a tiered JIT compiler that generates x86_64 machine code.
//!
//! WASI support covers both Preview 1 (`wasi_snapshot_preview1`) and Preview 2 with
//! streams, filesystem, clocks, random, CLI, sockets, and HTTP interfaces.
//!
//! # Architecture
//!
//! - `parser`: WASM binary parser (sections + instruction decoding)
//! - `module`: Parsed module representation + structural validation
//! - `instance`: Instantiation + import resolution
//! - `executor` / `interpreter`: Stack-machine execution and traps
//! - `wasi`: WASI Preview 1 context + in-memory VFS
//! - `wasi2`: WASI Preview 2 (streams, filesystem, clocks, random, CLI, sockets, HTTP)
//! - `host`: Host function bindings (WASI + KPIO/GPU/NET)
//! - `host_gui` / `host_system` / `host_net`: KPIO-specific host API bindings
//! - `memory` / `sandbox`: Linear memory + resource limiting
//! - `jit`: Tiered JIT compiler (IR + x86_64 codegen + cache + profiling + benchmarks)
//! - `wit`: WebAssembly Interface Types (WIT) parser and type system
//! - `component`: WASM Component Model (canonical ABI, linker, instances, WASI bridge)
//! - `package`: `.kpioapp` ZIP-based application package format
//! - `app_launcher`: Application lifecycle management (load → instantiate → run → update)
//! - `registry`: Application registry (install/uninstall/list)
//! - `posix_shim`: POSIX → WASI P2 function mapping (22 functions)
//! - `service_worker`: Service Worker runtime for PWA support

#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;

pub mod app_launcher;
pub mod component;
pub mod engine;
pub mod executor;
pub mod host;
pub mod host_gui;
pub mod host_net;
pub mod host_system;
pub mod instance;
pub mod interpreter;
pub mod jit;
pub mod memory;
pub mod module;
pub mod opcodes;
pub mod package;
pub mod parser;
pub mod posix_shim;
pub mod registry;
pub mod sandbox;
pub mod service_worker;
pub mod wasi;
pub mod wasi2;
pub mod wit;

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
pub fn execute(wasm_bytes: &[u8], entry_point: &str, args: &[u8]) -> Result<Vec<u8>, RuntimeError> {
    let module = module::Module::from_bytes(wasm_bytes)?;
    let mut inst = instance::Instance::new(&module)?;
    inst.call(entry_point, args)
}

/// Load and validate a WASM module without executing.
pub fn validate(wasm_bytes: &[u8]) -> Result<(), RuntimeError> {
    module::Module::validate(wasm_bytes)
}
