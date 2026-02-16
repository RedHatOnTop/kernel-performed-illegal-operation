//! WASM instruction executor.
//!
//! Executes parsed WASM instructions using the stack machine defined in
//! `interpreter.rs`. This is the core execution loop that processes all
//! ~200 WASM MVP instructions.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::interpreter::{
    BlockFrame, BlockKind, CallFrame, GlobalValue, Table, TrapError, ValueStack, WasmValue,
    MAX_CALL_STACK_DEPTH,
};
use crate::memory::LinearMemory;
use crate::module::{ExportKind, FunctionType, ImportKind, Module, ValueType};
use crate::opcodes::Instruction;
use crate::parser::BlockType;
use crate::wasi::WasiCtx;

/// Host function signature.
pub type HostFn = fn(&mut ExecutorContext, &[WasmValue]) -> Result<Vec<WasmValue>, TrapError>;

/// Registered host function.
#[derive(Clone)]
pub struct HostFunction {
    pub module: String,
    pub name: String,
    pub func: HostFn,
    pub type_idx: Option<u32>,
}

/// The execution context holds all runtime state.
pub struct ExecutorContext {
    /// The WASM module being executed.
    pub module: Module,
    /// Linear memories.
    pub memories: Vec<LinearMemory>,
    /// Tables.
    pub tables: Vec<Table>,
    /// Global variables.
    pub globals: Vec<GlobalValue>,
    /// Host functions keyed by function index.
    pub host_functions: Vec<Option<HostFunction>>,
    /// Fuel remaining (None = unlimited).
    pub fuel: Option<u64>,
    /// stdout capture buffer.
    pub stdout: Vec<u8>,
    /// stderr capture buffer.
    pub stderr: Vec<u8>,
    /// Exit code if proc_exit was called.
    pub exit_code: Option<i32>,
    /// WASI context for WASI system calls.
    pub wasi_ctx: Option<WasiCtx>,
    /// WASI Preview 2 context for resource-based WASI P2 interfaces.
    pub wasi2_ctx: Option<crate::wasi2::Wasi2Ctx>,
}

impl ExecutorContext {
    /// Create a new execution context from a parsed module.
    pub fn new(module: Module) -> Result<Self, TrapError> {
        Self::new_with_host_functions(module, Vec::new())
    }

    /// Create with host function bindings.
    pub fn new_with_host_functions(
        module: Module,
        host_fns: Vec<HostFunction>,
    ) -> Result<Self, TrapError> {
        // Count imported functions
        let import_func_count = module.import_function_count();

        // Build host function table (indexed by function index)
        let total_funcs = module.total_function_count();
        let mut host_functions: Vec<Option<HostFunction>> = Vec::with_capacity(total_funcs);

        // Map imported functions to host functions
        let mut import_idx = 0;
        for import in &module.imports {
            if let ImportKind::Function(_) = &import.kind {
                let hf = host_fns
                    .iter()
                    .find(|hf| hf.module == import.module && hf.name == import.name);
                host_functions.push(hf.cloned());
                import_idx += 1;
            }
        }
        // Local functions don't have host bindings
        for _ in 0..module.functions.len() {
            host_functions.push(None);
        }

        // Initialize memories
        let mut memories = Vec::new();
        // Check imports for memory
        for import in &module.imports {
            if let ImportKind::Memory(ref mem_type) = import.kind {
                memories.push(
                    LinearMemory::new(mem_type.min, mem_type.max)
                        .map_err(|e| TrapError::ExecutionError(alloc::format!("{:?}", e)))?,
                );
            }
        }
        // Module-defined memories
        for mem_type in &module.memories {
            memories.push(
                LinearMemory::new(mem_type.min, mem_type.max)
                    .map_err(|e| TrapError::ExecutionError(alloc::format!("{:?}", e)))?,
            );
        }

        // Initialize tables
        let mut tables = Vec::new();
        for import in &module.imports {
            if let ImportKind::Table(ref tt) = import.kind {
                tables.push(Table::new(tt.min, tt.max));
            }
        }
        for tt in &module.tables {
            tables.push(Table::new(tt.min, tt.max));
        }

        // Initialize globals
        let mut globals = Vec::new();
        for import in &module.imports {
            if let ImportKind::Global(ref gt) = import.kind {
                globals.push(GlobalValue {
                    value: WasmValue::default_for(gt.value_type),
                    mutable: gt.mutable,
                });
            }
        }
        for global in &module.globals {
            let value = Self::eval_const_expr_static(&global.init_expr, &globals)?;
            globals.push(GlobalValue {
                value,
                mutable: global.global_type.mutable,
            });
        }

        let mut ctx = ExecutorContext {
            module,
            memories,
            tables,
            globals,
            host_functions,
            fuel: Some(10_000_000),
            stdout: Vec::new(),
            stderr: Vec::new(),
            exit_code: None,
            wasi_ctx: None,
            wasi2_ctx: None,
        };

        // Initialize data segments
        ctx.init_data_segments()?;

        // Initialize element segments
        ctx.init_element_segments()?;

        Ok(ctx)
    }

    /// Evaluate a constant expression (for globals/data/element init).
    fn eval_const_expr_static(
        instrs: &[Instruction],
        globals: &[GlobalValue],
    ) -> Result<WasmValue, TrapError> {
        for instr in instrs {
            match instr {
                Instruction::I32Const(v) => return Ok(WasmValue::I32(*v)),
                Instruction::I64Const(v) => return Ok(WasmValue::I64(*v)),
                Instruction::F32Const(v) => return Ok(WasmValue::F32(*v)),
                Instruction::F64Const(v) => return Ok(WasmValue::F64(*v)),
                Instruction::GlobalGet(idx) => {
                    let g = globals.get(*idx as usize).ok_or(TrapError::ExecutionError(
                        String::from("global index out of range in init expr"),
                    ))?;
                    return Ok(g.value);
                }
                Instruction::RefNull => return Ok(WasmValue::FuncRef(None)),
                Instruction::RefFunc(idx) => return Ok(WasmValue::FuncRef(Some(*idx))),
                _ => {}
            }
        }
        Ok(WasmValue::I32(0))
    }

    /// Initialize data segments into memory.
    fn init_data_segments(&mut self) -> Result<(), TrapError> {
        // Clone data to avoid borrow issues
        let data_segs: Vec<_> = self.module.data.clone();
        for seg in &data_segs {
            if seg.passive {
                continue;
            }
            let offset = Self::eval_const_expr_static(&seg.offset_expr, &self.globals)?;
            let offset = offset.as_i32().unwrap_or(0) as usize;
            let mem = self.memories.get_mut(seg.memory_idx as usize).ok_or(
                TrapError::MemoryOutOfBounds {
                    offset,
                    size: seg.data.len(),
                    memory_size: 0,
                },
            )?;
            mem.write_bytes(offset, &seg.data)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset,
                    size: seg.data.len(),
                    memory_size: mem.size(),
                })?;
        }
        Ok(())
    }

    /// Initialize element segments into tables.
    fn init_element_segments(&mut self) -> Result<(), TrapError> {
        let elem_segs: Vec<_> = self.module.elements.clone();
        for seg in &elem_segs {
            if seg.passive {
                continue;
            }
            let offset = Self::eval_const_expr_static(&seg.offset_expr, &self.globals)?;
            let offset = offset.as_i32().unwrap_or(0) as u32;
            let table =
                self.tables
                    .get_mut(seg.table_idx as usize)
                    .ok_or(TrapError::UndefinedElement {
                        index: seg.table_idx,
                    })?;
            for (i, &func_idx) in seg.func_indices.iter().enumerate() {
                table.set(offset + i as u32, Some(func_idx))?;
            }
        }
        Ok(())
    }

    /// Get the function type for a function index.
    pub fn func_type(&self, func_idx: u32) -> Option<&FunctionType> {
        self.module.function_type(func_idx)
    }

    /// Check if a function is an imported (host) function.
    pub fn is_host_function(&self, func_idx: u32) -> bool {
        (func_idx as usize) < self.module.import_function_count()
    }
}

// ============================================================================
// Executor
// ============================================================================

/// Execute a WASM function by export name.
pub fn execute_export(
    ctx: &mut ExecutorContext,
    export_name: &str,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    // Find export
    let export = ctx
        .module
        .find_export(export_name)
        .ok_or_else(|| TrapError::ExportNotFound(String::from(export_name)))?;

    if export.kind != ExportKind::Function {
        return Err(TrapError::ExportNotFound(String::from(export_name)));
    }

    let func_idx = export.index;
    execute_function(ctx, func_idx, args)
}

/// Execute a WASM function by function index.
pub fn execute_function(
    ctx: &mut ExecutorContext,
    func_idx: u32,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    // Check if host function
    if ctx.is_host_function(func_idx) {
        return call_host_function(ctx, func_idx, args);
    }

    let func_type = ctx
        .func_type(func_idx)
        .ok_or(TrapError::FunctionNotFound(func_idx))?
        .clone();

    let import_count = ctx.module.import_function_count();
    let local_idx = func_idx as usize - import_count;
    let body = ctx
        .module
        .code
        .get(local_idx)
        .ok_or(TrapError::FunctionNotFound(func_idx))?
        .clone();

    // Prepare locals: params + declared locals
    let mut locals = Vec::new();
    for (i, param_type) in func_type.params.iter().enumerate() {
        if i < args.len() {
            locals.push(args[i]);
        } else {
            locals.push(WasmValue::default_for(*param_type));
        }
    }
    for &(count, vtype) in &body.locals {
        for _ in 0..count {
            locals.push(WasmValue::default_for(vtype));
        }
    }

    let return_arity = func_type.results.len();

    // Pre-compute block end positions
    let block_map = compute_block_map(&body.instructions);

    // Execute
    let mut stack = ValueStack::new();
    let mut call_stack: Vec<CallFrame> = Vec::new();

    let frame = CallFrame {
        func_idx,
        locals,
        pc: 0,
        stack_base: 0,
        return_arity,
        block_stack: vec![BlockFrame {
            kind: BlockKind::Function,
            block_type: if return_arity == 0 {
                BlockType::Empty
            } else if return_arity == 1 {
                BlockType::Value(func_type.results[0])
            } else {
                BlockType::Empty
            },
            stack_depth: 0,
            start_pc: 0,
            end_pc: body.instructions.len().saturating_sub(1),
            else_pc: None,
            arity: return_arity,
            param_arity: 0,
        }],
        is_host: false,
    };
    call_stack.push(frame);

    // Main execution loop
    loop {
        let frame = match call_stack.last_mut() {
            Some(f) => f,
            None => break,
        };

        // Get current function body
        let current_func_idx = frame.func_idx;
        let import_count = ctx.module.import_function_count();
        let local_func_idx = current_func_idx as usize - import_count;
        let instructions = &ctx.module.code[local_func_idx].instructions;

        if frame.pc >= instructions.len() {
            // Function ended — return
            let arity = frame.return_arity;
            let base = frame.stack_base;
            call_stack.pop();

            // Collect return values
            let results = collect_results(&mut stack, arity, base);
            stack.truncate(base);
            for r in results {
                stack.push(r)?;
            }

            if call_stack.is_empty() {
                break;
            }
            continue;
        }

        let instr = instructions[frame.pc].clone();
        frame.pc += 1;

        // Consume fuel
        if let Some(ref mut fuel) = ctx.fuel {
            if *fuel == 0 {
                return Err(TrapError::FuelExhausted);
            }
            *fuel -= 1;
        }

        // Execute instruction
        match execute_instruction(ctx, &mut stack, &mut call_stack, &instr)? {
            ControlFlow::Continue => {}
            ControlFlow::Return => {
                // Return from current function
                let frame = call_stack.last().unwrap();
                let arity = frame.return_arity;
                let base = frame.stack_base;
                let results = collect_results(&mut stack, arity, base);
                call_stack.pop();
                if let Some(parent) = call_stack.last() {
                    stack.truncate(parent.stack_base + stack.len().saturating_sub(base));
                }
                stack.truncate(base);
                for r in results {
                    stack.push(r)?;
                }
                if call_stack.is_empty() {
                    break;
                }
            }
            ControlFlow::CallFunction(target_idx, target_args) => {
                if call_stack.len() >= MAX_CALL_STACK_DEPTH {
                    return Err(TrapError::CallStackOverflow);
                }

                if ctx.is_host_function(target_idx) {
                    let results = call_host_function(ctx, target_idx, &target_args)?;
                    for r in results {
                        stack.push(r)?;
                    }
                } else {
                    let ft = ctx
                        .func_type(target_idx)
                        .ok_or(TrapError::FunctionNotFound(target_idx))?
                        .clone();
                    let import_c = ctx.module.import_function_count();
                    let lidx = target_idx as usize - import_c;
                    let body = ctx
                        .module
                        .code
                        .get(lidx)
                        .ok_or(TrapError::FunctionNotFound(target_idx))?;

                    let mut new_locals = Vec::new();
                    for (i, pt) in ft.params.iter().enumerate() {
                        if i < target_args.len() {
                            new_locals.push(target_args[i]);
                        } else {
                            new_locals.push(WasmValue::default_for(*pt));
                        }
                    }
                    for &(count, vtype) in &body.locals {
                        for _ in 0..count {
                            new_locals.push(WasmValue::default_for(vtype));
                        }
                    }

                    let ret_arity = ft.results.len();
                    let new_frame = CallFrame {
                        func_idx: target_idx,
                        locals: new_locals,
                        pc: 0,
                        stack_base: stack.len(),
                        return_arity: ret_arity,
                        block_stack: vec![BlockFrame {
                            kind: BlockKind::Function,
                            block_type: if ret_arity == 0 {
                                BlockType::Empty
                            } else if ret_arity == 1 {
                                BlockType::Value(ft.results[0])
                            } else {
                                BlockType::Empty
                            },
                            stack_depth: stack.len(),
                            start_pc: 0,
                            end_pc: body.instructions.len().saturating_sub(1),
                            else_pc: None,
                            arity: ret_arity,
                            param_arity: 0,
                        }],
                        is_host: false,
                    };
                    call_stack.push(new_frame);
                }
            }
            ControlFlow::Exit(code) => {
                ctx.exit_code = Some(code);
                return Err(TrapError::ProcessExit(code));
            }
        }
    }

    // Collect final results
    let mut results = Vec::new();
    let n = return_arity.min(stack.len());
    for _ in 0..n {
        results.push(stack.pop()?);
    }
    results.reverse();
    Ok(results)
}

