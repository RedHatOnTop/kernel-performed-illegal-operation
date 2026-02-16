//! JIT Compiler Core
//!
//! This module implements the main JIT compiler that translates
//! WASM bytecode to native machine code.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use super::codegen::{CodeGenerator, NativeCode};
use super::ir::{IrFunction, IrInstruction, IrOpcode, IrType, TranslationError, WasmToIr};
use super::profile::ProfileData;
use super::FunctionId;

/// JIT compilation error.
#[derive(Debug, Clone)]
pub enum CompilationError {
    /// Function is below compilation threshold.
    BelowThreshold,
    /// AOT compilation is disabled.
    AotDisabled,
    /// Translation error.
    Translation(String),
    /// Code generation error.
    CodeGen(String),
    /// Invalid WASM module.
    InvalidModule(String),
    /// Out of memory.
    OutOfMemory,
}

impl From<TranslationError> for CompilationError {
    fn from(e: TranslationError) -> Self {
        CompilationError::Translation(alloc::format!("{:?}", e))
    }
}

/// Result of compilation.
#[derive(Debug, Clone)]
pub struct CompilationResult {
    /// Generated native code.
    pub code: NativeCode,
    /// Compilation statistics.
    pub stats: CompilationStats,
}

/// Compilation statistics.
#[derive(Debug, Clone, Default)]
pub struct CompilationStats {
    /// Time spent parsing WASM (microseconds).
    pub parse_time_us: u64,
    /// Time spent translating to IR (microseconds).
    pub translation_time_us: u64,
    /// Time spent generating code (microseconds).
    pub codegen_time_us: u64,
    /// Number of IR instructions.
    pub ir_instructions: usize,
    /// Size of generated code.
    pub code_size: usize,
}

/// JIT Compiler.
pub struct JitCompiler {
    /// IR translator.
    translator: WasmToIr,
    /// Code generator.
    codegen: CodeGenerator,
}

impl JitCompiler {
    /// Create a new JIT compiler.
    pub fn new() -> Self {
        Self {
            translator: WasmToIr::new(),
            codegen: CodeGenerator::new(),
        }
    }

    /// Compile a function at baseline tier.
    pub fn compile_baseline(
        &self,
        _func_id: FunctionId,
        wasm_bytes: &[u8],
    ) -> Result<NativeCode, CompilationError> {
        // Parse the function from WASM bytes
        let func_info = self.parse_function(wasm_bytes)?;

        // Translate to IR
        let mut translator = WasmToIr::new();
        let ir_func = translator.translate_function(
            func_info.index,
            func_info.params,
            func_info.results,
            &func_info.locals,
            &func_info.code,
        )?;

        // Generate baseline code (no optimizations)
        let code = self.codegen.generate_baseline(&ir_func)?;

        Ok(code)
    }

    /// Compile a function at optimized tier.
    pub fn compile_optimized(
        &self,
        _func_id: FunctionId,
        wasm_bytes: &[u8],
        profile: Option<&ProfileData>,
    ) -> Result<NativeCode, CompilationError> {
        // Parse the function
        let func_info = self.parse_function(wasm_bytes)?;

        // Translate to IR
        let mut translator = WasmToIr::new();
        let ir_func = translator.translate_function(
            func_info.index,
            func_info.params,
            func_info.results,
            &func_info.locals,
            &func_info.code,
        )?;

        // Apply optimizations based on profile
        let optimized_ir = self.optimize(ir_func, profile);

        // Generate optimized code
        let code = self.codegen.generate_optimized(&optimized_ir)?;

        Ok(code)
    }

    /// AOT compile an entire module.
    pub fn compile_module(
        &self,
        module_id: u64,
        wasm_bytes: &[u8],
    ) -> Result<Vec<(u32, Arc<NativeCode>)>, CompilationError> {
        let functions = self.parse_module(wasm_bytes)?;
        let mut compiled = Vec::with_capacity(functions.len());

        for func_info in functions {
            let mut translator = WasmToIr::new();
            let ir_func = translator.translate_function(
                func_info.index,
                func_info.params,
                func_info.results,
                &func_info.locals,
                &func_info.code,
            )?;

            // For AOT, use optimized compilation
            let code = self.codegen.generate_optimized(&ir_func)?;
            compiled.push((func_info.index, Arc::new(code)));
        }

        Ok(compiled)
    }

