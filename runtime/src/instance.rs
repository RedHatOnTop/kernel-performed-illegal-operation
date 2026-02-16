//! WASM module instantiation and execution.
//!
//! This module handles creating instances of compiled modules
//! and executing their functions. Uses the interpreter engine
//! from `executor.rs` for actual execution.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::executor::{self, ExecutorContext, HostFn, HostFunction as ExecHostFunction};
use crate::interpreter::WasmValue;
use crate::memory::LinearMemory;
use crate::module::Module;
use crate::RuntimeError;

/// An instantiated WASM module with execution context.
pub struct Instance {
    /// Execution context holding memories, tables, globals.
    ctx: ExecutorContext,

    /// Instance store for host-side data.
    store: Store,
}

impl Instance {
    /// Create a new instance from a module.
    pub fn new(module: &Module) -> Result<Self, RuntimeError> {
        Self::new_with_imports(module, Imports::default())
    }

    /// Create a new instance with imports.
    pub fn new_with_imports(module: &Module, imports: Imports) -> Result<Self, RuntimeError> {
        // Convert Imports to executor HostFunction list
        let host_fns = imports.to_exec_host_functions();

        let ctx = ExecutorContext::new_with_host_functions(module.clone(), host_fns)
            .map_err(|e| RuntimeError::InstantiationError(alloc::format!("{}", e)))?;

        Ok(Instance {
            ctx,
            store: Store::new(),
        })
    }

    /// Call an exported function with WasmValue args and results.
    pub fn call_typed(
        &mut self,
        name: &str,
        args: &[WasmValue],
    ) -> Result<Vec<WasmValue>, RuntimeError> {
        executor::execute_export(&mut self.ctx, name, args)
            .map_err(|e| RuntimeError::ExecutionError(alloc::format!("{}", e)))
    }

    /// Call an exported function (legacy API, returns empty bytes).
    pub fn call(&mut self, name: &str, args: &[u8]) -> Result<Vec<u8>, RuntimeError> {
        let wasm_args: Vec<WasmValue> = args
            .chunks(4)
            .map(|chunk| {
                let mut bytes = [0u8; 4];
                for (i, &b) in chunk.iter().enumerate() {
                    bytes[i] = b;
                }
                WasmValue::I32(i32::from_le_bytes(bytes))
            })
            .collect();

        let results = self.call_typed(name, &wasm_args)?;

        // Encode results as bytes
        let mut output = Vec::new();
        for val in results {
            match val {
                WasmValue::I32(v) => output.extend_from_slice(&v.to_le_bytes()),
                WasmValue::I64(v) => output.extend_from_slice(&v.to_le_bytes()),
                WasmValue::F32(v) => output.extend_from_slice(&v.to_bits().to_le_bytes()),
                WasmValue::F64(v) => output.extend_from_slice(&v.to_bits().to_le_bytes()),
                _ => {}
            }
        }
        Ok(output)
    }

    /// Get the first linear memory.
    pub fn memory(&self) -> Option<&LinearMemory> {
        self.ctx.memories.first()
    }

    /// Get mutable access to the first linear memory.
    pub fn memory_mut(&mut self) -> Option<&mut LinearMemory> {
        self.ctx.memories.first_mut()
    }

    /// Get remaining fuel.
    pub fn fuel(&self) -> Option<u64> {
        self.ctx.fuel
    }

    /// Add fuel for execution.
    pub fn add_fuel(&mut self, fuel: u64) {
        if let Some(ref mut f) = self.ctx.fuel {
            *f = f.saturating_add(fuel);
        }
    }

    /// Set fuel for execution.
    pub fn set_fuel(&mut self, fuel: Option<u64>) {
        self.ctx.fuel = fuel;
    }

    /// Get a reference to the store.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Get mutable access to the store.
    pub fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    /// Get a reference to the executor context.
    pub fn context(&self) -> &ExecutorContext {
        &self.ctx
    }

    /// Get mutable access to the executor context.
    pub fn context_mut(&mut self) -> &mut ExecutorContext {
        &mut self.ctx
    }

    /// Get the stdout buffer captured by host functions.
    pub fn stdout(&self) -> &[u8] {
        &self.ctx.stdout
    }

    /// Get the stderr buffer captured by host functions.
    pub fn stderr(&self) -> &[u8] {
        &self.ctx.stderr
    }

    /// Get exit code if process exited.
    pub fn exit_code(&self) -> Option<i32> {
        self.ctx.exit_code
    }
}

/// Import resolver for host functions.
#[derive(Default)]
pub struct Imports {
    /// Function imports: (module, name) -> HostFn.
    functions: BTreeMap<(String, String), HostFn>,

    /// Memory imports.
    memories: BTreeMap<(String, String), LinearMemory>,
}

impl Imports {
    /// Create a new empty import set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a host function import.
    pub fn add_function(&mut self, module: &str, name: &str, func: HostFn) {
        self.functions.insert((module.into(), name.into()), func);
    }

    /// Add a memory import.
    pub fn add_memory(&mut self, module: &str, name: &str, memory: LinearMemory) {
        self.memories.insert((module.into(), name.into()), memory);
    }

    /// Convert to executor-compatible host function list.
    pub fn to_exec_host_functions(&self) -> Vec<ExecHostFunction> {
        self.functions
            .iter()
            .map(|((module, name), func)| ExecHostFunction {
                module: module.clone(),
                name: name.clone(),
                func: *func,
                type_idx: None,
            })
            .collect()
    }
}

/// Instance store for host-side data.
pub struct Store {
    /// Host data indexed by key.
    data: BTreeMap<u64, Vec<u8>>,

    /// Next data key.
    next_key: u64,
}

impl Store {
    /// Create a new store.
    pub fn new() -> Self {
        Store {
            data: BTreeMap::new(),
            next_key: 0,
        }
    }

    /// Store data and return a key.
    pub fn store(&mut self, data: Vec<u8>) -> u64 {
        let key = self.next_key;
        self.next_key += 1;
        self.data.insert(key, data);
        key
    }

    /// Retrieve data by key.
    pub fn get(&self, key: u64) -> Option<&[u8]> {
        self.data.get(&key).map(|v| v.as_slice())
    }

    /// Remove data by key.
    pub fn remove(&mut self, key: u64) -> Option<Vec<u8>> {
        self.data.remove(&key)
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}
