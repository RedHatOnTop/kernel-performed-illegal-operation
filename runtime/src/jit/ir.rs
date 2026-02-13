//! WASM to Native IR Translation
//!
//! This module provides an intermediate representation for translating
//! WebAssembly bytecode to native machine code.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// IR Opcode for the JIT compiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrOpcode {
    // Constants
    Const32(i32),
    Const64(i64),
    ConstF32(u32),  // Bit pattern
    ConstF64(u64),  // Bit pattern
    
    // Local variables
    LocalGet(u32),
    LocalSet(u32),
    LocalTee(u32),
    
    // Global variables
    GlobalGet(u32),
    GlobalSet(u32),
    
    // Memory operations
    Load32(u32),      // offset
    Load64(u32),
    Load8S(u32),
    Load8U(u32),
    Load16S(u32),
    Load16U(u32),
    Store32(u32),
    Store64(u32),
    Store8(u32),
    Store16(u32),
    
    // Arithmetic (i32)
    I32Add,
    I32Sub,
    I32Mul,
    I32DivS,
    I32DivU,
    I32RemS,
    I32RemU,
    I32And,
    I32Or,
    I32Xor,
    I32Shl,
    I32ShrS,
    I32ShrU,
    I32Rotl,
    I32Rotr,
    I32Clz,
    I32Ctz,
    I32Popcnt,
    I32Eqz,
    
    // Arithmetic (i64)
    I64Add,
    I64Sub,
    I64Mul,
    I64DivS,
    I64DivU,
    I64RemS,
    I64RemU,
    I64And,
    I64Or,
    I64Xor,
    I64Shl,
    I64ShrS,
    I64ShrU,
    I64Rotl,
    I64Rotr,
    I64Clz,
    I64Ctz,
    I64Popcnt,
    I64Eqz,
    
    // Floating point (f32)
    F32Add,
    F32Sub,
    F32Mul,
    F32Div,
    F32Sqrt,
    F32Abs,
    F32Neg,
    F32Ceil,
    F32Floor,
    F32Trunc,
    F32Nearest,
    F32Min,
    F32Max,
    F32Copysign,
    
    // Floating point (f64)
    F64Add,
    F64Sub,
    F64Mul,
    F64Div,
    F64Sqrt,
    F64Abs,
    F64Neg,
    F64Ceil,
    F64Floor,
    F64Trunc,
    F64Nearest,
    F64Min,
    F64Max,
    F64Copysign,
    
    // Comparisons (i32)
    I32Eq,
    I32Ne,
    I32LtS,
    I32LtU,
    I32GtS,
    I32GtU,
    I32LeS,
    I32LeU,
    I32GeS,
    I32GeU,
    
    // Comparisons (i64)
    I64Eq,
    I64Ne,
    I64LtS,
    I64LtU,
    I64GtS,
    I64GtU,
    I64LeS,
    I64LeU,
    I64GeS,
    I64GeU,
    
    // Comparisons (f32)
    F32Eq,
    F32Ne,
    F32Lt,
    F32Gt,
    F32Le,
    F32Ge,
    
    // Comparisons (f64)
    F64Eq,
    F64Ne,
    F64Lt,
    F64Gt,
    F64Le,
    F64Ge,
    
    // Conversions
    I32WrapI64,
    I64ExtendI32S,
    I64ExtendI32U,
    I32TruncF32S,
    I32TruncF32U,
    I32TruncF64S,
    I32TruncF64U,
    I64TruncF32S,
    I64TruncF32U,
    I64TruncF64S,
    I64TruncF64U,
    F32ConvertI32S,
    F32ConvertI32U,
    F32ConvertI64S,
    F32ConvertI64U,
    F64ConvertI32S,
    F64ConvertI32U,
    F64ConvertI64S,
    F64ConvertI64U,
    F32DemoteF64,
    F64PromoteF32,
    I32ReinterpretF32,
    I64ReinterpretF64,
    F32ReinterpretI32,
    F64ReinterpretI64,
    I32Extend8S,
    I32Extend16S,
    I64Extend8S,
    I64Extend16S,
    I64Extend32S,
    
    // Control flow
    Block(BlockId),
    Loop(BlockId),
    If(BlockId),
    Else,
    End,
    Br(u32),
    BrIf(u32),
    BrTable(u32),  // Index into branch table
    Return,
    Unreachable,
    
    // Calls
    Call(u32),           // Function index
    CallIndirect(u32),   // Type index
    
    // Stack operations
    Drop,
    Select,
    
    // Memory size
    MemorySize,
    MemoryGrow,
    
    // Reference types
    RefNull,
    RefIsNull,
    RefFunc(u32),
}

/// Block identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(pub u32);

/// IR Value type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrType {
    I32,
    I64,
    F32,
    F64,
    V128,
    FuncRef,
    ExternRef,
}

impl IrType {
    /// Size in bytes.
    pub fn size(&self) -> usize {
        match self {
            IrType::I32 | IrType::F32 => 4,
            IrType::I64 | IrType::F64 => 8,
            IrType::V128 => 16,
            IrType::FuncRef | IrType::ExternRef => 8, // Pointer size
        }
    }
}

/// IR instruction with metadata.
#[derive(Debug, Clone)]
pub struct IrInstruction {
    pub opcode: IrOpcode,
    pub offset: u32,      // Offset in original WASM bytecode
}

impl IrInstruction {
    pub fn new(opcode: IrOpcode, offset: u32) -> Self {
        Self { opcode, offset }
    }
}

/// IR Function.
#[derive(Debug, Clone)]
pub struct IrFunction {
    /// Function index.
    pub index: u32,
    /// Parameter types.
    pub params: Vec<IrType>,
    /// Result types.
    pub results: Vec<IrType>,
    /// Local variable types.
    pub locals: Vec<IrType>,
    /// IR instructions.
    pub body: Vec<IrInstruction>,
    /// Branch tables for br_table.
    pub branch_tables: Vec<Vec<u32>>,
    /// Block information.
    pub blocks: BTreeMap<BlockId, BlockInfo>,
}