/// Call a host function.
fn call_host_function(
    ctx: &mut ExecutorContext,
    func_idx: u32,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let hf = ctx
        .host_functions
        .get(func_idx as usize)
        .and_then(|h| h.clone())
        .ok_or(TrapError::FunctionNotFound(func_idx))?;
    (hf.func)(ctx, args)
}

/// Control flow outcome from instruction execution.
enum ControlFlow {
    Continue,
    Return,
    CallFunction(u32, Vec<WasmValue>),
    Exit(i32),
}

/// Collect N result values from the stack.
fn collect_results(stack: &mut ValueStack, arity: usize, base: usize) -> Vec<WasmValue> {
    let available = stack.len().saturating_sub(base);
    let n = arity.min(available);
    let mut results = Vec::with_capacity(n);
    for _ in 0..n {
        if let Ok(v) = stack.pop() {
            results.push(v);
        }
    }
    results.reverse();
    results
}

/// Execute a single instruction.
fn execute_instruction(
    ctx: &mut ExecutorContext,
    stack: &mut ValueStack,
    call_stack: &mut Vec<CallFrame>,
    instr: &Instruction,
) -> Result<ControlFlow, TrapError> {
    let frame = call_stack.last_mut().unwrap();

    match instr {
        // ====================================================================
        // Control Flow
        // ====================================================================
        Instruction::Unreachable => {
            return Err(TrapError::Unreachable);
        }
        Instruction::Nop => {}

        Instruction::Block(bt) => {
            let arity = block_arity(bt, &ctx.module);
            let param = block_param_arity(bt, &ctx.module);
            let instructions = get_current_instructions(ctx, frame);
            let end_pc = find_block_end(instructions, frame.pc - 1);
            frame.block_stack.push(BlockFrame {
                kind: BlockKind::Block,
                block_type: bt.clone(),
                stack_depth: stack.len() - param,
                start_pc: frame.pc - 1,
                end_pc,
                else_pc: None,
                arity,
                param_arity: param,
            });
        }

        Instruction::Loop(bt) => {
            let arity = block_arity(bt, &ctx.module);
            let param = block_param_arity(bt, &ctx.module);
            let instructions = get_current_instructions(ctx, frame);
            let end_pc = find_block_end(instructions, frame.pc - 1);
            frame.block_stack.push(BlockFrame {
                kind: BlockKind::Loop,
                block_type: bt.clone(),
                stack_depth: stack.len() - param,
                start_pc: frame.pc - 1,
                end_pc,
                else_pc: None,
                arity: param, // loop branches go to start, using params
                param_arity: param,
            });
        }

        Instruction::If(bt) => {
            let cond = stack.pop_i32()?;
            let arity = block_arity(bt, &ctx.module);
            let param = block_param_arity(bt, &ctx.module);
            let instructions = get_current_instructions(ctx, frame);
            let end_pc = find_block_end(instructions, frame.pc - 1);
            let else_pc = find_else(instructions, frame.pc - 1);

            frame.block_stack.push(BlockFrame {
                kind: BlockKind::If,
                block_type: bt.clone(),
                stack_depth: stack.len() - param,
                start_pc: frame.pc - 1,
                end_pc,
                else_pc,
                arity,
                param_arity: param,
            });

            if cond == 0 {
                // Jump to else or end
                if let Some(else_pc) = else_pc {
                    frame.pc = else_pc + 1; // skip the Else instruction
                } else {
                    frame.pc = end_pc + 1; // skip past end
                    frame.block_stack.pop();
                }
            }
        }

        Instruction::Else => {
            // We're in the true branch of an if, jumping to end
            if let Some(block) = frame.block_stack.last() {
                let end_pc = block.end_pc;
                let arity = block.arity;
                let base = block.stack_depth;
                let results = collect_results(stack, arity, base);
                stack.truncate(base);
                for r in results {
                    stack.push(r)?;
                }
                frame.pc = end_pc + 1;
                frame.block_stack.pop();
            }
        }

        Instruction::End => {
            if let Some(block) = frame.block_stack.pop() {
                if block.kind == BlockKind::Function {
                    // Function end — handled by caller
                    frame.block_stack.push(block);
                    // Set PC past end so the main loop returns
                    let instructions = get_current_instructions(ctx, frame);
                    frame.pc = instructions.len();
                } else {
                    let arity = block.arity;
                    let base = block.stack_depth;
                    let results = collect_results(stack, arity, base);
                    stack.truncate(base);
                    for r in results {
                        stack.push(r)?;
                    }
                }
            }
        }

        Instruction::Br(label_idx) => {
            branch(stack, frame, *label_idx)?;
        }

        Instruction::BrIf(label_idx) => {
            let cond = stack.pop_i32()?;
            if cond != 0 {
                branch(stack, frame, *label_idx)?;
            }
        }

        Instruction::BrTable(targets, default) => {
            let idx = stack.pop_i32()? as u32;
            let label = if (idx as usize) < targets.len() {
                targets[idx as usize]
            } else {
                *default
            };
            branch(stack, frame, label)?;
        }

        Instruction::Return => {
            return Ok(ControlFlow::Return);
        }

        Instruction::Call(func_idx) => {
            let ft = ctx
                .func_type(*func_idx)
                .ok_or(TrapError::FunctionNotFound(*func_idx))?
                .clone();
            let mut args = Vec::with_capacity(ft.params.len());
            for _ in 0..ft.params.len() {
                args.push(stack.pop()?);
            }
            args.reverse();
            return Ok(ControlFlow::CallFunction(*func_idx, args));
        }

        Instruction::CallIndirect(type_idx, table_idx) => {
            let elem_idx = stack.pop_i32()? as u32;
            let table = ctx
                .tables
                .get(*table_idx as usize)
                .ok_or(TrapError::UndefinedElement { index: *table_idx })?;
            let func_idx = table
                .get(elem_idx)?
                .ok_or(TrapError::UninitializedElement { index: elem_idx })?;

            // Verify type
            let expected_type = ctx
                .module
                .types
                .get(*type_idx as usize)
                .ok_or(TrapError::ExecutionError(String::from("type index OOB")))?;
            let actual_type = ctx
                .func_type(func_idx)
                .ok_or(TrapError::FunctionNotFound(func_idx))?;
            if expected_type != actual_type {
                return Err(TrapError::IndirectCallTypeMismatch {
                    expected_type: *type_idx,
                    actual_type: 0, // simplified
                });
            }

            let ft = actual_type.clone();
            let mut args = Vec::with_capacity(ft.params.len());
            for _ in 0..ft.params.len() {
                args.push(stack.pop()?);
            }
            args.reverse();
            return Ok(ControlFlow::CallFunction(func_idx, args));
        }

        // ====================================================================
        // Reference
        // ====================================================================
        Instruction::RefNull => {
            stack.push(WasmValue::FuncRef(None))?;
        }
        Instruction::RefIsNull => {
            let v = stack.pop()?;
            let is_null = match v {
                WasmValue::FuncRef(None) | WasmValue::ExternRef(None) => 1i32,
                _ => 0i32,
            };
            stack.push(WasmValue::I32(is_null))?;
        }
        Instruction::RefFunc(idx) => {
            stack.push(WasmValue::FuncRef(Some(*idx)))?;
        }

        // ====================================================================
        // Parametric
        // ====================================================================
        Instruction::Drop => {
            stack.pop()?;
        }
        Instruction::Select => {
            let cond = stack.pop_i32()?;
            let val2 = stack.pop()?;
            let val1 = stack.pop()?;
            stack.push(if cond != 0 { val1 } else { val2 })?;
        }

        // ====================================================================
        // Variable Access
        // ====================================================================
        Instruction::LocalGet(idx) => {
            let val = frame
                .locals
                .get(*idx as usize)
                .copied()
                .ok_or(TrapError::ExecutionError(String::from("local index OOB")))?;
            stack.push(val)?;
        }
        Instruction::LocalSet(idx) => {
            let val = stack.pop()?;
            if let Some(local) = frame.locals.get_mut(*idx as usize) {
                *local = val;
            }
        }
        Instruction::LocalTee(idx) => {
            let val = *stack.peek()?;
            if let Some(local) = frame.locals.get_mut(*idx as usize) {
                *local = val;
            }
        }
        Instruction::GlobalGet(idx) => {
            let val = ctx
                .globals
                .get(*idx as usize)
                .ok_or(TrapError::ExecutionError(String::from("global index OOB")))?
                .value;
            stack.push(val)?;
        }
        Instruction::GlobalSet(idx) => {
            let val = stack.pop()?;
            if let Some(g) = ctx.globals.get_mut(*idx as usize) {
                g.value = val;
            }
        }

        // ====================================================================
        // Table Operations
        // ====================================================================
        Instruction::TableGet(table_idx) => {
            let idx = stack.pop_i32()? as u32;
            let table = ctx
                .tables
                .get(*table_idx as usize)
                .ok_or(TrapError::UndefinedElement { index: *table_idx })?;
            let val = table.get(idx)?;
            stack.push(WasmValue::FuncRef(val))?;
        }
        Instruction::TableSet(table_idx) => {
            let val = stack.pop()?;
            let idx = stack.pop_i32()? as u32;
            let func_ref = match val {
                WasmValue::FuncRef(v) => v,
                _ => None,
            };
            let table = ctx
                .tables
                .get_mut(*table_idx as usize)
                .ok_or(TrapError::UndefinedElement { index: *table_idx })?;
            table.set(idx, func_ref)?;
        }
        Instruction::TableSize(table_idx) => {
            let table = ctx
                .tables
                .get(*table_idx as usize)
                .ok_or(TrapError::UndefinedElement { index: *table_idx })?;
            stack.push(WasmValue::I32(table.size() as i32))?;
        }
        Instruction::TableGrow(table_idx) => {
            let n = stack.pop_i32()? as u32;
            let init = stack.pop()?;
            let init_ref = match init {
                WasmValue::FuncRef(v) => v,
                _ => None,
            };
            let table = ctx
                .tables
                .get_mut(*table_idx as usize)
                .ok_or(TrapError::UndefinedElement { index: *table_idx })?;
            match table.grow(n, init_ref) {
                Ok(old) => stack.push(WasmValue::I32(old as i32))?,
                Err(_) => stack.push(WasmValue::I32(-1))?,
            }
        }
        Instruction::TableInit(elem_idx, table_idx) => {
            // table.init: copy elements from passive element segment to table
            let n = stack.pop_i32()? as u32;   // count
            let s = stack.pop_i32()? as u32;   // source offset in element segment
            let d = stack.pop_i32()? as u32;   // destination offset in table
            let elem = ctx.module.elements.get(*elem_idx as usize).ok_or(
                TrapError::ExecutionError(String::from("element segment index OOB")),
            )?;
            if s.checked_add(n).map_or(true, |end| end as usize > elem.func_indices.len())
            {
                return Err(TrapError::ExecutionError(String::from(
                    "table.init: source range out of bounds",
                )));
            }
            let table = ctx
                .tables
                .get_mut(*table_idx as usize)
                .ok_or(TrapError::UndefinedElement { index: *table_idx })?;
            for i in 0..n {
                let func_idx = elem.func_indices[(s + i) as usize];
                table.set(d + i, Some(func_idx))?;
            }
        }
        Instruction::ElemDrop(elem_idx) => {
            // elem.drop: mark element segment as dropped by clearing its
            // function indices. This frees memory and prevents future
            // table.init from using this segment (as required by spec).
            if let Some(elem) = ctx.module.elements.get_mut(*elem_idx as usize) {
                elem.func_indices.clear();
                elem.passive = false; // mark as dropped
            }
        }
        Instruction::TableCopy(dst_table, src_table) => {
            // table.copy: copy entries between tables (or within one table)
            let n = stack.pop_i32()? as u32;
            let s = stack.pop_i32()? as u32;
            let d = stack.pop_i32()? as u32;
            if *dst_table == *src_table {
                // Overlapping copy within the same table
                let table = ctx
                    .tables
                    .get_mut(*dst_table as usize)
                    .ok_or(TrapError::UndefinedElement { index: *dst_table })?;
                if d <= s {
                    for i in 0..n {
                        let val = table.get(s + i)?;
                        table.set(d + i, val)?;
                    }
                } else {
                    for i in (0..n).rev() {
                        let val = table.get(s + i)?;
                        table.set(d + i, val)?;
                    }
                }
            } else {
                // Different tables: read source entries first, then write
                let src = ctx
                    .tables
                    .get(*src_table as usize)
                    .ok_or(TrapError::UndefinedElement { index: *src_table })?;
                let mut entries = Vec::new();
                for i in 0..n {
                    entries.push(src.get(s + i)?);
                }
                let dst = ctx
                    .tables
                    .get_mut(*dst_table as usize)
                    .ok_or(TrapError::UndefinedElement { index: *dst_table })?;
                for (i, val) in entries.into_iter().enumerate() {
                    dst.set(d + i as u32, val)?;
                }
            }
        }
        Instruction::TableFill(table_idx) => {
            // table.fill: fill a range of table entries with a value
            let n = stack.pop_i32()? as u32;
            let val = stack.pop()?;
            let d = stack.pop_i32()? as u32;
            let func_ref = match val {
                WasmValue::FuncRef(v) => v,
                _ => None,
            };
            let table = ctx
                .tables
                .get_mut(*table_idx as usize)
                .ok_or(TrapError::UndefinedElement { index: *table_idx })?;
            for i in 0..n {
                table.set(d + i, func_ref)?;
            }
        }

        // ====================================================================
        // Memory Load
        // ====================================================================
        Instruction::I32Load(_, offset) => {
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx.memories.first().ok_or(TrapError::MemoryOutOfBounds {
                offset: addr,
                size: 4,
                memory_size: 0,
            })?;
            let val = mem
                .read_u32(addr)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 4,
                    memory_size: mem.size(),
                })?;
            stack.push(WasmValue::I32(val as i32))?;
        }
        Instruction::I64Load(_, offset) => {
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx.memories.first().ok_or(TrapError::MemoryOutOfBounds {
                offset: addr,
                size: 8,
                memory_size: 0,
            })?;
            let val = mem
                .read_u64(addr)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 8,
                    memory_size: mem.size(),
                })?;
            stack.push(WasmValue::I64(val as i64))?;
        }
        Instruction::F32Load(_, offset) => {
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx.memories.first().ok_or(TrapError::MemoryOutOfBounds {
                offset: addr,
                size: 4,
                memory_size: 0,
            })?;
            let bits = mem
                .read_u32(addr)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 4,
                    memory_size: mem.size(),
                })?;
            stack.push(WasmValue::F32(f32::from_bits(bits)))?;
        }
        Instruction::F64Load(_, offset) => {
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx.memories.first().ok_or(TrapError::MemoryOutOfBounds {
                offset: addr,
                size: 8,
                memory_size: 0,
            })?;
            let bits = mem
                .read_u64(addr)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 8,
                    memory_size: mem.size(),
                })?;
            stack.push(WasmValue::F64(f64::from_bits(bits)))?;
        }

        // i32 partial loads
        Instruction::I32Load8S(_, offset) => {
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx.memories.first().ok_or(TrapError::MemoryOutOfBounds {
                offset: addr,
                size: 1,
                memory_size: 0,
            })?;
            let val = mem
                .read_u8(addr)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 1,
                    memory_size: mem.size(),
                })?;
            stack.push(WasmValue::I32(val as i8 as i32))?;
        }
        Instruction::I32Load8U(_, offset) => {
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx.memories.first().ok_or(TrapError::MemoryOutOfBounds {
                offset: addr,
                size: 1,
                memory_size: 0,
            })?;
            let val = mem
                .read_u8(addr)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 1,
                    memory_size: mem.size(),
                })?;
            stack.push(WasmValue::I32(val as i32))?;
        }
        Instruction::I32Load16S(_, offset) => {
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx.memories.first().ok_or(TrapError::MemoryOutOfBounds {
                offset: addr,
                size: 2,
                memory_size: 0,
            })?;
            let val = mem
                .read_u16(addr)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 2,
                    memory_size: mem.size(),
                })?;
            stack.push(WasmValue::I32(val as i16 as i32))?;
        }
        Instruction::I32Load16U(_, offset) => {
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx.memories.first().ok_or(TrapError::MemoryOutOfBounds {
                offset: addr,
                size: 2,
                memory_size: 0,
            })?;
            let val = mem
                .read_u16(addr)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 2,
                    memory_size: mem.size(),
                })?;
            stack.push(WasmValue::I32(val as i32))?;
        }

        // i64 partial loads
        Instruction::I64Load8S(_, offset) => {
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx.memories.first().ok_or(TrapError::MemoryOutOfBounds {
                offset: addr,
                size: 1,
                memory_size: 0,
            })?;
            let val = mem
                .read_u8(addr)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 1,
                    memory_size: mem.size(),
                })?;
            stack.push(WasmValue::I64(val as i8 as i64))?;
        }
        Instruction::I64Load8U(_, offset) => {
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx.memories.first().ok_or(TrapError::MemoryOutOfBounds {
                offset: addr,
                size: 1,
                memory_size: 0,
            })?;
            let val = mem
                .read_u8(addr)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 1,
                    memory_size: mem.size(),
                })?;
            stack.push(WasmValue::I64(val as i64))?;
        }
        Instruction::I64Load16S(_, offset) => {
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx.memories.first().ok_or(TrapError::MemoryOutOfBounds {
                offset: addr,
                size: 2,
                memory_size: 0,
            })?;
            let val = mem
                .read_u16(addr)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 2,
                    memory_size: mem.size(),
                })?;
            stack.push(WasmValue::I64(val as i16 as i64))?;
        }
        Instruction::I64Load16U(_, offset) => {
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx.memories.first().ok_or(TrapError::MemoryOutOfBounds {
                offset: addr,
                size: 2,
                memory_size: 0,
            })?;
            let val = mem
                .read_u16(addr)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 2,
                    memory_size: mem.size(),
                })?;
            stack.push(WasmValue::I64(val as i64))?;
        }
        Instruction::I64Load32S(_, offset) => {
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx.memories.first().ok_or(TrapError::MemoryOutOfBounds {
                offset: addr,
                size: 4,
                memory_size: 0,
            })?;
            let val = mem
                .read_u32(addr)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 4,
                    memory_size: mem.size(),
                })?;
            stack.push(WasmValue::I64(val as i32 as i64))?;
        }
        Instruction::I64Load32U(_, offset) => {
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx.memories.first().ok_or(TrapError::MemoryOutOfBounds {
                offset: addr,
                size: 4,
                memory_size: 0,
            })?;
            let val = mem
                .read_u32(addr)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 4,
                    memory_size: mem.size(),
                })?;
            stack.push(WasmValue::I64(val as i64))?;
        }

        // ====================================================================
        // Memory Store
        // ====================================================================
        Instruction::I32Store(_, offset) => {
            let val = stack.pop_i32()?;
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx
                .memories
                .first_mut()
                .ok_or(TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 4,
                    memory_size: 0,
                })?;
            mem.write_u32(addr, val as u32)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 4,
                    memory_size: mem.size(),
                })?;
        }
        Instruction::I64Store(_, offset) => {
            let val = stack.pop_i64()?;
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx
                .memories
                .first_mut()
                .ok_or(TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 8,
                    memory_size: 0,
                })?;
            mem.write_u64(addr, val as u64)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 8,
                    memory_size: mem.size(),
                })?;
        }
        Instruction::F32Store(_, offset) => {
            let val = stack.pop_f32()?;
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx
                .memories
                .first_mut()
                .ok_or(TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 4,
                    memory_size: 0,
                })?;
            mem.write_u32(addr, val.to_bits())
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 4,
                    memory_size: mem.size(),
                })?;
        }
        Instruction::F64Store(_, offset) => {
            let val = stack.pop_f64()?;
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx
                .memories
                .first_mut()
                .ok_or(TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 8,
                    memory_size: 0,
                })?;
            mem.write_u64(addr, val.to_bits())
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 8,
                    memory_size: mem.size(),
                })?;
        }

        // Partial stores
        Instruction::I32Store8(_, offset) => {
            let val = stack.pop_i32()?;
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx
                .memories
                .first_mut()
                .ok_or(TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 1,
                    memory_size: 0,
                })?;
            mem.write_u8(addr, val as u8)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 1,
                    memory_size: mem.size(),
                })?;
        }
        Instruction::I32Store16(_, offset) => {
            let val = stack.pop_i32()?;
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx
                .memories
                .first_mut()
                .ok_or(TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 2,
                    memory_size: 0,
                })?;
            mem.write_u16(addr, val as u16)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 2,
                    memory_size: mem.size(),
                })?;
        }
        Instruction::I64Store8(_, offset) => {
            let val = stack.pop_i64()?;
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx
                .memories
                .first_mut()
                .ok_or(TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 1,
                    memory_size: 0,
                })?;
            mem.write_u8(addr, val as u8)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 1,
                    memory_size: mem.size(),
                })?;
        }
        Instruction::I64Store16(_, offset) => {
            let val = stack.pop_i64()?;
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx
                .memories
                .first_mut()
                .ok_or(TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 2,
                    memory_size: 0,
                })?;
            mem.write_u16(addr, val as u16)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 2,
                    memory_size: mem.size(),
                })?;
        }
        Instruction::I64Store32(_, offset) => {
            let val = stack.pop_i64()?;
            let base = stack.pop_i32()? as u32;
            let addr = (base as u64 + *offset as u64) as usize;
            let mem = ctx
                .memories
                .first_mut()
                .ok_or(TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 4,
                    memory_size: 0,
                })?;
            mem.write_u32(addr, val as u32)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: addr,
                    size: 4,
                    memory_size: mem.size(),
                })?;
        }

        // ====================================================================
        // Memory Size/Grow
        // ====================================================================
        Instruction::MemorySize => {
            let pages = ctx.memories.first().map(|m| m.pages()).unwrap_or(0);
            stack.push(WasmValue::I32(pages as i32))?;
        }
        Instruction::MemoryGrow => {
            let delta = stack.pop_i32()? as u32;
            if let Some(mem) = ctx.memories.first_mut() {
                match mem.grow(delta) {
                    Ok(old_pages) => stack.push(WasmValue::I32(old_pages as i32))?,
                    Err(_) => stack.push(WasmValue::I32(-1))?,
                }
            } else {
                stack.push(WasmValue::I32(-1))?;
            }
        }
        Instruction::MemoryInit(data_idx) => {
            // memory.init: copy data from passive data segment into memory
            let n = stack.pop_i32()? as u32;     // byte count
            let s = stack.pop_i32()? as u32;     // source offset in data segment
            let d = stack.pop_i32()? as u32;     // destination offset in memory
            let seg = ctx.module.data.get(*data_idx as usize).ok_or(
                TrapError::ExecutionError(String::from("data segment index OOB")),
            )?;
            let src_end = s.checked_add(n).ok_or(TrapError::MemoryOutOfBounds {
                offset: s as usize,
                size: n as usize,
                memory_size: seg.data.len(),
            })? as usize;
            if src_end > seg.data.len() {
                return Err(TrapError::MemoryOutOfBounds {
                    offset: s as usize,
                    size: n as usize,
                    memory_size: seg.data.len(),
                });
            }
            let data_bytes = seg.data[s as usize..src_end].to_vec();
            let mem = ctx.memories.first_mut().ok_or(TrapError::MemoryOutOfBounds {
                offset: d as usize,
                size: n as usize,
                memory_size: 0,
            })?;
            mem.write_bytes(d as usize, &data_bytes)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: d as usize,
                    size: n as usize,
                    memory_size: mem.size(),
                })?;
        }
        Instruction::DataDrop(data_idx) => {
            // data.drop: mark data segment as dropped by clearing its
            // data bytes. This frees memory and prevents future
            // memory.init from using this segment (as required by spec).
            if let Some(seg) = ctx.module.data.get_mut(*data_idx as usize) {
                seg.data.clear();
                seg.passive = false; // mark as dropped
            }
        }
        Instruction::MemoryCopy => {
            // memory.copy: copy bytes within the same memory (overlapping safe)
            let n = stack.pop_i32()? as usize;   // byte count
            let s = stack.pop_i32()? as usize;   // source offset
            let d = stack.pop_i32()? as usize;   // destination offset
            let mem = ctx.memories.first_mut().ok_or(TrapError::MemoryOutOfBounds {
                offset: d,
                size: n,
                memory_size: 0,
            })?;
            let mem_size = mem.size();
            if s.checked_add(n).map_or(true, |end| end > mem_size)
                || d.checked_add(n).map_or(true, |end| end > mem_size)
            {
                return Err(TrapError::MemoryOutOfBounds {
                    offset: d.max(s),
                    size: n,
                    memory_size: mem_size,
                });
            }
            mem.copy_within(s, d, n)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: d,
                    size: n,
                    memory_size: mem_size,
                })?;
        }
        Instruction::MemoryFill => {
            // memory.fill: fill a memory region with a byte value
            let n = stack.pop_i32()? as usize;   // byte count
            let val = stack.pop_i32()? as u8;     // fill value
            let d = stack.pop_i32()? as usize;    // destination offset
            let mem = ctx.memories.first_mut().ok_or(TrapError::MemoryOutOfBounds {
                offset: d,
                size: n,
                memory_size: 0,
            })?;
            let mem_size = mem.size();
            if d.checked_add(n).map_or(true, |end| end > mem_size) {
                return Err(TrapError::MemoryOutOfBounds {
                    offset: d,
                    size: n,
                    memory_size: mem_size,
                });
            }
            mem.fill(d, n, val)
                .map_err(|_| TrapError::MemoryOutOfBounds {
                    offset: d,
                    size: n,
                    memory_size: mem_size,
                })?;
        }

        // ====================================================================
        // Constants
        // ====================================================================
        Instruction::I32Const(v) => stack.push(WasmValue::I32(*v))?,
        Instruction::I64Const(v) => stack.push(WasmValue::I64(*v))?,
        Instruction::F32Const(v) => stack.push(WasmValue::F32(*v))?,
        Instruction::F64Const(v) => stack.push(WasmValue::F64(*v))?,

        // ====================================================================
        // i32 Comparison
        // ====================================================================
        Instruction::I32Eqz => {
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(if a == 0 { 1 } else { 0 }))?;
        }
        Instruction::I32Eq => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(if a == b { 1 } else { 0 }))?;
        }
        Instruction::I32Ne => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(if a != b { 1 } else { 0 }))?;
        }
        Instruction::I32LtS => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(if a < b { 1 } else { 0 }))?;
        }
        Instruction::I32LtU => {
            let b = stack.pop_i32()? as u32;
            let a = stack.pop_i32()? as u32;
            stack.push(WasmValue::I32(if a < b { 1 } else { 0 }))?;
        }
        Instruction::I32GtS => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(if a > b { 1 } else { 0 }))?;
        }
        Instruction::I32GtU => {
            let b = stack.pop_i32()? as u32;
            let a = stack.pop_i32()? as u32;
            stack.push(WasmValue::I32(if a > b { 1 } else { 0 }))?;
        }
        Instruction::I32LeS => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(if a <= b { 1 } else { 0 }))?;
        }
        Instruction::I32LeU => {
            let b = stack.pop_i32()? as u32;
            let a = stack.pop_i32()? as u32;
            stack.push(WasmValue::I32(if a <= b { 1 } else { 0 }))?;
        }
        Instruction::I32GeS => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(if a >= b { 1 } else { 0 }))?;
        }
        Instruction::I32GeU => {
            let b = stack.pop_i32()? as u32;
            let a = stack.pop_i32()? as u32;
            stack.push(WasmValue::I32(if a >= b { 1 } else { 0 }))?;
        }

        // ====================================================================
        // i64 Comparison
        // ====================================================================
        Instruction::I64Eqz => {
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I32(if a == 0 { 1 } else { 0 }))?;
        }
        Instruction::I64Eq => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I32(if a == b { 1 } else { 0 }))?;
        }
        Instruction::I64Ne => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I32(if a != b { 1 } else { 0 }))?;
        }
        Instruction::I64LtS => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I32(if a < b { 1 } else { 0 }))?;
        }
        Instruction::I64LtU => {
            let b = stack.pop_i64()? as u64;
            let a = stack.pop_i64()? as u64;
            stack.push(WasmValue::I32(if a < b { 1 } else { 0 }))?;
        }
        Instruction::I64GtS => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I32(if a > b { 1 } else { 0 }))?;
        }
        Instruction::I64GtU => {
            let b = stack.pop_i64()? as u64;
            let a = stack.pop_i64()? as u64;
            stack.push(WasmValue::I32(if a > b { 1 } else { 0 }))?;
        }
        Instruction::I64LeS => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I32(if a <= b { 1 } else { 0 }))?;
        }
        Instruction::I64LeU => {
            let b = stack.pop_i64()? as u64;
            let a = stack.pop_i64()? as u64;
            stack.push(WasmValue::I32(if a <= b { 1 } else { 0 }))?;
        }
        Instruction::I64GeS => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I32(if a >= b { 1 } else { 0 }))?;
        }
        Instruction::I64GeU => {
            let b = stack.pop_i64()? as u64;
            let a = stack.pop_i64()? as u64;
            stack.push(WasmValue::I32(if a >= b { 1 } else { 0 }))?;
        }

        // ====================================================================
        // f32 Comparison
        // ====================================================================
        Instruction::F32Eq => {
            let b = stack.pop_f32()?;
            let a = stack.pop_f32()?;
            stack.push(WasmValue::I32(if a == b { 1 } else { 0 }))?;
        }
        Instruction::F32Ne => {
            let b = stack.pop_f32()?;
            let a = stack.pop_f32()?;
            stack.push(WasmValue::I32(if a != b { 1 } else { 0 }))?;
        }
        Instruction::F32Lt => {
            let b = stack.pop_f32()?;
            let a = stack.pop_f32()?;
            stack.push(WasmValue::I32(if a < b { 1 } else { 0 }))?;
        }
        Instruction::F32Gt => {
            let b = stack.pop_f32()?;
            let a = stack.pop_f32()?;
            stack.push(WasmValue::I32(if a > b { 1 } else { 0 }))?;
        }
        Instruction::F32Le => {
            let b = stack.pop_f32()?;
            let a = stack.pop_f32()?;
            stack.push(WasmValue::I32(if a <= b { 1 } else { 0 }))?;
        }
        Instruction::F32Ge => {
            let b = stack.pop_f32()?;
            let a = stack.pop_f32()?;
            stack.push(WasmValue::I32(if a >= b { 1 } else { 0 }))?;
        }

        // ====================================================================
        // f64 Comparison
        // ====================================================================
        Instruction::F64Eq => {
            let b = stack.pop_f64()?;
            let a = stack.pop_f64()?;
            stack.push(WasmValue::I32(if a == b { 1 } else { 0 }))?;
        }
        Instruction::F64Ne => {
            let b = stack.pop_f64()?;
            let a = stack.pop_f64()?;
            stack.push(WasmValue::I32(if a != b { 1 } else { 0 }))?;
        }
        Instruction::F64Lt => {
            let b = stack.pop_f64()?;
            let a = stack.pop_f64()?;
            stack.push(WasmValue::I32(if a < b { 1 } else { 0 }))?;
        }
        Instruction::F64Gt => {
            let b = stack.pop_f64()?;
            let a = stack.pop_f64()?;
            stack.push(WasmValue::I32(if a > b { 1 } else { 0 }))?;
        }
        Instruction::F64Le => {
            let b = stack.pop_f64()?;
            let a = stack.pop_f64()?;
            stack.push(WasmValue::I32(if a <= b { 1 } else { 0 }))?;
        }
        Instruction::F64Ge => {
            let b = stack.pop_f64()?;
            let a = stack.pop_f64()?;
            stack.push(WasmValue::I32(if a >= b { 1 } else { 0 }))?;
        }

        // ====================================================================
        // i32 Arithmetic
        // ====================================================================
        Instruction::I32Clz => {
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(a.leading_zeros() as i32))?;
        }
        Instruction::I32Ctz => {
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(a.trailing_zeros() as i32))?;
        }
        Instruction::I32Popcnt => {
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(a.count_ones() as i32))?;
        }
        Instruction::I32Add => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(a.wrapping_add(b)))?;
        }
        Instruction::I32Sub => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(a.wrapping_sub(b)))?;
        }
        Instruction::I32Mul => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(a.wrapping_mul(b)))?;
        }
        Instruction::I32DivS => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            if b == 0 {
                return Err(TrapError::DivisionByZero);
            }
            if a == i32::MIN && b == -1 {
                return Err(TrapError::IntegerOverflow);
            }
            stack.push(WasmValue::I32(a.wrapping_div(b)))?;
        }
        Instruction::I32DivU => {
            let b = stack.pop_i32()? as u32;
            let a = stack.pop_i32()? as u32;
            if b == 0 {
                return Err(TrapError::DivisionByZero);
            }
            stack.push(WasmValue::I32((a / b) as i32))?;
        }
        Instruction::I32RemS => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            if b == 0 {
                return Err(TrapError::DivisionByZero);
            }
            stack.push(WasmValue::I32(if a == i32::MIN && b == -1 {
                0
            } else {
                a.wrapping_rem(b)
            }))?;
        }
        Instruction::I32RemU => {
            let b = stack.pop_i32()? as u32;
            let a = stack.pop_i32()? as u32;
            if b == 0 {
                return Err(TrapError::DivisionByZero);
            }
            stack.push(WasmValue::I32((a % b) as i32))?;
        }
        Instruction::I32And => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(a & b))?;
        }
        Instruction::I32Or => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(a | b))?;
        }
        Instruction::I32Xor => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(a ^ b))?;
        }
        Instruction::I32Shl => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(a.wrapping_shl(b as u32 % 32)))?;
        }
        Instruction::I32ShrS => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(a.wrapping_shr(b as u32 % 32)))?;
        }
        Instruction::I32ShrU => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()? as u32;
            stack.push(WasmValue::I32((a.wrapping_shr(b as u32 % 32)) as i32))?;
        }
        Instruction::I32Rotl => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(a.rotate_left(b as u32 % 32)))?;
        }
        Instruction::I32Rotr => {
            let b = stack.pop_i32()?;
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(a.rotate_right(b as u32 % 32)))?;
        }

        // ====================================================================
        // i64 Arithmetic
        // ====================================================================
        Instruction::I64Clz => {
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a.leading_zeros() as i64))?;
        }
        Instruction::I64Ctz => {
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a.trailing_zeros() as i64))?;
        }
        Instruction::I64Popcnt => {
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a.count_ones() as i64))?;
        }
        Instruction::I64Add => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a.wrapping_add(b)))?;
        }
        Instruction::I64Sub => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a.wrapping_sub(b)))?;
        }
        Instruction::I64Mul => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a.wrapping_mul(b)))?;
        }
        Instruction::I64DivS => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            if b == 0 {
                return Err(TrapError::DivisionByZero);
            }
            if a == i64::MIN && b == -1 {
                return Err(TrapError::IntegerOverflow);
            }
            stack.push(WasmValue::I64(a.wrapping_div(b)))?;
        }
        Instruction::I64DivU => {
            let b = stack.pop_i64()? as u64;
            let a = stack.pop_i64()? as u64;
            if b == 0 {
                return Err(TrapError::DivisionByZero);
            }
            stack.push(WasmValue::I64((a / b) as i64))?;
        }
        Instruction::I64RemS => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            if b == 0 {
                return Err(TrapError::DivisionByZero);
            }
            stack.push(WasmValue::I64(if a == i64::MIN && b == -1 {
                0
            } else {
                a.wrapping_rem(b)
            }))?;
        }
        Instruction::I64RemU => {
            let b = stack.pop_i64()? as u64;
            let a = stack.pop_i64()? as u64;
            if b == 0 {
                return Err(TrapError::DivisionByZero);
            }
            stack.push(WasmValue::I64((a % b) as i64))?;
        }
        Instruction::I64And => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a & b))?;
        }
        Instruction::I64Or => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a | b))?;
        }
        Instruction::I64Xor => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a ^ b))?;
        }
        Instruction::I64Shl => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a.wrapping_shl((b as u32) % 64)))?;
        }
        Instruction::I64ShrS => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a.wrapping_shr((b as u32) % 64)))?;
        }
        Instruction::I64ShrU => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()? as u64;
            stack.push(WasmValue::I64((a.wrapping_shr((b as u32) % 64)) as i64))?;
        }
        Instruction::I64Rotl => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a.rotate_left((b as u32) % 64)))?;
        }
        Instruction::I64Rotr => {
            let b = stack.pop_i64()?;
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a.rotate_right((b as u32) % 64)))?;
        }

        // ====================================================================
        // f32 Arithmetic
        // ====================================================================
        Instruction::F32Abs => {
            let a = stack.pop_f32()?;
            stack.push(WasmValue::F32(f32_abs(a)))?;
        }
        Instruction::F32Neg => {
            let a = stack.pop_f32()?;
            stack.push(WasmValue::F32(f32_neg(a)))?;
        }
        Instruction::F32Ceil => {
            let a = stack.pop_f32()?;
            stack.push(WasmValue::F32(f32_ceil(a)))?;
        }
        Instruction::F32Floor => {
            let a = stack.pop_f32()?;
            stack.push(WasmValue::F32(f32_floor(a)))?;
        }
        Instruction::F32Trunc => {
            let a = stack.pop_f32()?;
            stack.push(WasmValue::F32(f32_trunc(a)))?;
        }
        Instruction::F32Nearest => {
            let a = stack.pop_f32()?;
            stack.push(WasmValue::F32(f32_nearest(a)))?;
        }
        Instruction::F32Sqrt => {
            let a = stack.pop_f32()?;
            stack.push(WasmValue::F32(f32_sqrt(a)))?;
        }
        Instruction::F32Add => {
            let b = stack.pop_f32()?;
            let a = stack.pop_f32()?;
            stack.push(WasmValue::F32(a + b))?;
        }
        Instruction::F32Sub => {
            let b = stack.pop_f32()?;
            let a = stack.pop_f32()?;
            stack.push(WasmValue::F32(a - b))?;
        }
        Instruction::F32Mul => {
            let b = stack.pop_f32()?;
            let a = stack.pop_f32()?;
            stack.push(WasmValue::F32(a * b))?;
        }
        Instruction::F32Div => {
            let b = stack.pop_f32()?;
            let a = stack.pop_f32()?;
            stack.push(WasmValue::F32(a / b))?;
        }
        Instruction::F32Min => {
            let b = stack.pop_f32()?;
            let a = stack.pop_f32()?;
            stack.push(WasmValue::F32(f32_min(a, b)))?;
        }
        Instruction::F32Max => {
            let b = stack.pop_f32()?;
            let a = stack.pop_f32()?;
            stack.push(WasmValue::F32(f32_max(a, b)))?;
        }
        Instruction::F32Copysign => {
            let b = stack.pop_f32()?;
            let a = stack.pop_f32()?;
            stack.push(WasmValue::F32(f32_copysign(a, b)))?;
        }

        // ====================================================================
        // f64 Arithmetic
        // ====================================================================
        Instruction::F64Abs => {
            let a = stack.pop_f64()?;
            stack.push(WasmValue::F64(f64_abs(a)))?;
        }
        Instruction::F64Neg => {
            let a = stack.pop_f64()?;
            stack.push(WasmValue::F64(f64_neg(a)))?;
        }
        Instruction::F64Ceil => {
            let a = stack.pop_f64()?;
            stack.push(WasmValue::F64(f64_ceil(a)))?;
        }
        Instruction::F64Floor => {
            let a = stack.pop_f64()?;
            stack.push(WasmValue::F64(f64_floor(a)))?;
        }
        Instruction::F64Trunc => {
            let a = stack.pop_f64()?;
            stack.push(WasmValue::F64(f64_trunc(a)))?;
        }
        Instruction::F64Nearest => {
            let a = stack.pop_f64()?;
            stack.push(WasmValue::F64(f64_nearest(a)))?;
        }
        Instruction::F64Sqrt => {
            let a = stack.pop_f64()?;
            stack.push(WasmValue::F64(f64_sqrt(a)))?;
        }
        Instruction::F64Add => {
            let b = stack.pop_f64()?;
            let a = stack.pop_f64()?;
            stack.push(WasmValue::F64(a + b))?;
        }
        Instruction::F64Sub => {
            let b = stack.pop_f64()?;
            let a = stack.pop_f64()?;
            stack.push(WasmValue::F64(a - b))?;
        }
        Instruction::F64Mul => {
            let b = stack.pop_f64()?;
            let a = stack.pop_f64()?;
            stack.push(WasmValue::F64(a * b))?;
        }
        Instruction::F64Div => {
            let b = stack.pop_f64()?;
            let a = stack.pop_f64()?;
            stack.push(WasmValue::F64(a / b))?;
        }
        Instruction::F64Min => {
            let b = stack.pop_f64()?;
            let a = stack.pop_f64()?;
            stack.push(WasmValue::F64(f64_min(a, b)))?;
        }
        Instruction::F64Max => {
            let b = stack.pop_f64()?;
            let a = stack.pop_f64()?;
            stack.push(WasmValue::F64(f64_max(a, b)))?;
        }
        Instruction::F64Copysign => {
            let b = stack.pop_f64()?;
            let a = stack.pop_f64()?;
            stack.push(WasmValue::F64(f64_copysign(a, b)))?;
        }

        // ====================================================================
        // Conversions
        // ====================================================================
        Instruction::I32WrapI64 => {
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I32(a as i32))?;
        }
        Instruction::I32TruncF32S => {
            let a = stack.pop_f32()?;
            if a.is_nan() {
                return Err(TrapError::InvalidConversionToInteger);
            }
            if a >= 2147483648.0_f32 || a < -2147483648.0_f32 {
                return Err(TrapError::IntegerOverflow);
            }
            stack.push(WasmValue::I32(a as i32))?;
        }
        Instruction::I32TruncF32U => {
            let a = stack.pop_f32()?;
            if a.is_nan() {
                return Err(TrapError::InvalidConversionToInteger);
            }
            if a >= 4294967296.0_f32 || a <= -1.0_f32 {
                return Err(TrapError::IntegerOverflow);
            }
            stack.push(WasmValue::I32(a as u32 as i32))?;
        }
        Instruction::I32TruncF64S => {
            let a = stack.pop_f64()?;
            if a.is_nan() {
                return Err(TrapError::InvalidConversionToInteger);
            }
            if a >= 2147483648.0_f64 || a <= -2147483649.0_f64 {
                return Err(TrapError::IntegerOverflow);
            }
            stack.push(WasmValue::I32(a as i32))?;
        }
        Instruction::I32TruncF64U => {
            let a = stack.pop_f64()?;
            if a.is_nan() {
                return Err(TrapError::InvalidConversionToInteger);
            }
            if a >= 4294967296.0_f64 || a <= -1.0_f64 {
                return Err(TrapError::IntegerOverflow);
            }
            stack.push(WasmValue::I32(a as u32 as i32))?;
        }
        Instruction::I64ExtendI32S => {
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I64(a as i64))?;
        }
        Instruction::I64ExtendI32U => {
            let a = stack.pop_i32()? as u32;
            stack.push(WasmValue::I64(a as i64))?;
        }
        Instruction::I64TruncF32S => {
            let a = stack.pop_f32()?;
            if a.is_nan() {
                return Err(TrapError::InvalidConversionToInteger);
            }
            if a >= 9223372036854775808.0_f32 || a < -9223372036854775808.0_f32 {
                return Err(TrapError::IntegerOverflow);
            }
            stack.push(WasmValue::I64(a as i64))?;
        }
        Instruction::I64TruncF32U => {
            let a = stack.pop_f32()?;
            if a.is_nan() {
                return Err(TrapError::InvalidConversionToInteger);
            }
            if a >= 18446744073709551616.0_f32 || a <= -1.0_f32 {
                return Err(TrapError::IntegerOverflow);
            }
            stack.push(WasmValue::I64(a as u64 as i64))?;
        }
        Instruction::I64TruncF64S => {
            let a = stack.pop_f64()?;
            if a.is_nan() {
                return Err(TrapError::InvalidConversionToInteger);
            }
            if a >= 9223372036854775808.0_f64 || a < -9223372036854775808.0_f64 {
                return Err(TrapError::IntegerOverflow);
            }
            stack.push(WasmValue::I64(a as i64))?;
        }
        Instruction::I64TruncF64U => {
            let a = stack.pop_f64()?;
            if a.is_nan() {
                return Err(TrapError::InvalidConversionToInteger);
            }
            if a >= 18446744073709551616.0_f64 || a <= -1.0_f64 {
                return Err(TrapError::IntegerOverflow);
            }
            stack.push(WasmValue::I64(a as u64 as i64))?;
        }
        Instruction::F32ConvertI32S => {
            let a = stack.pop_i32()?;
            stack.push(WasmValue::F32(a as f32))?;
        }
        Instruction::F32ConvertI32U => {
            let a = stack.pop_i32()? as u32;
            stack.push(WasmValue::F32(a as f32))?;
        }
        Instruction::F32ConvertI64S => {
            let a = stack.pop_i64()?;
            stack.push(WasmValue::F32(a as f32))?;
        }
        Instruction::F32ConvertI64U => {
            let a = stack.pop_i64()? as u64;
            stack.push(WasmValue::F32(a as f32))?;
        }
        Instruction::F32DemoteF64 => {
            let a = stack.pop_f64()?;
            stack.push(WasmValue::F32(a as f32))?;
        }
        Instruction::F64ConvertI32S => {
            let a = stack.pop_i32()?;
            stack.push(WasmValue::F64(a as f64))?;
        }
        Instruction::F64ConvertI32U => {
            let a = stack.pop_i32()? as u32;
            stack.push(WasmValue::F64(a as f64))?;
        }
        Instruction::F64ConvertI64S => {
            let a = stack.pop_i64()?;
            stack.push(WasmValue::F64(a as f64))?;
        }
        Instruction::F64ConvertI64U => {
            let a = stack.pop_i64()? as u64;
            stack.push(WasmValue::F64(a as f64))?;
        }
        Instruction::F64PromoteF32 => {
            let a = stack.pop_f32()?;
            stack.push(WasmValue::F64(a as f64))?;
        }

        // ====================================================================
        // Reinterpretations
        // ====================================================================
        Instruction::I32ReinterpretF32 => {
            let a = stack.pop_f32()?;
            stack.push(WasmValue::I32(a.to_bits() as i32))?;
        }
        Instruction::I64ReinterpretF64 => {
            let a = stack.pop_f64()?;
            stack.push(WasmValue::I64(a.to_bits() as i64))?;
        }
        Instruction::F32ReinterpretI32 => {
            let a = stack.pop_i32()?;
            stack.push(WasmValue::F32(f32::from_bits(a as u32)))?;
        }
        Instruction::F64ReinterpretI64 => {
            let a = stack.pop_i64()?;
            stack.push(WasmValue::F64(f64::from_bits(a as u64)))?;
        }

        // ====================================================================
        // Sign Extension (post-MVP)
        // ====================================================================
        Instruction::I32Extend8S => {
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(a as i8 as i32))?;
        }
        Instruction::I32Extend16S => {
            let a = stack.pop_i32()?;
            stack.push(WasmValue::I32(a as i16 as i32))?;
        }
        Instruction::I64Extend8S => {
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a as i8 as i64))?;
        }
        Instruction::I64Extend16S => {
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a as i16 as i64))?;
        }
        Instruction::I64Extend32S => {
            let a = stack.pop_i64()?;
            stack.push(WasmValue::I64(a as i32 as i64))?;
        }

        // ====================================================================
        // Saturating Truncation
        // ====================================================================
        Instruction::I32TruncSatF32S => {
            let a = stack.pop_f32()?;
            stack.push(WasmValue::I32(trunc_sat_f32_i32(a)))?;
        }
        Instruction::I32TruncSatF32U => {
            let a = stack.pop_f32()?;
            stack.push(WasmValue::I32(trunc_sat_f32_u32(a) as i32))?;
        }
        Instruction::I32TruncSatF64S => {
            let a = stack.pop_f64()?;
            stack.push(WasmValue::I32(trunc_sat_f64_i32(a)))?;
        }
        Instruction::I32TruncSatF64U => {
            let a = stack.pop_f64()?;
            stack.push(WasmValue::I32(trunc_sat_f64_u32(a) as i32))?;
        }
        Instruction::I64TruncSatF32S => {
            let a = stack.pop_f32()?;
            stack.push(WasmValue::I64(trunc_sat_f32_i64(a)))?;
        }
        Instruction::I64TruncSatF32U => {
            let a = stack.pop_f32()?;
            stack.push(WasmValue::I64(trunc_sat_f32_u64(a) as i64))?;
        }
        Instruction::I64TruncSatF64S => {
            let a = stack.pop_f64()?;
            stack.push(WasmValue::I64(trunc_sat_f64_i64(a)))?;
        }
        Instruction::I64TruncSatF64U => {
            let a = stack.pop_f64()?;
            stack.push(WasmValue::I64(trunc_sat_f64_u64(a) as i64))?;
        }
    }

    Ok(ControlFlow::Continue)
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get instructions for the current frame's function.
fn get_current_instructions<'a>(ctx: &'a ExecutorContext, frame: &CallFrame) -> &'a [Instruction] {
    let import_count = ctx.module.import_function_count();
    let local_idx = frame.func_idx as usize - import_count;
    &ctx.module.code[local_idx].instructions
}