    /// Parse a single function from WASM bytes.
    fn parse_function(&self, wasm_bytes: &[u8]) -> Result<ParsedFunction, CompilationError> {
        // Simplified parsing - in reality this would parse the full WASM structure
        // For now, assume the bytes are the function body
        Ok(ParsedFunction {
            index: 0,
            params: Vec::new(),
            results: Vec::new(),
            locals: Vec::new(),
            code: wasm_bytes.to_vec(),
        })
    }

    /// Parse all functions from a WASM module.
    fn parse_module(&self, wasm_bytes: &[u8]) -> Result<Vec<ParsedFunction>, CompilationError> {
        let mut functions = Vec::new();
        let mut reader = ModuleReader::new(wasm_bytes);

        // Validate magic and version
        if !reader.validate_header() {
            return Err(CompilationError::InvalidModule(
                "Invalid WASM header".into(),
            ));
        }

        // Parsed type section: Vec<(params, results)>
        let mut types: Vec<(Vec<IrType>, Vec<IrType>)> = Vec::new();
        // Function type indices
        let mut type_indices: Vec<u32> = Vec::new();

        // Parse sections
        while let Some(section) = reader.next_section() {
            match section.id {
                // Type section (1) — parse function signatures
                1 => {
                    let mut sr = ModuleReader::new(section.data);
                    let count = sr.read_leb128() as usize;
                    for _ in 0..count {
                        let form = sr.read_byte();
                        if form != 0x60 {
                            return Err(CompilationError::InvalidModule(
                                "Invalid function type form".into(),
                            ));
                        }
                        let param_count = sr.read_leb128() as usize;
                        let mut params = Vec::with_capacity(param_count);
                        for _ in 0..param_count {
                            params.push(Self::valtype_to_ir(sr.read_byte()));
                        }
                        let result_count = sr.read_leb128() as usize;
                        let mut results = Vec::with_capacity(result_count);
                        for _ in 0..result_count {
                            results.push(Self::valtype_to_ir(sr.read_byte()));
                        }
                        types.push((params, results));
                    }
                }
                // Function section (3) — map function index → type index
                3 => {
                    let mut sr = ModuleReader::new(section.data);
                    let count = sr.read_leb128() as usize;
                    for _ in 0..count {
                        let type_idx = sr.read_leb128() as u32;
                        type_indices.push(type_idx);
                    }
                }
                // Code section (10)
                10 => {
                    // Parse function bodies
                    let mut sr = ModuleReader::new(section.data);
                    let func_count = sr.read_leb128() as usize;
                    for i in 0..func_count {
                        let body_size = sr.read_leb128() as usize;
                        let code = sr.read_bytes(body_size);

                        // Look up type info from type_indices → types
                        let (params, results) = if i < type_indices.len() {
                            let tidx = type_indices[i] as usize;
                            if tidx < types.len() {
                                types[tidx].clone()
                            } else {
                                (Vec::new(), Vec::new())
                            }
                        } else {
                            (Vec::new(), Vec::new())
                        };

                        functions.push(ParsedFunction {
                            index: i as u32,
                            params,
                            results,
                            locals: Vec::new(),
                            code: code.to_vec(),
                        });
                    }
                }
                _ => {
                    // Skip unknown sections
                }
            }
        }

        Ok(functions)
    }

    /// Convert a WASM valtype byte to IrType.
    fn valtype_to_ir(byte: u8) -> IrType {
        match byte {
            0x7F => IrType::I32,
            0x7E => IrType::I64,
            0x7D => IrType::F32,
            0x7C => IrType::F64,
            0x7B => IrType::V128,
            0x70 => IrType::FuncRef,
            0x6F => IrType::ExternRef,
            _ => IrType::I32, // fallback
        }
    }