/// Information about a block.
#[derive(Debug, Clone)]
pub struct BlockInfo {
    pub kind: BlockKind,
    pub params: Vec<IrType>,
    pub results: Vec<IrType>,
    pub start_offset: u32,
    pub end_offset: Option<u32>,
}

/// Kind of block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Block,
    Loop,
    If,
}

impl IrFunction {
    pub fn new(index: u32, params: Vec<IrType>, results: Vec<IrType>) -> Self {
        Self {
            index,
            params,
            results,
            locals: Vec::new(),
            body: Vec::new(),
            branch_tables: Vec::new(),
            blocks: BTreeMap::new(),
        }
    }
    
    pub fn add_local(&mut self, ty: IrType) {
        self.locals.push(ty);
    }
    
    pub fn add_instruction(&mut self, inst: IrInstruction) {
        self.body.push(inst);
    }
    
    pub fn add_branch_table(&mut self, table: Vec<u32>) -> u32 {
        let idx = self.branch_tables.len() as u32;
        self.branch_tables.push(table);
        idx
    }
    
    /// Total number of locals (params + locals).
    pub fn total_locals(&self) -> usize {
        self.params.len() + self.locals.len()
    }
    
    /// Get local type by index.
    pub fn local_type(&self, idx: u32) -> Option<IrType> {
        let idx = idx as usize;
        if idx < self.params.len() {
            Some(self.params[idx])
        } else {
            self.locals.get(idx - self.params.len()).copied()
        }
    }
}

/// WASM to IR translator.
pub struct WasmToIr {
    /// Current function being translated.
    current_func: Option<IrFunction>,
    /// Current block stack.
    block_stack: Vec<BlockId>,
    /// Next block ID.
    next_block_id: u32,
}

impl WasmToIr {
    pub fn new() -> Self {
        Self {
            current_func: None,
            block_stack: Vec::new(),
            next_block_id: 0,
        }
    }
    
    /// Translate a function from WASM bytecode.
    pub fn translate_function(
        &mut self,
        index: u32,
        params: Vec<IrType>,
        results: Vec<IrType>,
        locals: &[(u32, IrType)],
        code: &[u8],
    ) -> Result<IrFunction, TranslationError> {
        let mut func = IrFunction::new(index, params, results);
        
        // Add locals
        for &(count, ty) in locals {
            for _ in 0..count {
                func.add_local(ty);
            }
        }
        
        self.current_func = Some(func);
        self.block_stack.clear();
        self.next_block_id = 0;
        
        // Translate bytecode
        self.translate_bytecode(code)?;
        
        Ok(self.current_func.take().unwrap())
    }
    
    fn translate_bytecode(&mut self, code: &[u8]) -> Result<(), TranslationError> {
        let mut offset = 0u32;
        let mut reader = ByteReader::new(code);
        
        while !reader.is_empty() {
            let opcode_byte = reader.read_u8()?;
            let inst = self.translate_opcode(opcode_byte, &mut reader, offset)?;
            
            if let Some(ref mut func) = self.current_func {
                func.add_instruction(inst);
            }
            
            offset = (code.len() - reader.remaining()) as u32;
        }
        
        Ok(())
    }
    