/// Branch to a label by depth index.
fn branch(stack: &mut ValueStack, frame: &mut CallFrame, label_idx: u32) -> Result<(), TrapError> {
    let block_idx = frame.block_stack.len() - 1 - label_idx as usize;
    let target_block = &frame.block_stack[block_idx];

    let arity = target_block.arity;
    let base = target_block.stack_depth;

    if target_block.kind == BlockKind::Loop {
        // Branch to loop start
        let results = collect_results(stack, arity, base);
        stack.truncate(base);
        for r in results {
            stack.push(r)?;
        }
        frame.pc = target_block.start_pc + 1; // right after the Loop instruction
    } else {
        // Branch to block end
        let results = collect_results(stack, arity, base);
        stack.truncate(base);
        for r in results {
            stack.push(r)?;
        }
        let end_pc = target_block.end_pc;
        // Pop all blocks up to and including the target
        frame.block_stack.truncate(block_idx);
        frame.pc = end_pc + 1;
    }

    Ok(())
}

/// Find the matching `end` for a block/loop/if at the given pc.
fn find_block_end(instructions: &[Instruction], start_pc: usize) -> usize {
    let mut depth = 1u32;
    for i in (start_pc + 1)..instructions.len() {
        match &instructions[i] {
            Instruction::Block(_) | Instruction::Loop(_) | Instruction::If(_) => depth += 1,
            Instruction::End => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
    }
    instructions.len().saturating_sub(1)
}

/// Find the `else` for an `if` at the given pc.
fn find_else(instructions: &[Instruction], start_pc: usize) -> Option<usize> {
    let mut depth = 1u32;
    for i in (start_pc + 1)..instructions.len() {
        match &instructions[i] {
            Instruction::Block(_) | Instruction::Loop(_) | Instruction::If(_) => depth += 1,
            Instruction::Else if depth == 1 => return Some(i),
            Instruction::End => {
                depth -= 1;
                if depth == 0 {
                    return None;
                }
            }
            _ => {}
        }
    }
    None
}

/// Pre-compute the block map (not used currently, but available for optimization).
fn compute_block_map(_instructions: &[Instruction]) -> () {
    // Could build a HashMap<pc, end_pc> for O(1) lookups
    // For now we do linear scan
}

/// Block return arity from block type.
fn block_arity(bt: &BlockType, module: &Module) -> usize {
    match bt {
        BlockType::Empty => 0,
        BlockType::Value(_) => 1,
        BlockType::TypeIndex(idx) => module
            .types
            .get(*idx as usize)
            .map(|t| t.results.len())
            .unwrap_or(0),
    }
}

/// Block parameter arity from block type.
fn block_param_arity(bt: &BlockType, module: &Module) -> usize {
    match bt {
        BlockType::Empty => 0,
        BlockType::Value(_) => 0,
        BlockType::TypeIndex(idx) => module
            .types
            .get(*idx as usize)
            .map(|t| t.params.len())
            .unwrap_or(0),
    }
}

// ============================================================================
// IEEE 754 Float Helpers (no_std compatible)
// ============================================================================

fn f32_abs(a: f32) -> f32 {
    f32::from_bits(a.to_bits() & 0x7FFF_FFFF)
}
fn f32_neg(a: f32) -> f32 {
    f32::from_bits(a.to_bits() ^ 0x8000_0000)
}
fn f64_abs(a: f64) -> f64 {
    f64::from_bits(a.to_bits() & 0x7FFF_FFFF_FFFF_FFFF)
}
fn f64_neg(a: f64) -> f64 {
    f64::from_bits(a.to_bits() ^ 0x8000_0000_0000_0000)
}

fn f32_copysign(a: f32, b: f32) -> f32 {
    f32::from_bits((a.to_bits() & 0x7FFF_FFFF) | (b.to_bits() & 0x8000_0000))
}
fn f64_copysign(a: f64, b: f64) -> f64 {
    f64::from_bits((a.to_bits() & 0x7FFF_FFFF_FFFF_FFFF) | (b.to_bits() & 0x8000_0000_0000_0000))
}

fn f32_min(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        return f32::NAN;
    }
    if a == 0.0 && b == 0.0 {
        return if a.to_bits() & 0x8000_0000 != 0 { a } else { b };
    }
    if a < b {
        a
    } else {
        b
    }
}

