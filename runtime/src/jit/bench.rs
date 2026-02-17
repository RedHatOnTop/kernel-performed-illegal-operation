//! JIT Compiler Benchmark Harness
//!
//! Provides benchmark scenarios for comparing interpreter vs JIT performance.
//! Each benchmark generates an IR function and measures compilation + execution metrics.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use super::codegen::CodeGenerator;
use super::ir::{IrFunction, IrInstruction, IrOpcode, IrType, BlockId};

/// Benchmark result for a single scenario.
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Name of the benchmark.
    pub name: String,
    /// Size of generated code in bytes.
    pub code_size: usize,
    /// Number of IR instructions.
    pub ir_instructions: usize,
    /// Frame size in bytes.
    pub frame_size: usize,
    /// Whether compilation succeeded.
    pub compiled: bool,
}

/// Benchmark harness for JIT compiler.
pub struct JitBenchmark;

impl JitBenchmark {
    /// Run all benchmarks and return results.
    pub fn run_all() -> Vec<BenchmarkResult> {
        let mut results = Vec::new();

        results.push(Self::bench_fibonacci());
        results.push(Self::bench_matrix_multiply());
        results.push(Self::bench_bubble_sort());
        results.push(Self::bench_tight_loop());
        results.push(Self::bench_heavy_arithmetic());

        results
    }

    /// Benchmark: Fibonacci computation (recursive-style with locals).
    pub fn bench_fibonacci() -> BenchmarkResult {
        let mut func = IrFunction::new(0, vec![IrType::I32], vec![IrType::I32]);
        // locals: n (param), a, b, temp, i
        func.add_local(IrType::I32); // local 1: a
        func.add_local(IrType::I32); // local 2: b
        func.add_local(IrType::I32); // local 3: temp
        func.add_local(IrType::I32); // local 4: i

        let body = vec![
            // a = 0
            IrOpcode::Const32(0),
            IrOpcode::LocalSet(1),
            // b = 1
            IrOpcode::Const32(1),
            IrOpcode::LocalSet(2),
            // i = 0
            IrOpcode::Const32(0),
            IrOpcode::LocalSet(4),
            // loop
            IrOpcode::Block(BlockId(0)),
            IrOpcode::Loop(BlockId(1)),
            // if i >= n, break
            IrOpcode::LocalGet(4),
            IrOpcode::LocalGet(0),
            IrOpcode::I32GeS,
            IrOpcode::BrIf(1),
            // temp = a + b
            IrOpcode::LocalGet(1),
            IrOpcode::LocalGet(2),
            IrOpcode::I32Add,
            IrOpcode::LocalSet(3),
            // a = b
            IrOpcode::LocalGet(2),
            IrOpcode::LocalSet(1),
            // b = temp
            IrOpcode::LocalGet(3),
            IrOpcode::LocalSet(2),
            // i = i + 1
            IrOpcode::LocalGet(4),
            IrOpcode::Const32(1),
            IrOpcode::I32Add,
            IrOpcode::LocalSet(4),
            // br loop
            IrOpcode::Br(0),
            IrOpcode::End, // loop
            IrOpcode::End, // block
            // return a
            IrOpcode::LocalGet(1),
            IrOpcode::Return,
        ];

        for op in &body {
            func.add_instruction(IrInstruction::new(*op, 0));
        }

        Self::compile_and_measure("fibonacci", &func, body.len())
    }

    /// Benchmark: Matrix multiply 4x4 (simulated with locals).
    pub fn bench_matrix_multiply() -> BenchmarkResult {
        let mut func = IrFunction::new(0, vec![], vec![IrType::I32]);
        // 16 locals for result matrix + 2 loop counters
        for _ in 0..18 {
            func.add_local(IrType::I32);
        }

        let mut body = Vec::new();
        // Initialize result[0..16] to 0
        for i in 0..16u32 {
            body.push(IrOpcode::Const32(0));
            body.push(IrOpcode::LocalSet(i));
        }

        // Simulate result[0] = a[0]*b[0] + a[1]*b[4] + a[2]*b[8] + a[3]*b[12]
        // Using constants as mock matrix values
        for i in 0..4u32 {
            for j in 0..4u32 {
                body.push(IrOpcode::Const32(0));
                for k in 0..4u32 {
                    // accumulate: acc += (i*4+k+1) * (k*4+j+1)
                    body.push(IrOpcode::Const32((i * 4 + k + 1) as i32));
                    body.push(IrOpcode::Const32((k * 4 + j + 1) as i32));
                    body.push(IrOpcode::I32Mul);
                    body.push(IrOpcode::I32Add);
                }
                body.push(IrOpcode::LocalSet(i * 4 + j));
            }
        }

        // Return result[0]
        body.push(IrOpcode::LocalGet(0));
        body.push(IrOpcode::Return);

        let count = body.len();
        for op in &body {
            func.add_instruction(IrInstruction::new(*op, 0));
        }

        Self::compile_and_measure("matrix_multiply_4x4", &func, count)
    }