    fn translate_opcode(
        &mut self,
        opcode: u8,
        reader: &mut ByteReader,
        offset: u32,
    ) -> Result<IrInstruction, TranslationError> {
        let ir_opcode = match opcode {
            // Control flow
            0x00 => IrOpcode::Unreachable,
            0x01 => return Ok(IrInstruction::new(IrOpcode::End, offset)), // nop
            0x02 => {
                let block_id = self.new_block(BlockKind::Block);
                let _block_type = reader.read_signed_leb128()?;
                IrOpcode::Block(block_id)
            }
            0x03 => {
                let block_id = self.new_block(BlockKind::Loop);
                let _block_type = reader.read_signed_leb128()?;
                IrOpcode::Loop(block_id)
            }
            0x04 => {
                let block_id = self.new_block(BlockKind::If);
                let _block_type = reader.read_signed_leb128()?;
                IrOpcode::If(block_id)
            }
            0x05 => IrOpcode::Else,
            0x0B => {
                self.block_stack.pop();
                IrOpcode::End
            }
            0x0C => {
                let depth = reader.read_unsigned_leb128()? as u32;
                IrOpcode::Br(depth)
            }
            0x0D => {
                let depth = reader.read_unsigned_leb128()? as u32;
                IrOpcode::BrIf(depth)
            }
            0x0E => {
                let count = reader.read_unsigned_leb128()? as usize;
                let mut targets = Vec::with_capacity(count + 1);
                for _ in 0..=count {
                    targets.push(reader.read_unsigned_leb128()? as u32);
                }
                let table_idx = if let Some(ref mut func) = self.current_func {
                    func.add_branch_table(targets)
                } else {
                    0
                };
                IrOpcode::BrTable(table_idx)
            }
            0x0F => IrOpcode::Return,
            
            // Calls
            0x10 => {
                let func_idx = reader.read_unsigned_leb128()? as u32;
                IrOpcode::Call(func_idx)
            }
            0x11 => {
                let type_idx = reader.read_unsigned_leb128()? as u32;
                let _table_idx = reader.read_unsigned_leb128()?; // table index (must be 0)
                IrOpcode::CallIndirect(type_idx)
            }
            
            // Parametric
            0x1A => IrOpcode::Drop,
            0x1B => IrOpcode::Select,
            
            // Variables
            0x20 => {
                let idx = reader.read_unsigned_leb128()? as u32;
                IrOpcode::LocalGet(idx)
            }
            0x21 => {
                let idx = reader.read_unsigned_leb128()? as u32;
                IrOpcode::LocalSet(idx)
            }
            0x22 => {
                let idx = reader.read_unsigned_leb128()? as u32;
                IrOpcode::LocalTee(idx)
            }
            0x23 => {
                let idx = reader.read_unsigned_leb128()? as u32;
                IrOpcode::GlobalGet(idx)
            }
            0x24 => {
                let idx = reader.read_unsigned_leb128()? as u32;
                IrOpcode::GlobalSet(idx)
            }
            
            // Memory loads
            0x28 => {
                let _align = reader.read_unsigned_leb128()?;
                let offset = reader.read_unsigned_leb128()? as u32;
                IrOpcode::Load32(offset)
            }
            0x29 => {
                let _align = reader.read_unsigned_leb128()?;
                let offset = reader.read_unsigned_leb128()? as u32;
                IrOpcode::Load64(offset)
            }
            0x2C => {
                let _align = reader.read_unsigned_leb128()?;
                let offset = reader.read_unsigned_leb128()? as u32;
                IrOpcode::Load8S(offset)
            }
            0x2D => {
                let _align = reader.read_unsigned_leb128()?;
                let offset = reader.read_unsigned_leb128()? as u32;
                IrOpcode::Load8U(offset)
            }
            0x2E => {
                let _align = reader.read_unsigned_leb128()?;
                let offset = reader.read_unsigned_leb128()? as u32;
                IrOpcode::Load16S(offset)
            }
            0x2F => {
                let _align = reader.read_unsigned_leb128()?;
                let offset = reader.read_unsigned_leb128()? as u32;
                IrOpcode::Load16U(offset)
            }
            
            // Memory stores
            0x36 => {
                let _align = reader.read_unsigned_leb128()?;
                let offset = reader.read_unsigned_leb128()? as u32;
                IrOpcode::Store32(offset)
            }
            0x37 => {
                let _align = reader.read_unsigned_leb128()?;
                let offset = reader.read_unsigned_leb128()? as u32;
                IrOpcode::Store64(offset)
            }
            0x3A => {
                let _align = reader.read_unsigned_leb128()?;
                let offset = reader.read_unsigned_leb128()? as u32;
                IrOpcode::Store8(offset)
            }
            0x3B => {
                let _align = reader.read_unsigned_leb128()?;
                let offset = reader.read_unsigned_leb128()? as u32;
                IrOpcode::Store16(offset)
            }
            
            // Memory size/grow
            0x3F => {
                let _mem_idx = reader.read_u8()?;
                IrOpcode::MemorySize
            }
            0x40 => {
                let _mem_idx = reader.read_u8()?;
                IrOpcode::MemoryGrow
            }
            
            // Constants
            0x41 => {
                let val = reader.read_signed_leb128()? as i32;
                IrOpcode::Const32(val)
            }
            0x42 => {
                let val = reader.read_signed_leb128()?;
                IrOpcode::Const64(val)
            }
            0x43 => {
                let bits = reader.read_u32()?;
                IrOpcode::ConstF32(bits)
            }
            0x44 => {
                let bits = reader.read_u64()?;
                IrOpcode::ConstF64(bits)
            }
            
            // i32 comparison
            0x45 => IrOpcode::I32Eqz,
            0x46 => IrOpcode::I32Eq,
            0x47 => IrOpcode::I32Ne,
            0x48 => IrOpcode::I32LtS,
            0x49 => IrOpcode::I32LtU,
            0x4A => IrOpcode::I32GtS,
            0x4B => IrOpcode::I32GtU,
            0x4C => IrOpcode::I32LeS,
            0x4D => IrOpcode::I32LeU,
            0x4E => IrOpcode::I32GeS,
            0x4F => IrOpcode::I32GeU,
            
            // i64 comparison
            0x50 => IrOpcode::I64Eqz,
            0x51 => IrOpcode::I64Eq,
            0x52 => IrOpcode::I64Ne,
            0x53 => IrOpcode::I64LtS,
            0x54 => IrOpcode::I64LtU,
            0x55 => IrOpcode::I64GtS,
            0x56 => IrOpcode::I64GtU,
            0x57 => IrOpcode::I64LeS,
            0x58 => IrOpcode::I64LeU,
            0x59 => IrOpcode::I64GeS,
            0x5A => IrOpcode::I64GeU,
            
            // f32 comparison
            0x5B => IrOpcode::F32Eq,
            0x5C => IrOpcode::F32Ne,
            0x5D => IrOpcode::F32Lt,
            0x5E => IrOpcode::F32Gt,
            0x5F => IrOpcode::F32Le,
            0x60 => IrOpcode::F32Ge,
            
            // f64 comparison
            0x61 => IrOpcode::F64Eq,
            0x62 => IrOpcode::F64Ne,
            0x63 => IrOpcode::F64Lt,
            0x64 => IrOpcode::F64Gt,
            0x65 => IrOpcode::F64Le,
            0x66 => IrOpcode::F64Ge,
            
            // i32 arithmetic
            0x67 => IrOpcode::I32Clz,
            0x68 => IrOpcode::I32Ctz,
            0x69 => IrOpcode::I32Popcnt,
            0x6A => IrOpcode::I32Add,
            0x6B => IrOpcode::I32Sub,
            0x6C => IrOpcode::I32Mul,
            0x6D => IrOpcode::I32DivS,
            0x6E => IrOpcode::I32DivU,
            0x6F => IrOpcode::I32RemS,
            0x70 => IrOpcode::I32RemU,
            0x71 => IrOpcode::I32And,
            0x72 => IrOpcode::I32Or,
            0x73 => IrOpcode::I32Xor,
            0x74 => IrOpcode::I32Shl,
            0x75 => IrOpcode::I32ShrS,
            0x76 => IrOpcode::I32ShrU,
            0x77 => IrOpcode::I32Rotl,
            0x78 => IrOpcode::I32Rotr,
            
            // i64 arithmetic
            0x79 => IrOpcode::I64Clz,
            0x7A => IrOpcode::I64Ctz,
            0x7B => IrOpcode::I64Popcnt,
            0x7C => IrOpcode::I64Add,
            0x7D => IrOpcode::I64Sub,
            0x7E => IrOpcode::I64Mul,
            0x7F => IrOpcode::I64DivS,
            0x80 => IrOpcode::I64DivU,
            0x81 => IrOpcode::I64RemS,
            0x82 => IrOpcode::I64RemU,
            0x83 => IrOpcode::I64And,
            0x84 => IrOpcode::I64Or,
            0x85 => IrOpcode::I64Xor,
            0x86 => IrOpcode::I64Shl,
            0x87 => IrOpcode::I64ShrS,
            0x88 => IrOpcode::I64ShrU,
            0x89 => IrOpcode::I64Rotl,
            0x8A => IrOpcode::I64Rotr,
            
            // f32 arithmetic
            0x8B => IrOpcode::F32Abs,
            0x8C => IrOpcode::F32Neg,
            0x8D => IrOpcode::F32Ceil,
            0x8E => IrOpcode::F32Floor,
            0x8F => IrOpcode::F32Trunc,
            0x90 => IrOpcode::F32Nearest,
            0x91 => IrOpcode::F32Sqrt,
            0x92 => IrOpcode::F32Add,
            0x93 => IrOpcode::F32Sub,
            0x94 => IrOpcode::F32Mul,
            0x95 => IrOpcode::F32Div,
            0x96 => IrOpcode::F32Min,
            0x97 => IrOpcode::F32Max,
            0x98 => IrOpcode::F32Copysign,
            
            // f64 arithmetic
            0x99 => IrOpcode::F64Abs,
            0x9A => IrOpcode::F64Neg,
            0x9B => IrOpcode::F64Ceil,
            0x9C => IrOpcode::F64Floor,
            0x9D => IrOpcode::F64Trunc,
            0x9E => IrOpcode::F64Nearest,
            0x9F => IrOpcode::F64Sqrt,
            0xA0 => IrOpcode::F64Add,
            0xA1 => IrOpcode::F64Sub,
            0xA2 => IrOpcode::F64Mul,
            0xA3 => IrOpcode::F64Div,
            0xA4 => IrOpcode::F64Min,
            0xA5 => IrOpcode::F64Max,
            0xA6 => IrOpcode::F64Copysign,
            
            // Conversions
            0xA7 => IrOpcode::I32WrapI64,
            0xA8 => IrOpcode::I32TruncF32S,
            0xA9 => IrOpcode::I32TruncF32U,
            0xAA => IrOpcode::I32TruncF64S,
            0xAB => IrOpcode::I32TruncF64U,
            0xAC => IrOpcode::I64ExtendI32S,
            0xAD => IrOpcode::I64ExtendI32U,
            0xAE => IrOpcode::I64TruncF32S,
            0xAF => IrOpcode::I64TruncF32U,
            0xB0 => IrOpcode::I64TruncF64S,
            0xB1 => IrOpcode::I64TruncF64U,
            0xB2 => IrOpcode::F32ConvertI32S,
            0xB3 => IrOpcode::F32ConvertI32U,
            0xB4 => IrOpcode::F32ConvertI64S,
            0xB5 => IrOpcode::F32ConvertI64U,
            0xB6 => IrOpcode::F32DemoteF64,
            0xB7 => IrOpcode::F64ConvertI32S,
            0xB8 => IrOpcode::F64ConvertI32U,
            0xB9 => IrOpcode::F64ConvertI64S,
            0xBA => IrOpcode::F64ConvertI64U,
            0xBB => IrOpcode::F64PromoteF32,
            0xBC => IrOpcode::I32ReinterpretF32,
            0xBD => IrOpcode::I64ReinterpretF64,
            0xBE => IrOpcode::F32ReinterpretI32,
            0xBF => IrOpcode::F64ReinterpretI64,
            
            // Sign extension (0xC0-0xC4)
            0xC0 => IrOpcode::I32Extend8S,
            0xC1 => IrOpcode::I32Extend16S,
            0xC2 => IrOpcode::I64Extend8S,
            0xC3 => IrOpcode::I64Extend16S,
            0xC4 => IrOpcode::I64Extend32S,
            
            _ => return Err(TranslationError::UnsupportedOpcode(opcode)),
        };
        
        Ok(IrInstruction::new(ir_opcode, offset))
    }
    
