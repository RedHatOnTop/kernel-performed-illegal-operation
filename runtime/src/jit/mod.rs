//! JIT Compiler for WebAssembly
//!
//! This module implements a baseline JIT compiler for WASM bytecode,
//! providing significant performance improvements over interpretation.
//!
//! # Architecture
//!
//! The JIT compiler operates in multiple tiers:
//!
//! 1. **Interpreter** - Initial execution, collecting profiling data
//! 2. **Baseline JIT** - Quick compilation for warm functions
//! 3. **Optimizing JIT** - Full optimization for hot functions (future)
//!
//! # Design Decisions
//!
//! - Uses Cranelift as the code generator backend
//! - Function-level compilation granularity
//! - Profile-guided tiering decisions
//! - AOT compilation support for system services

pub mod compiler;
pub mod codegen;
pub mod cache;
pub mod profile;
pub mod ir;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::sync::Arc;
use spin::RwLock;
use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};

pub use compiler::{JitCompiler, CompilationResult, CompilationError};
pub use codegen::{CodeGenerator, NativeCode};
pub use cache::{CodeCache, CacheEntry};
pub use profile::{ProfileData, HotnessCounter};

/// JIT compilation tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompilationTier {
    /// Interpreted execution (tier 0).
    Interpreter = 0,
    /// Baseline JIT with minimal optimization (tier 1).
    Baseline = 1,
    /// Optimized JIT with full optimization (tier 2).
    Optimized = 2,
}

/// JIT compilation options.
#[derive(Debug, Clone)]
pub struct JitOptions {
    /// Enable baseline JIT compilation.
    pub baseline_enabled: bool,
    /// Enable optimizing JIT compilation.
    pub optimized_enabled: bool,
    /// Threshold for baseline compilation (call count).
    pub baseline_threshold: u32,
    /// Threshold for optimizing compilation (call count).
    pub optimized_threshold: u32,
    /// Maximum code cache size in bytes.
    pub max_cache_size: usize,
    /// Enable AOT compilation.
    pub aot_enabled: bool,
    /// Enable on-stack replacement (OSR).
    pub osr_enabled: bool,
}

impl Default for JitOptions {
    fn default() -> Self {
        Self {
            baseline_enabled: true,
            optimized_enabled: true,
            baseline_threshold: 100,      // Compile after 100 calls
            optimized_threshold: 10_000,  // Optimize after 10k calls
            max_cache_size: 64 * 1024 * 1024, // 64 MB code cache
            aot_enabled: true,
            osr_enabled: false, // OSR is complex, disabled by default
        }
    }
}

/// Statistics for JIT compilation.
#[derive(Debug, Default)]
pub struct JitStats {
    /// Number of functions compiled at baseline tier.
    pub baseline_compilations: AtomicU64,
    /// Number of functions compiled at optimized tier.
    pub optimized_compilations: AtomicU64,
    /// Number of functions deoptimized.
    pub deoptimizations: AtomicU64,
    /// Total compilation time in microseconds.
    pub compilation_time_us: AtomicU64,
    /// Total bytes of generated code.
    pub generated_code_bytes: AtomicU64,
    /// Cache hits.
    pub cache_hits: AtomicU64,
    /// Cache misses.
    pub cache_misses: AtomicU64,
}

impl JitStats {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn record_baseline_compilation(&self, time_us: u64, code_size: usize) {
        self.baseline_compilations.fetch_add(1, Ordering::Relaxed);
        self.compilation_time_us.fetch_add(time_us, Ordering::Relaxed);
        self.generated_code_bytes.fetch_add(code_size as u64, Ordering::Relaxed);
    }
    
    pub fn record_optimized_compilation(&self, time_us: u64, code_size: usize) {
        self.optimized_compilations.fetch_add(1, Ordering::Relaxed);
        self.compilation_time_us.fetch_add(time_us, Ordering::Relaxed);
        self.generated_code_bytes.fetch_add(code_size as u64, Ordering::Relaxed);
    }
}

/// JIT engine managing compilation and execution.
pub struct JitEngine {
    /// Compilation options.
    options: JitOptions,
    /// Code cache.
    code_cache: Arc<RwLock<CodeCache>>,
    /// Profile data per function.
    profiles: RwLock<BTreeMap<FunctionId, ProfileData>>,
    /// JIT statistics.
    stats: JitStats,
    /// Compiler instance.
    compiler: JitCompiler,
}

