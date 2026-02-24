# WebAssembly Runtime Design Document

**Document Version:** 2.1.0  
**Last Updated:** 2026-02-15  
**Status:** Historical design draft (partially reflects implementation; refer to latest code and Phase 7 docs)

> **Consistency Note:** This document contains early/mid-stage design drafts and may not fully align with the current codebase.
> For the current implementation status, refer to `runtime/src/*` and `docs/phase7/PHASE_7-2_WASM_APP_RUNTIME_EN.md`.

---

## Table of Contents

1. [Overview](#1-overview)
2. [Design Goals](#2-design-goals)
3. [Runtime Architecture](#3-runtime-architecture)
4. [wasmi Integration](#4-wasmi-integration)
5. [Browser WASM (SpiderMonkey)](#5-browser-wasm-spidermonkey)
6. [WASI Implementation](#6-wasi-implementation)
7. [Custom System Extensions](#7-custom-system-extensions)
8. [Memory Management](#8-memory-management)
9. [Sandboxing and Isolation](#9-sandboxing-and-isolation)
10. [Performance Optimizations](#10-performance-optimizations)
11. [Debugging and Profiling](#11-debugging-and-profiling)
12. [Module Loading](#12-module-loading)
13. [Inter-Module Communication](#13-inter-module-communication)

---

## 1. Overview

### 1.1 Purpose

This document specifies the design of the KPIO WebAssembly runtime system, which provides two execution environments:

1. **Kernel WASM Runtime (wasmi):** Lightweight interpreter for kernel-side WASM execution
2. **Browser WASM Runtime (SpiderMonkey):** Full-featured JIT for web content (Phase 2)

### 1.2 Strategic Decision: Dual Runtime

| Runtime | Context | Use Case | Performance |
|---------|---------|----------|-------------|
| wasmi | Kernel (no_std) | System services, drivers | Interpreted |
| SpiderMonkey | Browser (userspace) | Web apps, games | JIT compiled |

**Key Insight:** Web WASM via browser handles performance-critical workloads, allowing kernel runtime to prioritize simplicity and security.

### 1.3 Scope

This document covers:
- wasmi interpreter integration (kernel-side, no_std)
- SpiderMonkey integration plans (browser-side, Phase 2)
- Shared WASM module caching between kernel and browser
- WASI (WebAssembly System Interface) implementation
- Custom syscall extensions for GPU, IPC, and capabilities
- Memory management for WASM instances
- Security isolation mechanisms
- Performance optimization strategies

This document does NOT cover:
- Kernel internals (see `kernel.md`)
- Graphics pipeline details (see `graphics.md`)
- Browser engine architecture (see Phase 2 design docs)

### 1.4 Current Implementation Status

```
runtime/src/
    lib.rs              # ✅ Runtime entry, config, error types
    parser.rs           # ✅ WASM binary parser (all sections)
    module.rs           # ✅ Module representation + validation
    instance.rs         # ✅ Instantiation + import resolution
    interpreter.rs      # ✅ Stack-machine interpreter
    executor.rs         # ✅ Execution coordinator
    engine.rs           # ✅ load/instantiate/execute API
    memory.rs           # ✅ Linear memory with bounds checking
    opcodes.rs          # ✅ Opcode definitions
    sandbox.rs          # ✅ Resource limiting (CPU/memory/FD)
    wasi.rs             # ✅ WASI Preview 1 (full, in-memory VFS)
    wasi2/              # ✅ WASI Preview 2 (streams, clocks, random, CLI, real sockets & HTTP)
    host.rs             # ✅ Host functions (wasi + kpio/gpu/gui/system/net)
    host_gui.rs         # ✅ KPIO GUI API bindings
    host_system.rs      # ✅ KPIO System API bindings
    host_net.rs         # ✅ KPIO Network API bindings
    wit/                # ✅ WIT parser + type system
    component/          # ✅ Component Model (canonical ABI, linker, instances)
    jit/                # ✅ JIT compiler (IR + x86_64 codegen + benchmarks)
    package.rs          # ✅ .kpioapp ZIP package format
    app_launcher.rs     # ✅ App lifecycle (load → run → update)
    registry.rs         # ✅ App registry (install/uninstall/list)
    posix_shim.rs       # ✅ POSIX → WASI P2 mapping (22 functions)
    service_worker.rs   # ✅ Service worker runtime
```

### 1.5 Source Location (Actual Structure)

```
runtime/
    Cargo.toml
    src/
        lib.rs              # Runtime entry, RuntimeConfig, RuntimeError
        parser.rs           # WASM binary parser
        module.rs           # Module data structure + validation
        instance.rs         # Instance + import binding
        interpreter.rs      # Stack-machine interpreter
        executor.rs         # Execution coordinator
        engine.rs           # High-level load/instantiate/execute
        memory.rs           # Linear memory management
        opcodes.rs          # WASM opcode definitions
        sandbox.rs          # Resource limiting
        wasi.rs             # WASI Preview 1 (VFS-backed)
        wasi2/              # WASI Preview 2
            mod.rs          # Resource table, stream abstraction
            streams.rs      # Input/output streams
            filesystem.rs   # File operations
            clocks.rs       # Monotonic + wall clock
            random.rs       # CSPRNG random
            cli.rs          # Args, env, exit, stdout/stderr
            sockets.rs      # TCP/UDP sockets (real kernel network with `kernel` feature)
            http.rs         # HTTP outgoing handler (real HTTP with `kernel` feature)
        host.rs             # WASI + KPIO host function bindings
        host_gui.rs         # kpio:gui host (create-window, draw-*)
        host_system.rs      # kpio:system host (time, hostname, notify)
        host_net.rs         # kpio:net host (fetch, connect, listen)
        wit/                # WIT parser + AST types
        component/          # WASM Component Model
            mod.rs          # ComponentValue, ComponentType enums
            canonical.rs    # Canonical ABI lower/lift
            linker.rs       # Component linker
            instance.rs     # Component instances
            wasi_bridge.rs  # WASI P2 bridge for components
        jit/                # JIT compiler infrastructure
            mod.rs          # Tiered compilation framework
            ir.rs           # IR opcode definitions
            codegen.rs      # x86_64 machine code generation
            cache.rs        # LRU code cache
            profile.rs      # Call count profiling
            bench.rs        # Performance benchmarks
        package.rs          # .kpioapp ZIP package handling
        app_launcher.rs     # App lifecycle + update management
        registry.rs         # App install/uninstall registry
        posix_shim.rs       # POSIX → WASI P2 function mapping
        service_worker.rs   # Service Worker runtime
```

---

## 2. Design Goals

### 2.1 Primary Goals

| Goal | Priority | Description |
|------|----------|-------------|
| Security | Critical | Complete isolation between WASM instances |
| Performance | Critical | Near-native execution speed |
| Compatibility | High | WASI Preview 2 compliance |
| Determinism | High | Reproducible execution for debugging |
| Resource Control | High | Fine-grained resource limits |

### 2.2 Non-Goals

| Non-Goal | Rationale |
|----------|-----------|
| JavaScript Support | Not a browser; pure WASM only |
| DOM Access | No browser APIs |
| Legacy WASI | Only Preview 2+ supported |
| Multi-threading (initial) | Phase 2+ feature |

---

## 3. Runtime Architecture

### 3.1 Component Overview

```
+=========================================================================+
|                        WASM APPLICATION                                  |
|                         (.wasm binary)                                   |
+=========================================================================+
                                  |
                                  v
+=========================================================================+
|                        RUNTIME LAYER                                     |
+=========================================================================+
|                                                                          |
|  +---------------------------+  +------------------------------------+   |
|  |    Module Loader          |  |    Capability Validator            |   |
|  |  - Validation             |  |  - Permission checking             |   |
|  |  - Compilation            |  |  - Capability derivation           |   |
|  |  - Caching                |  +------------------------------------+   |
|  +---------------------------+                                           |
|                                                                          |
|  +--------------------------------------------------------------------+  |
|  |                      WASMTIME ENGINE                                |  |
|  |  - Cranelift JIT                                                    |  |
|  |  - AOT compilation                                                  |  |
|  |  - WASM validation                                                  |  |
|  +--------------------------------------------------------------------+  |
|                                                                          |
|  +---------------------------+  +------------------------------------+   |
|  |    WASI Layer             |  |    Custom Extensions               |   |
|  |  - File I/O               |  |  - GPU (WebGPU)                    |   |
|  |  - Clock                  |  |  - IPC                             |   |
|  |  - Random                 |  |  - Capability                      |   |
|  |  - Sockets                |  |  - Process                         |   |
|  +---------------------------+  +------------------------------------+   |
|                                                                          |
+=========================================================================+
                                  |
                                  v
+=========================================================================+
|                        KERNEL INTERFACE                                  |
|                      (Syscalls / IPC)                                    |
+=========================================================================+
```

### 3.2 Instance Lifecycle

```
+-------------+     +-------------+     +-------------+     +-------------+
|   Module    | --> |  Validate   | --> |   Compile   | --> |  Instance   |
|   (.wasm)   |     |   (WASM)    |     |  (Native)   |     |  (Running)  |
+-------------+     +-------------+     +-------------+     +-------------+
                                              |                    |
                                              v                    v
                                        +-------------+     +-------------+
                                        |   Cache     |     |   Cleanup   |
                                        |  (Compiled) |     |  (On exit)  |
                                        +-------------+     +-------------+
```

---

## 4. Wasmtime Integration

### 4.1 Engine Configuration

```rust
// runtime/src/engine.rs

use wasmtime::*;

pub struct RuntimeEngine {
    engine: Engine,
    linker: Linker<WasiCtx>,
    module_cache: ModuleCache,
}

impl RuntimeEngine {
    pub fn new(config: RuntimeConfig) -> Result<Self, EngineError> {
        let mut wasmtime_config = Config::new();
        
        // Compilation settings
        wasmtime_config.cranelift_opt_level(OptLevel::Speed);
        wasmtime_config.cranelift_nan_canonicalization(true);
        
        // WASM features
        wasmtime_config.wasm_simd(true);
        wasmtime_config.wasm_bulk_memory(true);
        wasmtime_config.wasm_multi_value(true);
        wasmtime_config.wasm_reference_types(true);
        wasmtime_config.wasm_tail_call(true);
        wasmtime_config.wasm_relaxed_simd(config.allow_relaxed_simd);
        wasmtime_config.wasm_threads(config.allow_threads);
        
        // Memory settings
        wasmtime_config.static_memory_maximum_size(config.max_memory_per_instance);
        wasmtime_config.static_memory_guard_size(config.guard_page_size);
        wasmtime_config.dynamic_memory_guard_size(config.guard_page_size);
        
        // Compilation cache
        if let Some(cache_path) = &config.compilation_cache {
            wasmtime_config.cache_config_load(cache_path)?;
        }
        
        // Async support for cooperative scheduling
        wasmtime_config.async_support(true);
        wasmtime_config.consume_fuel(true);
        
        let engine = Engine::new(&wasmtime_config)?;
        
        // Create linker with WASI and custom extensions
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::add_to_linker(&mut linker, |ctx| ctx)?;
        add_gpu_extensions(&mut linker)?;
        add_ipc_extensions(&mut linker)?;
        add_capability_extensions(&mut linker)?;
        
        Ok(Self {
            engine,
            linker,
            module_cache: ModuleCache::new(config.cache_size),
        })
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Maximum memory per WASM instance (default: 4GB)
    pub max_memory_per_instance: u64,
    
    /// Guard page size (default: 2MB)
    pub guard_page_size: u64,
    
    /// Compilation cache directory
    pub compilation_cache: Option<PathBuf>,
    
    /// Module cache size (number of modules)
    pub cache_size: usize,
    
    /// Allow WASM threads proposal
    pub allow_threads: bool,
    
    /// Allow relaxed SIMD
    pub allow_relaxed_simd: bool,
    
    /// Fuel limit per execution quantum
    pub fuel_per_quantum: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_memory_per_instance: 4 * 1024 * 1024 * 1024, // 4GB
            guard_page_size: 2 * 1024 * 1024, // 2MB
            compilation_cache: None,
            cache_size: 100,
            allow_threads: false,
            allow_relaxed_simd: true,
            fuel_per_quantum: 10_000_000,
        }
    }
}
```

### 4.2 Instance Management

```rust
// runtime/src/instance.rs

use wasmtime::*;

pub struct WasmInstance {
    /// Unique instance ID
    pub id: InstanceId,
    
    /// Wasmtime store containing instance state
    store: Store<WasiCtx>,
    
    /// The instantiated module
    instance: Instance,
    
    /// Exported functions cache
    exports: ExportCache,
    
    /// Resource limits
    limits: ResourceLimits,
    
    /// Capabilities granted to this instance
    capabilities: CapabilitySet,
}

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum memory pages (64KB each)
    pub max_memory_pages: u32,
    
    /// Maximum table elements
    pub max_table_elements: u32,
    
    /// Maximum fuel (instruction count)
    pub max_fuel: u64,
    
    /// Maximum open file descriptors
    pub max_fds: u32,
    
    /// Maximum IPC channels
    pub max_ipc_channels: u32,
}

impl WasmInstance {
    pub async fn new(
        engine: &RuntimeEngine,
        module_bytes: &[u8],
        caps: CapabilitySet,
        limits: ResourceLimits,
    ) -> Result<Self, InstanceError> {
        // Compile module (or retrieve from cache)
        let module = engine.compile_or_cache(module_bytes)?;
        
        // Create WASI context
        let wasi_ctx = WasiCtxBuilder::new()
            .inherit_stdio()  // For debugging
            .build();
        
        // Create store with resource limits
        let mut store = Store::new(&engine.engine, wasi_ctx);
        store.set_fuel(limits.max_fuel)?;
        store.limiter(|_| Box::new(InstanceLimiter::new(&limits)));
        
        // Instantiate module
        let instance = engine.linker.instantiate_async(&mut store, &module).await?;
        
        // Cache exports
        let exports = ExportCache::build(&instance, &store)?;
        
        Ok(Self {
            id: InstanceId::new(),
            store,
            instance,
            exports,
            limits,
            capabilities: caps,
        })
    }
    
    /// Call the WASM _start function
    pub async fn run(&mut self) -> Result<i32, RuntimeError> {
        let start = self.exports.get_func::<(), ()>("_start")?;
        
        match start.call_async(&mut self.store, ()).await {
            Ok(()) => Ok(0),
            Err(trap) => {
                if let Some(exit_code) = trap.downcast_ref::<WasiExitCode>() {
                    Ok(exit_code.0)
                } else {
                    Err(RuntimeError::Trap(trap))
                }
            }
        }
    }
    
    /// Call an exported function
    pub async fn call<P, R>(&mut self, name: &str, params: P) -> Result<R, RuntimeError>
    where
        P: WasmParams,
        R: WasmResults,
    {
        // Refuel for this call
        self.store.set_fuel(self.limits.max_fuel)?;
        
        let func = self.exports.get_func::<P, R>(name)?;
        let result = func.call_async(&mut self.store, params).await?;
        
        Ok(result)
    }
}

struct InstanceLimiter {
    max_memory_pages: u32,
    max_table_elements: u32,
}

impl ResourceLimiter for InstanceLimiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> Result<bool> {
        let desired_pages = (desired / 65536) as u32;
        Ok(desired_pages <= self.max_memory_pages)
    }
    
    fn table_growing(
        &mut self,
        current: u32,
        desired: u32,
        maximum: Option<u32>,
    ) -> Result<bool> {
        Ok(desired <= self.max_table_elements)
    }
}
```

### 4.3 Module Compilation Cache

```rust
// runtime/src/loader/cache.rs

use std::collections::HashMap;
use std::path::PathBuf;
use blake3::Hash;

pub struct ModuleCache {
    /// In-memory cache of compiled modules
    memory_cache: HashMap<Hash, Arc<Module>>,
    
    /// On-disk cache directory
    disk_cache: Option<PathBuf>,
    
    /// Maximum entries in memory cache
    max_entries: usize,
    
    /// LRU tracking
    lru: VecDeque<Hash>,
}

impl ModuleCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            memory_cache: HashMap::new(),
            disk_cache: None,
            max_entries,
            lru: VecDeque::new(),
        }
    }
    
    pub fn with_disk_cache(mut self, path: PathBuf) -> Self {
        std::fs::create_dir_all(&path).ok();
        self.disk_cache = Some(path);
        self
    }
    
    pub fn get_or_compile(
        &mut self,
        engine: &Engine,
        bytes: &[u8],
    ) -> Result<Arc<Module>, CompileError> {
        let hash = blake3::hash(bytes);
        
        // Check memory cache
        if let Some(module) = self.memory_cache.get(&hash) {
            self.touch_lru(hash);
            return Ok(module.clone());
        }
        
        // Check disk cache
        if let Some(ref disk_path) = self.disk_cache {
            let cache_file = disk_path.join(hash.to_hex().as_str());
            if cache_file.exists() {
                if let Ok(module) = unsafe { Module::deserialize_file(engine, &cache_file) } {
                    let module = Arc::new(module);
                    self.insert(hash, module.clone());
                    return Ok(module);
                }
            }
        }
        
        // Compile from source
        let module = Module::new(engine, bytes)?;
        let module = Arc::new(module);
        
        // Save to disk cache
        if let Some(ref disk_path) = self.disk_cache {
            let cache_file = disk_path.join(hash.to_hex().as_str());
            let _ = module.serialize_to_file(&cache_file);
        }
        
        self.insert(hash, module.clone());
        Ok(module)
    }
    
    fn insert(&mut self, hash: Hash, module: Arc<Module>) {
        // Evict if necessary
        while self.memory_cache.len() >= self.max_entries {
            if let Some(old_hash) = self.lru.pop_front() {
                self.memory_cache.remove(&old_hash);
            }
        }
        
        self.memory_cache.insert(hash, module);
        self.lru.push_back(hash);
    }
    
    fn touch_lru(&mut self, hash: Hash) {
        if let Some(pos) = self.lru.iter().position(|h| *h == hash) {
            self.lru.remove(pos);
            self.lru.push_back(hash);
        }
    }
}
```

---

## 5. WASI Implementation

### 5.1 WASI Preview 2 Interface

```rust
// runtime/src/wasi/mod.rs

use wasmtime_wasi::preview2::*;

/// KPIO's WASI context
pub struct KpioWasiCtx {
    /// Preview 2 table for resources
    table: ResourceTable,
    
    /// WASI context
    ctx: WasiCtx,
    
    /// File descriptor table
    fds: FdTable,
    
    /// Environment variables
    env: Vec<(String, String)>,
    
    /// Command line arguments
    args: Vec<String>,
    
    /// Working directory
    cwd: PathBuf,
    
    /// Mounted directories
    mounts: Vec<Mount>,
}

impl WasiView for KpioWasiCtx {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
    
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
}

pub struct KpioWasiCtxBuilder {
    ctx: KpioWasiCtx,
}

impl KpioWasiCtxBuilder {
    pub fn new() -> Self {
        Self {
            ctx: KpioWasiCtx {
                table: ResourceTable::new(),
                ctx: WasiCtx::new(),
                fds: FdTable::new(),
                env: Vec::new(),
                args: Vec::new(),
                cwd: PathBuf::from("/"),
                mounts: Vec::new(),
            },
        }
    }
    
    /// Add command line arguments
    pub fn args(mut self, args: &[impl AsRef<str>]) -> Self {
        self.ctx.args = args.iter().map(|s| s.as_ref().to_string()).collect();
        self
    }
    
    /// Add environment variable
    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.ctx.env.push((key.to_string(), value.to_string()));
        self
    }
    
    /// Mount a directory
    pub fn preopened_dir(
        mut self,
        host_path: impl AsRef<Path>,
        guest_path: impl AsRef<str>,
        perms: DirPerms,
        file_perms: FilePerms,
    ) -> Result<Self, WasiError> {
        let mount = Mount {
            host_path: host_path.as_ref().to_path_buf(),
            guest_path: guest_path.as_ref().to_string(),
            dir_perms: perms,
            file_perms,
        };
        self.ctx.mounts.push(mount);
        
        // Add to fd table
        let fd = self.ctx.fds.insert_dir(
            host_path.as_ref(),
            guest_path.as_ref(),
            perms,
            file_perms,
        )?;
        
        Ok(self)
    }
    
    /// Inherit stdio from host
    pub fn inherit_stdio(mut self) -> Self {
        // Connect to host's serial console for debugging
        self
    }
    
    pub fn build(self) -> KpioWasiCtx {
        self.ctx
    }
}
```

### 5.2 File System Operations

```rust
// runtime/src/wasi/fs.rs

use wasmtime_wasi::preview2::filesystem::*;

impl HostFilesystem for KpioWasiCtx {
    async fn stat(
        &mut self,
        fd: Resource<Descriptor>,
    ) -> Result<DescriptorStat, FsError> {
        let descriptor = self.table.get(&fd)?;
        
        match descriptor {
            Descriptor::File(file) => {
                let stat = file.stat().await?;
                Ok(DescriptorStat {
                    type_: DescriptorType::RegularFile,
                    link_count: stat.nlink,
                    size: stat.size,
                    data_access_timestamp: stat.atime.into(),
                    data_modification_timestamp: stat.mtime.into(),
                    status_change_timestamp: stat.ctime.into(),
                })
            }
            Descriptor::Dir(dir) => {
                let stat = dir.stat().await?;
                Ok(DescriptorStat {
                    type_: DescriptorType::Directory,
                    link_count: stat.nlink,
                    size: stat.size,
                    data_access_timestamp: stat.atime.into(),
                    data_modification_timestamp: stat.mtime.into(),
                    status_change_timestamp: stat.ctime.into(),
                })
            }
        }
    }
    
    async fn read(
        &mut self,
        fd: Resource<Descriptor>,
        length: u64,
        offset: u64,
    ) -> Result<(Vec<u8>, bool), FsError> {
        let descriptor = self.table.get(&fd)?;
        
        let file = descriptor.as_file()
            .ok_or(FsError::InvalidDescriptor)?;
        
        // Validate capability
        if !self.check_capability(&file.path, Permission::Read) {
            return Err(FsError::PermissionDenied);
        }
        
        let mut buf = vec![0u8; length as usize];
        let bytes_read = file.read_at(&mut buf, offset).await?;
        buf.truncate(bytes_read);
        
        let eof = bytes_read < length as usize;
        Ok((buf, eof))
    }
    
    async fn write(
        &mut self,
        fd: Resource<Descriptor>,
        data: Vec<u8>,
        offset: u64,
    ) -> Result<u64, FsError> {
        let descriptor = self.table.get(&fd)?;
        
        let file = descriptor.as_file()
            .ok_or(FsError::InvalidDescriptor)?;
        
        // Validate capability
        if !self.check_capability(&file.path, Permission::Write) {
            return Err(FsError::PermissionDenied);
        }
        
        let bytes_written = file.write_at(&data, offset).await?;
        Ok(bytes_written as u64)
    }
    
    async fn open_at(
        &mut self,
        dir_fd: Resource<Descriptor>,
        path_flags: PathFlags,
        path: String,
        open_flags: OpenFlags,
        descriptor_flags: DescriptorFlags,
    ) -> Result<Resource<Descriptor>, FsError> {
        let dir = self.table.get(&dir_fd)?
            .as_dir()
            .ok_or(FsError::NotADirectory)?;
        
        // Resolve path
        let resolved = dir.resolve_path(&path, path_flags.contains(PathFlags::SYMLINK_FOLLOW))?;
        
        // Validate capability
        let perms = if descriptor_flags.contains(DescriptorFlags::WRITE) {
            Permission::Write
        } else {
            Permission::Read
        };
        if !self.check_capability(&resolved, perms) {
            return Err(FsError::PermissionDenied);
        }
        
        // Open file via IPC to filesystem service
        let file = self.fs_service.open(
            &resolved,
            open_flags,
            descriptor_flags,
        ).await?;
        
        let fd = self.table.push(Descriptor::File(file))?;
        Ok(fd)
    }
}
```

### 5.3 Clock Operations

```rust
// runtime/src/wasi/clock.rs

use wasmtime_wasi::preview2::clocks::*;

impl HostMonotonicClock for KpioWasiCtx {
    fn now(&mut self) -> Instant {
        // Read from kernel monotonic clock
        let ticks = kernel_syscall::clock_monotonic_now();
        Instant::from_nanos(ticks)
    }
    
    fn resolution(&mut self) -> Duration {
        // APIC timer resolution
        Duration::from_nanos(100) // 100ns typical
    }
}

impl HostWallClock for KpioWasiCtx {
    fn now(&mut self) -> DateTime {
        // Read from kernel wall clock
        let (secs, nanos) = kernel_syscall::clock_realtime_now();
        DateTime {
            seconds: secs,
            nanoseconds: nanos,
        }
    }
    
    fn resolution(&mut self) -> Duration {
        Duration::from_nanos(1000) // 1us
    }
}
```

### 5.4 Random Number Generation

```rust
// runtime/src/wasi/random.rs

use wasmtime_wasi::preview2::random::*;

impl HostRandom for KpioWasiCtx {
    fn get_random_bytes(&mut self, len: u64) -> Result<Vec<u8>, RandomError> {
        // Request from kernel CSPRNG
        let mut buf = vec![0u8; len as usize];
        kernel_syscall::random_get(&mut buf)?;
        Ok(buf)
    }
    
    fn get_random_u64(&mut self) -> Result<u64, RandomError> {
        let bytes = self.get_random_bytes(8)?;
        Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
    }
}
```

---

## 6. Custom System Extensions

### 6.1 GPU Extension (WebGPU)

```rust
// runtime/src/extensions/gpu.rs

use wasmtime::*;

/// Add GPU extensions to linker
pub fn add_gpu_extensions(linker: &mut Linker<KpioWasiCtx>) -> Result<()> {
    // Adapter and device
    linker.func_wrap_async("kpio_gpu", "request_adapter", request_adapter)?;
    linker.func_wrap_async("kpio_gpu", "adapter_request_device", adapter_request_device)?;
    linker.func_wrap("kpio_gpu", "device_drop", device_drop)?;
    
    // Buffer operations
    linker.func_wrap("kpio_gpu", "device_create_buffer", device_create_buffer)?;
    linker.func_wrap("kpio_gpu", "buffer_map_async", buffer_map_async)?;
    linker.func_wrap("kpio_gpu", "buffer_get_mapped_range", buffer_get_mapped_range)?;
    linker.func_wrap("kpio_gpu", "buffer_unmap", buffer_unmap)?;
    linker.func_wrap("kpio_gpu", "buffer_drop", buffer_drop)?;
    
    // Texture operations
    linker.func_wrap("kpio_gpu", "device_create_texture", device_create_texture)?;
    linker.func_wrap("kpio_gpu", "texture_create_view", texture_create_view)?;
    linker.func_wrap("kpio_gpu", "texture_drop", texture_drop)?;
    
    // Shader operations
    linker.func_wrap("kpio_gpu", "device_create_shader_module", device_create_shader_module)?;
    linker.func_wrap("kpio_gpu", "shader_module_drop", shader_module_drop)?;
    
    // Pipeline operations
    linker.func_wrap("kpio_gpu", "device_create_render_pipeline", device_create_render_pipeline)?;
    linker.func_wrap("kpio_gpu", "device_create_compute_pipeline", device_create_compute_pipeline)?;
    
    // Command encoding
    linker.func_wrap("kpio_gpu", "device_create_command_encoder", device_create_command_encoder)?;
    linker.func_wrap("kpio_gpu", "command_encoder_begin_render_pass", command_encoder_begin_render_pass)?;
    linker.func_wrap("kpio_gpu", "command_encoder_begin_compute_pass", command_encoder_begin_compute_pass)?;
    linker.func_wrap("kpio_gpu", "command_encoder_finish", command_encoder_finish)?;
    
    // Render pass operations
    linker.func_wrap("kpio_gpu", "render_pass_set_pipeline", render_pass_set_pipeline)?;
    linker.func_wrap("kpio_gpu", "render_pass_set_bind_group", render_pass_set_bind_group)?;
    linker.func_wrap("kpio_gpu", "render_pass_set_vertex_buffer", render_pass_set_vertex_buffer)?;
    linker.func_wrap("kpio_gpu", "render_pass_draw", render_pass_draw)?;
    linker.func_wrap("kpio_gpu", "render_pass_end", render_pass_end)?;
    
    // Queue operations
    linker.func_wrap_async("kpio_gpu", "queue_submit", queue_submit)?;
    linker.func_wrap("kpio_gpu", "queue_write_buffer", queue_write_buffer)?;
    
    // Surface operations
    linker.func_wrap("kpio_gpu", "create_surface", create_surface)?;
    linker.func_wrap("kpio_gpu", "surface_configure", surface_configure)?;
    linker.func_wrap_async("kpio_gpu", "surface_get_current_texture", surface_get_current_texture)?;
    linker.func_wrap("kpio_gpu", "surface_present", surface_present)?;
    
    Ok(())
}

async fn request_adapter(
    mut caller: Caller<'_, KpioWasiCtx>,
    options_ptr: u32,
) -> Result<u32> {
    // Validate GPU capability
    if !caller.data().capabilities.has_gpu_access() {
        return Err(anyhow!("GPU access denied"));
    }
    
    let memory = get_memory(&caller)?;
    let options: AdapterOptions = memory.read_struct(options_ptr)?;
    
    // Request adapter from GPU service
    let gpu = caller.data().gpu_service.as_ref()
        .ok_or_else(|| anyhow!("GPU service not available"))?;
    
    let adapter = gpu.request_adapter(options).await?;
    let handle = caller.data_mut().gpu_resources.insert_adapter(adapter);
    
    Ok(handle)
}

fn device_create_buffer(
    mut caller: Caller<'_, KpioWasiCtx>,
    device_handle: u32,
    desc_ptr: u32,
) -> Result<u32> {
    let memory = get_memory(&caller)?;
    let desc: BufferDescriptor = memory.read_struct(desc_ptr)?;
    
    // Validate resource limits
    let limits = &caller.data().resource_limits;
    if desc.size > limits.max_buffer_size {
        return Err(anyhow!("Buffer size exceeds limit"));
    }
    
    let device = caller.data().gpu_resources.get_device(device_handle)?;
    let buffer = device.create_buffer(&desc.into())?;
    
    let handle = caller.data_mut().gpu_resources.insert_buffer(buffer);
    Ok(handle)
}
```

### 6.2 IPC Extension

```rust
// runtime/src/extensions/ipc.rs

pub fn add_ipc_extensions(linker: &mut Linker<KpioWasiCtx>) -> Result<()> {
    // Channel management
    linker.func_wrap("kpio_ipc", "channel_create", channel_create)?;
    linker.func_wrap("kpio_ipc", "channel_connect", channel_connect)?;
    linker.func_wrap("kpio_ipc", "channel_close", channel_close)?;
    
    // Message passing
    linker.func_wrap_async("kpio_ipc", "channel_send", channel_send)?;
    linker.func_wrap_async("kpio_ipc", "channel_recv", channel_recv)?;
    linker.func_wrap_async("kpio_ipc", "channel_try_recv", channel_try_recv)?;
    
    // Shared memory
    linker.func_wrap("kpio_ipc", "shm_create", shm_create)?;
    linker.func_wrap("kpio_ipc", "shm_map", shm_map)?;
    linker.func_wrap("kpio_ipc", "shm_unmap", shm_unmap)?;
    
    // Service registration
    linker.func_wrap("kpio_ipc", "service_register", service_register)?;
    linker.func_wrap("kpio_ipc", "service_lookup", service_lookup)?;
    
    Ok(())
}

fn channel_create(
    mut caller: Caller<'_, KpioWasiCtx>,
    capacity: u32,
) -> Result<u32> {
    // Validate capability
    if !caller.data().capabilities.has_ipc_create() {
        return Err(anyhow!("IPC create denied"));
    }
    
    // Validate limits
    let current_channels = caller.data().ipc_channels.len();
    if current_channels >= caller.data().resource_limits.max_ipc_channels as usize {
        return Err(anyhow!("IPC channel limit reached"));
    }
    
    // Create channel via kernel syscall
    let channel_id = kernel_syscall::ipc_create_channel(capacity)?;
    
    let handle = caller.data_mut().ipc_channels.insert(channel_id);
    Ok(handle)
}

async fn channel_send(
    mut caller: Caller<'_, KpioWasiCtx>,
    channel_handle: u32,
    msg_ptr: u32,
    msg_len: u32,
) -> Result<()> {
    let channel_id = caller.data().ipc_channels.get(channel_handle)?;
    
    // Read message from WASM memory
    let memory = get_memory(&caller)?;
    let msg_data = memory.read_bytes(msg_ptr, msg_len as usize)?;
    
    // Send via kernel
    kernel_syscall::ipc_send(*channel_id, &msg_data).await?;
    
    Ok(())
}

async fn channel_recv(
    mut caller: Caller<'_, KpioWasiCtx>,
    channel_handle: u32,
    buf_ptr: u32,
    buf_len: u32,
) -> Result<u32> {
    let channel_id = caller.data().ipc_channels.get(channel_handle)?;
    
    // Receive via kernel
    let msg_data = kernel_syscall::ipc_recv(*channel_id).await?;
    
    // Validate buffer size
    if msg_data.len() > buf_len as usize {
        return Err(anyhow!("Buffer too small"));
    }
    
    // Write to WASM memory
    let memory = get_memory_mut(&mut caller)?;
    memory.write_bytes(buf_ptr, &msg_data)?;
    
    Ok(msg_data.len() as u32)
}
```

### 6.3 Capability Extension

```rust
// runtime/src/extensions/capability.rs

pub fn add_capability_extensions(linker: &mut Linker<KpioWasiCtx>) -> Result<()> {
    // Capability introspection
    linker.func_wrap("kpio_cap", "has_capability", has_capability)?;
    linker.func_wrap("kpio_cap", "list_capabilities", list_capabilities)?;
    
    // Capability delegation
    linker.func_wrap("kpio_cap", "derive_capability", derive_capability)?;
    linker.func_wrap("kpio_cap", "revoke_capability", revoke_capability)?;
    
    // Capability request
    linker.func_wrap_async("kpio_cap", "request_capability", request_capability)?;
    
    Ok(())
}

fn has_capability(
    caller: Caller<'_, KpioWasiCtx>,
    cap_type: u32,
    cap_data_ptr: u32,
    cap_data_len: u32,
) -> Result<u32> {
    let memory = get_memory(&caller)?;
    let cap_data = memory.read_bytes(cap_data_ptr, cap_data_len as usize)?;
    
    let cap = Capability::deserialize(cap_type, &cap_data)?;
    let has = caller.data().capabilities.contains(&cap);
    
    Ok(if has { 1 } else { 0 })
}

fn derive_capability(
    mut caller: Caller<'_, KpioWasiCtx>,
    parent_cap_handle: u32,
    restriction_ptr: u32,
    restriction_len: u32,
) -> Result<u32> {
    let memory = get_memory(&caller)?;
    let parent_cap = caller.data().capabilities.get(parent_cap_handle)?;
    let restriction = memory.read_struct(restriction_ptr)?;
    
    // Derive (attenuate) capability
    let derived = parent_cap.derive(restriction)?;
    
    let handle = caller.data_mut().capabilities.insert(derived);
    Ok(handle)
}
```

### 6.4 Process Extension

```rust
// runtime/src/extensions/process.rs

pub fn add_process_extensions(linker: &mut Linker<KpioWasiCtx>) -> Result<()> {
    // Process lifecycle
    linker.func_wrap_async("kpio_proc", "spawn", spawn)?;
    linker.func_wrap_async("kpio_proc", "wait", wait)?;
    linker.func_wrap("kpio_proc", "exit", exit)?;
    
    // Process info
    linker.func_wrap("kpio_proc", "get_pid", get_pid)?;
    linker.func_wrap("kpio_proc", "get_parent_pid", get_parent_pid)?;
    
    Ok(())
}

async fn spawn(
    mut caller: Caller<'_, KpioWasiCtx>,
    wasm_ptr: u32,
    wasm_len: u32,
    caps_ptr: u32,
    caps_len: u32,
) -> Result<u32> {
    // Validate spawn capability
    if !caller.data().capabilities.has_process_spawn() {
        return Err(anyhow!("Spawn denied"));
    }
    
    let memory = get_memory(&caller)?;
    let wasm_bytes = memory.read_bytes(wasm_ptr, wasm_len as usize)?;
    let caps_data = memory.read_bytes(caps_ptr, caps_len as usize)?;
    
    // Parse capabilities to transfer
    let caps = CapabilitySet::deserialize(&caps_data)?;
    
    // Validate that we own all capabilities being transferred
    for cap in caps.iter() {
        if !caller.data().capabilities.contains(cap) {
            return Err(anyhow!("Cannot transfer unowned capability"));
        }
    }
    
    // Spawn via kernel
    let child_id = kernel_syscall::spawn(&wasm_bytes, &caps).await?;
    
    // Track child process
    caller.data_mut().children.insert(child_id);
    
    Ok(child_id.as_u32())
}

async fn wait(
    caller: Caller<'_, KpioWasiCtx>,
    child_id: u32,
) -> Result<i32> {
    // Verify this is our child
    let child = ProcessId::from_u32(child_id);
    if !caller.data().children.contains(&child) {
        return Err(anyhow!("Not a child process"));
    }
    
    // Wait for child exit
    let exit_code = kernel_syscall::wait(child).await?;
    
    Ok(exit_code)
}
```

---

## 7. Memory Management

### 7.1 Linear Memory Handling

```rust
// runtime/src/memory/linear.rs

/// WASM linear memory interface
pub struct LinearMemory {
    /// Base address in host memory
    base: *mut u8,
    
    /// Current size in pages (64KB each)
    current_pages: u32,
    
    /// Maximum allowed pages
    maximum_pages: u32,
    
    /// Guard pages size
    guard_size: usize,
}

impl LinearMemory {
    pub fn new(initial_pages: u32, maximum_pages: u32) -> Result<Self, MemoryError> {
        let initial_size = initial_pages as usize * WASM_PAGE_SIZE;
        let maximum_size = maximum_pages as usize * WASM_PAGE_SIZE;
        let guard_size = 2 * 1024 * 1024; // 2MB guard
        
        // Reserve virtual address space
        let total_size = maximum_size + guard_size;
        let base = unsafe {
            kernel_syscall::mmap(
                None,
                total_size,
                ProtFlags::NONE,
                MapFlags::PRIVATE | MapFlags::ANONYMOUS,
            )?
        };
        
        // Commit initial pages
        unsafe {
            kernel_syscall::mprotect(
                base,
                initial_size,
                ProtFlags::READ | ProtFlags::WRITE,
            )?;
        }
        
        Ok(Self {
            base,
            current_pages: initial_pages,
            maximum_pages,
            guard_size,
        })
    }
    
    pub fn grow(&mut self, delta_pages: u32) -> Result<u32, MemoryError> {
        let new_pages = self.current_pages + delta_pages;
        
        if new_pages > self.maximum_pages {
            return Err(MemoryError::OutOfMemory);
        }
        
        let old_size = self.current_pages as usize * WASM_PAGE_SIZE;
        let new_size = new_pages as usize * WASM_PAGE_SIZE;
        
        // Commit new pages
        unsafe {
            kernel_syscall::mprotect(
                self.base.add(old_size),
                new_size - old_size,
                ProtFlags::READ | ProtFlags::WRITE,
            )?;
            
            // Zero the new memory
            core::ptr::write_bytes(
                self.base.add(old_size),
                0,
                new_size - old_size,
            );
        }
        
        let old_pages = self.current_pages;
        self.current_pages = new_pages;
        
        Ok(old_pages)
    }
    
    pub fn read<T: Copy>(&self, offset: u32) -> Result<T, MemoryError> {
        self.bounds_check(offset, core::mem::size_of::<T>())?;
        
        unsafe {
            Ok(core::ptr::read_unaligned(
                self.base.add(offset as usize) as *const T
            ))
        }
    }
    
    pub fn write<T: Copy>(&mut self, offset: u32, value: T) -> Result<(), MemoryError> {
        self.bounds_check(offset, core::mem::size_of::<T>())?;
        
        unsafe {
            core::ptr::write_unaligned(
                self.base.add(offset as usize) as *mut T,
                value,
            );
        }
        Ok(())
    }
    
    fn bounds_check(&self, offset: u32, len: usize) -> Result<(), MemoryError> {
        let end = offset as usize + len;
        let size = self.current_pages as usize * WASM_PAGE_SIZE;
        
        if end > size {
            Err(MemoryError::OutOfBounds { offset, len, size })
        } else {
            Ok(())
        }
    }
}

const WASM_PAGE_SIZE: usize = 65536; // 64KB
```

### 7.2 Shared Memory (for threads)

```rust
// runtime/src/memory/shared.rs

/// Shared memory for WASM threads
pub struct SharedMemory {
    /// Linear memory backing
    memory: LinearMemory,
    
    /// Reference count
    refcount: AtomicUsize,
    
    /// Lock for grow operations
    grow_lock: Mutex<()>,
}

impl SharedMemory {
    pub fn new(initial_pages: u32, maximum_pages: u32) -> Result<Arc<Self>, MemoryError> {
        let memory = LinearMemory::new(initial_pages, maximum_pages)?;
        
        Ok(Arc::new(Self {
            memory,
            refcount: AtomicUsize::new(1),
            grow_lock: Mutex::new(()),
        }))
    }
    
    pub fn grow(&self, delta_pages: u32) -> Result<u32, MemoryError> {
        let _lock = self.grow_lock.lock();
        
        // Atomically grow underlying memory
        unsafe {
            let memory = &mut *((&self.memory) as *const _ as *mut LinearMemory);
            memory.grow(delta_pages)
        }
    }
    
    /// Atomic operations for thread synchronization
    pub fn atomic_load(&self, offset: u32, width: AtomicWidth) -> Result<u64, MemoryError> {
        self.memory.bounds_check(offset, width.size())?;
        
        let ptr = unsafe { self.memory.base.add(offset as usize) };
        
        match width {
            AtomicWidth::I32 => {
                let atomic = unsafe { &*(ptr as *const AtomicU32) };
                Ok(atomic.load(Ordering::SeqCst) as u64)
            }
            AtomicWidth::I64 => {
                let atomic = unsafe { &*(ptr as *const AtomicU64) };
                Ok(atomic.load(Ordering::SeqCst))
            }
        }
    }
    
    pub fn atomic_store(&self, offset: u32, value: u64, width: AtomicWidth) -> Result<(), MemoryError> {
        self.memory.bounds_check(offset, width.size())?;
        
        let ptr = unsafe { self.memory.base.add(offset as usize) };
        
        match width {
            AtomicWidth::I32 => {
                let atomic = unsafe { &*(ptr as *const AtomicU32) };
                atomic.store(value as u32, Ordering::SeqCst);
            }
            AtomicWidth::I64 => {
                let atomic = unsafe { &*(ptr as *const AtomicU64) };
                atomic.store(value, Ordering::SeqCst);
            }
        }
        
        Ok(())
    }
    
    pub fn atomic_wait(
        &self,
        offset: u32,
        expected: u64,
        timeout: Option<Duration>,
    ) -> Result<WaitResult, MemoryError> {
        // Use kernel futex-like primitive
        kernel_syscall::futex_wait(
            unsafe { self.memory.base.add(offset as usize) },
            expected,
            timeout,
        )
    }
    
    pub fn atomic_notify(&self, offset: u32, count: u32) -> Result<u32, MemoryError> {
        kernel_syscall::futex_wake(
            unsafe { self.memory.base.add(offset as usize) },
            count,
        )
    }
}
```

---

## 8. Sandboxing and Isolation

### 8.1 Isolation Boundaries

```
+------------------------------------------------------------------+
|                    WASM INSTANCE A                                |
+------------------------------------------------------------------+
|  Linear Memory A     |  Tables A     |  Globals A                 |
|  (Isolated)          |  (Isolated)   |  (Isolated)               |
+------------------------------------------------------------------+
                              |
                              | IPC (explicit message passing)
                              |
+------------------------------------------------------------------+
|                    WASM INSTANCE B                                |
+------------------------------------------------------------------+
|  Linear Memory B     |  Tables B     |  Globals B                 |
|  (Isolated)          |  (Isolated)   |  (Isolated)               |
+------------------------------------------------------------------+
```

### 8.2 Sandbox Configuration

```rust
// runtime/src/security/sandbox.rs

pub struct SandboxConfig {
    /// Memory isolation
    pub memory: MemoryIsolation,
    
    /// System call filtering
    pub syscall_filter: SyscallFilter,
    
    /// Resource limits
    pub resource_limits: ResourceLimits,
    
    /// Capability restrictions
    pub capability_mask: CapabilityMask,
}

#[derive(Debug, Clone)]
pub struct MemoryIsolation {
    /// Enable guard pages
    pub guard_pages: bool,
    
    /// Guard page size
    pub guard_size: usize,
    
    /// Randomize memory base (ASLR)
    pub randomize_base: bool,
    
    /// Zero memory on allocation
    pub zero_on_alloc: bool,
    
    /// Zero memory on deallocation
    pub zero_on_free: bool,
}

#[derive(Debug, Clone)]
pub struct SyscallFilter {
    /// Allowed WASI calls
    pub allowed_wasi: HashSet<WasiCall>,
    
    /// Allowed custom extensions
    pub allowed_extensions: HashSet<ExtensionCall>,
    
    /// Deny by default
    pub deny_by_default: bool,
}

impl SandboxConfig {
    /// Maximum security profile (for untrusted code)
    pub fn maximum_security() -> Self {
        Self {
            memory: MemoryIsolation {
                guard_pages: true,
                guard_size: 16 * 1024 * 1024, // 16MB
                randomize_base: true,
                zero_on_alloc: true,
                zero_on_free: true,
            },
            syscall_filter: SyscallFilter {
                allowed_wasi: HashSet::from([
                    WasiCall::FdRead,
                    WasiCall::FdWrite,
                    WasiCall::ClockTimeGet,
                    WasiCall::RandomGet,
                    WasiCall::ProcExit,
                ]),
                allowed_extensions: HashSet::new(),
                deny_by_default: true,
            },
            resource_limits: ResourceLimits {
                max_memory_pages: 256, // 16MB
                max_table_elements: 1000,
                max_fuel: 1_000_000,
                max_fds: 16,
                max_ipc_channels: 0,
            },
            capability_mask: CapabilityMask::empty(),
        }
    }
    
    /// Normal security profile (for system services)
    pub fn system_service() -> Self {
        Self {
            memory: MemoryIsolation {
                guard_pages: true,
                guard_size: 2 * 1024 * 1024,
                randomize_base: true,
                zero_on_alloc: true,
                zero_on_free: false,
            },
            syscall_filter: SyscallFilter {
                allowed_wasi: WasiCall::all(),
                allowed_extensions: HashSet::from([
                    ExtensionCall::IpcCreate,
                    ExtensionCall::IpcSend,
                    ExtensionCall::IpcRecv,
                ]),
                deny_by_default: true,
            },
            resource_limits: ResourceLimits {
                max_memory_pages: 65536, // 4GB
                max_table_elements: 100000,
                max_fuel: u64::MAX,
                max_fds: 1024,
                max_ipc_channels: 256,
            },
            capability_mask: CapabilityMask::IPC,
        }
    }
}
```

### 8.3 Capability Validation

```rust
// runtime/src/security/capability.rs

pub struct CapabilityValidator {
    /// Granted capabilities
    granted: CapabilitySet,
    
    /// Capability derivation tree
    derivations: BTreeMap<CapabilityId, CapabilityId>,
}

impl CapabilityValidator {
    pub fn check(&self, required: &Capability) -> Result<(), CapabilityError> {
        if self.granted.implies(required) {
            Ok(())
        } else {
            Err(CapabilityError::Denied {
                required: required.clone(),
                granted: self.granted.clone(),
            })
        }
    }
    
    pub fn derive(
        &mut self,
        parent: CapabilityId,
        restriction: CapabilityRestriction,
    ) -> Result<CapabilityId, CapabilityError> {
        let parent_cap = self.granted.get(parent)
            .ok_or(CapabilityError::NotFound(parent))?;
        
        // Apply restriction (attenuation)
        let child_cap = parent_cap.attenuate(restriction)?;
        
        // Verify child is weaker than parent
        if !parent_cap.implies(&child_cap) {
            return Err(CapabilityError::Amplification);
        }
        
        let child_id = self.granted.insert(child_cap);
        self.derivations.insert(child_id, parent);
        
        Ok(child_id)
    }
    
    pub fn revoke(&mut self, id: CapabilityId) {
        // Revoke this capability
        self.granted.remove(id);
        
        // Revoke all derived capabilities
        let derived: Vec<_> = self.derivations.iter()
            .filter(|(_, parent)| **parent == id)
            .map(|(child, _)| *child)
            .collect();
        
        for child in derived {
            self.revoke(child);
        }
        
        self.derivations.remove(&id);
    }
}
```

---

## 9. Performance Optimizations

### 9.1 JIT Compilation Settings

```rust
// runtime/src/engine.rs

pub fn configure_jit_for_performance(config: &mut wasmtime::Config) {
    // Cranelift settings for maximum performance
    config.cranelift_opt_level(OptLevel::Speed);
    
    // Enable optimizations
    config.cranelift_flag_set("enable_simd", "true");
    config.cranelift_flag_set("enable_atomics", "true");
    
    // x86_64 specific optimizations
    #[cfg(target_arch = "x86_64")]
    {
        config.cranelift_flag_set("has_sse3", "true");
        config.cranelift_flag_set("has_ssse3", "true");
        config.cranelift_flag_set("has_sse41", "true");
        config.cranelift_flag_set("has_sse42", "true");
        config.cranelift_flag_set("has_avx", "true");
        config.cranelift_flag_set("has_avx2", "true");
        config.cranelift_flag_set("has_bmi1", "true");
        config.cranelift_flag_set("has_bmi2", "true");
        config.cranelift_flag_set("has_lzcnt", "true");
        config.cranelift_flag_set("has_popcnt", "true");
    }
    
    // Memory settings for performance
    config.static_memory_maximum_size(4 * 1024 * 1024 * 1024); // 4GB
    config.static_memory_guard_size(2 * 1024 * 1024); // 2MB
    
    // Enable parallel compilation
    config.parallel_compilation(true);
}
```

### 9.2 Instance Pooling

```rust
// runtime/src/instance.rs

pub struct InstancePool {
    /// Pool of pre-allocated instance slots
    slots: Vec<InstanceSlot>,
    
    /// Free slot indices
    free_slots: SegQueue<usize>,
    
    /// Maximum instances
    max_instances: usize,
}

struct InstanceSlot {
    /// Pre-allocated memory
    memory: Option<LinearMemory>,
    
    /// Pre-allocated tables
    tables: Vec<Table>,
    
    /// Slot state
    state: SlotState,
}

enum SlotState {
    Free,
    Allocated(InstanceId),
}

impl InstancePool {
    pub fn new(max_instances: usize, memory_pages: u32) -> Result<Self, PoolError> {
        let mut slots = Vec::with_capacity(max_instances);
        
        for i in 0..max_instances {
            let memory = LinearMemory::new(0, memory_pages)?;
            slots.push(InstanceSlot {
                memory: Some(memory),
                tables: Vec::new(),
                state: SlotState::Free,
            });
        }
        
        let free_slots = SegQueue::new();
        for i in 0..max_instances {
            free_slots.push(i);
        }
        
        Ok(Self {
            slots,
            free_slots,
            max_instances,
        })
    }
    
    pub fn allocate(&mut self) -> Result<PooledInstance, PoolError> {
        let slot_idx = self.free_slots.pop()
            .ok_or(PoolError::NoFreeSlots)?;
        
        let slot = &mut self.slots[slot_idx];
        let instance_id = InstanceId::new();
        slot.state = SlotState::Allocated(instance_id);
        
        Ok(PooledInstance {
            pool: self,
            slot_idx,
            id: instance_id,
        })
    }
    
    pub fn deallocate(&mut self, slot_idx: usize) {
        let slot = &mut self.slots[slot_idx];
        
        // Reset memory (zero without deallocating)
        if let Some(ref mut memory) = slot.memory {
            memory.reset();
        }
        
        slot.state = SlotState::Free;
        self.free_slots.push(slot_idx);
    }
}
```

### 9.3 Function Call Optimization

```rust
// runtime/src/instance.rs

impl WasmInstance {
    /// Fast path for calling exported functions without full validation
    pub unsafe fn call_unchecked<P, R>(
        &mut self,
        func: &TypedFunc<P, R>,
        params: P,
    ) -> R
    where
        P: WasmParams,
        R: WasmResults,
    {
        // Skip validation checks - only use when func is known valid
        func.call_unchecked(&mut self.store, params)
    }
    
    /// Cached function lookup
    pub fn get_cached_func<P, R>(&self, name: &str) -> Option<&TypedFunc<P, R>>
    where
        P: WasmParams,
        R: WasmResults,
    {
        self.exports.get_typed_func(name)
    }
}

/// Pre-computed export cache for fast function lookup
pub struct ExportCache {
    funcs: HashMap<String, Box<dyn Any + Send + Sync>>,
    memories: HashMap<String, Memory>,
    globals: HashMap<String, Global>,
    tables: HashMap<String, Table>,
}

impl ExportCache {
    pub fn build(instance: &Instance, store: &Store<KpioWasiCtx>) -> Result<Self, CacheError> {
        let mut cache = Self {
            funcs: HashMap::new(),
            memories: HashMap::new(),
            globals: HashMap::new(),
            tables: HashMap::new(),
        };
        
        for export in instance.exports(store) {
            match export.into_extern() {
                Extern::Func(func) => {
                    cache.funcs.insert(export.name().to_string(), Box::new(func));
                }
                Extern::Memory(mem) => {
                    cache.memories.insert(export.name().to_string(), mem);
                }
                Extern::Global(global) => {
                    cache.globals.insert(export.name().to_string(), global);
                }
                Extern::Table(table) => {
                    cache.tables.insert(export.name().to_string(), table);
                }
            }
        }
        
        Ok(cache)
    }
}
```

---

## 10. Debugging and Profiling

### 10.1 DWARF Debug Info

```rust
// runtime/src/debug.rs

pub struct DebugInfo {
    /// Source map from WASM addresses to source locations
    source_map: BTreeMap<u32, SourceLocation>,
    
    /// Function names
    function_names: HashMap<u32, String>,
    
    /// Local variable info
    locals: HashMap<u32, Vec<LocalVariable>>,
}

pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

pub struct LocalVariable {
    pub name: String,
    pub type_: ValueType,
    pub location: VariableLocation,
}

impl DebugInfo {
    pub fn parse_from_wasm(wasm_bytes: &[u8]) -> Option<Self> {
        let parser = wasmparser::Parser::new(0);
        let mut debug_info = Self::default();
        
        for payload in parser.parse_all(wasm_bytes) {
            match payload {
                Ok(Payload::CustomSection(section)) => {
                    match section.name() {
                        ".debug_info" => {
                            debug_info.parse_debug_info(section.data());
                        }
                        ".debug_line" => {
                            debug_info.parse_debug_line(section.data());
                        }
                        "name" => {
                            debug_info.parse_names(section.data());
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        
        Some(debug_info)
    }
    
    pub fn resolve_address(&self, addr: u32) -> Option<&SourceLocation> {
        self.source_map.range(..=addr).next_back().map(|(_, loc)| loc)
    }
}
```

### 10.2 Profiling Support

```rust
// runtime/src/profile.rs

pub struct Profiler {
    /// Instruction counter per function
    instruction_counts: HashMap<u32, AtomicU64>,
    
    /// Time spent per function
    function_times: HashMap<u32, AtomicU64>,
    
    /// Call counts per function
    call_counts: HashMap<u32, AtomicU64>,
    
    /// Memory allocation tracking
    memory_stats: MemoryStats,
}

pub struct MemoryStats {
    pub current_pages: AtomicU32,
    pub peak_pages: AtomicU32,
    pub grow_count: AtomicU64,
}

impl Profiler {
    pub fn new() -> Self {
        Self {
            instruction_counts: HashMap::new(),
            function_times: HashMap::new(),
            call_counts: HashMap::new(),
            memory_stats: MemoryStats::default(),
        }
    }
    
    pub fn on_function_enter(&self, func_idx: u32) {
        if let Some(count) = self.call_counts.get(&func_idx) {
            count.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    pub fn on_fuel_consumed(&self, func_idx: u32, fuel: u64) {
        if let Some(count) = self.instruction_counts.get(&func_idx) {
            count.fetch_add(fuel, Ordering::Relaxed);
        }
    }
    
    pub fn generate_report(&self) -> ProfileReport {
        let mut functions: Vec<_> = self.instruction_counts.iter()
            .map(|(idx, count)| {
                FunctionProfile {
                    index: *idx,
                    instruction_count: count.load(Ordering::Relaxed),
                    call_count: self.call_counts.get(idx)
                        .map(|c| c.load(Ordering::Relaxed))
                        .unwrap_or(0),
                }
            })
            .collect();
        
        functions.sort_by(|a, b| b.instruction_count.cmp(&a.instruction_count));
        
        ProfileReport {
            functions,
            memory: self.memory_stats.snapshot(),
        }
    }
}
```

---

## 11. Module Loading

### 11.1 Module Validator

```rust
// runtime/src/loader/validator.rs

pub struct ModuleValidator {
    /// Maximum allowed module size
    max_size: usize,
    
    /// Allowed imports
    allowed_imports: ImportFilter,
    
    /// Feature flags
    features: WasmFeatures,
}

#[derive(Debug)]
pub struct WasmFeatures {
    pub simd: bool,
    pub threads: bool,
    pub reference_types: bool,
    pub bulk_memory: bool,
    pub multi_value: bool,
    pub tail_call: bool,
    pub relaxed_simd: bool,
    pub gc: bool,
}

impl ModuleValidator {
    pub fn validate(&self, bytes: &[u8]) -> Result<ValidatedModule, ValidationError> {
        // Size check
        if bytes.len() > self.max_size {
            return Err(ValidationError::TooLarge {
                size: bytes.len(),
                max: self.max_size,
            });
        }
        
        // Parse and validate structure
        let parser = wasmparser::Validator::new_with_features(self.features.into());
        parser.validate_all(bytes)?;
        
        // Validate imports
        let mut parser = wasmparser::Parser::new(0);
        for payload in parser.parse_all(bytes) {
            if let Ok(Payload::ImportSection(imports)) = payload {
                for import in imports {
                    let import = import?;
                    self.validate_import(import.module, import.name, &import.ty)?;
                }
            }
        }
        
        Ok(ValidatedModule { bytes: bytes.to_vec() })
    }
    
    fn validate_import(
        &self,
        module: &str,
        name: &str,
        ty: &wasmparser::TypeRef,
    ) -> Result<(), ValidationError> {
        // Check if import is allowed
        if !self.allowed_imports.allows(module, name) {
            return Err(ValidationError::DisallowedImport {
                module: module.to_string(),
                name: name.to_string(),
            });
        }
        
        Ok(())
    }
}

pub struct ImportFilter {
    /// Allowed modules (e.g., "wasi_snapshot_preview1", "kpio_gpu")
    allowed_modules: HashSet<String>,
    
    /// Specific denied imports
    denied: HashSet<(String, String)>,
}

impl ImportFilter {
    pub fn allows(&self, module: &str, name: &str) -> bool {
        if self.denied.contains(&(module.to_string(), name.to_string())) {
            return false;
        }
        self.allowed_modules.contains(module)
    }
}
```

### 11.2 InitRAMFS Loading

```rust
// runtime/src/loader/initramfs.rs

pub struct InitRamFs {
    /// CPIO archive contents
    entries: BTreeMap<PathBuf, FsEntry>,
}

pub enum FsEntry {
    File {
        data: Vec<u8>,
        mode: u32,
    },
    Directory {
        mode: u32,
    },
    Symlink {
        target: PathBuf,
    },
}

impl InitRamFs {
    pub fn parse_cpio(data: &[u8]) -> Result<Self, ParseError> {
        let mut entries = BTreeMap::new();
        let mut offset = 0;
        
        while offset < data.len() {
            let (entry, path, new_offset) = parse_cpio_entry(&data[offset..])?;
            
            if path == "TRAILER!!!" {
                break;
            }
            
            entries.insert(PathBuf::from(path), entry);
            offset += new_offset;
        }
        
        Ok(Self { entries })
    }
    
    pub fn load_wasm(&self, path: &Path) -> Result<Vec<u8>, FsError> {
        match self.entries.get(path) {
            Some(FsEntry::File { data, .. }) => Ok(data.clone()),
            Some(_) => Err(FsError::NotAFile),
            None => Err(FsError::NotFound),
        }
    }
}
```

---

## 12. Inter-Module Communication

### 12.1 Component Model Support

```rust
// runtime/src/component.rs

/// Component instance for WASM component model
pub struct ComponentInstance {
    /// Core instances
    core_instances: Vec<WasmInstance>,
    
    /// Resource handles
    resources: ResourceTable,
    
    /// Interface bindings
    interfaces: HashMap<String, InterfaceBinding>,
}

pub struct InterfaceBinding {
    /// Function mappings
    functions: HashMap<String, FunctionBinding>,
    
    /// Type definitions
    types: HashMap<String, TypeDef>,
}

pub struct FunctionBinding {
    /// Source instance
    source_instance: usize,
    
    /// Source function index
    source_func: u32,
    
    /// Parameter lifting/lowering
    param_adapters: Vec<ValueAdapter>,
    
    /// Result lifting/lowering
    result_adapters: Vec<ValueAdapter>,
}

impl ComponentInstance {
    pub async fn call_export(
        &mut self,
        interface: &str,
        function: &str,
        params: &[ComponentValue],
    ) -> Result<Vec<ComponentValue>, ComponentError> {
        let binding = self.interfaces.get(interface)
            .and_then(|i| i.functions.get(function))
            .ok_or(ComponentError::NotFound)?;
        
        // Lower params to core WASM values
        let core_params = self.lower_values(params, &binding.param_adapters)?;
        
        // Call core function
        let instance = &mut self.core_instances[binding.source_instance];
        let core_results = instance.call_dynamic(binding.source_func, &core_params).await?;
        
        // Lift results to component values
        let results = self.lift_values(&core_results, &binding.result_adapters)?;
        
        Ok(results)
    }
}
```

### 12.2 Resource Sharing

```rust
// runtime/src/component.rs

pub struct ResourceTable {
    /// Active resources
    resources: HashMap<u32, Box<dyn Resource>>,
    
    /// Next resource ID
    next_id: AtomicU32,
}

pub trait Resource: Send + Sync {
    fn type_name(&self) -> &'static str;
    fn drop_resource(&mut self);
}

impl ResourceTable {
    pub fn insert<R: Resource + 'static>(&mut self, resource: R) -> u32 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.resources.insert(id, Box::new(resource));
        id
    }
    
    pub fn get<R: Resource + 'static>(&self, id: u32) -> Option<&R> {
        self.resources.get(&id)
            .and_then(|r| r.downcast_ref())
    }
    
    pub fn remove(&mut self, id: u32) -> Option<Box<dyn Resource>> {
        self.resources.remove(&id)
    }
}
```

---

## Appendix A: WASI Preview 2 Function List

| Module | Function | Status |
|--------|----------|--------|
| wasi:filesystem | stat | Implemented |
| wasi:filesystem | read | Implemented |
| wasi:filesystem | write | Implemented |
| wasi:filesystem | open-at | Implemented |
| wasi:filesystem | close | Implemented |
| wasi:clocks | monotonic-clock.now | Implemented |
| wasi:clocks | wall-clock.now | Implemented |
| wasi:random | get-random-bytes | Implemented |
| wasi:cli | get-arguments | Implemented |
| wasi:cli | get-environment | Implemented |
| wasi:cli | exit | Implemented |
| wasi:sockets | tcp.connect | Planned |
| wasi:sockets | tcp.listen | Planned |
| wasi:sockets | udp.bind | Planned |

---

## Appendix B: Custom Extension Function List

| Module | Function | Description |
|--------|----------|-------------|
| kpio_gpu | request_adapter | Get GPU adapter |
| kpio_gpu | adapter_request_device | Create GPU device |
| kpio_gpu | device_create_buffer | Create GPU buffer |
| kpio_gpu | queue_submit | Submit GPU commands |
| kpio_ipc | channel_create | Create IPC channel |
| kpio_ipc | channel_send | Send IPC message |
| kpio_ipc | channel_recv | Receive IPC message |
| kpio_cap | has_capability | Check capability |
| kpio_cap | derive_capability | Derive child capability |
| kpio_proc | spawn | Spawn WASM process |
| kpio_proc | wait | Wait for process exit |