    /// Benchmark: Bubble sort simulation with comparisons and swaps.
    pub fn bench_bubble_sort() -> BenchmarkResult {
        let mut func = IrFunction::new(0, vec![], vec![IrType::I32]);
        // 8 locals for array + 2 for loop counters + 1 for temp
        for _ in 0..11 {
            func.add_local(IrType::I32);
        }

        let mut body = Vec::new();
        // Initialize array with descending values
        for i in 0..8u32 {
            body.push(IrOpcode::Const32((8 - i) as i32));
            body.push(IrOpcode::LocalSet(i));
        }

        // Outer loop: i = 0..7
        body.push(IrOpcode::Const32(0));
        body.push(IrOpcode::LocalSet(8)); // i
        body.push(IrOpcode::Block(BlockId(0)));
        body.push(IrOpcode::Loop(BlockId(1)));
        body.push(IrOpcode::LocalGet(8));
        body.push(IrOpcode::Const32(7));
        body.push(IrOpcode::I32GeS);
        body.push(IrOpcode::BrIf(1));

        // Inner loop body (simplified: just compare adjacent and swap)
        // Compare local[0] > local[1]
        body.push(IrOpcode::LocalGet(0));
        body.push(IrOpcode::LocalGet(1));
        body.push(IrOpcode::I32GtS);
        body.push(IrOpcode::If(BlockId(2)));
        // Swap
        body.push(IrOpcode::LocalGet(0));
        body.push(IrOpcode::LocalSet(10)); // temp
        body.push(IrOpcode::LocalGet(1));
        body.push(IrOpcode::LocalSet(0));
        body.push(IrOpcode::LocalGet(10));
        body.push(IrOpcode::LocalSet(1));
        body.push(IrOpcode::End); // if

        // i++
        body.push(IrOpcode::LocalGet(8));
        body.push(IrOpcode::Const32(1));
        body.push(IrOpcode::I32Add);
        body.push(IrOpcode::LocalSet(8));
        body.push(IrOpcode::Br(0));
        body.push(IrOpcode::End); // loop
        body.push(IrOpcode::End); // block

        // Return first element
        body.push(IrOpcode::LocalGet(0));
        body.push(IrOpcode::Return);

        let count = body.len();
        for op in &body {
            func.add_instruction(IrInstruction::new(*op, 0));
        }

        Self::compile_and_measure("bubble_sort", &func, count)
    }

    /// Benchmark: Tight loop with i64 arithmetic.
    pub fn bench_tight_loop() -> BenchmarkResult {
        let mut func = IrFunction::new(0, vec![], vec![IrType::I64]);
        func.add_local(IrType::I64); // local 0: accumulator
        func.add_local(IrType::I64); // local 1: counter

        let body = vec![
            IrOpcode::Const64(0),
            IrOpcode::LocalSet(0),
            IrOpcode::Const64(0),
            IrOpcode::LocalSet(1),
            IrOpcode::Block(BlockId(0)),
            IrOpcode::Loop(BlockId(1)),
            // if counter >= 1000, break
            IrOpcode::LocalGet(1),
            IrOpcode::Const64(1000),
            IrOpcode::I64GeS,
            IrOpcode::BrIf(1),
            // acc = acc + counter * counter
            IrOpcode::LocalGet(0),
            IrOpcode::LocalGet(1),
            IrOpcode::LocalGet(1),
            IrOpcode::I64Mul,
            IrOpcode::I64Add,
            IrOpcode::LocalSet(0),
            // counter++
            IrOpcode::LocalGet(1),
            IrOpcode::Const64(1),
            IrOpcode::I64Add,
            IrOpcode::LocalSet(1),
            IrOpcode::Br(0),
            IrOpcode::End,
            IrOpcode::End,
            IrOpcode::LocalGet(0),
            IrOpcode::Return,
        ];

        let count = body.len();
        for op in &body {
            func.add_instruction(IrInstruction::new(*op, 0));
        }

        Self::compile_and_measure("tight_loop_i64", &func, count)
    }