    fn new_block(&mut self, kind: BlockKind) -> BlockId {
        let id = BlockId(self.next_block_id);
        self.next_block_id += 1;
        self.block_stack.push(id);
        id
    }
}

impl Default for WasmToIr {
    fn default() -> Self {
        Self::new()
    }
}

/// Translation error.
#[derive(Debug, Clone)]
pub enum TranslationError {
    UnexpectedEnd,
    UnsupportedOpcode(u8),
    InvalidLeb128,
    InvalidBlockType,
}

/// Simple byte reader for WASM bytecode.
struct ByteReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    
    fn is_empty(&self) -> bool {
        self.pos >= self.data.len()
    }
    
    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }
    
    fn read_u8(&mut self) -> Result<u8, TranslationError> {
        if self.pos >= self.data.len() {
            return Err(TranslationError::UnexpectedEnd);
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }
    
    fn read_u32(&mut self) -> Result<u32, TranslationError> {
        let mut result = 0u32;
        for i in 0..4 {
            let b = self.read_u8()?;
            result |= (b as u32) << (i * 8);
        }
        Ok(result)
    }
    
    fn read_u64(&mut self) -> Result<u64, TranslationError> {
        let mut result = 0u64;
        for i in 0..8 {
            let b = self.read_u8()?;
            result |= (b as u64) << (i * 8);
        }
        Ok(result)
    }
    
    fn read_unsigned_leb128(&mut self) -> Result<u64, TranslationError> {
        let mut result = 0u64;
        let mut shift = 0;
        
        loop {
            let byte = self.read_u8()?;
            result |= ((byte & 0x7F) as u64) << shift;
            
            if byte & 0x80 == 0 {
                break;
            }
            
            shift += 7;
            if shift >= 64 {
                return Err(TranslationError::InvalidLeb128);
            }
        }
        
        Ok(result)
    }
    
    fn read_signed_leb128(&mut self) -> Result<i64, TranslationError> {
        let mut result = 0i64;
        let mut shift = 0;
        let mut byte;
        
        loop {
            byte = self.read_u8()?;
            result |= ((byte & 0x7F) as i64) << shift;
            shift += 7;
            
            if byte & 0x80 == 0 {
                break;
            }
            
            if shift >= 64 {
                return Err(TranslationError::InvalidLeb128);
            }
        }
        
        // Sign extend
        if shift < 64 && (byte & 0x40) != 0 {
            result |= !0i64 << shift;
        }
        
        Ok(result)
    }
}

