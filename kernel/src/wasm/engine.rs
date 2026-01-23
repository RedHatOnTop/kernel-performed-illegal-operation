//! WASM Engine using wasmi interpreter.
//!
//! This provides the core WASM execution engine for the kernel.

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

use wasmi::{
    Engine, Linker, Module, Store, TypedFunc,
    Caller, Func, AsContextMut, AsContext,
};

use super::WasmError;
use super::host::HostFunctions;

/// Global WASM engine instance.
static WASM_ENGINE: Mutex<Option<Arc<Engine>>> = Mutex::new(None);

/// Initialize the global WASM engine.
pub fn init() {
    let engine = Engine::default();
    *WASM_ENGINE.lock() = Some(Arc::new(engine));
}

/// Get the global engine.
fn get_engine() -> Result<Arc<Engine>, WasmError> {
    WASM_ENGINE.lock()
        .as_ref()
        .cloned()
        .ok_or_else(|| WasmError::ExecutionError("Engine not initialized".into()))
}

/// WASM Engine wrapper.
pub struct WasmEngine {
    engine: Arc<Engine>,
}

impl WasmEngine {
    /// Create a new WASM engine.
    pub fn new() -> Result<Self, WasmError> {
        let engine = get_engine()?;
        Ok(WasmEngine { engine })
    }
    
    /// Load a WASM module from bytes.
    pub fn load_module(&self, bytes: &[u8]) -> Result<WasmModule, WasmError> {
        let module = Module::new(&*self.engine, bytes)
            .map_err(|e| WasmError::ParseError(alloc::format!("{:?}", e)))?;
        
        Ok(WasmModule {
            engine: self.engine.clone(),
            module,
        })
    }
    
    /// Instantiate a module with default host functions.
    pub fn instantiate(&self, module: &WasmModule) -> Result<WasmInstance, WasmError> {
        // Create a store with host state
        let mut store = Store::new(&*self.engine, HostFunctions::new());
        
        // Create a linker for imports
        let linker: Linker<HostFunctions> = Linker::new(&*self.engine);
        
        // For now, we don't add any host imports
        // In a full implementation, we'd add WASI functions here
        
        // Instantiate the module
        let instance = linker
            .instantiate(&mut store, &module.module)
            .map_err(|e| WasmError::InstantiationError(alloc::format!("{:?}", e)))?
            .start(&mut store)
            .map_err(|e| WasmError::InstantiationError(alloc::format!("{:?}", e)))?;
        
        Ok(WasmInstance {
            store,
            instance,
        })
    }
}

/// A loaded WASM module.
pub struct WasmModule {
    engine: Arc<Engine>,
    module: Module,
}

impl WasmModule {
    /// Get the number of exports.
    pub fn export_count(&self) -> usize {
        self.module.exports().count()
    }
}

/// An instantiated WASM module.
pub struct WasmInstance {
    store: Store<HostFunctions>,
    instance: wasmi::Instance,
}

impl WasmInstance {
    /// Call a function that takes two i32s and returns an i32.
    pub fn call_i32_i32_ret_i32(&mut self, name: &str, a: i32, b: i32) -> Result<i32, WasmError> {
        let func = self.instance
            .get_typed_func::<(i32, i32), i32>(&self.store, name)
            .map_err(|e| WasmError::FunctionNotFound(alloc::format!("{}: {:?}", name, e)))?;
        
        func.call(&mut self.store, (a, b))
            .map_err(|e| WasmError::ExecutionError(alloc::format!("{:?}", e)))
    }
    
    /// Call a function with no parameters and no return value.
    pub fn call_void(&mut self, name: &str) -> Result<(), WasmError> {
        let func = self.instance
            .get_typed_func::<(), ()>(&self.store, name)
            .map_err(|e| WasmError::FunctionNotFound(alloc::format!("{}: {:?}", name, e)))?;
        
        func.call(&mut self.store, ())
            .map_err(|e| WasmError::ExecutionError(alloc::format!("{:?}", e)))
    }
    
    /// Get exported function by name.
    pub fn get_func(&self, name: &str) -> Option<Func> {
        self.instance.get_func(&self.store, name)
    }
    
    /// Read memory at offset.
    pub fn read_memory(&self, offset: usize, buf: &mut [u8]) -> Result<(), WasmError> {
        let memory = self.instance
            .get_memory(&self.store, "memory")
            .ok_or_else(|| WasmError::MemoryError("No memory export".into()))?;
        
        let data = memory.data(&self.store);
        if offset + buf.len() > data.len() {
            return Err(WasmError::MemoryError("Out of bounds read".into()));
        }
        
        buf.copy_from_slice(&data[offset..offset + buf.len()]);
        Ok(())
    }
    
    /// Write memory at offset.
    pub fn write_memory(&mut self, offset: usize, data: &[u8]) -> Result<(), WasmError> {
        let memory = self.instance
            .get_memory(&self.store, "memory")
            .ok_or_else(|| WasmError::MemoryError("No memory export".into()))?;
        
        let mem_data = memory.data_mut(&mut self.store);
        if offset + data.len() > mem_data.len() {
            return Err(WasmError::MemoryError("Out of bounds write".into()));
        }
        
        mem_data[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }
}
