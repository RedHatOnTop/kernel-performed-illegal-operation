//! Native Code Generation
//!
//! This module generates native x86-64 machine code from the IR representation.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::ptr::NonNull;

use super::compiler::CompilationError;
use super::ir::{IrFunction, IrInstruction, IrOpcode, IrType};

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
    fn compile_function(
        &mut self,
        ir: &IrFunction,
        _optimize: bool,
    ) -> Result<NativeCode, CompilationError> {
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
    fn compile_instruction(
        &mut self,
        inst: &IrInstruction,
        ir: &IrFunction,
    ) -> Result<(), CompilationError> {
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
            IrOpcode::I64DivS => {
                self.emit_byte(0x59); // pop rcx (divisor)
                self.emit_byte(0x58); // pop rax (dividend)
                // cqo: sign-extend rax into rdx:rax
                self.emit_bytes(&[0x48, 0x99]);
                // idiv rcx
                self.emit_bytes(&[0x48, 0xF7, 0xF9]);
                self.emit_byte(0x50); // push rax (quotient)
            }
            IrOpcode::I64DivU => {
                self.emit_byte(0x59); // pop rcx (divisor)
                self.emit_byte(0x58); // pop rax (dividend)
                // xor rdx, rdx
                self.emit_bytes(&[0x48, 0x31, 0xD2]);
                // div rcx
                self.emit_bytes(&[0x48, 0xF7, 0xF1]);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64RemS => {
                self.emit_byte(0x59); // pop rcx
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x48, 0x99]); // cqo
                self.emit_bytes(&[0x48, 0xF7, 0xF9]); // idiv rcx
                self.emit_byte(0x52); // push rdx (remainder)
            }
            IrOpcode::I64RemU => {
                self.emit_byte(0x59); // pop rcx
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x48, 0x31, 0xD2]); // xor rdx, rdx
                self.emit_bytes(&[0x48, 0xF7, 0xF1]); // div rcx
                self.emit_byte(0x52); // push rdx (remainder)
            }
            IrOpcode::I64And => {
                self.emit_byte(0x58); // pop rax
                self.emit_byte(0x59); // pop rcx
                self.emit_bytes(&[0x48, 0x21, 0xC8]); // and rax, rcx
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64Or => {
                self.emit_byte(0x58); // pop rax
                self.emit_byte(0x59); // pop rcx
                self.emit_bytes(&[0x48, 0x09, 0xC8]); // or rax, rcx
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64Xor => {
                self.emit_byte(0x58); // pop rax
                self.emit_byte(0x59); // pop rcx
                self.emit_bytes(&[0x48, 0x31, 0xC8]); // xor rax, rcx
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64Shl => {
                self.emit_byte(0x59); // pop rcx (shift amount)
                self.emit_byte(0x58); // pop rax (value)
                self.emit_bytes(&[0x48, 0xD3, 0xE0]); // shl rax, cl
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64ShrU => {
                self.emit_byte(0x59); // pop rcx
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x48, 0xD3, 0xE8]); // shr rax, cl
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64ShrS => {
                self.emit_byte(0x59); // pop rcx
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x48, 0xD3, 0xF8]); // sar rax, cl
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64Rotl => {
                self.emit_byte(0x59); // pop rcx
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x48, 0xD3, 0xC0]); // rol rax, cl
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64Rotr => {
                self.emit_byte(0x59); // pop rcx
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x48, 0xD3, 0xC8]); // ror rax, cl
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64Clz => {
                self.emit_byte(0x58); // pop rax
                // Use bsr + xor trick for clz (no LZCNT dependency)
                // test rax, rax; jz zero_case; bsr rcx, rax; xor rcx, 63; jmp done; zero_case: mov ecx, 64; done:
                self.emit_bytes(&[0x48, 0x85, 0xC0]); // test rax, rax
                self.emit_bytes(&[0x74, 0x09]); // jz +9
                self.emit_bytes(&[0x48, 0x0F, 0xBD, 0xC8]); // bsr rcx, rax
                self.emit_bytes(&[0x48, 0x83, 0xF1, 0x3F]); // xor rcx, 63
                self.emit_bytes(&[0xEB, 0x05]); // jmp +5
                self.emit_bytes(&[0xB9, 0x40, 0x00, 0x00, 0x00]); // mov ecx, 64
                self.emit_byte(0x51); // push rcx
            }
            IrOpcode::I64Ctz => {
                self.emit_byte(0x58); // pop rax
                // test rax, rax; jz zero_case; bsf rcx, rax; jmp done; zero_case: mov ecx, 64; done:
                self.emit_bytes(&[0x48, 0x85, 0xC0]); // test rax, rax
                self.emit_bytes(&[0x74, 0x07]); // jz +7
                self.emit_bytes(&[0x48, 0x0F, 0xBC, 0xC8]); // bsf rcx, rax
                self.emit_bytes(&[0xEB, 0x05]); // jmp +5
                self.emit_bytes(&[0xB9, 0x40, 0x00, 0x00, 0x00]); // mov ecx, 64
                self.emit_byte(0x51); // push rcx
            }
            IrOpcode::I64Popcnt => {
                self.emit_byte(0x58); // pop rax
                // Software popcnt: parallel bit counting
                // Use Hamming weight algorithm
                self.emit_bytes(&[0x48, 0x89, 0xC1]); // mov rcx, rax
                self.emit_bytes(&[0x48, 0xD1, 0xE9]); // shr rcx, 1
                self.emit_bytes(&[0x48, 0xBA]); // movabs rdx, 0x5555555555555555
                self.emit_i64(0x5555555555555555u64 as i64);
                self.emit_bytes(&[0x48, 0x21, 0xD1]); // and rcx, rdx
                self.emit_bytes(&[0x48, 0x29, 0xC8]); // sub rax, rcx
                self.emit_bytes(&[0x48, 0x89, 0xC1]); // mov rcx, rax
                self.emit_bytes(&[0x48, 0xC1, 0xE9, 0x02]); // shr rcx, 2
                self.emit_bytes(&[0x48, 0xBA]); // movabs rdx, 0x3333333333333333
                self.emit_i64(0x3333333333333333u64 as i64);
                self.emit_bytes(&[0x48, 0x21, 0xD0]); // and rax, rdx
                self.emit_bytes(&[0x48, 0x21, 0xD1]); // and rcx, rdx
                self.emit_bytes(&[0x48, 0x01, 0xC8]); // add rax, rcx
                self.emit_bytes(&[0x48, 0x89, 0xC1]); // mov rcx, rax
                self.emit_bytes(&[0x48, 0xC1, 0xE9, 0x04]); // shr rcx, 4
                self.emit_bytes(&[0x48, 0x01, 0xC8]); // add rax, rcx
                self.emit_bytes(&[0x48, 0xBA]); // movabs rdx, 0x0F0F0F0F0F0F0F0F
                self.emit_i64(0x0F0F0F0F0F0F0F0Fu64 as i64);
                self.emit_bytes(&[0x48, 0x21, 0xD0]); // and rax, rdx
                self.emit_bytes(&[0x48, 0xBA]); // movabs rdx, 0x0101010101010101
                self.emit_i64(0x0101010101010101u64 as i64);
                self.emit_bytes(&[0x48, 0x0F, 0xAF, 0xC2]); // imul rax, rdx
                self.emit_bytes(&[0x48, 0xC1, 0xE8, 0x38]); // shr rax, 56
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64Eqz => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x48, 0x85, 0xC0]); // test rax, rax
                self.emit_bytes(&[0x0F, 0x94, 0xC0]); // sete al
                self.emit_bytes(&[0x0F, 0xB6, 0xC0]); // movzx eax, al
                self.emit_byte(0x50); // push rax
            }

            // Comparisons (i64)
            IrOpcode::I64Eq => {
                self.emit_comparison_64(0x94); // sete
            }
            IrOpcode::I64Ne => {
                self.emit_comparison_64(0x95); // setne
            }
            IrOpcode::I64LtS => {
                self.emit_comparison_64(0x9C); // setl
            }
            IrOpcode::I64LtU => {
                self.emit_comparison_64(0x92); // setb
            }
            IrOpcode::I64GtS => {
                self.emit_comparison_64(0x9F); // setg
            }
            IrOpcode::I64GtU => {
                self.emit_comparison_64(0x97); // seta
            }
            IrOpcode::I64LeS => {
                self.emit_comparison_64(0x9E); // setle
            }
            IrOpcode::I64LeU => {
                self.emit_comparison_64(0x96); // setbe
            }
            IrOpcode::I64GeS => {
                self.emit_comparison_64(0x9D); // setge
            }
            IrOpcode::I64GeU => {
                self.emit_comparison_64(0x93); // setae
            }

            // i32 bit manipulation
            IrOpcode::I32Rotl => {
                self.emit_byte(0x59); // pop rcx
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0xD3, 0xC0]); // rol eax, cl
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I32Rotr => {
                self.emit_byte(0x59); // pop rcx
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0xD3, 0xC8]); // ror eax, cl
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I32Clz => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x85, 0xC0]); // test eax, eax
                self.emit_bytes(&[0x74, 0x08]); // jz +8
                self.emit_bytes(&[0x0F, 0xBD, 0xC8]); // bsr ecx, eax
                self.emit_bytes(&[0x83, 0xF1, 0x1F]); // xor ecx, 31
                self.emit_bytes(&[0xEB, 0x05]); // jmp +5
                self.emit_bytes(&[0xB9, 0x20, 0x00, 0x00, 0x00]); // mov ecx, 32
                self.emit_byte(0x51); // push rcx
            }
            IrOpcode::I32Ctz => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x85, 0xC0]); // test eax, eax
                self.emit_bytes(&[0x74, 0x06]); // jz +6
                self.emit_bytes(&[0x0F, 0xBC, 0xC8]); // bsf ecx, eax
                self.emit_bytes(&[0xEB, 0x05]); // jmp +5
                self.emit_bytes(&[0xB9, 0x20, 0x00, 0x00, 0x00]); // mov ecx, 32
                self.emit_byte(0x51); // push rcx
            }
            IrOpcode::I32Popcnt => {
                self.emit_byte(0x58); // pop rax
                // Software popcnt (32-bit Hamming weight)
                self.emit_bytes(&[0x89, 0xC1]); // mov ecx, eax
                self.emit_bytes(&[0xD1, 0xE9]); // shr ecx, 1
                self.emit_bytes(&[0x81, 0xE1]); // and ecx, 0x55555555
                self.emit_i32(0x55555555u32 as i32);
                self.emit_bytes(&[0x29, 0xC8]); // sub eax, ecx
                self.emit_bytes(&[0x89, 0xC1]); // mov ecx, eax
                self.emit_bytes(&[0xC1, 0xE9, 0x02]); // shr ecx, 2
                self.emit_bytes(&[0x25]); // and eax, 0x33333333
                self.emit_i32(0x33333333u32 as i32);
                self.emit_bytes(&[0x81, 0xE1]); // and ecx, 0x33333333
                self.emit_i32(0x33333333u32 as i32);
                self.emit_bytes(&[0x01, 0xC8]); // add eax, ecx
                self.emit_bytes(&[0x89, 0xC1]); // mov ecx, eax
                self.emit_bytes(&[0xC1, 0xE9, 0x04]); // shr ecx, 4
                self.emit_bytes(&[0x01, 0xC8]); // add eax, ecx
                self.emit_bytes(&[0x25]); // and eax, 0x0F0F0F0F
                self.emit_i32(0x0F0F0F0Fu32 as i32);
                self.emit_bytes(&[0x69, 0xC0]); // imul eax, eax, 0x01010101
                self.emit_i32(0x01010101u32 as i32);
                self.emit_bytes(&[0xC1, 0xE8, 0x18]); // shr eax, 24
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

            // Floating point (f32) - SSE
            IrOpcode::ConstF32(bits) => {
                // Push f32 bit pattern as i32, then push
                self.emit_byte(0xB8); // mov eax, imm32
                self.emit_i32(bits as i32);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::ConstF64(bits) => {
                // Push f64 bit pattern as i64
                self.emit_bytes(&[0x48, 0xB8]); // movabs rax, imm64
                self.emit_i64(bits as i64);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F32Add => {
                self.emit_f32_binop(&[0xF3, 0x0F, 0x58, 0xC1]); // addss xmm0, xmm1
            }
            IrOpcode::F32Sub => {
                // Pop right into xmm1, left into xmm0
                self.emit_byte(0x59); // pop rcx (right)
                self.emit_byte(0x58); // pop rax (left)
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC9]); // movq xmm1, rcx
                self.emit_bytes(&[0xF3, 0x0F, 0x5C, 0xC1]); // subss xmm0, xmm1
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F32Mul => {
                self.emit_f32_binop(&[0xF3, 0x0F, 0x59, 0xC1]); // mulss xmm0, xmm1
            }
            IrOpcode::F32Div => {
                self.emit_f32_binop_ordered(&[0xF3, 0x0F, 0x5E, 0xC1]); // divss xmm0, xmm1
            }
            IrOpcode::F32Sqrt => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                self.emit_bytes(&[0xF3, 0x0F, 0x51, 0xC0]); // sqrtss xmm0, xmm0
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F32Abs => {
                self.emit_byte(0x58); // pop rax
                // Clear sign bit: and eax, 0x7FFFFFFF
                self.emit_bytes(&[0x25]);
                self.emit_i32(0x7FFFFFFFu32 as i32);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F32Neg => {
                self.emit_byte(0x58); // pop rax
                // Flip sign bit: xor eax, 0x80000000
                self.emit_bytes(&[0x35]);
                self.emit_i32(0x80000000u32 as i32);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F32Ceil => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                // roundss xmm0, xmm0, 0x02 (round toward +inf)
                self.emit_bytes(&[0x66, 0x0F, 0x3A, 0x0A, 0xC0, 0x02]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F32Floor => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                // roundss xmm0, xmm0, 0x01 (round toward -inf)
                self.emit_bytes(&[0x66, 0x0F, 0x3A, 0x0A, 0xC0, 0x01]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F32Trunc => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                // roundss xmm0, xmm0, 0x03 (round toward zero)
                self.emit_bytes(&[0x66, 0x0F, 0x3A, 0x0A, 0xC0, 0x03]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F32Nearest => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                // roundss xmm0, xmm0, 0x00 (round to nearest even)
                self.emit_bytes(&[0x66, 0x0F, 0x3A, 0x0A, 0xC0, 0x00]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F32Min => {
                self.emit_f32_binop(&[0xF3, 0x0F, 0x5D, 0xC1]); // minss xmm0, xmm1
            }
            IrOpcode::F32Max => {
                self.emit_f32_binop(&[0xF3, 0x0F, 0x5F, 0xC1]); // maxss xmm0, xmm1
            }
            IrOpcode::F32Copysign => {
                // Copy sign of second operand to first
                self.emit_byte(0x59); // pop rcx (sign source)
                self.emit_byte(0x58); // pop rax (value)
                self.emit_bytes(&[0x25]); // and eax, 0x7FFFFFFF
                self.emit_i32(0x7FFFFFFFu32 as i32);
                self.emit_bytes(&[0x81, 0xE1]); // and ecx, 0x80000000
                self.emit_i32(0x80000000u32 as i32);
                self.emit_bytes(&[0x09, 0xC8]); // or eax, ecx
                self.emit_byte(0x50); // push rax
            }

            // Floating point (f64) - SSE2
            IrOpcode::F64Add => {
                self.emit_f64_binop(&[0xF2, 0x0F, 0x58, 0xC1]); // addsd xmm0, xmm1
            }
            IrOpcode::F64Sub => {
                self.emit_byte(0x59); // pop rcx (right)
                self.emit_byte(0x58); // pop rax (left)
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC9]); // movq xmm1, rcx
                self.emit_bytes(&[0xF2, 0x0F, 0x5C, 0xC1]); // subsd xmm0, xmm1
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F64Mul => {
                self.emit_f64_binop(&[0xF2, 0x0F, 0x59, 0xC1]); // mulsd xmm0, xmm1
            }
            IrOpcode::F64Div => {
                self.emit_f64_binop_ordered(&[0xF2, 0x0F, 0x5E, 0xC1]); // divsd xmm0, xmm1
            }
            IrOpcode::F64Sqrt => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                self.emit_bytes(&[0xF2, 0x0F, 0x51, 0xC0]); // sqrtsd xmm0, xmm0
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F64Abs => {
                self.emit_byte(0x58); // pop rax
                // Clear sign bit: movabs rdx, 0x7FFFFFFFFFFFFFFF; and rax, rdx
                self.emit_bytes(&[0x48, 0xBA]); // movabs rdx
                self.emit_i64(0x7FFFFFFFFFFFFFFFu64 as i64);
                self.emit_bytes(&[0x48, 0x21, 0xD0]); // and rax, rdx
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F64Neg => {
                self.emit_byte(0x58); // pop rax
                // Flip sign bit: movabs rdx, 0x8000000000000000; xor rax, rdx
                self.emit_bytes(&[0x48, 0xBA]); // movabs rdx
                self.emit_i64(0x8000000000000000u64 as i64);
                self.emit_bytes(&[0x48, 0x31, 0xD0]); // xor rax, rdx
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F64Ceil => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                // roundsd xmm0, xmm0, 0x02 (round toward +inf)
                self.emit_bytes(&[0x66, 0x0F, 0x3A, 0x0B, 0xC0, 0x02]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F64Floor => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                // roundsd xmm0, xmm0, 0x01
                self.emit_bytes(&[0x66, 0x0F, 0x3A, 0x0B, 0xC0, 0x01]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F64Trunc => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                // roundsd xmm0, xmm0, 0x03
                self.emit_bytes(&[0x66, 0x0F, 0x3A, 0x0B, 0xC0, 0x03]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F64Nearest => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                // roundsd xmm0, xmm0, 0x00
                self.emit_bytes(&[0x66, 0x0F, 0x3A, 0x0B, 0xC0, 0x00]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F64Min => {
                self.emit_f64_binop(&[0xF2, 0x0F, 0x5D, 0xC1]); // minsd xmm0, xmm1
            }
            IrOpcode::F64Max => {
                self.emit_f64_binop(&[0xF2, 0x0F, 0x5F, 0xC1]); // maxsd xmm0, xmm1
            }
            IrOpcode::F64Copysign => {
                self.emit_byte(0x59); // pop rcx (sign source)
                self.emit_byte(0x58); // pop rax (value)
                self.emit_bytes(&[0x48, 0xBA]); // movabs rdx, 0x7FFFFFFFFFFFFFFF
                self.emit_i64(0x7FFFFFFFFFFFFFFFu64 as i64);
                self.emit_bytes(&[0x48, 0x21, 0xD0]); // and rax, rdx
                self.emit_bytes(&[0x48, 0xBA]); // movabs rdx, 0x8000000000000000
                self.emit_i64(0x8000000000000000u64 as i64);
                self.emit_bytes(&[0x48, 0x21, 0xD1]); // and rcx, rdx
                self.emit_bytes(&[0x48, 0x09, 0xC8]); // or rax, rcx
                self.emit_byte(0x50); // push rax
            }

            // Floating point comparisons (f32)
            IrOpcode::F32Eq => {
                self.emit_f32_comparison(0x00); // cmpeqss (==)
            }
            IrOpcode::F32Ne => {
                self.emit_f32_comparison(0x04); // cmpneqss (!=)
            }
            IrOpcode::F32Lt => {
                self.emit_f32_comparison_ordered(0x01); // cmpltss (<)
            }
            IrOpcode::F32Gt => {
                // gt: swap operands and use lt
                self.emit_f32_comparison_ordered_swap(0x01);
            }
            IrOpcode::F32Le => {
                self.emit_f32_comparison_ordered(0x02); // cmpless (<=)
            }
            IrOpcode::F32Ge => {
                // ge: swap operands and use le
                self.emit_f32_comparison_ordered_swap(0x02);
            }

            // Floating point comparisons (f64)
            IrOpcode::F64Eq => {
                self.emit_f64_comparison(0x00); // cmpeqsd
            }
            IrOpcode::F64Ne => {
                self.emit_f64_comparison(0x04); // cmpneqsd
            }
            IrOpcode::F64Lt => {
                self.emit_f64_comparison_ordered(0x01); // cmpltsd
            }
            IrOpcode::F64Gt => {
                self.emit_f64_comparison_ordered_swap(0x01);
            }
            IrOpcode::F64Le => {
                self.emit_f64_comparison_ordered(0x02); // cmplesd
            }
            IrOpcode::F64Ge => {
                self.emit_f64_comparison_ordered_swap(0x02);
            }

            // Conversions - int to float
            IrOpcode::F32ConvertI32S => {
                self.emit_byte(0x58); // pop rax
                // cvtsi2ss xmm0, eax
                self.emit_bytes(&[0xF3, 0x0F, 0x2A, 0xC0]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F32ConvertI32U => {
                self.emit_byte(0x58); // pop rax
                // Zero-extend eax to rax, then cvtsi2ss from 64-bit
                self.emit_bytes(&[0x89, 0xC0]); // mov eax, eax (zero-extend)
                // cvtsi2ss xmm0, rax
                self.emit_bytes(&[0xF3, 0x48, 0x0F, 0x2A, 0xC0]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F32ConvertI64S => {
                self.emit_byte(0x58); // pop rax
                // cvtsi2ss xmm0, rax (64-bit)
                self.emit_bytes(&[0xF3, 0x48, 0x0F, 0x2A, 0xC0]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F32ConvertI64U => {
                // For unsigned i64, handle large values
                self.emit_byte(0x58); // pop rax
                // cvtsi2ss xmm0, rax (treat as signed, works for < 2^63)
                self.emit_bytes(&[0xF3, 0x48, 0x0F, 0x2A, 0xC0]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F64ConvertI32S => {
                self.emit_byte(0x58); // pop rax
                // cvtsi2sd xmm0, eax
                self.emit_bytes(&[0xF2, 0x0F, 0x2A, 0xC0]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F64ConvertI32U => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x89, 0xC0]); // mov eax, eax (zero-extend)
                // cvtsi2sd xmm0, rax
                self.emit_bytes(&[0xF2, 0x48, 0x0F, 0x2A, 0xC0]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F64ConvertI64S => {
                self.emit_byte(0x58); // pop rax
                // cvtsi2sd xmm0, rax
                self.emit_bytes(&[0xF2, 0x48, 0x0F, 0x2A, 0xC0]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F64ConvertI64U => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0xF2, 0x48, 0x0F, 0x2A, 0xC0]); // cvtsi2sd xmm0, rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }

            // Conversions - float to int
            IrOpcode::I32TruncF32S => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                // cvttss2si eax, xmm0
                self.emit_bytes(&[0xF3, 0x0F, 0x2C, 0xC0]);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I32TruncF32U => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                // cvttss2si rax, xmm0 (64-bit to handle unsigned range)
                self.emit_bytes(&[0xF3, 0x48, 0x0F, 0x2C, 0xC0]);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I32TruncF64S => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                // cvttsd2si eax, xmm0
                self.emit_bytes(&[0xF2, 0x0F, 0x2C, 0xC0]);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I32TruncF64U => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                self.emit_bytes(&[0xF2, 0x48, 0x0F, 0x2C, 0xC0]); // cvttsd2si rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64TruncF32S => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                // cvttss2si rax, xmm0 (64-bit)
                self.emit_bytes(&[0xF3, 0x48, 0x0F, 0x2C, 0xC0]);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64TruncF32U => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                self.emit_bytes(&[0xF3, 0x48, 0x0F, 0x2C, 0xC0]); // cvttss2si rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64TruncF64S => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                // cvttsd2si rax, xmm0 (64-bit)
                self.emit_bytes(&[0xF2, 0x48, 0x0F, 0x2C, 0xC0]);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64TruncF64U => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                self.emit_bytes(&[0xF2, 0x48, 0x0F, 0x2C, 0xC0]); // cvttsd2si rax, xmm0
                self.emit_byte(0x50); // push rax
            }

            // Conversions - float to float
            IrOpcode::F32DemoteF64 => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                // cvtsd2ss xmm0, xmm0
                self.emit_bytes(&[0xF2, 0x0F, 0x5A, 0xC0]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::F64PromoteF32 => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
                // cvtss2sd xmm0, xmm0
                self.emit_bytes(&[0xF3, 0x0F, 0x5A, 0xC0]);
                self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
                self.emit_byte(0x50); // push rax
            }

            // Reinterpret operations (bit-level casts, no actual computation)
            IrOpcode::I32ReinterpretF32 | IrOpcode::F32ReinterpretI32 => {
                // f32 and i32 share the same 32-bit representation on stack
                // No operation needed - value stays as-is
            }
            IrOpcode::I64ReinterpretF64 | IrOpcode::F64ReinterpretI64 => {
                // f64 and i64 share the same 64-bit representation on stack 
                // No operation needed - value stays as-is
            }

            // Sign-extension operations
            IrOpcode::I32Extend8S => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x0F, 0xBE, 0xC0]); // movsx eax, al
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I32Extend16S => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x0F, 0xBF, 0xC0]); // movsx eax, ax
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64Extend8S => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x48, 0x0F, 0xBE, 0xC0]); // movsx rax, al
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64Extend16S => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x48, 0x0F, 0xBF, 0xC0]); // movsx rax, ax
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::I64Extend32S => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x48, 0x63, 0xC0]); // movsxd rax, eax
                self.emit_byte(0x50); // push rax
            }

            // Control flow - BrIf
            IrOpcode::BrIf(_depth) => {
                // Pop condition, conditionally branch
                self.emit_byte(0x58); // pop rax (condition)
                self.emit_bytes(&[0x85, 0xC0]); // test eax, eax
                // jnz rel32 (placeholder)
                self.emit_bytes(&[0x0F, 0x85]);
                self.emit_i32(0); // placeholder
            }
            IrOpcode::BrTable(table_idx) => {
                // Pop index, branch to target
                self.emit_byte(0x58); // pop rax (index)
                // For now, emit a simple bounds check + indirect jump placeholder
                // The actual table dispatch will be patched by the runtime
                self.emit_bytes(&[0x0F, 0x0B]); // ud2 (placeholder - table dispatch)
            }
            IrOpcode::Else => {
                // Jump to end of if-else, mark else label position
                self.emit_byte(0xE9); // jmp rel32
                self.emit_i32(0); // placeholder
            }
            IrOpcode::CallIndirect(_type_idx) => {
                // Pop function index, validate type, call
                self.emit_byte(0x58); // pop rax (table index)
                // For now, emit trap for safety (full impl needs table lookup)
                self.emit_bytes(&[0x0F, 0x0B]); // ud2 (placeholder)
            }

            // Memory size/grow
            IrOpcode::MemorySize => {
                // Push current memory size (pages) - placeholder
                self.emit_bytes(&[0x31, 0xC0]); // xor eax, eax (0 pages default)
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::MemoryGrow => {
                // Pop requested pages, return old size or -1
                self.emit_byte(0x58); // pop rax (requested pages)
                // Return -1 (failure) for now
                self.emit_bytes(&[0x48, 0xC7, 0xC0, 0xFF, 0xFF, 0xFF, 0xFF]); // mov rax, -1
                self.emit_byte(0x50); // push rax
            }

            // Additional load/store variants
            IrOpcode::Load8S(offset) => {
                self.emit_byte(0x58); // pop rax (address)
                if offset != 0 {
                    self.emit_bytes(&[0x48, 0x05]);
                    self.emit_i32(offset as i32);
                }
                // movsx eax, byte [rax]
                self.emit_bytes(&[0x0F, 0xBE, 0x00]);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::Load8U(offset) => {
                self.emit_byte(0x58); // pop rax
                if offset != 0 {
                    self.emit_bytes(&[0x48, 0x05]);
                    self.emit_i32(offset as i32);
                }
                // movzx eax, byte [rax]
                self.emit_bytes(&[0x0F, 0xB6, 0x00]);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::Load16S(offset) => {
                self.emit_byte(0x58); // pop rax
                if offset != 0 {
                    self.emit_bytes(&[0x48, 0x05]);
                    self.emit_i32(offset as i32);
                }
                // movsx eax, word [rax]
                self.emit_bytes(&[0x0F, 0xBF, 0x00]);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::Load16U(offset) => {
                self.emit_byte(0x58); // pop rax
                if offset != 0 {
                    self.emit_bytes(&[0x48, 0x05]);
                    self.emit_i32(offset as i32);
                }
                // movzx eax, word [rax]
                self.emit_bytes(&[0x0F, 0xB7, 0x00]);
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::Store8(offset) => {
                self.emit_byte(0x58); // pop rax (value)
                self.emit_byte(0x59); // pop rcx (address)
                if offset != 0 {
                    self.emit_bytes(&[0x48, 0x81, 0xC1]);
                    self.emit_i32(offset as i32);
                }
                // mov [rcx], al
                self.emit_bytes(&[0x88, 0x01]);
            }
            IrOpcode::Store16(offset) => {
                self.emit_byte(0x58); // pop rax (value)
                self.emit_byte(0x59); // pop rcx (address)
                if offset != 0 {
                    self.emit_bytes(&[0x48, 0x81, 0xC1]);
                    self.emit_i32(offset as i32);
                }
                // mov [rcx], ax
                self.emit_bytes(&[0x66, 0x89, 0x01]);
            }

            // Reference types
            IrOpcode::RefNull => {
                // Push null reference (0)
                self.emit_bytes(&[0x31, 0xC0]); // xor eax, eax
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::RefIsNull => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x48, 0x85, 0xC0]); // test rax, rax
                self.emit_bytes(&[0x0F, 0x94, 0xC0]); // sete al
                self.emit_bytes(&[0x0F, 0xB6, 0xC0]); // movzx eax, al
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::RefFunc(func_idx) => {
                // Push function reference as index
                self.emit_byte(0xB8); // mov eax, func_idx
                self.emit_i32(func_idx as i32);
                self.emit_byte(0x50); // push rax
            }

            // Global variables
            IrOpcode::GlobalGet(_idx) => {
                // Placeholder: push 0 (global access needs runtime support)
                self.emit_bytes(&[0x31, 0xC0]); // xor eax, eax
                self.emit_byte(0x50); // push rax
            }
            IrOpcode::GlobalSet(_idx) => {
                // Placeholder: pop and discard
                self.emit_bytes(&[0x48, 0x83, 0xC4, 0x08]); // add rsp, 8
            }

            // Memory operations
            IrOpcode::Load32(offset) => {
                self.emit_byte(0x58); // pop rax (address)
                if offset != 0 {
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
                if !ir.results.is_empty() {
                    self.emit_byte(0x58); // pop rax (return value)
                }
            }
            IrOpcode::End => {
                // Block/loop end - resolve labels
            }
            IrOpcode::Drop => {
                self.emit_bytes(&[0x48, 0x83, 0xC4, 0x08]); // add rsp, 8
            }
            IrOpcode::Select => {
                self.emit_byte(0x58); // pop rax (condition)
                self.emit_byte(0x5A); // pop rdx (val2 / false)
                self.emit_byte(0x59); // pop rcx (val1 / true)
                self.emit_bytes(&[0x85, 0xC0]); // test eax, eax
                self.emit_bytes(&[0x48, 0x0F, 0x44, 0xCA]); // cmovz rcx, rdx
                self.emit_byte(0x51); // push rcx
            }
            IrOpcode::Block(_) => {
                let _label = self.labels.len();
                self.labels.push(None);
            }
            IrOpcode::Loop(_) => {
                let _label = self.labels.len();
                self.labels.push(Some(self.code.len()));
            }
            IrOpcode::If(_) => {
                self.emit_byte(0x58); // pop rax
                self.emit_bytes(&[0x85, 0xC0]); // test eax, eax
                self.emit_bytes(&[0x0F, 0x84]); // jz rel32
                let label = self.labels.len();
                self.labels.push(None);
                let patch_offset = self.code.len();
                self.emit_i32(0); // placeholder
                self.pending_labels.push((patch_offset, label, 0));
            }
            IrOpcode::Br(_depth) => {
                self.emit_byte(0xE9); // jmp rel32
                self.emit_i32(0); // placeholder
            }
            IrOpcode::Call(_func_idx) => {
                self.emit_byte(0xE8); // call rel32
                self.emit_i32(0); // placeholder
            }
            IrOpcode::Unreachable => {
                self.emit_bytes(&[0x0F, 0x0B]); // ud2
            }

            // Conversions
            IrOpcode::I32WrapI64 => {
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

    /// Emit 64-bit comparison with setcc (REX.W prefix).
    fn emit_comparison_64(&mut self, setcc: u8) {
        self.emit_byte(0x58); // pop rax (right)
        self.emit_byte(0x59); // pop rcx (left)
        self.emit_bytes(&[0x48, 0x39, 0xC1]); // cmp rcx, rax
        self.emit_bytes(&[0x0F, setcc, 0xC0]); // setcc al
        self.emit_bytes(&[0x0F, 0xB6, 0xC0]); // movzx eax, al
        self.emit_byte(0x50); // push rax
    }

    /// Emit f32 binary operation (commutative: pop both, op, push).
    fn emit_f32_binop(&mut self, op_bytes: &[u8]) {
        self.emit_byte(0x58); // pop rax (right)
        self.emit_byte(0x59); // pop rcx (left)
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC1]); // movq xmm0, rcx
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC8]); // movq xmm1, rax
        self.emit_bytes(op_bytes);
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
        self.emit_byte(0x50); // push rax
    }

    /// Emit f32 binary operation (ordered: left in xmm0, right in xmm1).
    fn emit_f32_binop_ordered(&mut self, op_bytes: &[u8]) {
        self.emit_byte(0x59); // pop rcx (right)
        self.emit_byte(0x58); // pop rax (left)
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC9]); // movq xmm1, rcx
        self.emit_bytes(op_bytes);
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
        self.emit_byte(0x50); // push rax
    }

    /// Emit f64 binary operation (commutative).
    fn emit_f64_binop(&mut self, op_bytes: &[u8]) {
        self.emit_byte(0x58); // pop rax (right)
        self.emit_byte(0x59); // pop rcx (left)
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC1]); // movq xmm0, rcx
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC8]); // movq xmm1, rax
        self.emit_bytes(op_bytes);
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
        self.emit_byte(0x50); // push rax
    }

    /// Emit f64 binary operation (ordered: left in xmm0, right in xmm1).
    fn emit_f64_binop_ordered(&mut self, op_bytes: &[u8]) {
        self.emit_byte(0x59); // pop rcx (right)
        self.emit_byte(0x58); // pop rax (left)
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC9]); // movq xmm1, rcx
        self.emit_bytes(op_bytes);
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
        self.emit_byte(0x50); // push rax
    }

    /// Emit f32 comparison using cmpss.
    fn emit_f32_comparison(&mut self, imm: u8) {
        self.emit_byte(0x59); // pop rcx (right)
        self.emit_byte(0x58); // pop rax (left)
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC9]); // movq xmm1, rcx
        // cmpss xmm0, xmm1, imm
        self.emit_bytes(&[0xF3, 0x0F, 0xC2, 0xC1, imm]);
        // movd eax, xmm0; and eax, 1
        self.emit_bytes(&[0x66, 0x0F, 0x7E, 0xC0]); // movd eax, xmm0
        self.emit_bytes(&[0x83, 0xE0, 0x01]); // and eax, 1
        self.emit_byte(0x50); // push rax
    }

    /// Emit f32 ordered comparison (left op right).
    fn emit_f32_comparison_ordered(&mut self, imm: u8) {
        self.emit_byte(0x59); // pop rcx (right)
        self.emit_byte(0x58); // pop rax (left)
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC9]); // movq xmm1, rcx
        self.emit_bytes(&[0xF3, 0x0F, 0xC2, 0xC1, imm]); // cmpss xmm0, xmm1, imm
        self.emit_bytes(&[0x66, 0x0F, 0x7E, 0xC0]); // movd eax, xmm0
        self.emit_bytes(&[0x83, 0xE0, 0x01]); // and eax, 1
        self.emit_byte(0x50); // push rax
    }

    /// Emit f32 ordered comparison with swapped operands.
    fn emit_f32_comparison_ordered_swap(&mut self, imm: u8) {
        self.emit_byte(0x58); // pop rax (right - becomes left after swap)
        self.emit_byte(0x59); // pop rcx (left - becomes right)
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC9]); // movq xmm1, rcx
        self.emit_bytes(&[0xF3, 0x0F, 0xC2, 0xC1, imm]); // cmpss xmm0, xmm1, imm
        self.emit_bytes(&[0x66, 0x0F, 0x7E, 0xC0]); // movd eax, xmm0
        self.emit_bytes(&[0x83, 0xE0, 0x01]); // and eax, 1
        self.emit_byte(0x50); // push rax
    }

    /// Emit f64 comparison using cmpsd.
    fn emit_f64_comparison(&mut self, imm: u8) {
        self.emit_byte(0x59); // pop rcx (right)
        self.emit_byte(0x58); // pop rax (left)
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC9]); // movq xmm1, rcx
        // cmpsd xmm0, xmm1, imm
        self.emit_bytes(&[0xF2, 0x0F, 0xC2, 0xC1, imm]);
        self.emit_bytes(&[0x66, 0x0F, 0x7E, 0xC0]); // movd eax, xmm0
        self.emit_bytes(&[0x83, 0xE0, 0x01]); // and eax, 1
        self.emit_byte(0x50); // push rax
    }

    /// Emit f64 ordered comparison.
    fn emit_f64_comparison_ordered(&mut self, imm: u8) {
        self.emit_byte(0x59); // pop rcx (right)
        self.emit_byte(0x58); // pop rax (left)
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC9]); // movq xmm1, rcx
        self.emit_bytes(&[0xF2, 0x0F, 0xC2, 0xC1, imm]); // cmpsd xmm0, xmm1, imm
        self.emit_bytes(&[0x66, 0x0F, 0x7E, 0xC0]); // movd eax, xmm0
        self.emit_bytes(&[0x83, 0xE0, 0x01]); // and eax, 1
        self.emit_byte(0x50); // push rax
    }

    /// Emit f64 ordered comparison with swapped operands.
    fn emit_f64_comparison_ordered_swap(&mut self, imm: u8) {
        self.emit_byte(0x58); // pop rax (right - becomes left after swap)
        self.emit_byte(0x59); // pop rcx (left - becomes right)
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
        self.emit_bytes(&[0x66, 0x48, 0x0F, 0x6E, 0xC9]); // movq xmm1, rcx
        self.emit_bytes(&[0xF2, 0x0F, 0xC2, 0xC1, imm]); // cmpsd xmm0, xmm1, imm
        self.emit_bytes(&[0x66, 0x0F, 0x7E, 0xC0]); // movd eax, xmm0
        self.emit_bytes(&[0x83, 0xE0, 0x01]); // and eax, 1
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jit::ir::{BlockId, IrFunction, IrInstruction, IrOpcode, IrType};
    use alloc::vec;
    use alloc::vec::Vec;

    /// Helper: compile a simple function with given body instructions.
    fn compile_func(body: Vec<IrOpcode>, params: Vec<IrType>, results: Vec<IrType>) -> NativeCode {
        let mut func = IrFunction::new(0, params, results);
        for op in body {
            func.add_instruction(IrInstruction::new(op, 0));
        }
        let gen = CodeGenerator::new();
        gen.generate_baseline(&func).expect("compilation failed")
    }

    /// Helper: compile with default empty params/results.
    fn compile_body(body: Vec<IrOpcode>) -> NativeCode {
        compile_func(body, vec![], vec![])
    }

    // ====== i32 arithmetic ======

    #[test]
    fn test_i32_add_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(10),
            IrOpcode::Const32(20),
            IrOpcode::I32Add,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i32_sub_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(30),
            IrOpcode::Const32(10),
            IrOpcode::I32Sub,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i32_mul_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(6),
            IrOpcode::Const32(7),
            IrOpcode::I32Mul,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i32_div_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(42),
            IrOpcode::Const32(6),
            IrOpcode::I32DivS,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i32_rem_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(17),
            IrOpcode::Const32(5),
            IrOpcode::I32RemS,
            IrOpcode::Const32(17),
            IrOpcode::Const32(5),
            IrOpcode::I32RemU,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i32_bitwise_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(0xFF00),
            IrOpcode::Const32(0x0FF0),
            IrOpcode::I32And,
            IrOpcode::Const32(0xFF00),
            IrOpcode::Const32(0x0FF0),
            IrOpcode::I32Or,
            IrOpcode::Const32(0xFF00),
            IrOpcode::Const32(0x0FF0),
            IrOpcode::I32Xor,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i32_shift_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(1),
            IrOpcode::Const32(4),
            IrOpcode::I32Shl,
            IrOpcode::Const32(16),
            IrOpcode::Const32(2),
            IrOpcode::I32ShrU,
            IrOpcode::Const32(-16),
            IrOpcode::Const32(2),
            IrOpcode::I32ShrS,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i32_rotate_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(1),
            IrOpcode::Const32(4),
            IrOpcode::I32Rotl,
            IrOpcode::Const32(0x80000000u32 as i32),
            IrOpcode::Const32(1),
            IrOpcode::I32Rotr,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i32_clz_ctz_popcnt_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(0x00FF0000),
            IrOpcode::I32Clz,
            IrOpcode::Const32(0x00FF0000),
            IrOpcode::I32Ctz,
            IrOpcode::Const32(0x55555555),
            IrOpcode::I32Popcnt,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i32_eqz_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(0),
            IrOpcode::I32Eqz,
            IrOpcode::Const32(42),
            IrOpcode::I32Eqz,
        ]);
        assert!(code.size() > 0);
    }

    // ====== i32 comparisons ======

    #[test]
    fn test_i32_comparisons_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(5),
            IrOpcode::Const32(10),
            IrOpcode::I32Eq,
            IrOpcode::Const32(5),
            IrOpcode::Const32(10),
            IrOpcode::I32Ne,
            IrOpcode::Const32(5),
            IrOpcode::Const32(10),
            IrOpcode::I32LtS,
            IrOpcode::Const32(5),
            IrOpcode::Const32(10),
            IrOpcode::I32LtU,
            IrOpcode::Const32(5),
            IrOpcode::Const32(10),
            IrOpcode::I32GtS,
            IrOpcode::Const32(5),
            IrOpcode::Const32(10),
            IrOpcode::I32GtU,
            IrOpcode::Const32(5),
            IrOpcode::Const32(10),
            IrOpcode::I32LeS,
            IrOpcode::Const32(5),
            IrOpcode::Const32(10),
            IrOpcode::I32LeU,
            IrOpcode::Const32(5),
            IrOpcode::Const32(10),
            IrOpcode::I32GeS,
            IrOpcode::Const32(5),
            IrOpcode::Const32(10),
            IrOpcode::I32GeU,
        ]);
        assert!(code.size() > 0);
    }

    // ====== i64 arithmetic ======

    #[test]
    fn test_i64_add_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const64(100),
            IrOpcode::Const64(200),
            IrOpcode::I64Add,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i64_sub_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const64(300),
            IrOpcode::Const64(100),
            IrOpcode::I64Sub,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i64_mul_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const64(1000000),
            IrOpcode::Const64(1000000),
            IrOpcode::I64Mul,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i64_div_s_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const64(-100),
            IrOpcode::Const64(7),
            IrOpcode::I64DivS,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i64_div_u_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const64(100),
            IrOpcode::Const64(7),
            IrOpcode::I64DivU,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i64_rem_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const64(17),
            IrOpcode::Const64(5),
            IrOpcode::I64RemS,
            IrOpcode::Const64(17),
            IrOpcode::Const64(5),
            IrOpcode::I64RemU,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i64_bitwise_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const64(0xFF00FF00FF00FF00u64 as i64),
            IrOpcode::Const64(0x0FF00FF00FF00FF0u64 as i64),
            IrOpcode::I64And,
            IrOpcode::Const64(0xFF00FF00FF00FF00u64 as i64),
            IrOpcode::Const64(0x0FF00FF00FF00FF0u64 as i64),
            IrOpcode::I64Or,
            IrOpcode::Const64(0xFF00FF00FF00FF00u64 as i64),
            IrOpcode::Const64(0x0FF00FF00FF00FF0u64 as i64),
            IrOpcode::I64Xor,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i64_shift_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const64(1),
            IrOpcode::Const64(32),
            IrOpcode::I64Shl,
            IrOpcode::Const64(0x100000000i64),
            IrOpcode::Const64(16),
            IrOpcode::I64ShrU,
            IrOpcode::Const64(-1),
            IrOpcode::Const64(32),
            IrOpcode::I64ShrS,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i64_rotate_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const64(1),
            IrOpcode::Const64(63),
            IrOpcode::I64Rotl,
            IrOpcode::Const64(0x8000000000000000u64 as i64),
            IrOpcode::Const64(1),
            IrOpcode::I64Rotr,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i64_clz_ctz_popcnt_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const64(0x00FF000000000000i64),
            IrOpcode::I64Clz,
            IrOpcode::Const64(0x00FF000000000000i64),
            IrOpcode::I64Ctz,
            IrOpcode::Const64(0x5555555555555555u64 as i64),
            IrOpcode::I64Popcnt,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_i64_eqz_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const64(0),
            IrOpcode::I64Eqz,
            IrOpcode::Const64(42),
            IrOpcode::I64Eqz,
        ]);
        assert!(code.size() > 0);
    }

    // ====== i64 comparisons ======

    #[test]
    fn test_i64_comparisons_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const64(5),
            IrOpcode::Const64(10),
            IrOpcode::I64Eq,
            IrOpcode::Const64(5),
            IrOpcode::Const64(10),
            IrOpcode::I64Ne,
            IrOpcode::Const64(5),
            IrOpcode::Const64(10),
            IrOpcode::I64LtS,
            IrOpcode::Const64(5),
            IrOpcode::Const64(10),
            IrOpcode::I64LtU,
            IrOpcode::Const64(5),
            IrOpcode::Const64(10),
            IrOpcode::I64GtS,
            IrOpcode::Const64(5),
            IrOpcode::Const64(10),
            IrOpcode::I64GtU,
            IrOpcode::Const64(5),
            IrOpcode::Const64(10),
            IrOpcode::I64LeS,
            IrOpcode::Const64(5),
            IrOpcode::Const64(10),
            IrOpcode::I64LeU,
            IrOpcode::Const64(5),
            IrOpcode::Const64(10),
            IrOpcode::I64GeS,
            IrOpcode::Const64(5),
            IrOpcode::Const64(10),
            IrOpcode::I64GeU,
        ]);
        assert!(code.size() > 0);
    }

    // ====== f32 arithmetic ======

    #[test]
    fn test_f32_add_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF32(1.5f32.to_bits()),
            IrOpcode::ConstF32(2.5f32.to_bits()),
            IrOpcode::F32Add,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_f32_sub_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF32(10.0f32.to_bits()),
            IrOpcode::ConstF32(3.0f32.to_bits()),
            IrOpcode::F32Sub,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_f32_mul_div_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF32(6.0f32.to_bits()),
            IrOpcode::ConstF32(7.0f32.to_bits()),
            IrOpcode::F32Mul,
            IrOpcode::ConstF32(42.0f32.to_bits()),
            IrOpcode::ConstF32(6.0f32.to_bits()),
            IrOpcode::F32Div,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_f32_sqrt_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF32(9.0f32.to_bits()),
            IrOpcode::F32Sqrt,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_f32_abs_neg_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF32((-5.0f32).to_bits()),
            IrOpcode::F32Abs,
            IrOpcode::ConstF32(5.0f32.to_bits()),
            IrOpcode::F32Neg,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_f32_rounding_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF32(3.7f32.to_bits()),
            IrOpcode::F32Ceil,
            IrOpcode::ConstF32(3.7f32.to_bits()),
            IrOpcode::F32Floor,
            IrOpcode::ConstF32(3.7f32.to_bits()),
            IrOpcode::F32Trunc,
            IrOpcode::ConstF32(3.5f32.to_bits()),
            IrOpcode::F32Nearest,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_f32_min_max_copysign_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF32(3.0f32.to_bits()),
            IrOpcode::ConstF32(5.0f32.to_bits()),
            IrOpcode::F32Min,
            IrOpcode::ConstF32(3.0f32.to_bits()),
            IrOpcode::ConstF32(5.0f32.to_bits()),
            IrOpcode::F32Max,
            IrOpcode::ConstF32(5.0f32.to_bits()),
            IrOpcode::ConstF32((-1.0f32).to_bits()),
            IrOpcode::F32Copysign,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_f32_comparisons_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF32(1.0f32.to_bits()),
            IrOpcode::ConstF32(2.0f32.to_bits()),
            IrOpcode::F32Eq,
            IrOpcode::ConstF32(1.0f32.to_bits()),
            IrOpcode::ConstF32(2.0f32.to_bits()),
            IrOpcode::F32Ne,
            IrOpcode::ConstF32(1.0f32.to_bits()),
            IrOpcode::ConstF32(2.0f32.to_bits()),
            IrOpcode::F32Lt,
            IrOpcode::ConstF32(2.0f32.to_bits()),
            IrOpcode::ConstF32(1.0f32.to_bits()),
            IrOpcode::F32Gt,
            IrOpcode::ConstF32(1.0f32.to_bits()),
            IrOpcode::ConstF32(2.0f32.to_bits()),
            IrOpcode::F32Le,
            IrOpcode::ConstF32(2.0f32.to_bits()),
            IrOpcode::ConstF32(1.0f32.to_bits()),
            IrOpcode::F32Ge,
        ]);
        assert!(code.size() > 0);
    }

    // ====== f64 arithmetic ======

    #[test]
    fn test_f64_add_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF64(1.5f64.to_bits()),
            IrOpcode::ConstF64(2.5f64.to_bits()),
            IrOpcode::F64Add,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_f64_sub_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF64(10.0f64.to_bits()),
            IrOpcode::ConstF64(3.0f64.to_bits()),
            IrOpcode::F64Sub,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_f64_mul_div_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF64(6.0f64.to_bits()),
            IrOpcode::ConstF64(7.0f64.to_bits()),
            IrOpcode::F64Mul,
            IrOpcode::ConstF64(42.0f64.to_bits()),
            IrOpcode::ConstF64(6.0f64.to_bits()),
            IrOpcode::F64Div,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_f64_sqrt_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF64(9.0f64.to_bits()),
            IrOpcode::F64Sqrt,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_f64_abs_neg_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF64((-5.0f64).to_bits()),
            IrOpcode::F64Abs,
            IrOpcode::ConstF64(5.0f64.to_bits()),
            IrOpcode::F64Neg,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_f64_rounding_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF64(3.7f64.to_bits()),
            IrOpcode::F64Ceil,
            IrOpcode::ConstF64(3.7f64.to_bits()),
            IrOpcode::F64Floor,
            IrOpcode::ConstF64(3.7f64.to_bits()),
            IrOpcode::F64Trunc,
            IrOpcode::ConstF64(3.5f64.to_bits()),
            IrOpcode::F64Nearest,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_f64_min_max_copysign_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF64(3.0f64.to_bits()),
            IrOpcode::ConstF64(5.0f64.to_bits()),
            IrOpcode::F64Min,
            IrOpcode::ConstF64(3.0f64.to_bits()),
            IrOpcode::ConstF64(5.0f64.to_bits()),
            IrOpcode::F64Max,
            IrOpcode::ConstF64(5.0f64.to_bits()),
            IrOpcode::ConstF64((-1.0f64).to_bits()),
            IrOpcode::F64Copysign,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_f64_comparisons_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF64(1.0f64.to_bits()),
            IrOpcode::ConstF64(2.0f64.to_bits()),
            IrOpcode::F64Eq,
            IrOpcode::ConstF64(1.0f64.to_bits()),
            IrOpcode::ConstF64(2.0f64.to_bits()),
            IrOpcode::F64Ne,
            IrOpcode::ConstF64(1.0f64.to_bits()),
            IrOpcode::ConstF64(2.0f64.to_bits()),
            IrOpcode::F64Lt,
            IrOpcode::ConstF64(2.0f64.to_bits()),
            IrOpcode::ConstF64(1.0f64.to_bits()),
            IrOpcode::F64Gt,
            IrOpcode::ConstF64(1.0f64.to_bits()),
            IrOpcode::ConstF64(2.0f64.to_bits()),
            IrOpcode::F64Le,
            IrOpcode::ConstF64(2.0f64.to_bits()),
            IrOpcode::ConstF64(1.0f64.to_bits()),
            IrOpcode::F64Ge,
        ]);
        assert!(code.size() > 0);
    }

    // ====== Conversions ======

    #[test]
    fn test_int_conversions_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const64(0x1FFFFFFFF),
            IrOpcode::I32WrapI64,
            IrOpcode::Const32(-1),
            IrOpcode::I64ExtendI32S,
            IrOpcode::Const32(-1),
            IrOpcode::I64ExtendI32U,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_float_to_int_conversions_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF32(42.9f32.to_bits()),
            IrOpcode::I32TruncF32S,
            IrOpcode::ConstF32(42.9f32.to_bits()),
            IrOpcode::I32TruncF32U,
            IrOpcode::ConstF64(42.9f64.to_bits()),
            IrOpcode::I32TruncF64S,
            IrOpcode::ConstF64(42.9f64.to_bits()),
            IrOpcode::I32TruncF64U,
            IrOpcode::ConstF32(42.9f32.to_bits()),
            IrOpcode::I64TruncF32S,
            IrOpcode::ConstF64(42.9f64.to_bits()),
            IrOpcode::I64TruncF64S,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_int_to_float_conversions_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(42),
            IrOpcode::F32ConvertI32S,
            IrOpcode::Const32(42),
            IrOpcode::F32ConvertI32U,
            IrOpcode::Const64(42),
            IrOpcode::F32ConvertI64S,
            IrOpcode::Const32(42),
            IrOpcode::F64ConvertI32S,
            IrOpcode::Const32(42),
            IrOpcode::F64ConvertI32U,
            IrOpcode::Const64(42),
            IrOpcode::F64ConvertI64S,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_float_float_conversions_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF64(3.14f64.to_bits()),
            IrOpcode::F32DemoteF64,
            IrOpcode::ConstF32(3.14f32.to_bits()),
            IrOpcode::F64PromoteF32,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_reinterpret_codegen() {
        let code = compile_body(vec![
            IrOpcode::ConstF32(1.0f32.to_bits()),
            IrOpcode::I32ReinterpretF32,
            IrOpcode::Const32(0x3F800000),
            IrOpcode::F32ReinterpretI32,
            IrOpcode::ConstF64(1.0f64.to_bits()),
            IrOpcode::I64ReinterpretF64,
            IrOpcode::Const64(0x3FF0000000000000),
            IrOpcode::F64ReinterpretI64,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_sign_extension_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(0x80),
            IrOpcode::I32Extend8S,
            IrOpcode::Const32(0x8000),
            IrOpcode::I32Extend16S,
            IrOpcode::Const64(0x80),
            IrOpcode::I64Extend8S,
            IrOpcode::Const64(0x8000),
            IrOpcode::I64Extend16S,
            IrOpcode::Const64(0x80000000),
            IrOpcode::I64Extend32S,
        ]);
        assert!(code.size() > 0);
    }

    // ====== Control flow ======

    #[test]
    fn test_block_codegen() {
        let code = compile_body(vec![
            IrOpcode::Block(BlockId(0)),
            IrOpcode::Const32(42),
            IrOpcode::Drop,
            IrOpcode::End,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_loop_codegen() {
        let code = compile_body(vec![
            IrOpcode::Loop(BlockId(0)),
            IrOpcode::Const32(1),
            IrOpcode::Drop,
            IrOpcode::End,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_if_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(1),
            IrOpcode::If(BlockId(0)),
            IrOpcode::Const32(42),
            IrOpcode::Drop,
            IrOpcode::End,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_if_else_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(0),
            IrOpcode::If(BlockId(0)),
            IrOpcode::Const32(1),
            IrOpcode::Drop,
            IrOpcode::Else,
            IrOpcode::Const32(2),
            IrOpcode::Drop,
            IrOpcode::End,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_br_codegen() {
        let code = compile_body(vec![
            IrOpcode::Block(BlockId(0)),
            IrOpcode::Br(0),
            IrOpcode::End,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_br_if_codegen() {
        let code = compile_body(vec![
            IrOpcode::Block(BlockId(0)),
            IrOpcode::Const32(1),
            IrOpcode::BrIf(0),
            IrOpcode::End,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_br_table_codegen() {
        let code = compile_body(vec![
            IrOpcode::Block(BlockId(0)),
            IrOpcode::Const32(0),
            IrOpcode::BrTable(0),
            IrOpcode::End,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_call_codegen() {
        let code = compile_body(vec![
            IrOpcode::Call(0),
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_call_indirect_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(0),
            IrOpcode::CallIndirect(0),
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_unreachable_codegen() {
        let code = compile_body(vec![IrOpcode::Unreachable]);
        assert!(code.size() > 0);
        // Should contain ud2 (0x0F, 0x0B)
        let code_slice = code.code();
        assert!(code_slice.windows(2).any(|w| w == [0x0F, 0x0B]));
    }

    // ====== Stack operations ======

    #[test]
    fn test_select_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(10),
            IrOpcode::Const32(20),
            IrOpcode::Const32(1),
            IrOpcode::Select,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_drop_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(42),
            IrOpcode::Drop,
        ]);
        assert!(code.size() > 0);
    }

    // ====== Memory operations ======

    #[test]
    fn test_memory_load_store_variants_codegen() {
        let code = compile_body(vec![
            // Load variants
            IrOpcode::Const32(0),
            IrOpcode::Load8S(0),
            IrOpcode::Const32(0),
            IrOpcode::Load8U(4),
            IrOpcode::Const32(0),
            IrOpcode::Load16S(0),
            IrOpcode::Const32(0),
            IrOpcode::Load16U(8),
            IrOpcode::Const32(0),
            IrOpcode::Load32(0),
            IrOpcode::Const32(0),
            IrOpcode::Load64(16),
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_memory_store_variants_codegen() {
        let code = compile_body(vec![
            IrOpcode::Const32(0),
            IrOpcode::Const32(42),
            IrOpcode::Store8(0),
            IrOpcode::Const32(0),
            IrOpcode::Const32(42),
            IrOpcode::Store16(4),
            IrOpcode::Const32(0),
            IrOpcode::Const32(42),
            IrOpcode::Store32(0),
            IrOpcode::Const32(0),
            IrOpcode::Const64(42),
            IrOpcode::Store64(8),
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_memory_size_grow_codegen() {
        let code = compile_body(vec![
            IrOpcode::MemorySize,
            IrOpcode::Const32(1),
            IrOpcode::MemoryGrow,
        ]);
        assert!(code.size() > 0);
    }

    // ====== Reference types ======

    #[test]
    fn test_ref_null_is_null_codegen() {
        let code = compile_body(vec![
            IrOpcode::RefNull,
            IrOpcode::RefIsNull,
        ]);
        assert!(code.size() > 0);
    }

    #[test]
    fn test_ref_func_codegen() {
        let code = compile_body(vec![IrOpcode::RefFunc(42)]);
        assert!(code.size() > 0);
    }

    // ====== Local variables ======

    #[test]
    fn test_local_get_set_tee_codegen() {
        let mut func = IrFunction::new(0, vec![IrType::I32], vec![IrType::I32]);
        func.add_local(IrType::I32);
        func.add_instruction(IrInstruction::new(IrOpcode::Const32(42), 0));
        func.add_instruction(IrInstruction::new(IrOpcode::LocalSet(1), 0));
        func.add_instruction(IrInstruction::new(IrOpcode::LocalGet(1), 0));
        func.add_instruction(IrInstruction::new(IrOpcode::LocalTee(0), 0));
        func.add_instruction(IrInstruction::new(IrOpcode::LocalGet(0), 0));
        let gen = CodeGenerator::new();
        let code = gen.generate_baseline(&func).expect("compilation failed");
        assert!(code.size() > 0);
    }

    // ====== Global variables ======

    #[test]
    fn test_global_get_set_codegen() {
        let code = compile_body(vec![
            IrOpcode::GlobalGet(0),
            IrOpcode::Const32(42),
            IrOpcode::GlobalSet(0),
        ]);
        assert!(code.size() > 0);
    }

    // ====== Return ======

    #[test]
    fn test_return_with_value_codegen() {
        let code = compile_func(
            vec![IrOpcode::Const32(42), IrOpcode::Return],
            vec![],
            vec![IrType::I32],
        );
        assert!(code.size() > 0);
    }

    // ====== Prologue/Epilogue ======

    #[test]
    fn test_prologue_epilogue_small_frame() {
        let code = compile_body(vec![]);
        // Should contain push rbp (0x55) and pop rbp (0x5D), ret (0xC3)
        let s = code.code();
        assert_eq!(s[0], 0x55); // push rbp
        assert_eq!(*s.last().unwrap(), 0xC3); // ret
    }

    #[test]
    fn test_prologue_epilogue_large_frame() {
        let mut func = IrFunction::new(0, vec![], vec![]);
        // Add enough locals to exceed 127-byte frame
        for _ in 0..20 {
            func.add_local(IrType::I64);
        }
        let gen = CodeGenerator::new();
        let code = gen.generate_baseline(&func).expect("compilation failed");
        assert_eq!(code.code()[0], 0x55); // push rbp
    }

    // ====== Code generation properties ======

    #[test]
    fn test_codegen_deterministic() {
        let body = vec![
            IrOpcode::Const32(42),
            IrOpcode::Const32(10),
            IrOpcode::I32Add,
            IrOpcode::Drop,
        ];
        let code1 = compile_body(body.clone());
        let code2 = compile_body(body);
        assert_eq!(code1.code(), code2.code());
    }

    #[test]
    fn test_codegen_empty_function() {
        let code = compile_body(vec![]);
        // Even empty function should have prologue + epilogue
        assert!(code.size() > 0);
    }

    #[test]
    fn test_native_code_clone() {
        let code = compile_body(vec![IrOpcode::Const32(42)]);
        let cloned = code.clone();
        assert_eq!(code.code(), cloned.code());
        assert_eq!(code.entry_offset(), cloned.entry_offset());
        assert_eq!(code.frame_size(), cloned.frame_size());
    }

    #[test]
    fn test_optimized_generation() {
        let mut func = IrFunction::new(0, vec![], vec![]);
        func.add_instruction(IrInstruction::new(IrOpcode::Const32(42), 0));
        func.add_instruction(IrInstruction::new(IrOpcode::Drop, 0));
        let gen = CodeGenerator::new();
        let code = gen.generate_optimized(&func).expect("optimized compilation failed");
        assert!(code.size() > 0);
    }

    #[test]
    fn test_complex_arithmetic_sequence() {
        // Mix of i32, i64, f32, f64 operations
        let code = compile_body(vec![
            IrOpcode::Const32(10),
            IrOpcode::Const32(20),
            IrOpcode::I32Add,
            IrOpcode::Drop,
            IrOpcode::Const64(100),
            IrOpcode::Const64(200),
            IrOpcode::I64Mul,
            IrOpcode::Drop,
            IrOpcode::ConstF32(3.14f32.to_bits()),
            IrOpcode::ConstF32(2.0f32.to_bits()),
            IrOpcode::F32Add,
            IrOpcode::Drop,
            IrOpcode::ConstF64(1.0f64.to_bits()),
            IrOpcode::ConstF64(2.0f64.to_bits()),
            IrOpcode::F64Mul,
            IrOpcode::Drop,
        ]);
        assert!(code.size() > 0);
    }
}