fn f32_max(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        return f32::NAN;
    }
    if a == 0.0 && b == 0.0 {
        return if a.to_bits() & 0x8000_0000 != 0 { b } else { a };
    }
    if a > b {
        a
    } else {
        b
    }
}

fn f64_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == 0.0 && b == 0.0 {
        return if a.to_bits() & 0x8000_0000_0000_0000 != 0 {
            a
        } else {
            b
        };
    }
    if a < b {
        a
    } else {
        b
    }
}

fn f64_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == 0.0 && b == 0.0 {
        return if a.to_bits() & 0x8000_0000_0000_0000 != 0 {
            b
        } else {
            a
        };
    }
    if a > b {
        a
    } else {
        b
    }
}

// no_std ceil/floor/trunc/nearest/sqrt via bit manipulation
fn f32_ceil(a: f32) -> f32 {
    if a.is_nan() || a.is_infinite() || a == 0.0 {
        return a;
    }
    let i = a as i32 as f32;
    if a > 0.0 && i < a {
        i + 1.0
    } else {
        i
    }
}

fn f32_floor(a: f32) -> f32 {
    if a.is_nan() || a.is_infinite() || a == 0.0 {
        return a;
    }
    let i = a as i32 as f32;
    if a < 0.0 && i > a {
        i - 1.0
    } else {
        i
    }
}

