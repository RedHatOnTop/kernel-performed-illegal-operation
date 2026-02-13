//! Native Code Generation
//!
//! This module generates native x86-64 machine code from the IR representation.

use alloc::vec::Vec;
use alloc::string::String;
use alloc::boxed::Box;
use core::ptr::NonNull;

use super::ir::{IrFunction, IrInstruction, IrOpcode, IrType};
use super::compiler::CompilationError;

/// Generated native code.
#[derive(Debug)]
pub struct NativeCode {
    /// Executable code buffer.
    code: Box<[u8]>,
    /// Entry point offset.
    entry_offset: usize,
    /// Stack frame size.
    frame_size: usize,
    /// Relocation information.
    relocations: Vec<Relocation>,
}

impl NativeCode {
    /// Create new native code.
    pub fn new(code: Vec<u8>, entry_offset: usize, frame_size: usize) -> Self {
        Self {
            code: code.into_boxed_slice(),
            entry_offset,
            frame_size,
            relocations: Vec::new(),
        }
    }
    
    /// Get code size in bytes.
    pub fn size(&self) -> usize {
        self.code.len()
    }
    
    /// Get the code bytes.
    pub fn code(&self) -> &[u8] {
        &self.code
    }
    
    /// Get entry point offset.
    pub fn entry_offset(&self) -> usize {
        self.entry_offset
    }
    
    /// Get stack frame size.
    pub fn frame_size(&self) -> usize {
        self.frame_size
    }
}

impl Clone for NativeCode {
    fn clone(&self) -> Self {
        Self {
            code: self.code.clone(),
            entry_offset: self.entry_offset,
            frame_size: self.frame_size,
            relocations: self.relocations.clone(),
        }
    }
}

/// Code relocation.
#[derive(Debug, Clone)]
pub struct Relocation {
    /// Offset in code where relocation applies.
    pub offset: usize,
    /// Kind of relocation.
    pub kind: RelocKind,
    /// Target function index (for calls).
    pub target: u32,
}

/// Relocation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocKind {
    /// PC-relative 32-bit call.
    Call32,
    /// Absolute 64-bit address.
    Abs64,
    /// PC-relative 32-bit branch.
    Branch32,
}

/// x86-64 Register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Reg {
    Rax = 0,
    Rcx = 1,
    Rdx = 2,
    Rbx = 3,
    Rsp = 4,
    Rbp = 5,
    Rsi = 6,
    Rdi = 7,
    R8 = 8,
    R9 = 9,
    R10 = 10,
    R11 = 11,
    R12 = 12,
    R13 = 13,
    R14 = 14,
    R15 = 15,
}

impl Reg {
    fn needs_rex(&self) -> bool {
        (*self as u8) >= 8
    }
    
    fn encoding(&self) -> u8 {
        (*self as u8) & 0x7
    }
}

/// XMM Register for floating point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum XmmReg {
    Xmm0 = 0,
    Xmm1 = 1,
    Xmm2 = 2,
    Xmm3 = 3,
    Xmm4 = 4,
    Xmm5 = 5,
    Xmm6 = 6,
    Xmm7 = 7,
    Xmm8 = 8,
    Xmm9 = 9,
    Xmm10 = 10,
    Xmm11 = 11,
    Xmm12 = 12,
    Xmm13 = 13,
    Xmm14 = 14,
    Xmm15 = 15,
}

/// Code generator for x86-64.
pub struct CodeGenerator {
    /// Generated code buffer.
    code: Vec<u8>,
    /// Current stack offset.
    stack_offset: i32,
    /// Label positions for branches.
    labels: Vec<Option<usize>>,
    /// Pending label references.
    pending_labels: Vec<(usize, usize, i32)>, // (code_offset, label_idx, addend)
}