/// Function identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunctionId {
    /// Module ID.
    pub module_id: u64,
    /// Function index within module.
    pub func_index: u32,
}

impl FunctionId {
    pub fn new(module_id: u64, func_index: u32) -> Self {
        Self { module_id, func_index }
    }
}

impl JitEngine {
    /// Create a new JIT engine with default options.
    pub fn new() -> Self {
        Self::with_options(JitOptions::default())
    }
    
    /// Create a new JIT engine with custom options.
    pub fn with_options(options: JitOptions) -> Self {
        Self {
            code_cache: Arc::new(RwLock::new(CodeCache::new(options.max_cache_size))),
            profiles: RwLock::new(BTreeMap::new()),
            stats: JitStats::new(),
            compiler: JitCompiler::new(),
            options,
        }
    }
    
    /// Get or compile code for a function.
    pub fn get_or_compile(
        &self,
        func_id: FunctionId,
        wasm_bytes: &[u8],
    ) -> Result<Arc<NativeCode>, CompilationError> {
        // Check cache first
        {
            let cache = self.code_cache.read();
            if let Some(entry) = cache.get(&func_id) {
                self.stats.cache_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(entry.code.clone());
            }
        }
        
        self.stats.cache_misses.fetch_add(1, Ordering::Relaxed);
        
        // Update profile and check if we should compile
        let tier = self.update_profile_and_get_tier(func_id);
        
        if tier == CompilationTier::Interpreter {
            // Still in interpreter tier, don't compile yet
            return Err(CompilationError::BelowThreshold);
        }
        
        // Compile the function
        let result = self.compile_function(func_id, wasm_bytes, tier)?;
        
        // Store in cache
        {
            let mut cache = self.code_cache.write();
            cache.insert(func_id, CacheEntry {
                code: result.clone(),
                tier,
                compilation_time_us: 0, // TODO: measure actual time
            });
        }
        
        Ok(result)
    }
    
    /// Compile a function at the specified tier.
    fn compile_function(
        &self,
        func_id: FunctionId,
        wasm_bytes: &[u8],
        tier: CompilationTier,
    ) -> Result<Arc<NativeCode>, CompilationError> {
        match tier {
            CompilationTier::Interpreter => {
                Err(CompilationError::BelowThreshold)
            }
            CompilationTier::Baseline => {
                let code = self.compiler.compile_baseline(func_id, wasm_bytes)?;
                self.stats.record_baseline_compilation(0, code.size());
                Ok(Arc::new(code))
            }
            CompilationTier::Optimized => {
                let profile = self.profiles.read().get(&func_id).cloned();
                let code = self.compiler.compile_optimized(func_id, wasm_bytes, profile.as_ref())?;
                self.stats.record_optimized_compilation(0, code.size());
                Ok(Arc::new(code))
            }
        }
    }
    
    /// Update profile data and determine compilation tier.
    fn update_profile_and_get_tier(&self, func_id: FunctionId) -> CompilationTier {
        let mut profiles = self.profiles.write();
        let profile = profiles.entry(func_id).or_insert_with(ProfileData::new);
        
        profile.call_count += 1;
        
        if profile.call_count >= self.options.optimized_threshold {
            CompilationTier::Optimized
        } else if profile.call_count >= self.options.baseline_threshold {
            CompilationTier::Baseline
        } else {
            CompilationTier::Interpreter
        }
    }
    
    /// AOT compile an entire module.
    pub fn aot_compile(
        &self,
        module_id: u64,
        wasm_bytes: &[u8],
    ) -> Result<Vec<(u32, Arc<NativeCode>)>, CompilationError> {
        if !self.options.aot_enabled {
            return Err(CompilationError::AotDisabled);
        }
        
        self.compiler.compile_module(module_id, wasm_bytes)
    }
    
    /// Invalidate cached code for a function.
    pub fn invalidate(&self, func_id: FunctionId) {
        let mut cache = self.code_cache.write();
        cache.remove(&func_id);
    }
    
    /// Get JIT statistics.
    pub fn stats(&self) -> &JitStats {
        &self.stats
    }
    
    /// Get cache statistics.
    pub fn cache_stats(&self) -> CacheStats {
        let cache = self.code_cache.read();
        CacheStats {
            entries: cache.len(),
            total_size: cache.total_size(),
            max_size: self.options.max_cache_size,
        }
    }
}

/// Cache statistics.
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub total_size: usize,
    pub max_size: usize,
}

impl Default for JitEngine {
    fn default() -> Self {
        Self::new()
    }
}