fn f32_trunc(a: f32) -> f32 {
    if a.is_nan() || a.is_infinite() || a == 0.0 {
        return a;
    }
    (a as i32) as f32
}

fn f32_nearest(a: f32) -> f32 {
    if a.is_nan() || a.is_infinite() || a == 0.0 {
        return a;
    }
    let r = f32_floor(a + 0.5);
    // Ties to even
    if (a + 0.5) == r && r as i32 % 2 != 0 {
        r - 1.0
    } else {
        r
    }
}

fn f32_sqrt(a: f32) -> f32 {
    if a.is_nan() || a < 0.0 {
        return f32::NAN;
    }
    if a == 0.0 || a.is_infinite() {
        return a;
    }
    // Newton's method
    let mut x = f32::from_bits((a.to_bits() >> 1) + 0x1FC00000);
    for _ in 0..8 {
        x = 0.5 * (x + a / x);
    }
    x
}

fn f64_ceil(a: f64) -> f64 {
    if a.is_nan() || a.is_infinite() || a == 0.0 {
        return a;
    }
    let i = a as i64 as f64;
    if a > 0.0 && i < a {
        i + 1.0
    } else {
        i
    }
}

fn f64_floor(a: f64) -> f64 {
    if a.is_nan() || a.is_infinite() || a == 0.0 {
        return a;
    }
    let i = a as i64 as f64;
    if a < 0.0 && i > a {
        i - 1.0
    } else {
        i
    }
}