    /// Apply optimizations to IR.
    fn optimize(&self, mut ir: IrFunction, profile: Option<&ProfileData>) -> IrFunction {
        // Optimization passes based on profile data

        if let Some(profile) = profile {
            // Inline frequently called functions
            if profile.call_count > 10000 {
                self.inline_small_calls(&mut ir);
            }

            // Loop unrolling for hot loops
            self.unroll_hot_loops(&mut ir, profile);

            // Constant propagation
            self.propagate_constants(&mut ir);
        }

        // Always run basic optimizations
        self.dead_code_elimination(&mut ir);
        self.common_subexpression_elimination(&mut ir);

        ir
    }

    fn inline_small_calls(&self, ir: &mut IrFunction) {
        // Inline small functions (< 10 instructions)
        // For each Call instruction, if the target is small enough,
        // replace the Call with the function body. Currently we detect
        // call-sites and mark them; actual inlining requires access to
        // other function bodies, so here we do the control-flow prep:
        // replace Call(idx) with a no-op marker when the estimated
        // instruction count is small and the function is non-recursive.
        let call_indices: Vec<usize> = ir
            .body
            .iter()
            .enumerate()
            .filter_map(|(i, inst)| {
                if let IrOpcode::Call(target) = inst.opcode {
                    // Don't inline self-recursive calls
                    if target != ir.index {
                        Some(i)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        // Placeholder: in a full implementation we would look up the
        // callee IR and splice it in. For now we leave an annotation
        // that the codegen can later use for inline expansion. We
        // record the indices so a future pass can substitute bodies.
        let _ = call_indices;
    }

    fn unroll_hot_loops(&self, ir: &mut IrFunction, profile: &ProfileData) {
        // Unroll loops that execute many iterations.
        // Strategy: find Loop blocks, check profile iteration count,
        // and duplicate the loop body up to UNROLL_FACTOR times.
        const UNROLL_FACTOR: usize = 4;

        if profile.loop_iterations < 1000 {
            return; // not hot enough to unroll
        }

        // Identify loop blocks
        let mut loop_ranges: Vec<(usize, usize)> = Vec::new();
        let mut loop_start_stack: Vec<usize> = Vec::new();

        for (i, inst) in ir.body.iter().enumerate() {
            match inst.opcode {
                IrOpcode::Loop(_) => {
                    loop_start_stack.push(i);
                }
                IrOpcode::End => {
                    if let Some(start) = loop_start_stack.pop() {
                        loop_ranges.push((start, i));
                    }
                }
                _ => {}
            }
        }

        // Unroll small loops (body < 20 instructions)
        for (start, end) in loop_ranges.iter().rev() {
            let body_len = end - start - 1; // exclude Loop and End
            if body_len > 0 && body_len < 20 {
                // Clone the loop body instructions for unrolling
                let body: Vec<IrInstruction> = ir.body[start + 1..*end].to_vec();
                let insert_pos = *end; // before End

                // Insert duplicated bodies (unroll factor - 1 additional copies)
                for _ in 0..(UNROLL_FACTOR - 1).min(3) {
                    let mut cloned = body.clone();
                    // Adjust instruction insertions
                    for inst in &mut cloned {
                        // Keep the same offsets for debugging
                    }
                    // Insert before the End of the loop
                    let splice_pos = insert_pos.min(ir.body.len());
                    for (j, inst) in cloned.into_iter().enumerate() {
                        ir.body.insert(splice_pos + j, inst);
                    }
                }
            }
        }
    }

    fn propagate_constants(&self, ir: &mut IrFunction) {
        // Replace constant expressions with their computed values.
        // Pattern: Const32(a) + Const32(b) + I32Add → Const32(a+b)
        let mut i = 0;
        while i + 2 < ir.body.len() {
            let folded = match (&ir.body[i].opcode, &ir.body[i + 1].opcode, &ir.body[i + 2].opcode) {
                // i32 constant folding
                (IrOpcode::Const32(a), IrOpcode::Const32(b), IrOpcode::I32Add) => {
                    Some(IrOpcode::Const32(a.wrapping_add(*b)))
                }
                (IrOpcode::Const32(a), IrOpcode::Const32(b), IrOpcode::I32Sub) => {
                    Some(IrOpcode::Const32(a.wrapping_sub(*b)))
                }
                (IrOpcode::Const32(a), IrOpcode::Const32(b), IrOpcode::I32Mul) => {
                    Some(IrOpcode::Const32(a.wrapping_mul(*b)))
                }
                (IrOpcode::Const32(a), IrOpcode::Const32(b), IrOpcode::I32And) => {
                    Some(IrOpcode::Const32(a & b))
                }
                (IrOpcode::Const32(a), IrOpcode::Const32(b), IrOpcode::I32Or) => {
                    Some(IrOpcode::Const32(a | b))
                }
                (IrOpcode::Const32(a), IrOpcode::Const32(b), IrOpcode::I32Xor) => {
                    Some(IrOpcode::Const32(a ^ b))
                }
                (IrOpcode::Const32(a), IrOpcode::Const32(b), IrOpcode::I32Shl) => {
                    Some(IrOpcode::Const32(a.wrapping_shl(*b as u32)))
                }
                (IrOpcode::Const32(a), IrOpcode::Const32(b), IrOpcode::I32ShrS) => {
                    Some(IrOpcode::Const32(a.wrapping_shr(*b as u32)))
                }
                // i64 constant folding
                (IrOpcode::Const64(a), IrOpcode::Const64(b), IrOpcode::I64Add) => {
                    Some(IrOpcode::Const64(a.wrapping_add(*b)))
                }
                (IrOpcode::Const64(a), IrOpcode::Const64(b), IrOpcode::I64Sub) => {
                    Some(IrOpcode::Const64(a.wrapping_sub(*b)))
                }
                (IrOpcode::Const64(a), IrOpcode::Const64(b), IrOpcode::I64Mul) => {
                    Some(IrOpcode::Const64(a.wrapping_mul(*b)))
                }
                // Comparison folding
                (IrOpcode::Const32(a), IrOpcode::Const32(b), IrOpcode::I32Eq) => {
                    Some(IrOpcode::Const32(if a == b { 1 } else { 0 }))
                }
                (IrOpcode::Const32(a), IrOpcode::Const32(b), IrOpcode::I32Ne) => {
                    Some(IrOpcode::Const32(if a != b { 1 } else { 0 }))
                }
                (IrOpcode::Const32(a), IrOpcode::Const32(b), IrOpcode::I32LtS) => {
                    Some(IrOpcode::Const32(if a < b { 1 } else { 0 }))
                }
                _ => None,
            };

            if let Some(result) = folded {
                let offset = ir.body[i].offset;
                ir.body[i] = IrInstruction::new(result, offset);
                ir.body.remove(i + 2);
                ir.body.remove(i + 1);
                // Don't advance: try folding again with the new constant
            } else {
                i += 1;
            }
        }
    }

    fn dead_code_elimination(&self, ir: &mut IrFunction) {
        // Remove unreachable code after Return or Unreachable.
        // Walk instructions; after seeing Return/Unreachable, remove
        // subsequent instructions up to the next End/Else/Block/Loop/If.
        let mut removing = false;
        let mut to_remove: Vec<usize> = Vec::new();

        for (i, inst) in ir.body.iter().enumerate() {
            if removing {
                match inst.opcode {
                    IrOpcode::End | IrOpcode::Else | IrOpcode::Block(_)
                    | IrOpcode::Loop(_) | IrOpcode::If(_) => {
                        removing = false;
                    }
                    _ => {
                        to_remove.push(i);
                    }
                }
            } else {
                match inst.opcode {
                    IrOpcode::Return | IrOpcode::Unreachable => {
                        removing = true;
                    }
                    _ => {}
                }
            }
        }

        // Remove in reverse order to keep indices valid
        for &idx in to_remove.iter().rev() {
            ir.body.remove(idx);
        }

        // Also remove Const+Drop sequences (value pushed then immediately dropped)
        let mut i = 0;
        while i + 1 < ir.body.len() {
            let is_const_drop = matches!(
                (&ir.body[i].opcode, &ir.body[i + 1].opcode),
                (IrOpcode::Const32(_) | IrOpcode::Const64(_) | IrOpcode::ConstF32(_) | IrOpcode::ConstF64(_), IrOpcode::Drop)
            );
            if is_const_drop {
                ir.body.remove(i + 1);
                ir.body.remove(i);
                // Don't advance
            } else {
                i += 1;
            }
        }
    }

    fn common_subexpression_elimination(&self, ir: &mut IrFunction) {
        // Eliminate redundant local.get operations within a basic block.
        // If we see LocalGet(n) and no intervening LocalSet(n), the second
        // LocalGet(n) can be replaced with a stack Dup (simulated by another
        // LocalGet since we don't have a Dup opcode, but we can hoist it).
        //
        // Also eliminates consecutive duplicate GlobalGet(n) sequences.
        let mut i = 0;
        while i + 1 < ir.body.len() {
            // Pattern: LocalGet(n) followed later by LocalGet(n) with no
            // LocalSet(n) in between → the value is still on stack/in register.
            // We leave these for the register allocator in codegen.
            //
            // Pattern: same GlobalGet(n) twice in a row
            if let IrOpcode::GlobalGet(n) = ir.body[i].opcode {
                if i + 1 < ir.body.len() {
                    if let IrOpcode::GlobalGet(m) = ir.body[i + 1].opcode {
                        if n == m {
                            // Second GlobalGet is redundant if no GlobalSet(n)
                            // between them; since they're adjacent, it's safe.
                            // In a stack machine, we'd need to duplicate the top
                            // of stack. For now, leave as-is since the codegen
                            // can detect this pattern.
                        }
                    }
                }
            }
            i += 1;
        }
    }
}

impl Default for JitCompiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Parsed function information.
struct ParsedFunction {
    index: u32,
    params: Vec<IrType>,
    results: Vec<IrType>,
    locals: Vec<(u32, IrType)>,
    code: Vec<u8>,
}

/// Simple module reader.
struct ModuleReader<'a> {
    data: &'a [u8],
    pos: usize,
}

struct Section<'a> {
    id: u8,
    data: &'a [u8],
}

impl<'a> ModuleReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn validate_header(&mut self) -> bool {
        // WASM magic: 0x00 0x61 0x73 0x6D
        // Version: 0x01 0x00 0x00 0x00
        if self.data.len() < 8 {
            return false;
        }

        let magic = &self.data[0..4];
        let version = &self.data[4..8];

        if magic != [0x00, 0x61, 0x73, 0x6D] {
            return false;
        }

        if version != [0x01, 0x00, 0x00, 0x00] {
            return false;
        }

        self.pos = 8;
        true
    }

    fn next_section(&mut self) -> Option<Section<'a>> {
        if self.pos >= self.data.len() {
            return None;
        }

        let id = self.data[self.pos];
        self.pos += 1;

        let size = self.read_leb128() as usize;

        if self.pos + size > self.data.len() {
            return None;
        }

        let data = &self.data[self.pos..self.pos + size];
        self.pos += size;

        Some(Section { id, data })
    }

    fn read_leb128(&mut self) -> u64 {
        let mut result = 0u64;
        let mut shift = 0;

        while self.pos < self.data.len() {
            let byte = self.data[self.pos];
            self.pos += 1;

            result |= ((byte & 0x7F) as u64) << shift;

            if byte & 0x80 == 0 {
                break;
            }

            shift += 7;
        }

        result
    }

    fn read_bytes(&mut self, len: usize) -> &'a [u8] {
        let end = (self.pos + len).min(self.data.len());
        let bytes = &self.data[self.pos..end];
        self.pos = end;
        bytes
    }

    fn read_byte(&mut self) -> u8 {
        if self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;
            b
        } else {
            0
        }
    }
}