    /// Benchmark: Heavy arithmetic with mixed types.
    pub fn bench_heavy_arithmetic() -> BenchmarkResult {
        let mut func = IrFunction::new(0, vec![], vec![IrType::I32]);

        let body = vec![
            // i32 chain
            IrOpcode::Const32(100),
            IrOpcode::Const32(200),
            IrOpcode::I32Add,
            IrOpcode::Const32(3),
            IrOpcode::I32Mul,
            IrOpcode::Const32(7),
            IrOpcode::I32DivS,
            IrOpcode::Const32(5),
            IrOpcode::I32RemS,
            // Bitwise
            IrOpcode::Const32(0xFF),
            IrOpcode::I32And,
            IrOpcode::Const32(4),
            IrOpcode::I32Shl,
            IrOpcode::Const32(2),
            IrOpcode::I32ShrU,
            // Comparisons
            IrOpcode::Const32(42),
            IrOpcode::I32Eq,
            // Clz/Ctz/Popcnt
            IrOpcode::Const32(0x00FF0000),
            IrOpcode::I32Clz,
            IrOpcode::Drop,
            IrOpcode::Const32(0x00FF0000),
            IrOpcode::I32Ctz,
            IrOpcode::Drop,
            IrOpcode::Const32(0x55555555),
            IrOpcode::I32Popcnt,
            IrOpcode::Return,
        ];

        let count = body.len();
        for op in &body {
            func.add_instruction(IrInstruction::new(*op, 0));
        }

        Self::compile_and_measure("heavy_arithmetic", &func, count)
    }

    /// Compile a function and return benchmark metrics.
    fn compile_and_measure(name: &str, func: &IrFunction, ir_count: usize) -> BenchmarkResult {
        let gen = CodeGenerator::new();
        match gen.generate_baseline(func) {
            Ok(code) => BenchmarkResult {
                name: String::from(name),
                code_size: code.size(),
                ir_instructions: ir_count,
                frame_size: code.frame_size(),
                compiled: true,
            },
            Err(_) => BenchmarkResult {
                name: String::from(name),
                code_size: 0,
                ir_instructions: ir_count,
                frame_size: 0,
                compiled: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fibonacci_benchmark() {
        let result = JitBenchmark::bench_fibonacci();
        assert!(result.compiled, "fibonacci should compile");
        assert!(result.code_size > 0, "should generate code");
        assert!(result.ir_instructions > 20, "should have many IR instructions");
    }

    #[test]
    fn test_matrix_multiply_benchmark() {
        let result = JitBenchmark::bench_matrix_multiply();
        assert!(result.compiled, "matrix multiply should compile");
        assert!(result.code_size > 0);
        // 4x4 matrix mul generates many instructions
        assert!(result.ir_instructions > 100);
    }

    #[test]
    fn test_bubble_sort_benchmark() {
        let result = JitBenchmark::bench_bubble_sort();
        assert!(result.compiled, "bubble sort should compile");
        assert!(result.code_size > 0);
    }

    #[test]
    fn test_tight_loop_benchmark() {
        let result = JitBenchmark::bench_tight_loop();
        assert!(result.compiled, "tight loop should compile");
        assert!(result.code_size > 0);
    }

    #[test]
    fn test_heavy_arithmetic_benchmark() {
        let result = JitBenchmark::bench_heavy_arithmetic();
        assert!(result.compiled, "heavy arithmetic should compile");
        assert!(result.code_size > 0);
    }

    #[test]
    fn test_run_all_benchmarks() {
        let results = JitBenchmark::run_all();
        assert_eq!(results.len(), 5, "should have 5 benchmarks");
        for result in &results {
            assert!(result.compiled, "{} should compile", result.name);
            assert!(result.code_size > 0, "{} should have code", result.name);
        }
    }

    #[test]
    fn test_benchmark_code_size_reasonable() {
        let results = JitBenchmark::run_all();
        for result in &results {
            // Code size should be reasonable (not excessively large)
            assert!(
                result.code_size < 100_000,
                "{}: code_size {} too large",
                result.name,
                result.code_size
            );
            // Code should be at least as large as prologue+epilogue (~10 bytes)
            assert!(
                result.code_size >= 10,
                "{}: code_size {} too small",
                result.name,
                result.code_size
            );
        }
    }
}