fn f64_trunc(a: f64) -> f64 {
    if a.is_nan() || a.is_infinite() || a == 0.0 {
        return a;
    }
    (a as i64) as f64
}

fn f64_nearest(a: f64) -> f64 {
    if a.is_nan() || a.is_infinite() || a == 0.0 {
        return a;
    }
    let r = f64_floor(a + 0.5);
    if (a + 0.5) == r && r as i64 % 2 != 0 {
        r - 1.0
    } else {
        r
    }
}

fn f64_sqrt(a: f64) -> f64 {
    if a.is_nan() || a < 0.0 {
        return f64::NAN;
    }
    if a == 0.0 || a.is_infinite() {
        return a;
    }
    let mut x = f64::from_bits((a.to_bits() >> 1) + 0x1FF8_0000_0000_0000);
    for _ in 0..12 {
        x = 0.5 * (x + a / x);
    }
    x
}

// ============================================================================
// Saturating Truncation Helpers
// ============================================================================

fn trunc_sat_f32_i32(a: f32) -> i32 {
    if a.is_nan() {
        return 0;
    }
    if a >= i32::MAX as f32 {
        return i32::MAX;
    }
    if a <= i32::MIN as f32 {
        return i32::MIN;
    }
    a as i32
}

fn trunc_sat_f32_u32(a: f32) -> u32 {
    if a.is_nan() {
        return 0;
    }
    if a >= u32::MAX as f32 {
        return u32::MAX;
    }
    if a <= 0.0 {
        return 0;
    }
    a as u32
}

