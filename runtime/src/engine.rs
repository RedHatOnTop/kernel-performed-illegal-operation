//! WASM execution engine configuration and management.
//!
//! This module handles the core engine setup for compiling,
//! instantiating, and executing WASM modules.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

use crate::executor::{self, ExecutorContext, HostFunction};
use crate::instance::{Imports, Instance};
use crate::interpreter::{TrapError, WasmValue};
use crate::module::Module;
use crate::parser::WasmParser;
use crate::{RuntimeConfig, RuntimeError};

/// Global engine instance.
static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

/// Initialize the engine with default configuration.
pub fn init() -> Result<(), RuntimeError> {
    init_with_config(RuntimeConfig::default())
}

/// Initialize the engine with custom configuration.
pub fn init_with_config(config: RuntimeConfig) -> Result<(), RuntimeError> {
    let engine = Engine::new(config)?;
    *ENGINE.lock() = Some(engine);
    Ok(())
}

/// Get a reference to the global engine.
pub fn get() -> Result<Arc<Engine>, RuntimeError> {
    ENGINE
        .lock()
        .as_ref()
        .map(|e| Arc::new(e.clone()))
        .ok_or_else(|| RuntimeError::ExecutionError("Engine not initialized".into()))
}

/// WASM execution engine.
#[derive(Clone)]
pub struct Engine {
    /// Engine configuration.
    config: RuntimeConfig,

    /// Compilation cache enabled.
    cache_enabled: bool,
}

impl Engine {
    /// Create a new engine with the given configuration.
    pub fn new(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        Ok(Engine {
            config,
            cache_enabled: true,
        })
    }

    /// Get the engine configuration.
    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    /// Enable or disable compilation caching.
    pub fn set_cache_enabled(&mut self, enabled: bool) {
        self.cache_enabled = enabled;
    }

    /// Check if caching is enabled.
    pub fn cache_enabled(&self) -> bool {
        self.cache_enabled
    }

    /// Parse and validate a WASM module from bytes.
    pub fn load_module(&self, wasm_bytes: &[u8]) -> Result<Module, RuntimeError> {
        let module = Module::from_bytes(wasm_bytes)?;
        Ok(module)
    }

    /// Instantiate a module with default imports.
    pub fn instantiate(&self, module: &Module) -> Result<Instance, RuntimeError> {
        self.instantiate_with_imports(module, Imports::default())
    }

    /// Instantiate a module with custom imports.
    pub fn instantiate_with_imports(
        &self,
        module: &Module,
        imports: Imports,
    ) -> Result<Instance, RuntimeError> {
        let mut instance = Instance::new_with_imports(module, imports)?;
        if self.config.enable_fuel {
            instance.set_fuel(Some(self.config.initial_fuel));
        } else {
            instance.set_fuel(None);
        }
        Ok(instance)
    }

    /// Load, instantiate, and call a function in one step.
    pub fn execute(
        &self,
        wasm_bytes: &[u8],
        entry_point: &str,
        args: &[WasmValue],
    ) -> Result<Vec<WasmValue>, RuntimeError> {
        let module = self.load_module(wasm_bytes)?;
        let mut instance = self.instantiate(&module)?;
        instance.call_typed(entry_point, args)
    }
}

/// Compilation options for modules.
#[derive(Debug, Clone)]
pub struct CompilationOptions {
    /// Optimization level (0-3).
    pub opt_level: u8,
    /// Enable debug info.
    pub debug_info: bool,
    /// Enable position-independent code.
    pub pic: bool,
}

impl Default for CompilationOptions {
    fn default() -> Self {
        CompilationOptions {
            opt_level: 2,
            debug_info: false,
            pic: true,
        }
    }
}