// ─── IR Interpreter ────────────────────────────────────────────────

/// An interpreter that executes `IrFunction` instructions directly.
///
/// This is used to verify that WASM→IR translation preserves semantics,
/// by comparing IR interpreter results against the WASM interpreter.
pub struct IrInterpreter {
    /// Value stack.
    stack: Vec<i64>,
    /// Local variables (all stored as i64).
    locals: Vec<i64>,
    /// Block stack for control flow: (kind, start_pc, stack_depth).
    block_stack: Vec<IrBlock>,
    /// Optional linear memory for Load/Store.
    memory: Vec<u8>,
}

#[derive(Debug, Clone)]
struct IrBlock {
    kind: BlockKind,
    block_id: BlockId,
    start_pc: usize,
    stack_depth: usize,
}

/// Result of IR execution.
#[derive(Debug, Clone, PartialEq)]
pub enum IrExecResult {
    /// Execution completed with return values.
    Ok(Vec<i64>),
    /// Execution trapped.
    Trap(IrTrap),
}

/// IR execution trap.
#[derive(Debug, Clone, PartialEq)]
pub enum IrTrap {
    DivisionByZero,
    Unreachable,
    StackOverflow,
    StackUnderflow,
    InvalidLocal(u32),
    InvalidBranch,
    IntegerOverflow,
    MemoryBoundsViolation,
}

