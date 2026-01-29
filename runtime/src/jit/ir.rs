//! WASM to Native IR Translation
//!
//! This module provides an intermediate representation for translating
//! WebAssembly bytecode to native machine code.

use alloc::collections::BTreeMap;
use alloc::string::String;
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