impl CodeGenerator {
    /// Create a new code generator.
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            stack_offset: 0,
            labels: Vec::new(),
            pending_labels: Vec::new(),
        }
    }
    
    /// Reset the generator for a new function.
    fn reset(&mut self) {
        self.code.clear();
        self.stack_offset = 0;
        self.labels.clear();
        self.pending_labels.clear();
    }
    
    /// Generate baseline code (minimal optimization).
    pub fn generate_baseline(&self, ir: &IrFunction) -> Result<NativeCode, CompilationError> {
        let mut gen = Self::new();
        gen.compile_function(ir, false)
    }
    
    /// Generate optimized code.
    pub fn generate_optimized(&self, ir: &IrFunction) -> Result<NativeCode, CompilationError> {
        let mut gen = Self::new();
        gen.compile_function(ir, true)
    }
    
    /// Compile a function.
    fn compile_function(&mut self, ir: &IrFunction, _optimize: bool) -> Result<NativeCode, CompilationError> {
        self.reset();
        
        // Calculate frame size
        let locals_size = ir.total_locals() * 8;
        let frame_size = ((locals_size + 15) / 16) * 16; // 16-byte aligned
        
        // Function prologue
        self.emit_prologue(frame_size as i32);
        
        // Compile each IR instruction
        for inst in &ir.body {
            self.compile_instruction(inst, ir)?;
        }
        
        // Function epilogue (implicit return)
        self.emit_epilogue(frame_size as i32);
        
        // Resolve pending labels
        self.resolve_labels();
        
        Ok(NativeCode::new(self.code.clone(), 0, frame_size))
    }
    
    /// Emit function prologue.
    fn emit_prologue(&mut self, frame_size: i32) {
        // push rbp
        self.emit_byte(0x55);
        
        // mov rbp, rsp
        self.emit_bytes(&[0x48, 0x89, 0xE5]);
        
        if frame_size > 0 {
            // sub rsp, frame_size
            if frame_size <= 127 {
                self.emit_bytes(&[0x48, 0x83, 0xEC, frame_size as u8]);
            } else {
                self.emit_bytes(&[0x48, 0x81, 0xEC]);
                self.emit_i32(frame_size);
            }
        }
    }
    
    /// Emit function epilogue.
    fn emit_epilogue(&mut self, frame_size: i32) {
        if frame_size > 0 {
            // add rsp, frame_size
            if frame_size <= 127 {
                self.emit_bytes(&[0x48, 0x83, 0xC4, frame_size as u8]);
            } else {
                self.emit_bytes(&[0x48, 0x81, 0xC4]);
                self.emit_i32(frame_size);
            }
        }
        
        // pop rbp
        self.emit_byte(0x5D);
        
        // ret
        self.emit_byte(0xC3);
    }
    
    /// Compile a single IR instruction.
    fn compile_instruction(&mut self, inst: &IrInstruction, ir: &IrFunction) -> Result<(), CompilationError> {
        match inst.opcode {
            // Constants
            IrOpcode::Const32(val) => {
                // Push i32 constant onto value stack
                // mov eax, val
                self.emit_byte(0xB8);
                self.emit_i32(val);
                // push rax
                self.emit_byte(0x50);
            }
            IrOpcode::Const64(val) => {
                // movabs rax, val
                self.emit_bytes(&[0x48, 0xB8]);
                self.emit_i64(val);
                // push rax
                self.emit_byte(0x50);
            }
            
            // Local variables
            IrOpcode::LocalGet(idx) => {
                let offset = self.local_offset(idx, ir);
                // mov rax, [rbp + offset]
                self.emit_load_local(offset);
                // push rax
                self.emit_byte(0x50);
            }
            IrOpcode::LocalSet(idx) => {
                let offset = self.local_offset(idx, ir);
                // pop rax
                self.emit_byte(0x58);
                // mov [rbp + offset], rax
                self.emit_store_local(offset);
            }
            IrOpcode::LocalTee(idx) => {
                let offset = self.local_offset(idx, ir);
                // peek top of stack without popping
                // mov rax, [rsp]
                self.emit_bytes(&[0x48, 0x8B, 0x04, 0x24]);
                // mov [rbp + offset], rax
                self.emit_store_local(offset);
            }
            
            // Arithmetic (i32)
            IrOpcode::I32Add => {
                // pop rax, pop rcx, add, push
                self.emit_byte(0x58); // pop rax
                self.emit_byte(0x59); // pop rcx
                self.emit_bytes(&[0x01, 0xC8]); // add eax, ecx
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I32Sub => {
                self.emit_byte(0x58); // pop rax (subtrahend)
                self.emit_byte(0x59); // pop rcx (minuend)
                self.emit_bytes(&[0x29, 0xC1]); // sub ecx, eax
                self.emit_byte(0x51); // push rcx
            }
            IrOpcode::I32Mul => {
                self.emit_byte(0x58); // pop rax
                self.emit_byte(0x59); // pop rcx
                self.emit_bytes(&[0x0F, 0xAF, 0xC1]); // imul eax, ecx
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I32DivS => {
                self.emit_byte(0x59); // pop rcx (divisor)
                self.emit_byte(0x58); // pop rax (dividend)
                // cdq: sign-extend eax into edx:eax
                self.emit_byte(0x99);
                // idiv ecx
                self.emit_bytes(&[0xF7, 0xF9]);
                self.emit_byte(0x50); // push rax (quotient)
            }
            IrOpcode::I32DivU => {
                self.emit_byte(0x59); // pop rcx (divisor)
                self.emit_byte(0x58); // pop rax (dividend)
                // xor edx, edx
                self.emit_bytes(&[0x31, 0xD2]);
                // div ecx
                self.emit_bytes(&[0xF7, 0xF1]);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I32RemS => {
                self.emit_byte(0x59); // pop rcx
                self.emit_byte(0x58); // pop rax
                self.emit_byte(0x99); // cdq
                self.emit_bytes(&[0xF7, 0xF9]); // idiv ecx
                self.emit_byte(0x52); // push rdx (remainder)
            }
            IrOpcode::I32RemU => {
                self.emit_byte(0x59); // pop rcx
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x31, 0xD2]); // xor edx, edx
                self.emit_bytes(&[0xF7, 0xF1]); // div ecx
                self.emit_byte(0x52); // push rdx (remainder)
            }
            IrOpcode::I32And => {
                self.emit_byte(0x58); // pop rax
                self.emit_byte(0x59); // pop rcx
                self.emit_bytes(&[0x21, 0xC8]); // and eax, ecx
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I32Or => {
                self.emit_byte(0x58); // pop rax
                self.emit_byte(0x59); // pop rcx
                self.emit_bytes(&[0x09, 0xC8]); // or eax, ecx
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I32Xor => {
                self.emit_byte(0x58); // pop rax
                self.emit_byte(0x59); // pop rcx
                self.emit_bytes(&[0x31, 0xC8]); // xor eax, ecx
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I32Shl => {
                self.emit_byte(0x59); // pop rcx (shift amount)
                self.emit_byte(0x58); // pop rax (value)
                self.emit_bytes(&[0xD3, 0xE0]); // shl eax, cl
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I32ShrU => {
                self.emit_byte(0x59); // pop rcx
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0xD3, 0xE8]); // shr eax, cl
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I32ShrS => {
                self.emit_byte(0x59); // pop rcx
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0xD3, 0xF8]); // sar eax, cl
                self.emit_byte(0x50); // push rax
            }
            
            // Arithmetic (i64)
            IrOpcode::I64Add => {
                self.emit_byte(0x58); // pop rax
                self.emit_byte(0x59); // pop rcx
                self.emit_bytes(&[0x48, 0x01, 0xC8]); // add rax, rcx
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64Sub => {
                self.emit_byte(0x58); // pop rax
                self.emit_byte(0x59); // pop rcx
                self.emit_bytes(&[0x48, 0x29, 0xC1]); // sub rcx, rax
                self.emit_byte(0x51); // push rcx
            }
            IrOpcode::I64Mul => {
                self.emit_byte(0x58); // pop rax
                self.emit_byte(0x59); // pop rcx
                self.emit_bytes(&[0x48, 0x0F, 0xAF, 0xC1]); // imul rax, rcx
                self.emit_byte(0x50); // push rax
            }
            
            // Comparisons (i32)
            IrOpcode::I32Eq => {
                self.emit_comparison(0x94); // sete
            }
            IrOpcode::I32Ne => {
                self.emit_comparison(0x95); // setne
            }
            IrOpcode::I32LtS => {
                self.emit_comparison(0x9C); // setl
            }
            IrOpcode::I32LtU => {
                self.emit_comparison(0x92); // setb
            }
            IrOpcode::I32GtS => {
                self.emit_comparison(0x9F); // setg
            }
            IrOpcode::I32GtU => {
                self.emit_comparison(0x97); // seta
            }
            IrOpcode::I32LeS => {
                self.emit_comparison(0x9E); // setle
            }
            IrOpcode::I32LeU => {
                self.emit_comparison(0x96); // setbe
            }
            IrOpcode::I32GeS => {
                self.emit_comparison(0x9D); // setge
            }
            IrOpcode::I32GeU => {
                self.emit_comparison(0x93); // setae
            }
            IrOpcode::I32Eqz => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x85, 0xC0]); // test eax, eax
                self.emit_bytes(&[0x0F, 0x94, 0xC0]); // sete al
                self.emit_bytes(&[0x0F, 0xB6, 0xC0]); // movzx eax, al
                self.emit_byte(0x50); // push rax
            }
            
            // Memory operations
            IrOpcode::Load32(offset) => {
                self.emit_byte(0x58); // pop rax (address)
                if offset != 0 {
                    // add rax, offset
                    self.emit_bytes(&[0x48, 0x05]);
                    self.emit_i32(offset as i32);
                }
                // mov eax, [rax]
                self.emit_bytes(&[0x8B, 0x00]);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::Load64(offset) => {
                self.emit_byte(0x58); // pop rax (address)
                if offset != 0 {
                    self.emit_bytes(&[0x48, 0x05]);
                    self.emit_i32(offset as i32);
                }
                // mov rax, [rax]
                self.emit_bytes(&[0x48, 0x8B, 0x00]);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::Store32(offset) => {
                self.emit_byte(0x58); // pop rax (value)
                self.emit_byte(0x59); // pop rcx (address)
                if offset != 0 {
                    // add rcx, offset
                    self.emit_bytes(&[0x48, 0x81, 0xC1]);
                    self.emit_i32(offset as i32);
                }
                // mov [rcx], eax
                self.emit_bytes(&[0x89, 0x01]);
            }
            IrOpcode::Store64(offset) => {
                self.emit_byte(0x58); // pop rax (value)
                self.emit_byte(0x59); // pop rcx (address)
                if offset != 0 {
                    self.emit_bytes(&[0x48, 0x81, 0xC1]);
                    self.emit_i32(offset as i32);
                }
                // mov [rcx], rax
                self.emit_bytes(&[0x48, 0x89, 0x01]);
            }
            
            // Control flow
            IrOpcode::Return => {
                // Pop return value if any
                if !ir.results.is_empty() {
                    self.emit_byte(0x58); // pop rax (return value)
                }
                // Epilogue handled by end
            }
            IrOpcode::End => {
                // Block/loop end - may need to resolve labels
            }
            IrOpcode::Drop => {
                // pop and discard
                self.emit_bytes(&[0x48, 0x83, 0xC4, 0x08]); // add rsp, 8
            }
            IrOpcode::Select => {
                // pop condition, val2, val1; push val1 if cond!=0 else val2
                self.emit_byte(0x58); // pop rax (condition)
                self.emit_byte(0x5A); // pop rdx (val2 / false)
                self.emit_byte(0x59); // pop rcx (val1 / true)
                self.emit_bytes(&[0x85, 0xC0]); // test eax, eax
                // cmovz rcx, rdx  (if zero, pick val2)
                self.emit_bytes(&[0x48, 0x0F, 0x44, 0xCA]); // cmovz rcx, rdx
                self.emit_byte(0x51); // push rcx
            }
            IrOpcode::Block(_) => {
                // Create label for block end
                let label = self.labels.len();
                self.labels.push(None);
            }
            IrOpcode::Loop(_) => {
                // Record loop start position
                let label = self.labels.len();
                self.labels.push(Some(self.code.len()));
            }
            IrOpcode::If(_) => {
                // Conditional branch
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x85, 0xC0]); // test eax, eax
                // jz rel32 (placeholder)
                self.emit_bytes(&[0x0F, 0x84]);
                let label = self.labels.len();
                self.labels.push(None);
                let patch_offset = self.code.len();
                self.emit_i32(0); // placeholder
                self.pending_labels.push((patch_offset, label, 0));
            }
            IrOpcode::Br(_depth) => {
                // jmp rel32 (placeholder)
                self.emit_byte(0xE9);
                self.emit_i32(0); // placeholder
            }
            IrOpcode::Call(func_idx) => {
                // Emit call placeholder - will be relocated
                self.emit_byte(0xE8); // call rel32
                self.emit_i32(0); // placeholder
            }
            IrOpcode::Unreachable => {
                // ud2 - undefined instruction trap
                self.emit_bytes(&[0x0F, 0x0B]);
            }
            
            // Conversions
            IrOpcode::I32WrapI64 => {
                // Just keep lower 32 bits (already in place)
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x89, 0xC0]); // mov eax, eax (zero-extend)
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64ExtendI32S => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x48, 0x63, 0xC0]); // movsxd rax, eax
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64ExtendI32U => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x89, 0xC0]); // mov eax, eax (zero-extend)
                self.emit_byte(0x50); // push rax
            }
            
            // Default: unimplemented opcodes
            _ => {
                // For unimplemented opcodes, emit a trap
                self.emit_bytes(&[0x0F, 0x0B]); // ud2
            }
        }
        
        Ok(())
    }
    
    /// Calculate local variable offset from rbp.
    fn local_offset(&self, idx: u32, ir: &IrFunction) -> i32 {
        // Locals are stored at [rbp - 8], [rbp - 16], etc.
        let offset = (idx as i32 + 1) * -8;
        offset
    }
    
    /// Emit load from local variable.
    fn emit_load_local(&mut self, offset: i32) {
        if offset >= -128 && offset <= 127 {
            // mov rax, [rbp + offset8]
            self.emit_bytes(&[0x48, 0x8B, 0x45, offset as u8]);
        } else {
            // mov rax, [rbp + offset32]
            self.emit_bytes(&[0x48, 0x8B, 0x85]);
            self.emit_i32(offset);
        }
    }
    
    /// Emit store to local variable.
    fn emit_store_local(&mut self, offset: i32) {
        if offset >= -128 && offset <= 127 {
            // mov [rbp + offset8], rax
            self.emit_bytes(&[0x48, 0x89, 0x45, offset as u8]);
        } else {
            // mov [rbp + offset32], rax
            self.emit_bytes(&[0x48, 0x89, 0x85]);
            self.emit_i32(offset);
        }
    }
    
    /// Emit comparison with setcc.
    fn emit_comparison(&mut self, setcc: u8) {
        self.emit_byte(0x58); // pop rax (right)
        self.emit_byte(0x59); // pop rcx (left)
        self.emit_bytes(&[0x39, 0xC1]); // cmp ecx, eax
        self.emit_bytes(&[0x0F, setcc, 0xC0]); // setcc al
        self.emit_bytes(&[0x0F, 0xB6, 0xC0]); // movzx eax, al
        self.emit_byte(0x50); // push rax
    }
    
    /// Resolve pending label references.
    fn resolve_labels(&mut self) {
        for (offset, label_idx, addend) in &self.pending_labels {
            if let Some(Some(target)) = self.labels.get(*label_idx) {
                let rel = (*target as i32) - (*offset as i32) - 4 + addend;
                self.code[*offset..*offset + 4].copy_from_slice(&rel.to_le_bytes());
            }
        }
    }
    
    fn emit_byte(&mut self, b: u8) {
        self.code.push(b);
    }
    
    fn emit_bytes(&mut self, bytes: &[u8]) {
        self.code.extend_from_slice(bytes);
    }
    
    fn emit_i32(&mut self, val: i32) {
        self.code.extend_from_slice(&val.to_le_bytes());
    }
    
    fn emit_i64(&mut self, val: i64) {
        self.code.extend_from_slice(&val.to_le_bytes());
    }
}

impl Default for CodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}
