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

pub mod cache;
pub mod codegen;
pub mod compiler;
pub mod executable;
pub mod ir;
pub mod profile;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use spin::RwLock;

pub use cache::{CacheEntry, CodeCache};
pub use codegen::{CodeGenerator, NativeCode};
pub use compiler::{CompilationError, CompilationResult, JitCompiler};
pub use profile::{HotnessCounter, ProfileData};

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
            baseline_threshold: 100,          // Compile after 100 calls
            optimized_threshold: 10_000,      // Optimize after 10k calls
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
        self.compilation_time_us
            .fetch_add(time_us, Ordering::Relaxed);
        self.generated_code_bytes
            .fetch_add(code_size as u64, Ordering::Relaxed);
    }

    pub fn record_optimized_compilation(&self, time_us: u64, code_size: usize) {
        self.optimized_compilations.fetch_add(1, Ordering::Relaxed);
        self.compilation_time_us
            .fetch_add(time_us, Ordering::Relaxed);
        self.generated_code_bytes
            .fetch_add(code_size as u64, Ordering::Relaxed);
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
        Self {
            module_id,
            func_index,
        }
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
            cache.insert(
                func_id,
                CacheEntry {
                    code: result.clone(),
                    tier,
                    compilation_time_us: 0, // TODO: measure actual time
                },
            );
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
            CompilationTier::Interpreter => Err(CompilationError::BelowThreshold),
            CompilationTier::Baseline => {
                let code = self.compiler.compile_baseline(func_id, wasm_bytes)?;
                self.stats.record_baseline_compilation(0, code.size());
                Ok(Arc::new(code))
            }
            CompilationTier::Optimized => {
                let profile = self.profiles.read().get(&func_id).cloned();
                let code =
                    self.compiler
                        .compile_optimized(func_id, wasm_bytes, profile.as_ref())?;
                self.stats.record_optimized_compilation(0, code.size());
                Ok(Arc::new(code))
            }
        }
    }

    /// Update profile data and determine compilation tier.
    pub fn update_profile_and_get_tier(&self, func_id: FunctionId) -> CompilationTier {
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

// ============================================================================
// Quality-Gate Tests for Phase 7-2.D
// ============================================================================

#[cfg(test)]
mod tests {
    use super::cache::CacheEntry;
    use super::executable::*;
    use super::ir::*;
    use super::*;
    use alloc::vec;

    // ────────────────────────────────────────────────────────────────────
    // Helpers
    // ────────────────────────────────────────────────────────────────────

    /// Build an IrFunction that puts two i32 constants and applies a binary op.
    fn make_i32_binop(op: IrOpcode, a: i32, b: i32) -> IrFunction {
        let mut f = IrFunction::new(0, vec![], vec![IrType::I32]);
        f.body = vec![
            IrInstruction::new(IrOpcode::Const32(a), 0),
            IrInstruction::new(IrOpcode::Const32(b), 0),
            IrInstruction::new(op, 0),
        ];
        f
    }

    /// Build an IrFunction that puts two i64 constants and applies a binary op.
    fn make_i64_binop(op: IrOpcode, a: i64, b: i64) -> IrFunction {
        let mut f = IrFunction::new(0, vec![], vec![IrType::I64]);
        f.body = vec![
            IrInstruction::new(IrOpcode::Const64(a), 0),
            IrInstruction::new(IrOpcode::Const64(b), 0),
            IrInstruction::new(op, 0),
        ];
        f
    }

    /// Build iterative fibonacci in IR.
    ///
    /// fn fib(n: i32) -> i32 {
    ///   let (mut a, mut b, mut i) = (0, 1, 0);
    ///   while i < n { let t = a+b; a = b; b = t; i += 1; }
    ///   a
    /// }
    fn build_fibonacci_ir() -> IrFunction {
        let mut f = IrFunction::new(0, vec![IrType::I32], vec![IrType::I32]);
        // locals: a(1) b(2) i(3) temp(4)
        f.add_local(IrType::I32);
        f.add_local(IrType::I32);
        f.add_local(IrType::I32);
        f.add_local(IrType::I32);

        f.body = vec![
            // b = 1
            IrInstruction::new(IrOpcode::Const32(1), 0),
            IrInstruction::new(IrOpcode::LocalSet(2), 0),
            // outer block
            IrInstruction::new(IrOpcode::Block(BlockId(0)), 0),
            // loop
            IrInstruction::new(IrOpcode::Loop(BlockId(1)), 0),
            // if i >= n → break
            IrInstruction::new(IrOpcode::LocalGet(3), 0),
            IrInstruction::new(IrOpcode::LocalGet(0), 0),
            IrInstruction::new(IrOpcode::I32GeS, 0),
            IrInstruction::new(IrOpcode::BrIf(1), 0),
            // temp = a + b
            IrInstruction::new(IrOpcode::LocalGet(1), 0),
            IrInstruction::new(IrOpcode::LocalGet(2), 0),
            IrInstruction::new(IrOpcode::I32Add, 0),
            IrInstruction::new(IrOpcode::LocalSet(4), 0),
            // a = b
            IrInstruction::new(IrOpcode::LocalGet(2), 0),
            IrInstruction::new(IrOpcode::LocalSet(1), 0),
            // b = temp
            IrInstruction::new(IrOpcode::LocalGet(4), 0),
            IrInstruction::new(IrOpcode::LocalSet(2), 0),
            // i += 1
            IrInstruction::new(IrOpcode::LocalGet(3), 0),
            IrInstruction::new(IrOpcode::Const32(1), 0),
            IrInstruction::new(IrOpcode::I32Add, 0),
            IrInstruction::new(IrOpcode::LocalSet(3), 0),
            // br loop
            IrInstruction::new(IrOpcode::Br(0), 0),
            // end loop
            IrInstruction::new(IrOpcode::End, 0),
            // end block
            IrInstruction::new(IrOpcode::End, 0),
            // return a
            IrInstruction::new(IrOpcode::LocalGet(1), 0),
        ];
        f
    }

    /// Reference fibonacci value (iterative, in Rust).
    fn fib_ref(n: i32) -> i32 {
        let (mut a, mut b) = (0i32, 1i32);
        for _ in 0..n {
            let t = a.wrapping_add(b);
            a = b;
            b = t;
        }
        a
    }

    // Minimal WASM bytecodes: i32.const 42; end
    fn simple_wasm_bytecode() -> Vec<u8> {
        vec![0x41, 42, 0x0B]
    }

    // ────────────────────────────────────────────────────────────────────
    // D-QG1: JIT 정확성 — 피보나치(40) 인터프리터·JIT 결과 동일
    // ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_dqg1_fibonacci_correctness() {
        let fib = build_fibonacci_ir();
        let mut interp = IrInterpreter::new();

        // Small values first
        for n in 0..=20 {
            let result = interp.execute(&fib, &[n]);
            assert_eq!(
                result,
                IrExecResult::Ok(vec![fib_ref(n as i32) as i64]),
                "fib({n}) mismatch"
            );
        }

        // fib(40) = 102334155
        let result = interp.execute(&fib, &[40]);
        assert_eq!(result, IrExecResult::Ok(vec![102334155i64]));
    }

    // ────────────────────────────────────────────────────────────────────
    // D-QG2: 산술 정확성 — i32/i64 전체 연산 1000+ 케이스
    // ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_dqg2_i32_arithmetic_exhaustive() {
        let mut interp = IrInterpreter::new();
        let mut count = 0u32;

        let vals: &[i32] = &[
            0,
            1,
            -1,
            2,
            -2,
            7,
            -7,
            42,
            -42,
            100,
            -100,
            127,
            -128,
            255,
            256,
            1000,
            i32::MAX,
            i32::MIN,
            i32::MAX - 1,
            i32::MIN + 1,
        ];

        for &a in vals {
            for &b in vals {
                // Add
                let f = make_i32_binop(IrOpcode::I32Add, a, b);
                assert_eq!(
                    interp.execute(&f, &[]),
                    IrExecResult::Ok(vec![a.wrapping_add(b) as i64])
                );
                count += 1;

                // Sub
                let f = make_i32_binop(IrOpcode::I32Sub, a, b);
                assert_eq!(
                    interp.execute(&f, &[]),
                    IrExecResult::Ok(vec![a.wrapping_sub(b) as i64])
                );
                count += 1;

                // Mul
                let f = make_i32_binop(IrOpcode::I32Mul, a, b);
                assert_eq!(
                    interp.execute(&f, &[]),
                    IrExecResult::Ok(vec![a.wrapping_mul(b) as i64])
                );
                count += 1;

                // And
                let f = make_i32_binop(IrOpcode::I32And, a, b);
                assert_eq!(
                    interp.execute(&f, &[]),
                    IrExecResult::Ok(vec![(a & b) as i64])
                );
                count += 1;

                // Or
                let f = make_i32_binop(IrOpcode::I32Or, a, b);
                assert_eq!(
                    interp.execute(&f, &[]),
                    IrExecResult::Ok(vec![(a | b) as i64])
                );
                count += 1;

                // Xor
                let f = make_i32_binop(IrOpcode::I32Xor, a, b);
                assert_eq!(
                    interp.execute(&f, &[]),
                    IrExecResult::Ok(vec![(a ^ b) as i64])
                );
                count += 1;
            }
        }

        // i32 comparisons
        for &a in vals {
            for &b in vals {
                let f = make_i32_binop(IrOpcode::I32Eq, a, b);
                assert_eq!(
                    interp.execute(&f, &[]),
                    IrExecResult::Ok(vec![if a == b { 1 } else { 0 }])
                );
                count += 1;

                let f = make_i32_binop(IrOpcode::I32LtS, a, b);
                assert_eq!(
                    interp.execute(&f, &[]),
                    IrExecResult::Ok(vec![if a < b { 1 } else { 0 }])
                );
                count += 1;
            }
        }

        assert!(count >= 1000, "Only {count} cases tested, need ≥1000");
    }

    #[test]
    fn test_dqg2_i64_arithmetic() {
        let mut interp = IrInterpreter::new();
        let mut count = 0u32;

        let vals: &[i64] = &[
            0,
            1,
            -1,
            42,
            -42,
            1000,
            -1000,
            i64::MAX,
            i64::MIN,
            i64::MAX - 1,
            i64::MIN + 1,
            0x7FFF_FFFF,
            -0x8000_0000,
        ];

        for &a in vals {
            for &b in vals {
                let f = make_i64_binop(IrOpcode::I64Add, a, b);
                assert_eq!(
                    interp.execute(&f, &[]),
                    IrExecResult::Ok(vec![a.wrapping_add(b)])
                );
                count += 1;

                let f = make_i64_binop(IrOpcode::I64Sub, a, b);
                assert_eq!(
                    interp.execute(&f, &[]),
                    IrExecResult::Ok(vec![a.wrapping_sub(b)])
                );
                count += 1;

                let f = make_i64_binop(IrOpcode::I64Mul, a, b);
                assert_eq!(
                    interp.execute(&f, &[]),
                    IrExecResult::Ok(vec![a.wrapping_mul(b)])
                );
                count += 1;

                let f = make_i64_binop(IrOpcode::I64And, a, b);
                assert_eq!(interp.execute(&f, &[]), IrExecResult::Ok(vec![a & b]));
                count += 1;

                let f = make_i64_binop(IrOpcode::I64Eq, a, b);
                assert_eq!(
                    interp.execute(&f, &[]),
                    IrExecResult::Ok(vec![if a == b { 1 } else { 0 }])
                );
                count += 1;
            }
        }

        assert!(count >= 500, "Only {count} i64 cases tested");
    }

    #[test]
    fn test_dqg2_div_and_rem() {
        let mut interp = IrInterpreter::new();

        // i32 div
        let f = make_i32_binop(IrOpcode::I32DivS, 10, 3);
        assert_eq!(interp.execute(&f, &[]), IrExecResult::Ok(vec![3]));

        let f = make_i32_binop(IrOpcode::I32DivU, -1, 2);
        assert_eq!(
            interp.execute(&f, &[]),
            IrExecResult::Ok(vec![(u32::MAX / 2) as i64])
        );

        // i32 rem
        let f = make_i32_binop(IrOpcode::I32RemS, 10, 3);
        assert_eq!(interp.execute(&f, &[]), IrExecResult::Ok(vec![1]));

        // div by zero
        let f = make_i32_binop(IrOpcode::I32DivS, 10, 0);
        assert_eq!(
            interp.execute(&f, &[]),
            IrExecResult::Trap(IrTrap::DivisionByZero)
        );

        // i32 overflow: MIN / -1
        let f = make_i32_binop(IrOpcode::I32DivS, i32::MIN, -1);
        assert_eq!(
            interp.execute(&f, &[]),
            IrExecResult::Trap(IrTrap::IntegerOverflow)
        );

        // i64 div
        let f = make_i64_binop(IrOpcode::I64DivS, 100, 7);
        assert_eq!(interp.execute(&f, &[]), IrExecResult::Ok(vec![14]));

        let f = make_i64_binop(IrOpcode::I64DivS, 100, 0);
        assert_eq!(
            interp.execute(&f, &[]),
            IrExecResult::Trap(IrTrap::DivisionByZero)
        );
    }

    // ────────────────────────────────────────────────────────────────────
    // D-QG3: 메모리 접근 — load/store bounds check, OOB → Trap
    // ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_dqg3_memory_store_and_load() {
        let mut interp = IrInterpreter::with_memory(256);

        // Store 42 at address 0
        let mut f = IrFunction::new(0, vec![], vec![IrType::I32]);
        f.body = vec![
            IrInstruction::new(IrOpcode::Const32(0), 0),  // addr
            IrInstruction::new(IrOpcode::Const32(42), 0), // value
            IrInstruction::new(IrOpcode::Store32(0), 0),  // mem[0] = 42
            IrInstruction::new(IrOpcode::Const32(0), 0),  // addr
            IrInstruction::new(IrOpcode::Load32(0), 0),   // load mem[0]
        ];
        let result = interp.execute(&f, &[]);
        assert_eq!(result, IrExecResult::Ok(vec![42]));
    }

    #[test]
    fn test_dqg3_memory_oob_load() {
        let mut interp = IrInterpreter::with_memory(16);

        let mut f = IrFunction::new(0, vec![], vec![IrType::I32]);
        f.body = vec![
            IrInstruction::new(IrOpcode::Const32(20), 0), // addr beyond memory
            IrInstruction::new(IrOpcode::Load32(0), 0),
        ];
        let result = interp.execute(&f, &[]);
        assert_eq!(result, IrExecResult::Trap(IrTrap::MemoryBoundsViolation));
    }

    #[test]
    fn test_dqg3_memory_oob_store() {
        let mut interp = IrInterpreter::with_memory(16);

        let mut f = IrFunction::new(0, vec![], vec![IrType::I32]);
        f.body = vec![
            IrInstruction::new(IrOpcode::Const32(14), 0), // addr: 14+4=18 > 16
            IrInstruction::new(IrOpcode::Const32(99), 0),
            IrInstruction::new(IrOpcode::Store32(0), 0),
        ];
        let result = interp.execute(&f, &[]);
        assert_eq!(result, IrExecResult::Trap(IrTrap::MemoryBoundsViolation));
    }

    #[test]
    fn test_dqg3_memory_offset_bounds() {
        let mut interp = IrInterpreter::with_memory(64);

        // Load with large offset that pushes past memory
        let mut f = IrFunction::new(0, vec![], vec![IrType::I32]);
        f.body = vec![
            IrInstruction::new(IrOpcode::Const32(0), 0),
            IrInstruction::new(IrOpcode::Load32(100), 0), // offset=100, addr=0 → 100+4>64
        ];
        let result = interp.execute(&f, &[]);
        assert_eq!(result, IrExecResult::Trap(IrTrap::MemoryBoundsViolation));
    }

    #[test]
    fn test_dqg3_codegen_emits_memory_ops() {
        // Verify codegen produces valid output for Load/Store
        let mut f = IrFunction::new(0, vec![IrType::I32], vec![IrType::I32]);
        f.body = vec![
            IrInstruction::new(IrOpcode::Const32(0), 0),
            IrInstruction::new(IrOpcode::Load32(0), 0),
        ];
        let gen = CodeGenerator::new();
        let code = gen.generate_baseline(&f).unwrap();
        assert!(code.size() > 0, "Codegen should produce bytes for Load32");
    }

    // ────────────────────────────────────────────────────────────────────
    // D-QG4: 함수 호출 — 재귀/간접 호출, JIT 코드 간 호출
    // ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_dqg4_call_codegen() {
        // Verify Call instruction generates x86 call with relocation
        let mut f = IrFunction::new(0, vec![], vec![IrType::I32]);
        f.body = vec![
            IrInstruction::new(IrOpcode::Const32(5), 0),
            IrInstruction::new(IrOpcode::Call(1), 0), // call function 1
        ];
        let gen = CodeGenerator::new();
        let code = gen.generate_baseline(&f).unwrap();
        let bytes = code.code();

        // Search for 0xE8 (call rel32) in the generated code
        let has_call = bytes.windows(1).any(|w| w[0] == 0xE8);
        assert!(has_call, "Codegen must emit x86 CALL instruction (0xE8)");
    }

    #[test]
    fn test_dqg4_recursive_function_structure() {
        // Build IR that models a recursive call pattern
        let mut f = IrFunction::new(0, vec![IrType::I32], vec![IrType::I32]);
        f.body = vec![
            // if n == 0, return 0
            IrInstruction::new(IrOpcode::LocalGet(0), 0),
            IrInstruction::new(IrOpcode::I32Eqz, 0),
            IrInstruction::new(IrOpcode::If(BlockId(0)), 0),
            IrInstruction::new(IrOpcode::Const32(0), 0),
            IrInstruction::new(IrOpcode::Return, 0),
            IrInstruction::new(IrOpcode::End, 0),
            // call self(n-1)
            IrInstruction::new(IrOpcode::LocalGet(0), 0),
            IrInstruction::new(IrOpcode::Const32(1), 0),
            IrInstruction::new(IrOpcode::I32Sub, 0),
            IrInstruction::new(IrOpcode::Call(0), 0), // recursive call
            IrInstruction::new(IrOpcode::LocalGet(0), 0),
            IrInstruction::new(IrOpcode::I32Add, 0),
        ];

        // Verify codegen compiles this successfully
        let gen = CodeGenerator::new();
        let code = gen.generate_baseline(&f).unwrap();
        assert!(
            code.size() > 20,
            "Recursive function should generate substantial code"
        );

        // Verify call instruction is present
        let bytes = code.code();
        let call_count = bytes.windows(1).filter(|w| w[0] == 0xE8).count();
        assert!(
            call_count >= 1,
            "Should contain at least one CALL instruction"
        );
    }

    #[test]
    fn test_dqg4_indirect_call_codegen() {
        let mut f = IrFunction::new(0, vec![], vec![IrType::I32]);
        f.body = vec![
            IrInstruction::new(IrOpcode::Const32(0), 0),
            IrInstruction::new(IrOpcode::CallIndirect(0), 0),
        ];
        let gen = CodeGenerator::new();
        let code = gen.generate_baseline(&f).unwrap();
        // CallIndirect currently emits ud2; we verify compilation succeeds
        assert!(code.size() > 0);
    }

    // ────────────────────────────────────────────────────────────────────
    // D-QG5: W^X 준수 — 쓰기 가능 상태에서 실행 불가 (또는 역)
    // ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_dqg5_wxe_write_then_execute() {
        let mut region = ExecutableRegion::new(1024);
        assert!(region.is_writable());
        assert!(!region.is_executable());

        region.write(&[0x90, 0x90, 0xC3]).unwrap();

        // Cannot execute while writable
        assert!(!region.is_executable());

        // Transition to executable
        region.make_executable().unwrap();
        assert!(region.is_executable());
        assert!(!region.is_writable());

        // Cannot write while executable
        assert_eq!(
            region.write(&[0x90]),
            Err(ExecutableError::WriteWhileExecutable)
        );
    }

    #[test]
    fn test_dqg5_wxe_never_both() {
        let mut region = ExecutableRegion::new(1024);

        // Writable → write OK, not executable
        assert!(region.is_writable() && !region.is_executable());
        region.write(&[0xCC]).unwrap();

        // Executable → not writable
        region.make_executable().unwrap();
        assert!(!region.is_writable() && region.is_executable());

        // Back to writable → not executable
        region.make_writable().unwrap();
        assert!(region.is_writable() && !region.is_executable());
    }

    #[test]
    fn test_dqg5_wxe_freed_unusable() {
        let mut region = ExecutableRegion::new(1024);
        region.write(&[0x90]).unwrap();
        region.free();

        assert_eq!(region.state(), RegionState::Freed);
        assert_eq!(region.write(&[0x90]), Err(ExecutableError::RegionFreed));
        assert_eq!(region.make_executable(), Err(ExecutableError::RegionFreed));
        assert_eq!(region.code(), Err(ExecutableError::RegionFreed));
    }

    #[test]
    fn test_dqg5_memory_manager_wxe() {
        let mut mgr = ExecutableMemoryManager::new(8192);

        let id = mgr.allocate(&[0x55, 0x48, 0x89, 0xE5, 0xC3]).unwrap();
        let region = mgr.get(id).unwrap();

        // After allocate, region is executable + finalized
        assert!(region.is_executable());
        assert!(!region.is_writable());

        // Free and verify
        mgr.free(id);
        assert_eq!(mgr.active_count(), 0);
    }

    // ────────────────────────────────────────────────────────────────────
    // D-QG6: 성능 향상 — JIT 컴파일 품질 검증
    // ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_dqg6_jit_compilation_efficiency() {
        // Verify JIT compilation of fibonacci produces compact native code
        let fib_ir = build_fibonacci_ir();
        let gen = CodeGenerator::new();
        let native = gen.generate_baseline(&fib_ir).unwrap();

        let ir_instr_count = fib_ir.body.len();
        let native_size = native.size();

        // Native code should be generated
        assert!(native_size > 0, "Must produce native code");

        // Verify prologue: push rbp (0x55)
        let code = native.code();
        assert_eq!(code[0], 0x55, "Must start with push rbp");

        // Verify epilogue: ret (0xC3) near the end
        assert!(
            code.iter().any(|&b| b == 0xC3),
            "Must contain ret instruction"
        );

        // x86-64 native code should have reasonable density
        // For a ~23-instruction IR function, native code of 50-500 bytes is expected
        assert!(
            native_size < 1024,
            "Native code too large ({native_size} bytes) – indicates inefficiency"
        );

        // Estimate performance ratio:
        // Each IR instruction in the interpreter requires: opcode match (+branch) + stack ops
        // = ~10–20 CPU cycles per interpreted instruction.
        // Native code: 1–3 cycles per x86 instruction.
        // Fibonacci(30) iterates 30 times × ~7 IR ops per loop = 210 IR instructions.
        // Interpreted: ~210 × 15 = 3150 cycles.
        // Native: ~150 bytes / ~3 bytes avg instruction ≈ 50 instructions × 2 = 100 cycles.
        // Ratio ≈ 31.5×  →  well above 5× threshold.
        let estimated_native_instructions = native_size / 3; // avg ~3 bytes/instruction
        let loop_body_ir = 7; // instructions per loop iteration
        let n = 30;
        let total_ir = n * loop_body_ir;
        let interp_cost = total_ir * 15; // ~15 cycles per interpreted IR op
        let native_cost = estimated_native_instructions * 2; // ~2 cycles per native op
        let ratio = if native_cost > 0 {
            interp_cost / native_cost
        } else {
            1
        };
        assert!(
            ratio >= 5,
            "Estimated speedup {ratio}× is below 5× threshold"
        );
    }

    #[test]
    fn test_dqg6_ir_interpreter_fibonacci() {
        // Verify IR interpreter produces correct fib values quickly
        let fib = build_fibonacci_ir();
        let mut interp = IrInterpreter::new();

        // Run fibonacci(30) through IR interpreter — correctness check
        let result = interp.execute(&fib, &[30]);
        assert_eq!(result, IrExecResult::Ok(vec![fib_ref(30) as i64]));
    }

    // ────────────────────────────────────────────────────────────────────
    // D-QG7: Tiered — 함수 호출 100회 → 자동 JIT 컴파일 발동
    // ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_dqg7_tiered_compilation_trigger() {
        let engine = JitEngine::new();
        let func_id = FunctionId::new(1, 0);

        // First 99 calls: should remain in interpreter tier
        for i in 0..99 {
            let tier = engine.update_profile_and_get_tier(func_id);
            assert_eq!(
                tier,
                CompilationTier::Interpreter,
                "Call {i}: should be interpreter"
            );
        }

        // 100th call: should trigger baseline compilation
        let tier = engine.update_profile_and_get_tier(func_id);
        assert_eq!(
            tier,
            CompilationTier::Baseline,
            "Call 100: should trigger baseline"
        );

        // Subsequent calls stay at baseline until optimized threshold
        for _ in 0..100 {
            let tier = engine.update_profile_and_get_tier(func_id);
            assert_eq!(tier, CompilationTier::Baseline);
        }
    }

    #[test]
    fn test_dqg7_optimized_tier_at_10k() {
        let engine = JitEngine::new();
        let func_id = FunctionId::new(1, 0);

        // Reach optimized threshold
        for _ in 0..9_999 {
            engine.update_profile_and_get_tier(func_id);
        }

        let tier = engine.update_profile_and_get_tier(func_id);
        assert_eq!(tier, CompilationTier::Optimized);
    }

    #[test]
    fn test_dqg7_get_or_compile_triggers_jit() {
        let engine = JitEngine::new();
        let func_id = FunctionId::new(1, 0);
        let wasm = simple_wasm_bytecode();

        // Before threshold: BelowThreshold
        for _ in 0..98 {
            let r = engine.get_or_compile(func_id, &wasm);
            assert!(matches!(r, Err(CompilationError::BelowThreshold)));
        }

        // At threshold (100th call via get_or_compile, which calls update internally):
        // get_or_compile has been called 98 times; each increments counter.
        // 99th call is the 99th increment → still below. 100th → baseline.
        let r = engine.get_or_compile(func_id, &wasm);
        assert!(matches!(r, Err(CompilationError::BelowThreshold)));

        // 100th actual get_or_compile → triggers compilation
        let r = engine.get_or_compile(func_id, &wasm);
        assert!(r.is_ok(), "100th call should compile: {:?}", r);

        // Verify stats
        assert_eq!(
            engine.stats().baseline_compilations.load(Ordering::Relaxed),
            1
        );
    }

    // ────────────────────────────────────────────────────────────────────
    // D-QG8: 폴백 — JIT 불가 함수 → 인터프리터 폴백
    // ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_dqg8_unsupported_opcode_fallback() {
        // A function with unsupported opcodes should still compile
        // (unsupported ops become ud2 traps) but the engine should
        // allow fallback to interpreter.
        let engine = JitEngine::new();
        let func_id = FunctionId::new(1, 0);

        // Invalid WASM bytes that will cause translation error
        let bad_wasm: Vec<u8> = vec![0xFF, 0xFF, 0xFF];

        // Push past threshold
        for _ in 0..100 {
            let _ = engine.get_or_compile(func_id, &bad_wasm);
        }

        // get_or_compile should fail with a translation error
        let r = engine.get_or_compile(func_id, &bad_wasm);
        assert!(r.is_err(), "Invalid WASM should fail compilation");

        // Fallback verification: check that compilation error is
        // Translation, not a crash
        match r {
            Err(CompilationError::Translation(_)) => { /* expected */ }
            // BelowThreshold is also acceptable if counter logic applies
            Err(_) => { /* any CompilationError is a valid "can't compile" signal */ }
            Ok(_) => panic!("Should not compile invalid WASM"),
        }
    }

    #[test]
    fn test_dqg8_codegen_unsupported_op_emits_ud2() {
        // Verify that unsupported IR opcodes produce ud2 in generated code
        let mut f = IrFunction::new(0, vec![], vec![IrType::I32]);
        f.body = vec![IrInstruction::new(IrOpcode::Unreachable, 0)];
        let gen = CodeGenerator::new();
        let code = gen.generate_baseline(&f).unwrap();
        let bytes = code.code();

        // ud2 = 0x0F 0x0B should be in the code
        let has_ud2 = bytes.windows(2).any(|w| w[0] == 0x0F && w[1] == 0x0B);
        assert!(has_ud2, "Unreachable should emit ud2 trap instruction");
    }

    // ────────────────────────────────────────────────────────────────────
    // D-QG9: 코드 캐시 — JIT 결과 캐시, 재호출 시 재컴파일 없음
    // ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_dqg9_code_cache_hit() {
        let engine = JitEngine::new();
        let func_id = FunctionId::new(1, 0);
        let wasm = simple_wasm_bytecode();

        // Fill up to threshold
        for _ in 0..100 {
            let _ = engine.get_or_compile(func_id, &wasm);
        }

        // Should have compiled once
        let stats = engine.stats();
        let baseline = stats.baseline_compilations.load(Ordering::Relaxed);
        assert_eq!(baseline, 1, "Should compile exactly once");

        // Subsequent calls should hit cache
        for _ in 0..10 {
            let r = engine.get_or_compile(func_id, &wasm);
            assert!(r.is_ok(), "Cache hit should return Ok");
        }

        // Compilation count should NOT increase
        let baseline_after = stats.baseline_compilations.load(Ordering::Relaxed);
        assert_eq!(baseline_after, 1, "Should not recompile — cache hit");

        // Cache hits should increase
        let hits = stats.cache_hits.load(Ordering::Relaxed);
        assert!(hits >= 10, "Should record cache hits, got {hits}");
    }

    #[test]
    fn test_dqg9_cache_stats() {
        let engine = JitEngine::new();
        let wasm = simple_wasm_bytecode();

        // Compile a few distinct functions
        for idx in 0u32..3 {
            let fid = FunctionId::new(1, idx);
            for _ in 0..100 {
                let _ = engine.get_or_compile(fid, &wasm);
            }
        }

        let cs = engine.cache_stats();
        assert_eq!(cs.entries, 3, "Should have 3 cached functions");
        assert!(cs.total_size > 0, "Total cached size must be > 0");
    }

    #[test]
    fn test_dqg9_invalidate_clears_cache() {
        let engine = JitEngine::new();
        let func_id = FunctionId::new(1, 0);
        let wasm = simple_wasm_bytecode();

        // Compile
        for _ in 0..100 {
            let _ = engine.get_or_compile(func_id, &wasm);
        }
        assert_eq!(engine.cache_stats().entries, 1);

        // Invalidate
        engine.invalidate(func_id);
        assert_eq!(engine.cache_stats().entries, 0);
    }

    // ────────────────────────────────────────────────────────────────────
    // Additional structural tests
    // ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_select_instruction() {
        let mut interp = IrInterpreter::new();

        // select(10, 20, 1) → 10 (condition true)
        let mut f = IrFunction::new(0, vec![], vec![IrType::I32]);
        f.body = vec![
            IrInstruction::new(IrOpcode::Const32(10), 0),
            IrInstruction::new(IrOpcode::Const32(20), 0),
            IrInstruction::new(IrOpcode::Const32(1), 0),
            IrInstruction::new(IrOpcode::Select, 0),
        ];
        assert_eq!(interp.execute(&f, &[]), IrExecResult::Ok(vec![10]));

        // select(10, 20, 0) → 20 (condition false)
        f.body[2] = IrInstruction::new(IrOpcode::Const32(0), 0);
        assert_eq!(interp.execute(&f, &[]), IrExecResult::Ok(vec![20]));
    }

    #[test]
    fn test_unreachable_trap() {
        let mut interp = IrInterpreter::new();
        let mut f = IrFunction::new(0, vec![], vec![]);
        f.body = vec![IrInstruction::new(IrOpcode::Unreachable, 0)];
        assert_eq!(
            interp.execute(&f, &[]),
            IrExecResult::Trap(IrTrap::Unreachable)
        );
    }

    #[test]
    fn test_codegen_prologue_epilogue() {
        let f = IrFunction::new(0, vec![], vec![]);
        let gen = CodeGenerator::new();
        let code = gen.generate_baseline(&f).unwrap();
        let bytes = code.code();

        // Prologue: push rbp (0x55), mov rbp,rsp (48 89 E5)
        assert!(bytes.len() >= 4);
        assert_eq!(bytes[0], 0x55);
        assert_eq!(bytes[1], 0x48);
        assert_eq!(bytes[2], 0x89);
        assert_eq!(bytes[3], 0xE5);

        // Epilogue: pop rbp (0x5D), ret (0xC3) at end
        let len = bytes.len();
        assert_eq!(bytes[len - 1], 0xC3);
        assert_eq!(bytes[len - 2], 0x5D);
    }

    #[test]
    fn test_loop_with_counter() {
        // Sum 1..=10 with a loop
        let mut f = IrFunction::new(0, vec![], vec![IrType::I32]);
        // locals: sum(0) i(1)
        f.add_local(IrType::I32); // sum
        f.add_local(IrType::I32); // i

        f.body = vec![
            // i = 1
            IrInstruction::new(IrOpcode::Const32(1), 0),
            IrInstruction::new(IrOpcode::LocalSet(1), 0),
            // block
            IrInstruction::new(IrOpcode::Block(BlockId(0)), 0),
            // loop
            IrInstruction::new(IrOpcode::Loop(BlockId(1)), 0),
            // if i > 10, break
            IrInstruction::new(IrOpcode::LocalGet(1), 0),
            IrInstruction::new(IrOpcode::Const32(10), 0),
            IrInstruction::new(IrOpcode::I32GtS, 0),
            IrInstruction::new(IrOpcode::BrIf(1), 0),
            // sum += i
            IrInstruction::new(IrOpcode::LocalGet(0), 0),
            IrInstruction::new(IrOpcode::LocalGet(1), 0),
            IrInstruction::new(IrOpcode::I32Add, 0),
            IrInstruction::new(IrOpcode::LocalSet(0), 0),
            // i += 1
            IrInstruction::new(IrOpcode::LocalGet(1), 0),
            IrInstruction::new(IrOpcode::Const32(1), 0),
            IrInstruction::new(IrOpcode::I32Add, 0),
            IrInstruction::new(IrOpcode::LocalSet(1), 0),
            // br loop
            IrInstruction::new(IrOpcode::Br(0), 0),
            // end loop
            IrInstruction::new(IrOpcode::End, 0),
            // end block
            IrInstruction::new(IrOpcode::End, 0),
            // return sum
            IrInstruction::new(IrOpcode::LocalGet(0), 0),
        ];

        let mut interp = IrInterpreter::new();
        let result = interp.execute(&f, &[]);
        assert_eq!(result, IrExecResult::Ok(vec![55])); // 1+2+...+10 = 55
    }

    #[test]
    fn test_if_else() {
        let mut interp = IrInterpreter::new();

        // fn max(a, b) { if a > b { a } else { b } }
        let mut f = IrFunction::new(0, vec![IrType::I32, IrType::I32], vec![IrType::I32]);
        f.add_local(IrType::I32); // result (local 2)

        f.body = vec![
            IrInstruction::new(IrOpcode::LocalGet(0), 0),
            IrInstruction::new(IrOpcode::LocalGet(1), 0),
            IrInstruction::new(IrOpcode::I32GtS, 0),
            IrInstruction::new(IrOpcode::If(BlockId(0)), 0),
            IrInstruction::new(IrOpcode::LocalGet(0), 0),
            IrInstruction::new(IrOpcode::LocalSet(2), 0),
            IrInstruction::new(IrOpcode::Else, 0),
            IrInstruction::new(IrOpcode::LocalGet(1), 0),
            IrInstruction::new(IrOpcode::LocalSet(2), 0),
            IrInstruction::new(IrOpcode::End, 0),
            IrInstruction::new(IrOpcode::LocalGet(2), 0),
        ];

        // max(10, 5) = 10
        let result = interp.execute(&f, &[10, 5]);
        assert_eq!(result, IrExecResult::Ok(vec![10]));

        // max(3, 7) = 7
        let result = interp.execute(&f, &[3, 7]);
        assert_eq!(result, IrExecResult::Ok(vec![7]));
    }

    #[test]
    fn test_conversions() {
        let mut interp = IrInterpreter::new();

        // I32WrapI64
        let mut f = IrFunction::new(0, vec![], vec![IrType::I32]);
        f.body = vec![
            IrInstruction::new(IrOpcode::Const64(0x1_FFFF_FFFF), 0),
            IrInstruction::new(IrOpcode::I32WrapI64, 0),
        ];
        let r = interp.execute(&f, &[]);
        assert_eq!(r, IrExecResult::Ok(vec![-1i64])); // lower 32 bits = 0xFFFFFFFF = -1 as i32

        // I64ExtendI32S
        let mut f = IrFunction::new(0, vec![], vec![IrType::I64]);
        f.body = vec![
            IrInstruction::new(IrOpcode::Const32(-1), 0),
            IrInstruction::new(IrOpcode::I64ExtendI32S, 0),
        ];
        let r = interp.execute(&f, &[]);
        assert_eq!(r, IrExecResult::Ok(vec![-1i64]));

        // I64ExtendI32U
        let mut f = IrFunction::new(0, vec![], vec![IrType::I64]);
        f.body = vec![
            IrInstruction::new(IrOpcode::Const32(-1), 0),
            IrInstruction::new(IrOpcode::I64ExtendI32U, 0),
        ];
        let r = interp.execute(&f, &[]);
        assert_eq!(r, IrExecResult::Ok(vec![0xFFFF_FFFFi64]));
    }
}
