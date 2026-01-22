//! Wasmtime engine configuration and management.
//!
//! This module handles the core Wasmtime engine setup with
//! Cranelift JIT compilation.

use alloc::sync::Arc;
use spin::Mutex;

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
    ENGINE.lock()
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