fn trunc_sat_f64_i32(a: f64) -> i32 {
    if a.is_nan() {
        return 0;
    }
    if a >= i32::MAX as f64 {
        return i32::MAX;
    }
    if a <= i32::MIN as f64 {
        return i32::MIN;
    }
    a as i32
}

fn trunc_sat_f64_u32(a: f64) -> u32 {
    if a.is_nan() {
        return 0;
    }
    if a >= u32::MAX as f64 {
        return u32::MAX;
    }
    if a <= 0.0 {
        return 0;
    }
    a as u32
}

fn trunc_sat_f32_i64(a: f32) -> i64 {
    if a.is_nan() {
        return 0;
    }
    if a >= i64::MAX as f32 {
        return i64::MAX;
    }
    if a <= i64::MIN as f32 {
        return i64::MIN;
    }
    a as i64
}

fn trunc_sat_f32_u64(a: f32) -> u64 {
    if a.is_nan() {
        return 0;
    }
    if a >= u64::MAX as f32 {
        return u64::MAX;
    }
    if a <= 0.0 {
        return 0;
    }
    a as u64
}

fn trunc_sat_f64_i64(a: f64) -> i64 {
    if a.is_nan() {
        return 0;
    }
    if a >= i64::MAX as f64 {
        return i64::MAX;
    }
    if a <= i64::MIN as f64 {
        return i64::MIN;
    }
    a as i64
}