impl IrInterpreter {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            locals: Vec::new(),
            block_stack: Vec::new(),
            memory: Vec::new(),
        }
    }

    /// Create an interpreter with linear memory for Load/Store testing.
    pub fn with_memory(memory_size: usize) -> Self {
        Self {
            stack: Vec::new(),
            locals: Vec::new(),
            block_stack: Vec::new(),
            memory: vec![0u8; memory_size],
        }
    }

    /// Execute an IR function with the given arguments (as i64).
    pub fn execute(&mut self, func: &IrFunction, args: &[i64]) -> IrExecResult {
        self.stack.clear();
        self.block_stack.clear();

        // Initialize locals: params from args + declared locals as 0
        let total_locals = func.params.len() + func.locals.len();
        self.locals = vec![0i64; total_locals];
        for (i, &arg) in args.iter().enumerate() {
            if i < self.locals.len() {
                self.locals[i] = arg;
            }
        }

        let mut pc = 0usize;
        let body_len = func.body.len();

        while pc < body_len {
            let inst = &func.body[pc];
            pc += 1;

            match inst.opcode {
                // ── Constants ──
                IrOpcode::Const32(v) => self.stack.push(v as i64),
                IrOpcode::Const64(v) => self.stack.push(v),
                IrOpcode::ConstF32(bits) => self.stack.push(bits as i64),
                IrOpcode::ConstF64(bits) => self.stack.push(bits as i64),

                // ── Locals ──
                IrOpcode::LocalGet(idx) => {
                    let val = match self.locals.get(idx as usize) {
                        Some(&v) => v,
                        None => return IrExecResult::Trap(IrTrap::InvalidLocal(idx)),
                    };
                    self.stack.push(val);
                }
                IrOpcode::LocalSet(idx) => {
                    let val = match self.pop() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    if (idx as usize) >= self.locals.len() {
                        return IrExecResult::Trap(IrTrap::InvalidLocal(idx));
                    }
                    self.locals[idx as usize] = val;
                }
                IrOpcode::LocalTee(idx) => {
                    let val = match self.peek() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    if (idx as usize) >= self.locals.len() {
                        return IrExecResult::Trap(IrTrap::InvalidLocal(idx));
                    }
                    self.locals[idx as usize] = val;
                }

                // ── i32 Arithmetic ──
                IrOpcode::I32Add => { self.binop_i32(|a, b| Ok(a.wrapping_add(b))); }
                IrOpcode::I32Sub => { self.binop_i32(|a, b| Ok(a.wrapping_sub(b))); }
                IrOpcode::I32Mul => { self.binop_i32(|a, b| Ok(a.wrapping_mul(b))); }
                IrOpcode::I32DivS => {
                    let (a, b) = match self.pop2_i32() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    if b == 0 { return IrExecResult::Trap(IrTrap::DivisionByZero); }
                    if a == i32::MIN && b == -1 { return IrExecResult::Trap(IrTrap::IntegerOverflow); }
                    self.stack.push(a.wrapping_div(b) as i64);
                }
                IrOpcode::I32DivU => {
                    let (a, b) = match self.pop2_i32() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    if b == 0 { return IrExecResult::Trap(IrTrap::DivisionByZero); }
                    self.stack.push(((a as u32) / (b as u32)) as i64);
                }
                IrOpcode::I32RemS => {
                    let (a, b) = match self.pop2_i32() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    if b == 0 { return IrExecResult::Trap(IrTrap::DivisionByZero); }
                    self.stack.push(a.wrapping_rem(b) as i64);
                }
                IrOpcode::I32RemU => {
                    let (a, b) = match self.pop2_i32() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    if b == 0 { return IrExecResult::Trap(IrTrap::DivisionByZero); }
                    self.stack.push(((a as u32) % (b as u32)) as i64);
                }
                IrOpcode::I32And => { self.binop_i32(|a, b| Ok(a & b)); }
                IrOpcode::I32Or  => { self.binop_i32(|a, b| Ok(a | b)); }
                IrOpcode::I32Xor => { self.binop_i32(|a, b| Ok(a ^ b)); }
                IrOpcode::I32Shl => { self.binop_i32(|a, b| Ok(a.wrapping_shl(b as u32))); }
                IrOpcode::I32ShrS => { self.binop_i32(|a, b| Ok(a.wrapping_shr(b as u32))); }
                IrOpcode::I32ShrU => { self.binop_i32(|a, b| Ok(((a as u32).wrapping_shr(b as u32)) as i32)); }
                IrOpcode::I32Rotl => { self.binop_i32(|a, b| Ok((a as u32).rotate_left(b as u32) as i32)); }
                IrOpcode::I32Rotr => { self.binop_i32(|a, b| Ok((a as u32).rotate_right(b as u32) as i32)); }
                IrOpcode::I32Clz => { self.unop_i32(|a| (a as u32).leading_zeros() as i32); }
                IrOpcode::I32Ctz => { self.unop_i32(|a| (a as u32).trailing_zeros() as i32); }
                IrOpcode::I32Popcnt => { self.unop_i32(|a| (a as u32).count_ones() as i32); }
                IrOpcode::I32Eqz => { self.unop_i32(|a| if a == 0 { 1 } else { 0 }); }

                // ── i32 Comparisons ──
                IrOpcode::I32Eq  => { self.cmp_i32(|a, b| a == b); }
                IrOpcode::I32Ne  => { self.cmp_i32(|a, b| a != b); }
                IrOpcode::I32LtS => { self.cmp_i32(|a, b| a < b); }
                IrOpcode::I32GtS => { self.cmp_i32(|a, b| a > b); }
                IrOpcode::I32LeS => { self.cmp_i32(|a, b| a <= b); }
                IrOpcode::I32GeS => { self.cmp_i32(|a, b| a >= b); }
                IrOpcode::I32LtU => { self.cmp_i32(|a, b| (a as u32) < (b as u32)); }
                IrOpcode::I32GtU => { self.cmp_i32(|a, b| (a as u32) > (b as u32)); }
                IrOpcode::I32LeU => { self.cmp_i32(|a, b| (a as u32) <= (b as u32)); }
                IrOpcode::I32GeU => { self.cmp_i32(|a, b| (a as u32) >= (b as u32)); }

                // ── i64 Arithmetic ──
                IrOpcode::I64Add => { self.binop_i64(|a, b| a.wrapping_add(b)); }
                IrOpcode::I64Sub => { self.binop_i64(|a, b| a.wrapping_sub(b)); }
                IrOpcode::I64Mul => { self.binop_i64(|a, b| a.wrapping_mul(b)); }
                IrOpcode::I64DivS => {
                    let (a, b) = match self.pop2() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    if b == 0 { return IrExecResult::Trap(IrTrap::DivisionByZero); }
                    if a == i64::MIN && b == -1 { return IrExecResult::Trap(IrTrap::IntegerOverflow); }
                    self.stack.push(a.wrapping_div(b));
                }
                IrOpcode::I64DivU => {
                    let (a, b) = match self.pop2() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    if b == 0 { return IrExecResult::Trap(IrTrap::DivisionByZero); }
                    self.stack.push(((a as u64) / (b as u64)) as i64);
                }
                IrOpcode::I64RemS => {
                    let (a, b) = match self.pop2() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    if b == 0 { return IrExecResult::Trap(IrTrap::DivisionByZero); }
                    self.stack.push(a.wrapping_rem(b));
                }
                IrOpcode::I64RemU => {
                    let (a, b) = match self.pop2() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    if b == 0 { return IrExecResult::Trap(IrTrap::DivisionByZero); }
                    self.stack.push(((a as u64) % (b as u64)) as i64);
                }
                IrOpcode::I64And => { self.binop_i64(|a, b| a & b); }
                IrOpcode::I64Or  => { self.binop_i64(|a, b| a | b); }
                IrOpcode::I64Xor => { self.binop_i64(|a, b| a ^ b); }
                IrOpcode::I64Shl => { self.binop_i64(|a, b| a.wrapping_shl(b as u32)); }
                IrOpcode::I64ShrS => { self.binop_i64(|a, b| a.wrapping_shr(b as u32)); }
                IrOpcode::I64ShrU => { self.binop_i64(|a, b| ((a as u64).wrapping_shr(b as u32)) as i64); }
                IrOpcode::I64Rotl => { self.binop_i64(|a, b| (a as u64).rotate_left(b as u32) as i64); }
                IrOpcode::I64Rotr => { self.binop_i64(|a, b| (a as u64).rotate_right(b as u32) as i64); }
                IrOpcode::I64Clz => { self.unop_i64(|a| (a as u64).leading_zeros() as i64); }
                IrOpcode::I64Ctz => { self.unop_i64(|a| (a as u64).trailing_zeros() as i64); }
                IrOpcode::I64Popcnt => { self.unop_i64(|a| (a as u64).count_ones() as i64); }
                IrOpcode::I64Eqz => { self.unop_i64(|a| if a == 0 { 1 } else { 0 }); }

                // ── i64 Comparisons ──
                IrOpcode::I64Eq  => { self.cmp_i64(|a, b| a == b); }
                IrOpcode::I64Ne  => { self.cmp_i64(|a, b| a != b); }
                IrOpcode::I64LtS => { self.cmp_i64(|a, b| a < b); }
                IrOpcode::I64GtS => { self.cmp_i64(|a, b| a > b); }
                IrOpcode::I64LeS => { self.cmp_i64(|a, b| a <= b); }
                IrOpcode::I64GeS => { self.cmp_i64(|a, b| a >= b); }
                IrOpcode::I64LtU => { self.cmp_i64(|a, b| (a as u64) < (b as u64)); }
                IrOpcode::I64GtU => { self.cmp_i64(|a, b| (a as u64) > (b as u64)); }
                IrOpcode::I64LeU => { self.cmp_i64(|a, b| (a as u64) <= (b as u64)); }
                IrOpcode::I64GeU => { self.cmp_i64(|a, b| (a as u64) >= (b as u64)); }

                // ── Conversions ──
                IrOpcode::I32WrapI64 => { self.unop_i64(|a| (a as i32) as i64); }
                IrOpcode::I64ExtendI32S => { self.unop_i64(|a| (a as i32) as i64); }
                IrOpcode::I64ExtendI32U => { self.unop_i64(|a| (a as u32) as i64); }
                IrOpcode::I32Extend8S => { self.unop_i32(|a| (a as i8) as i32); }
                IrOpcode::I32Extend16S => { self.unop_i32(|a| (a as i16) as i32); }
                IrOpcode::I64Extend8S => { self.unop_i64(|a| (a as i8) as i64); }
                IrOpcode::I64Extend16S => { self.unop_i64(|a| (a as i16) as i64); }
                IrOpcode::I64Extend32S => { self.unop_i64(|a| (a as i32) as i64); }

                // ── Control Flow ──
                IrOpcode::Block(block_id) => {
                    self.block_stack.push(IrBlock {
                        kind: BlockKind::Block,
                        block_id,
                        start_pc: pc,
                        stack_depth: self.stack.len(),
                    });
                }
                IrOpcode::Loop(block_id) => {
                    self.block_stack.push(IrBlock {
                        kind: BlockKind::Loop,
                        block_id,
                        start_pc: pc, // loop target = start
                        stack_depth: self.stack.len(),
                    });
                }
                IrOpcode::If(block_id) => {
                    let cond = match self.pop() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    self.block_stack.push(IrBlock {
                        kind: BlockKind::If,
                        block_id,
                        start_pc: pc,
                        stack_depth: self.stack.len(),
                    });
                    if cond == 0 {
                        // Skip to matching Else or End
                        let mut depth = 1u32;
                        while pc < body_len && depth > 0 {
                            match func.body[pc].opcode {
                                IrOpcode::Block(_) | IrOpcode::Loop(_) | IrOpcode::If(_) => depth += 1,
                                IrOpcode::End => depth -= 1,
                                IrOpcode::Else if depth == 1 => { pc += 1; break; }
                                _ => {}
                            }
                            if depth > 0 { pc += 1; }
                        }
                    }
                }
                IrOpcode::Else => {
                    // Skip to matching End (true branch is done)
                    let mut depth = 1u32;
                    while pc < body_len && depth > 0 {
                        match func.body[pc].opcode {
                            IrOpcode::Block(_) | IrOpcode::Loop(_) | IrOpcode::If(_) => depth += 1,
                            IrOpcode::End => depth -= 1,
                            _ => {}
                        }
                        if depth > 0 { pc += 1; }
                    }
                    self.block_stack.pop();
                }
                IrOpcode::End => {
                    self.block_stack.pop();
                }
                IrOpcode::Br(depth) => {
                    match self.do_branch(depth, &func.body, &mut pc) {
                        Ok(()) => {}
                        Err(t) => return IrExecResult::Trap(t),
                    }
                }
                IrOpcode::BrIf(depth) => {
                    let cond = match self.pop() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    if cond != 0 {
                        match self.do_branch(depth, &func.body, &mut pc) {
                            Ok(()) => {}
                            Err(t) => return IrExecResult::Trap(t),
                        }
                    }
                }
                IrOpcode::Return => {
                    break;
                }
                IrOpcode::Unreachable => {
                    return IrExecResult::Trap(IrTrap::Unreachable);
                }

                // ── Stack Ops ──
                IrOpcode::Drop => { let _ = self.pop(); }
                IrOpcode::Select => {
                    let c = match self.pop() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    let b = match self.pop() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    let a = match self.pop() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    self.stack.push(if c != 0 { a } else { b });
                }

                // ── Call (simplified: not supported in IR interpreter standalone) ──
                IrOpcode::Call(_) | IrOpcode::CallIndirect(_) => {
                    // Calls require module context; skip for basic tests
                }

                // ── Memory operations with bounds checking ──
                IrOpcode::Load32(offset) => {
                    let addr = match self.pop() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    let effective = (addr as u32).wrapping_add(offset) as usize;
                    if effective + 4 > self.memory.len() {
                        return IrExecResult::Trap(IrTrap::MemoryBoundsViolation);
                    }
                    let val = i32::from_le_bytes([
                        self.memory[effective],
                        self.memory[effective + 1],
                        self.memory[effective + 2],
                        self.memory[effective + 3],
                    ]);
                    self.stack.push(val as i64);
                }
                IrOpcode::Load64(offset) => {
                    let addr = match self.pop() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    let effective = (addr as u32).wrapping_add(offset) as usize;
                    if effective + 8 > self.memory.len() {
                        return IrExecResult::Trap(IrTrap::MemoryBoundsViolation);
                    }
                    let val = i64::from_le_bytes([
                        self.memory[effective], self.memory[effective + 1],
                        self.memory[effective + 2], self.memory[effective + 3],
                        self.memory[effective + 4], self.memory[effective + 5],
                        self.memory[effective + 6], self.memory[effective + 7],
                    ]);
                    self.stack.push(val);
                }
                IrOpcode::Store32(offset) => {
                    let val = match self.pop() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    let addr = match self.pop() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    let effective = (addr as u32).wrapping_add(offset) as usize;
                    if effective + 4 > self.memory.len() {
                        return IrExecResult::Trap(IrTrap::MemoryBoundsViolation);
                    }
                    let bytes = (val as i32).to_le_bytes();
                    self.memory[effective..effective + 4].copy_from_slice(&bytes);
                }
                IrOpcode::Store64(offset) => {
                    let val = match self.pop() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    let addr = match self.pop() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    let effective = (addr as u32).wrapping_add(offset) as usize;
                    if effective + 8 > self.memory.len() {
                        return IrExecResult::Trap(IrTrap::MemoryBoundsViolation);
                    }
                    let bytes = val.to_le_bytes();
                    self.memory[effective..effective + 8].copy_from_slice(&bytes);
                }
                IrOpcode::MemorySize => {
                    let pages = (self.memory.len() / 65536) as i32;
                    self.stack.push(pages as i64);
                }
                IrOpcode::MemoryGrow => {
                    let delta = match self.pop() { Ok(v) => v, Err(t) => return IrExecResult::Trap(t) };
                    let old_pages = (self.memory.len() / 65536) as i32;
                    let new_size = self.memory.len() + (delta as usize) * 65536;
                    if new_size > 256 * 65536 { // max 256 pages (16 MB)
                        self.stack.push(-1i64);
                    } else {
                        self.memory.resize(new_size, 0);
                        self.stack.push(old_pages as i64);
                    }
                }

                // ── Everything else: no-op ──
                _ => {}
            }
        }

        // Collect results
        let result_count = func.results.len();
        let mut results = Vec::new();
        let available = self.stack.len().min(result_count);
        for _ in 0..available {
            results.push(self.stack.pop().unwrap_or(0));
        }
        results.reverse();
        IrExecResult::Ok(results)
    }

    fn pop(&mut self) -> Result<i64, IrTrap> {
        self.stack.pop().ok_or(IrTrap::StackUnderflow)
    }

    fn peek(&self) -> Result<i64, IrTrap> {
        self.stack.last().copied().ok_or(IrTrap::StackUnderflow)
    }

    fn pop2(&mut self) -> Result<(i64, i64), IrTrap> {
        let b = self.pop()?;
        let a = self.pop()?;
        Ok((a, b))
    }

    fn pop2_i32(&mut self) -> Result<(i32, i32), IrTrap> {
        let (a, b) = self.pop2()?;
        Ok((a as i32, b as i32))
    }

    fn binop_i32<F: Fn(i32, i32) -> Result<i32, IrTrap>>(&mut self, f: F) {
        if let Ok((a, b)) = self.pop2_i32() {
            if let Ok(r) = f(a, b) {
                self.stack.push(r as i64);
            }
        }
    }

    fn binop_i64<F: Fn(i64, i64) -> i64>(&mut self, f: F) {
        if let Ok((a, b)) = self.pop2() {
            self.stack.push(f(a, b));
        }
    }

    fn unop_i32<F: Fn(i32) -> i32>(&mut self, f: F) {
        if let Ok(a) = self.pop() {
            self.stack.push(f(a as i32) as i64);
        }
    }

    fn unop_i64<F: Fn(i64) -> i64>(&mut self, f: F) {
        if let Ok(a) = self.pop() {
            self.stack.push(f(a));
        }
    }

    fn cmp_i32<F: Fn(i32, i32) -> bool>(&mut self, f: F) {
        if let Ok((a, b)) = self.pop2_i32() {
            self.stack.push(if f(a, b) { 1 } else { 0 });
        }
    }

    fn cmp_i64<F: Fn(i64, i64) -> bool>(&mut self, f: F) {
        if let Ok((a, b)) = self.pop2() {
            self.stack.push(if f(a, b) { 1 } else { 0 });
        }
    }

    /// Branch by depth: pop blocks and jump.
    fn do_branch(&mut self, depth: u32, body: &[IrInstruction], pc: &mut usize) -> Result<(), IrTrap> {
        if depth as usize >= self.block_stack.len() {
            return Err(IrTrap::InvalidBranch);
        }

        let target_idx = self.block_stack.len() - 1 - depth as usize;
        let target_block = self.block_stack[target_idx].clone();

        // Pop blocks above the target
        while self.block_stack.len() > target_idx + 1 {
            self.block_stack.pop();
        }

        if target_block.kind == BlockKind::Loop {
            // Loop: branch to start_pc (re-enter loop)
            *pc = target_block.start_pc;
        } else {
            // Block/If: branch to end (skip to matching End)
            self.block_stack.pop(); // pop the target block too
            let mut depth_counter = 1u32;
            while *pc < body.len() && depth_counter > 0 {
                match body[*pc].opcode {
                    IrOpcode::Block(_) | IrOpcode::Loop(_) | IrOpcode::If(_) => depth_counter += 1,
                    IrOpcode::End => depth_counter -= 1,
                    _ => {}
                }
                if depth_counter > 0 { *pc += 1; }
            }
            if *pc < body.len() { *pc += 1; } // skip the End
        }

        Ok(())
    }
}