fn trunc_sat_f64_u64(a: f64) -> u64 {
    if a.is_nan() {
        return 0;
    }
    if a >= u64::MAX as f64 {
        return u64::MAX;
    }
    if a <= 0.0 {
        return 0;
    }
    a as u64
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{Export, ExportKind, FunctionBody, FunctionType, Module};
    use crate::opcodes::Instruction::*;
    use crate::parser::BlockType;
    use alloc::vec;

    /// Helper to create a minimal module with one function.
    fn make_module(
        params: Vec<ValueType>,
        results: Vec<ValueType>,
        locals: Vec<(u32, ValueType)>,
        instructions: Vec<crate::opcodes::Instruction>,
        export_name: &str,
    ) -> Module {
        let func_type = FunctionType {
            params: params.clone(),
            results: results.clone(),
        };
        Module {
            types: vec![func_type],
            imports: vec![],
            functions: vec![0], // function 0 has type 0
            tables: vec![],
            memories: vec![],
            globals: vec![],
            exports: vec![Export {
                name: String::from(export_name),
                kind: ExportKind::Function,
                index: 0,
            }],
            start: None,
            elements: vec![],
            code: vec![FunctionBody {
                locals,
                instructions,
                raw_bytes: vec![],
            }],
            data: vec![],
            name: None,
            data_count: None,
        }
    }

    /// Helper to make a module with memory.
    fn make_module_with_memory(
        params: Vec<ValueType>,
        results: Vec<ValueType>,
        locals: Vec<(u32, ValueType)>,
        instructions: Vec<crate::opcodes::Instruction>,
        export_name: &str,
        mem_min: u32,
        mem_max: Option<u32>,
    ) -> Module {
        let mut m = make_module(params, results, locals, instructions, export_name);
        m.memories.push(crate::module::MemoryType {
            min: mem_min,
            max: mem_max,
            shared: false,
        });
        m
    }

    // B-QG1: Hello World (simplified - just return a constant)
    #[test]
    fn test_return_constant() {
        let module = make_module(
            vec![],
            vec![ValueType::I32],
            vec![],
            vec![I32Const(42), End],
            "answer",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(&mut ctx, "answer", &[]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_i32(), Some(42));
    }

    // B-QG2: i32 arithmetic
    #[test]
    fn test_i32_add() {
        let module = make_module(
            vec![ValueType::I32, ValueType::I32],
            vec![ValueType::I32],
            vec![],
            vec![LocalGet(0), LocalGet(1), I32Add, End],
            "add",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result =
            execute_export(&mut ctx, "add", &[WasmValue::I32(3), WasmValue::I32(7)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(10));
    }

    #[test]
    fn test_i32_sub() {
        let module = make_module(
            vec![ValueType::I32, ValueType::I32],
            vec![ValueType::I32],
            vec![],
            vec![LocalGet(0), LocalGet(1), I32Sub, End],
            "sub",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result =
            execute_export(&mut ctx, "sub", &[WasmValue::I32(10), WasmValue::I32(3)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(7));
    }

    #[test]
    fn test_i32_mul() {
        let module = make_module(
            vec![ValueType::I32, ValueType::I32],
            vec![ValueType::I32],
            vec![],
            vec![LocalGet(0), LocalGet(1), I32Mul, End],
            "mul",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result =
            execute_export(&mut ctx, "mul", &[WasmValue::I32(6), WasmValue::I32(7)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(42));
    }

    #[test]
    fn test_i32_wrapping_arithmetic() {
        let module = make_module(
            vec![],
            vec![ValueType::I32],
            vec![],
            vec![I32Const(i32::MAX), I32Const(1), I32Add, End],
            "overflow",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(&mut ctx, "overflow", &[]).unwrap();
        assert_eq!(result[0].as_i32(), Some(i32::MIN));
    }

    #[test]
    fn test_i32_div_and_rem() {
        let module = make_module(
            vec![ValueType::I32, ValueType::I32],
            vec![ValueType::I32],
            vec![],
            vec![LocalGet(0), LocalGet(1), I32DivS, End],
            "div",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result =
            execute_export(&mut ctx, "div", &[WasmValue::I32(10), WasmValue::I32(3)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(3));
    }

    #[test]
    fn test_i32_bitwise() {
        // Test AND
        let module = make_module(
            vec![ValueType::I32, ValueType::I32],
            vec![ValueType::I32],
            vec![],
            vec![LocalGet(0), LocalGet(1), I32And, End],
            "and",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(
            &mut ctx,
            "and",
            &[WasmValue::I32(0xFF00), WasmValue::I32(0x0FF0)],
        )
        .unwrap();
        assert_eq!(result[0].as_i32(), Some(0x0F00));
    }

    #[test]
    fn test_i32_shifts() {
        let module = make_module(
            vec![ValueType::I32, ValueType::I32],
            vec![ValueType::I32],
            vec![],
            vec![LocalGet(0), LocalGet(1), I32Shl, End],
            "shl",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result =
            execute_export(&mut ctx, "shl", &[WasmValue::I32(1), WasmValue::I32(8)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(256));
    }

    #[test]
    fn test_i32_clz_ctz_popcnt() {
        let module = make_module(
            vec![ValueType::I32],
            vec![ValueType::I32],
            vec![],
            vec![LocalGet(0), I32Clz, End],
            "clz",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(&mut ctx, "clz", &[WasmValue::I32(1)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(31));
    }

    // i64 arithmetic
    #[test]
    fn test_i64_add() {
        let module = make_module(
            vec![ValueType::I64, ValueType::I64],
            vec![ValueType::I64],
            vec![],
            vec![LocalGet(0), LocalGet(1), I64Add, End],
            "add64",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(
            &mut ctx,
            "add64",
            &[WasmValue::I64(100), WasmValue::I64(200)],
        )
        .unwrap();
        assert_eq!(result[0].as_i64(), Some(300));
    }

    #[test]
    fn test_i64_mul() {
        let module = make_module(
            vec![ValueType::I64, ValueType::I64],
            vec![ValueType::I64],
            vec![],
            vec![LocalGet(0), LocalGet(1), I64Mul, End],
            "mul64",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(
            &mut ctx,
            "mul64",
            &[WasmValue::I64(1000000), WasmValue::I64(1000000)],
        )
        .unwrap();
        assert_eq!(result[0].as_i64(), Some(1000000000000));
    }

    // B-QG3: f32/f64 IEEE 754
    #[test]
    fn test_f32_arithmetic() {
        let module = make_module(
            vec![ValueType::F32, ValueType::F32],
            vec![ValueType::F32],
            vec![],
            vec![LocalGet(0), LocalGet(1), F32Add, End],
            "fadd",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(
            &mut ctx,
            "fadd",
            &[WasmValue::F32(1.5), WasmValue::F32(2.5)],
        )
        .unwrap();
        assert_eq!(result[0].as_f32(), Some(4.0));
    }

    #[test]
    fn test_f64_arithmetic() {
        let module = make_module(
            vec![ValueType::F64, ValueType::F64],
            vec![ValueType::F64],
            vec![],
            vec![LocalGet(0), LocalGet(1), F64Mul, End],
            "fmul",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(
            &mut ctx,
            "fmul",
            &[WasmValue::F64(3.14), WasmValue::F64(2.0)],
        )
        .unwrap();
        assert_eq!(result[0].as_f64(), Some(6.28));
    }

    #[test]
    fn test_f32_nan_propagation() {
        let module = make_module(
            vec![ValueType::F32, ValueType::F32],
            vec![ValueType::F32],
            vec![],
            vec![LocalGet(0), LocalGet(1), F32Add, End],
            "fadd",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(
            &mut ctx,
            "fadd",
            &[WasmValue::F32(f32::NAN), WasmValue::F32(1.0)],
        )
        .unwrap();
        assert!(result[0].as_f32().unwrap().is_nan());
    }

    #[test]
    fn test_f32_inf() {
        let module = make_module(
            vec![ValueType::F32],
            vec![ValueType::I32],
            vec![],
            vec![LocalGet(0), LocalGet(0), F32Eq, End],
            "eq_self",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        // Inf == Inf is true
        let result = execute_export(&mut ctx, "eq_self", &[WasmValue::F32(f32::INFINITY)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(1));
        // NaN == NaN is false
        let result = execute_export(&mut ctx, "eq_self", &[WasmValue::F32(f32::NAN)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(0));
    }

    // B-QG4: Fibonacci (recursive)
    #[test]
    fn test_fibonacci_recursive() {
        // fib(n) = if n <= 1 then n else fib(n-1) + fib(n-2)
        let module = make_module(
            vec![ValueType::I32],
            vec![ValueType::I32],
            vec![],
            vec![
                // if n <= 1
                LocalGet(0),
                I32Const(1),
                I32LeS,
                If(BlockType::Value(ValueType::I32)),
                // then n
                LocalGet(0),
                Else,
                // else fib(n-1) + fib(n-2)
                LocalGet(0),
                I32Const(1),
                I32Sub,
                Call(0), // fib(n-1)
                LocalGet(0),
                I32Const(2),
                I32Sub,
                Call(0), // fib(n-2)
                I32Add,
                End,
                End,
            ],
            "fib",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        ctx.fuel = Some(100_000_000);

        // fib(0) = 0
        let result = execute_export(&mut ctx, "fib", &[WasmValue::I32(0)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(0));

        // fib(1) = 1
        let result = execute_export(&mut ctx, "fib", &[WasmValue::I32(1)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(1));

        // fib(10) = 55
        let result = execute_export(&mut ctx, "fib", &[WasmValue::I32(10)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(55));

        // fib(20) = 6765
        let result = execute_export(&mut ctx, "fib", &[WasmValue::I32(20)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(6765));
    }

    // Factorial (iterative with loop)
    #[test]
    fn test_factorial_iterative() {
        // fact(n): result=1; while n>1 { result *= n; n -= 1; }
        let module = make_module(
            vec![ValueType::I32],
            vec![ValueType::I32],
            vec![(1, ValueType::I32)], // local 1 = result
            vec![
                I32Const(1),
                LocalSet(1), // result = 1
                Block(BlockType::Empty),
                Loop(BlockType::Empty),
                // if n <= 1 then break
                LocalGet(0),
                I32Const(1),
                I32LeS,
                BrIf(1), // break out of block
                // result *= n
                LocalGet(1),
                LocalGet(0),
                I32Mul,
                LocalSet(1),
                // n -= 1
                LocalGet(0),
                I32Const(1),
                I32Sub,
                LocalSet(0),
                Br(0), // continue loop
                End,
                End,
                LocalGet(1),
                End,
            ],
            "fact",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();

        // 0! = 1
        let result = execute_export(&mut ctx, "fact", &[WasmValue::I32(0)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(1));

        // 1! = 1
        let result = execute_export(&mut ctx, "fact", &[WasmValue::I32(1)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(1));

        // 5! = 120
        let result = execute_export(&mut ctx, "fact", &[WasmValue::I32(5)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(120));

        // 10! = 3628800
        let result = execute_export(&mut ctx, "fact", &[WasmValue::I32(10)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(3628800));
    }

    // B-QG5: memory.grow + load/store
    #[test]
    fn test_memory_store_load() {
        let module = make_module_with_memory(
            vec![ValueType::I32, ValueType::I32],
            vec![ValueType::I32],
            vec![],
            vec![
                // store value at address
                LocalGet(0), // address
                LocalGet(1), // value
                I32Store(0, 0),
                // load it back
                LocalGet(0),
                I32Load(0, 0),
                End,
            ],
            "store_load",
            1, // 1 page
            Some(10),
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(
            &mut ctx,
            "store_load",
            &[WasmValue::I32(0), WasmValue::I32(12345)],
        )
        .unwrap();
        assert_eq!(result[0].as_i32(), Some(12345));
    }

    #[test]
    fn test_memory_grow() {
        let module = make_module_with_memory(
            vec![],
            vec![ValueType::I32],
            vec![],
            vec![
                // memory.size (should be 1)
                MemorySize,
                // grow by 2 pages
                I32Const(2),
                MemoryGrow,
                // drop old size
                Drop,
                // memory.size (should be 3)
                Drop,
                MemorySize,
                End,
            ],
            "grow_test",
            1,
            Some(10),
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(&mut ctx, "grow_test", &[]).unwrap();
        assert_eq!(result[0].as_i32(), Some(3));
    }

    // B-QG6: Traps
    #[test]
    fn test_trap_division_by_zero() {
        let module = make_module(
            vec![],
            vec![ValueType::I32],
            vec![],
            vec![I32Const(1), I32Const(0), I32DivS, End],
            "divz",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(&mut ctx, "divz", &[]);
        assert!(matches!(result, Err(TrapError::DivisionByZero)));
    }

    #[test]
    fn test_trap_memory_oob() {
        let module = make_module_with_memory(
            vec![],
            vec![ValueType::I32],
            vec![],
            vec![
                I32Const(65536), // exactly at boundary (1 page = 65536)
                I32Load(0, 0),
                End,
            ],
            "oob",
            1,
            None,
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(&mut ctx, "oob", &[]);
        assert!(matches!(result, Err(TrapError::MemoryOutOfBounds { .. })));
    }

    #[test]
    fn test_trap_unreachable() {
        let module = make_module(
            vec![],
            vec![ValueType::I32],
            vec![],
            vec![Unreachable, End],
            "unreach",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(&mut ctx, "unreach", &[]);
        assert!(matches!(result, Err(TrapError::Unreachable)));
    }

    // B-QG7: call_indirect + table dispatch
    #[test]
    fn test_call_indirect() {
        // Two functions: add(a,b) at index 0, mul(a,b) at index 1
        // call_indirect picks based on arg
        let func_type = FunctionType {
            params: vec![ValueType::I32, ValueType::I32],
            results: vec![ValueType::I32],
        };
        // Dispatch function type: (i32, i32, i32) -> i32
        // Third param selects which function to call
        let dispatch_type = FunctionType {
            params: vec![ValueType::I32, ValueType::I32, ValueType::I32],
            results: vec![ValueType::I32],
        };

        let module = Module {
            types: vec![func_type.clone(), dispatch_type],
            imports: vec![],
            functions: vec![0, 0, 1], // add=type0, mul=type0, dispatch=type1
            tables: vec![crate::module::TableType {
                element_type: ValueType::FuncRef,
                min: 2,
                max: Some(2),
            }],
            memories: vec![],
            globals: vec![],
            exports: vec![Export {
                name: String::from("dispatch"),
                kind: ExportKind::Function,
                index: 2, // dispatch function
            }],
            start: None,
            elements: vec![crate::module::Element {
                table_idx: 0,
                offset_expr: vec![I32Const(0)],
                func_indices: vec![0, 1], // table[0]=add, table[1]=mul
                passive: false,
            }],
            code: vec![
                // Function 0: add(a, b) -> a + b
                FunctionBody {
                    locals: vec![],
                    instructions: vec![LocalGet(0), LocalGet(1), I32Add, End],
                    raw_bytes: vec![],
                },
                // Function 1: mul(a, b) -> a * b
                FunctionBody {
                    locals: vec![],
                    instructions: vec![LocalGet(0), LocalGet(1), I32Mul, End],
                    raw_bytes: vec![],
                },
                // Function 2: dispatch(a, b, selector) -> table[selector](a, b)
                FunctionBody {
                    locals: vec![],
                    instructions: vec![
                        LocalGet(0),        // a
                        LocalGet(1),        // b
                        LocalGet(2),        // selector
                        CallIndirect(0, 0), // call type 0 from table 0
                        End,
                    ],
                    raw_bytes: vec![],
                },
            ],
            data: vec![],
            name: None,
            data_count: None,
        };

        let mut ctx = ExecutorContext::new(module).unwrap();

        // dispatch(3, 7, 0) = add(3, 7) = 10
        let result = execute_export(
            &mut ctx,
            "dispatch",
            &[WasmValue::I32(3), WasmValue::I32(7), WasmValue::I32(0)],
        )
        .unwrap();
        assert_eq!(result[0].as_i32(), Some(10));

        // dispatch(3, 7, 1) = mul(3, 7) = 21
        let result = execute_export(
            &mut ctx,
            "dispatch",
            &[WasmValue::I32(3), WasmValue::I32(7), WasmValue::I32(1)],
        )
        .unwrap();
        assert_eq!(result[0].as_i32(), Some(21));
    }

    // Additional comparison tests
    #[test]
    fn test_i32_comparisons() {
        // le_s
        let module = make_module(
            vec![ValueType::I32, ValueType::I32],
            vec![ValueType::I32],
            vec![],
            vec![LocalGet(0), LocalGet(1), I32LeS, End],
            "le",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        assert_eq!(
            execute_export(&mut ctx, "le", &[WasmValue::I32(1), WasmValue::I32(2)]).unwrap()[0]
                .as_i32(),
            Some(1)
        );
        assert_eq!(
            execute_export(&mut ctx, "le", &[WasmValue::I32(2), WasmValue::I32(2)]).unwrap()[0]
                .as_i32(),
            Some(1)
        );
        assert_eq!(
            execute_export(&mut ctx, "le", &[WasmValue::I32(3), WasmValue::I32(2)]).unwrap()[0]
                .as_i32(),
            Some(0)
        );
    }

    // Conversion test
    #[test]
    fn test_conversions() {
        let module = make_module(
            vec![ValueType::I32],
            vec![ValueType::I64],
            vec![],
            vec![LocalGet(0), I64ExtendI32S, End],
            "extend",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(&mut ctx, "extend", &[WasmValue::I32(-1)]).unwrap();
        assert_eq!(result[0].as_i64(), Some(-1i64));
    }

    // Select test
    #[test]
    fn test_select() {
        let module = make_module(
            vec![ValueType::I32],
            vec![ValueType::I32],
            vec![],
            vec![
                I32Const(10), // val1
                I32Const(20), // val2
                LocalGet(0),  // condition
                Select,
                End,
            ],
            "sel",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        // cond=1 => val1=10
        let result = execute_export(&mut ctx, "sel", &[WasmValue::I32(1)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(10));
        // cond=0 => val2=20
        let result = execute_export(&mut ctx, "sel", &[WasmValue::I32(0)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(20));
    }

    // Global get/set test
    #[test]
    fn test_globals() {
        let mut module = make_module(
            vec![ValueType::I32],
            vec![ValueType::I32],
            vec![],
            vec![
                // Read global, add param, write back, return
                GlobalGet(0),
                LocalGet(0),
                I32Add,
                GlobalSet(0),
                GlobalGet(0),
                End,
            ],
            "inc_global",
        );
        module.globals.push(crate::module::Global {
            global_type: crate::module::GlobalType {
                value_type: ValueType::I32,
                mutable: true,
            },
            init_expr: vec![I32Const(100)],
        });

        let mut ctx = ExecutorContext::new(module).unwrap();
        let result = execute_export(&mut ctx, "inc_global", &[WasmValue::I32(5)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(105));

        // Call again: global should persist
        let result = execute_export(&mut ctx, "inc_global", &[WasmValue::I32(10)]).unwrap();
        assert_eq!(result[0].as_i32(), Some(115));
    }

    // i32 integer overflow trap test
    #[test]
    fn test_i32_div_overflow() {
        let module = make_module(
            vec![],
            vec![ValueType::I32],
            vec![],
            vec![I32Const(i32::MIN), I32Const(-1), I32DivS, End],
            "div_ovf",
        );
        let mut ctx = ExecutorContext::new(module).unwrap();
        assert!(matches!(
            execute_export(&mut ctx, "div_ovf", &[]),
            Err(TrapError::IntegerOverflow)
        ));
    }
}