impl Default for IrInterpreter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_ir_const_and_add() {
        let mut func = IrFunction::new(0, vec![], vec![IrType::I32]);
        func.body = vec![
            IrInstruction::new(IrOpcode::Const32(10), 0),
            IrInstruction::new(IrOpcode::Const32(20), 0),
            IrInstruction::new(IrOpcode::I32Add, 0),
        ];
        let mut interp = IrInterpreter::new();
        let result = interp.execute(&func, &[]);
        assert_eq!(result, IrExecResult::Ok(vec![30]));
    }

    #[test]
    fn test_ir_locals() {
        let mut func = IrFunction::new(0, vec![IrType::I32], vec![IrType::I32]);
        func.body = vec![
            IrInstruction::new(IrOpcode::LocalGet(0), 0),
            IrInstruction::new(IrOpcode::Const32(5), 0),
            IrInstruction::new(IrOpcode::I32Add, 0),
        ];
        let mut interp = IrInterpreter::new();
        let result = interp.execute(&func, &[7]);
        assert_eq!(result, IrExecResult::Ok(vec![12]));
    }

    #[test]
    fn test_ir_div_by_zero() {
        let mut func = IrFunction::new(0, vec![], vec![IrType::I32]);
        func.body = vec![
            IrInstruction::new(IrOpcode::Const32(10), 0),
            IrInstruction::new(IrOpcode::Const32(0), 0),
            IrInstruction::new(IrOpcode::I32DivS, 0),
        ];
        let mut interp = IrInterpreter::new();
        let result = interp.execute(&func, &[]);
        assert_eq!(result, IrExecResult::Trap(IrTrap::DivisionByZero));
    }

    #[test]
    fn test_ir_memory_bounds() {
        let mut func = IrFunction::new(0, vec![], vec![IrType::I32]);
        func.body = vec![
            IrInstruction::new(IrOpcode::Const32(0), 0),
            IrInstruction::new(IrOpcode::Load32(0), 0),
        ];
        // No memory — should trap
        let mut interp = IrInterpreter::new();
        let result = interp.execute(&func, &[]);
        assert_eq!(result, IrExecResult::Trap(IrTrap::MemoryBoundsViolation));

        // With memory — should succeed
        let mut interp = IrInterpreter::with_memory(64);
        let result = interp.execute(&func, &[]);
        assert_eq!(result, IrExecResult::Ok(vec![0]));
    }
}
