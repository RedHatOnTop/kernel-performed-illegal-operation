//! WASM instruction (opcode) definitions.
//!
//! Complete set of WebAssembly MVP instructions plus commonly-used post-MVP
//! extensions (sign extension, bulk memory, reference types, saturating truncation).
//!
//! Reference: <https://webassembly.github.io/spec/core/binary/instructions.html>

use alloc::vec::Vec;

use crate::module::ValueType;
use crate::parser::BlockType;

/// A single WASM instruction.
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    // ========================================================================
    // Control Flow Instructions
    // ========================================================================
    /// Trap immediately.
    Unreachable,
    /// No operation.
    Nop,
    /// Begin a block. Params: block type.
    Block(BlockType),
    /// Begin a loop. Params: block type.
    Loop(BlockType),
    /// Conditional block. Params: block type.
    If(BlockType),
    /// Else branch of an if block.
    Else,
    /// End of block/loop/if/function.
    End,
    /// Branch to label. Params: label index (depth).
    Br(u32),
    /// Conditional branch. Params: label index.
    BrIf(u32),
    /// Branch table. Params: (targets, default).
    BrTable(Vec<u32>, u32),
    /// Return from current function.
    Return,
    /// Call function by index.
    Call(u32),
    /// Indirect call via table. Params: (type_index, table_index).
    CallIndirect(u32, u32),

    // ========================================================================
    // Reference Instructions
    // ========================================================================
    /// Push null reference.
    RefNull,
    /// Test if reference is null.
    RefIsNull,
    /// Create reference to function. Params: function index.
    RefFunc(u32),

    // ========================================================================
    // Parametric Instructions
    // ========================================================================
    /// Drop top of stack.
    Drop,
    /// Select between two values based on condition.
    Select,

    // ========================================================================
    // Variable Instructions
    // ========================================================================
    /// Get local variable. Params: local index.
    LocalGet(u32),
    /// Set local variable. Params: local index.
    LocalSet(u32),
    /// Tee local variable (set + keep on stack). Params: local index.
    LocalTee(u32),
    /// Get global variable. Params: global index.
    GlobalGet(u32),
    /// Set global variable. Params: global index.
    GlobalSet(u32),

    // ========================================================================
    // Table Instructions
    // ========================================================================
    /// Get table element. Params: table index.
    TableGet(u32),
    /// Set table element. Params: table index.
    TableSet(u32),
    /// Initialize table from element segment. Params: (elem_idx, table_idx).
    TableInit(u32, u32),
    /// Drop element segment. Params: elem index.
    ElemDrop(u32),
    /// Copy table elements. Params: (dst_table, src_table).
    TableCopy(u32, u32),
    /// Grow table. Params: table index.
    TableGrow(u32),
    /// Get table size. Params: table index.
    TableSize(u32),
    /// Fill table. Params: table index.
    TableFill(u32),

    // ========================================================================
    // Memory Instructions — Load
    // ========================================================================
    /// Load i32 from memory. Params: (align, offset).
    I32Load(u32, u32),
    /// Load i64 from memory. Params: (align, offset).
    I64Load(u32, u32),
    /// Load f32 from memory. Params: (align, offset).
    F32Load(u32, u32),
    /// Load f64 from memory. Params: (align, offset).
    F64Load(u32, u32),
    /// Load i32 from i8 (sign-extend). Params: (align, offset).
    I32Load8S(u32, u32),
    /// Load i32 from u8 (zero-extend). Params: (align, offset).
    I32Load8U(u32, u32),
    /// Load i32 from i16 (sign-extend). Params: (align, offset).
    I32Load16S(u32, u32),
    /// Load i32 from u16 (zero-extend). Params: (align, offset).
    I32Load16U(u32, u32),
    /// Load i64 from i8 (sign-extend). Params: (align, offset).
    I64Load8S(u32, u32),
    /// Load i64 from u8 (zero-extend). Params: (align, offset).
    I64Load8U(u32, u32),
    /// Load i64 from i16 (sign-extend). Params: (align, offset).
    I64Load16S(u32, u32),
    /// Load i64 from u16 (zero-extend). Params: (align, offset).
    I64Load16U(u32, u32),
    /// Load i64 from i32 (sign-extend). Params: (align, offset).
    I64Load32S(u32, u32),
    /// Load i64 from u32 (zero-extend). Params: (align, offset).
    I64Load32U(u32, u32),

    // ========================================================================
    // Memory Instructions — Store
    // ========================================================================
    /// Store i32 to memory. Params: (align, offset).
    I32Store(u32, u32),
    /// Store i64 to memory. Params: (align, offset).
    I64Store(u32, u32),
    /// Store f32 to memory. Params: (align, offset).
    F32Store(u32, u32),
    /// Store f64 to memory. Params: (align, offset).
    F64Store(u32, u32),
    /// Store low 8 bits of i32. Params: (align, offset).
    I32Store8(u32, u32),
    /// Store low 16 bits of i32. Params: (align, offset).
    I32Store16(u32, u32),
    /// Store low 8 bits of i64. Params: (align, offset).
    I64Store8(u32, u32),
    /// Store low 16 bits of i64. Params: (align, offset).
    I64Store16(u32, u32),
    /// Store low 32 bits of i64. Params: (align, offset).
    I64Store32(u32, u32),

    // ========================================================================
    // Memory Instructions — Size/Grow
    // ========================================================================
    /// Get current memory size in pages.
    MemorySize,
    /// Grow memory by N pages.
    MemoryGrow,
    /// Initialize memory from data segment. Params: data index.
    MemoryInit(u32),
    /// Drop data segment. Params: data index.
    DataDrop(u32),
    /// Copy memory regions.
    MemoryCopy,
    /// Fill memory with byte value.
    MemoryFill,

    // ========================================================================
    // Numeric Instructions — Constants
    // ========================================================================
    /// Push i32 constant.
    I32Const(i32),
    /// Push i64 constant.
    I64Const(i64),
    /// Push f32 constant.
    F32Const(f32),
    /// Push f64 constant.
    F64Const(f64),

    // ========================================================================
    // Numeric Instructions — i32 Comparison
    // ========================================================================
    /// i32 equal to zero.
    I32Eqz,
    /// i32 equal.
    I32Eq,
    /// i32 not equal.
    I32Ne,
    /// i32 less than (signed).
    I32LtS,
    /// i32 less than (unsigned).
    I32LtU,
    /// i32 greater than (signed).
    I32GtS,
    /// i32 greater than (unsigned).
    I32GtU,
    /// i32 less than or equal (signed).
    I32LeS,
    /// i32 less than or equal (unsigned).
    I32LeU,
    /// i32 greater than or equal (signed).
    I32GeS,
    /// i32 greater than or equal (unsigned).
    I32GeU,

    // ========================================================================
    // Numeric Instructions — i64 Comparison
    // ========================================================================
    /// i64 equal to zero.
    I64Eqz,
    /// i64 equal.
    I64Eq,
    /// i64 not equal.
    I64Ne,
    /// i64 less than (signed).
    I64LtS,
    /// i64 less than (unsigned).
    I64LtU,
    /// i64 greater than (signed).
    I64GtS,
    /// i64 greater than (unsigned).
    I64GtU,
    /// i64 less than or equal (signed).
    I64LeS,
    /// i64 less than or equal (unsigned).
    I64LeU,
    /// i64 greater than or equal (signed).
    I64GeS,
    /// i64 greater than or equal (unsigned).
    I64GeU,

    // ========================================================================
    // Numeric Instructions — f32 Comparison
    // ========================================================================
    F32Eq,
    F32Ne,
    F32Lt,
    F32Gt,
    F32Le,
    F32Ge,

    // ========================================================================
    // Numeric Instructions — f64 Comparison
    // ========================================================================
    F64Eq,
    F64Ne,
    F64Lt,
    F64Gt,
    F64Le,
    F64Ge,

    // ========================================================================
    // Numeric Instructions — i32 Arithmetic
    // ========================================================================
    /// Count leading zeros.
    I32Clz,
    /// Count trailing zeros.
    I32Ctz,
    /// Population count (number of 1 bits).
    I32Popcnt,
    /// Add.
    I32Add,
    /// Subtract.
    I32Sub,
    /// Multiply.
    I32Mul,
    /// Divide (signed).
    I32DivS,
    /// Divide (unsigned).
    I32DivU,
    /// Remainder (signed).
    I32RemS,
    /// Remainder (unsigned).
    I32RemU,
    /// Bitwise AND.
    I32And,
    /// Bitwise OR.
    I32Or,
    /// Bitwise XOR.
    I32Xor,
    /// Shift left.
    I32Shl,
    /// Shift right (signed).
    I32ShrS,
    /// Shift right (unsigned).
    I32ShrU,
    /// Rotate left.
    I32Rotl,
    /// Rotate right.
    I32Rotr,

    // ========================================================================
    // Numeric Instructions — i64 Arithmetic
    // ========================================================================
    I64Clz,
    I64Ctz,
    I64Popcnt,
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

    // ========================================================================
    // Numeric Instructions — f32 Arithmetic
    // ========================================================================
    F32Abs,
    F32Neg,
    F32Ceil,
    F32Floor,
    F32Trunc,
    F32Nearest,
    F32Sqrt,
    F32Add,
    F32Sub,
    F32Mul,
    F32Div,
    F32Min,
    F32Max,
    F32Copysign,

    // ========================================================================
    // Numeric Instructions — f64 Arithmetic
    // ========================================================================
    F64Abs,
    F64Neg,
    F64Ceil,
    F64Floor,
    F64Trunc,
    F64Nearest,
    F64Sqrt,
    F64Add,
    F64Sub,
    F64Mul,
    F64Div,
    F64Min,
    F64Max,
    F64Copysign,

    // ========================================================================
    // Numeric Instructions — Conversions
    // ========================================================================
    /// Wrap i64 to i32.
    I32WrapI64,
    /// Truncate f32 to signed i32.
    I32TruncF32S,
    /// Truncate f32 to unsigned i32.
    I32TruncF32U,
    /// Truncate f64 to signed i32.
    I32TruncF64S,
    /// Truncate f64 to unsigned i32.
    I32TruncF64U,
    /// Sign-extend i32 to i64.
    I64ExtendI32S,
    /// Zero-extend i32 to i64.
    I64ExtendI32U,
    /// Truncate f32 to signed i64.
    I64TruncF32S,
    /// Truncate f32 to unsigned i64.
    I64TruncF32U,
    /// Truncate f64 to signed i64.
    I64TruncF64S,
    /// Truncate f64 to unsigned i64.
    I64TruncF64U,
    /// Convert signed i32 to f32.
    F32ConvertI32S,
    /// Convert unsigned i32 to f32.
    F32ConvertI32U,
    /// Convert signed i64 to f32.
    F32ConvertI64S,
    /// Convert unsigned i64 to f32.
    F32ConvertI64U,
    /// Demote f64 to f32.
    F32DemoteF64,
    /// Convert signed i32 to f64.
    F64ConvertI32S,
    /// Convert unsigned i32 to f64.
    F64ConvertI32U,
    /// Convert signed i64 to f64.
    F64ConvertI64S,
    /// Convert unsigned i64 to f64.
    F64ConvertI64U,
    /// Promote f32 to f64.
    F64PromoteF32,

    // ========================================================================
    // Numeric Instructions — Reinterpretations
    // ========================================================================
    /// Reinterpret f32 bits as i32.
    I32ReinterpretF32,
    /// Reinterpret f64 bits as i64.
    I64ReinterpretF64,
    /// Reinterpret i32 bits as f32.
    F32ReinterpretI32,
    /// Reinterpret i64 bits as f64.
    F64ReinterpretI64,

    // ========================================================================
    // Sign Extension Instructions (post-MVP)
    // ========================================================================
    /// Sign-extend 8-bit value to i32.
    I32Extend8S,
    /// Sign-extend 16-bit value to i32.
    I32Extend16S,
    /// Sign-extend 8-bit value to i64.
    I64Extend8S,
    /// Sign-extend 16-bit value to i64.
    I64Extend16S,
    /// Sign-extend 32-bit value to i64.
    I64Extend32S,

    // ========================================================================
    // Saturating Truncation Instructions (0xFC prefix)
    // ========================================================================
    /// Saturating truncate f32 to signed i32.
    I32TruncSatF32S,
    /// Saturating truncate f32 to unsigned i32.
    I32TruncSatF32U,
    /// Saturating truncate f64 to signed i32.
    I32TruncSatF64S,
    /// Saturating truncate f64 to unsigned i32.
    I32TruncSatF64U,
    /// Saturating truncate f32 to signed i64.
    I64TruncSatF32S,
    /// Saturating truncate f32 to unsigned i64.
    I64TruncSatF32U,
    /// Saturating truncate f64 to signed i64.
    I64TruncSatF64S,
    /// Saturating truncate f64 to unsigned i64.
    I64TruncSatF64U,
}

impl Instruction {
    /// Returns the number of values this instruction pops from the stack.
    /// Returns None for instructions with variable stack usage.
    pub fn stack_pop_count(&self) -> Option<usize> {
        use Instruction::*;
        match self {
            Nop | Unreachable | End | Else | Return => Some(0),
            Block(_) | Loop(_) => Some(0),
            Br(_) => Some(0),
            BrIf(_) => Some(1),
            I32Const(_) | I64Const(_) | F32Const(_) | F64Const(_) => Some(0),
            LocalGet(_) | GlobalGet(_) => Some(0),
            LocalSet(_) | GlobalSet(_) => Some(1),
            LocalTee(_) => Some(1),
            Drop => Some(1),
            Select => Some(3),
            I32Eqz | I64Eqz => Some(1),
            I32Clz | I32Ctz | I32Popcnt => Some(1),
            I64Clz | I64Ctz | I64Popcnt => Some(1),
            I32Add | I32Sub | I32Mul | I32DivS | I32DivU | I32RemS | I32RemU => Some(2),
            I32And | I32Or | I32Xor | I32Shl | I32ShrS | I32ShrU | I32Rotl | I32Rotr => Some(2),
            I64Add | I64Sub | I64Mul | I64DivS | I64DivU | I64RemS | I64RemU => Some(2),
            I64And | I64Or | I64Xor | I64Shl | I64ShrS | I64ShrU | I64Rotl | I64Rotr => Some(2),
            I32Eq | I32Ne | I32LtS | I32LtU | I32GtS | I32GtU |
            I32LeS | I32LeU | I32GeS | I32GeU => Some(2),
            I64Eq | I64Ne | I64LtS | I64LtU | I64GtS | I64GtU |
            I64LeS | I64LeU | I64GeS | I64GeU => Some(2),
            F32Add | F32Sub | F32Mul | F32Div | F32Min | F32Max | F32Copysign => Some(2),
            F64Add | F64Sub | F64Mul | F64Div | F64Min | F64Max | F64Copysign => Some(2),
            F32Eq | F32Ne | F32Lt | F32Gt | F32Le | F32Ge => Some(2),
            F64Eq | F64Ne | F64Lt | F64Gt | F64Le | F64Ge => Some(2),
            F32Abs | F32Neg | F32Ceil | F32Floor | F32Trunc | F32Nearest | F32Sqrt => Some(1),
            F64Abs | F64Neg | F64Ceil | F64Floor | F64Trunc | F64Nearest | F64Sqrt => Some(1),
            // Load: pop address
            I32Load(_, _) | I64Load(_, _) | F32Load(_, _) | F64Load(_, _) => Some(1),
            I32Load8S(_, _) | I32Load8U(_, _) | I32Load16S(_, _) | I32Load16U(_, _) => Some(1),
            I64Load8S(_, _) | I64Load8U(_, _) | I64Load16S(_, _) | I64Load16U(_, _) => Some(1),
            I64Load32S(_, _) | I64Load32U(_, _) => Some(1),
            // Store: pop address + value
            I32Store(_, _) | I64Store(_, _) | F32Store(_, _) | F64Store(_, _) => Some(2),
            I32Store8(_, _) | I32Store16(_, _) => Some(2),
            I64Store8(_, _) | I64Store16(_, _) | I64Store32(_, _) => Some(2),
            MemorySize => Some(0),
            MemoryGrow => Some(1),
            // Conversions: pop 1
            I32WrapI64 | I32TruncF32S | I32TruncF32U | I32TruncF64S | I32TruncF64U => Some(1),
            I64ExtendI32S | I64ExtendI32U => Some(1),
            I64TruncF32S | I64TruncF32U | I64TruncF64S | I64TruncF64U => Some(1),
            F32ConvertI32S | F32ConvertI32U | F32ConvertI64S | F32ConvertI64U => Some(1),
            F32DemoteF64 => Some(1),
            F64ConvertI32S | F64ConvertI32U | F64ConvertI64S | F64ConvertI64U => Some(1),
            F64PromoteF32 => Some(1),
            I32ReinterpretF32 | I64ReinterpretF64 | F32ReinterpretI32 | F64ReinterpretI64 => Some(1),
            I32Extend8S | I32Extend16S | I64Extend8S | I64Extend16S | I64Extend32S => Some(1),
            I32TruncSatF32S | I32TruncSatF32U | I32TruncSatF64S | I32TruncSatF64U => Some(1),
            I64TruncSatF32S | I64TruncSatF32U | I64TruncSatF64S | I64TruncSatF64U => Some(1),
            RefNull | RefFunc(_) => Some(0),
            RefIsNull => Some(1),
            If(_) => Some(1),
            Call(_) => None, // depends on signature
            CallIndirect(_, _) => None,
            BrTable(_, _) => Some(1),
            TableGet(_) => Some(1),
            TableSet(_) => Some(2),
            TableInit(_, _) => Some(3),
            ElemDrop(_) => Some(0),
            TableCopy(_, _) => Some(3),
            TableGrow(_) => Some(2),
            TableSize(_) => Some(0),
            TableFill(_) => Some(3),
            MemoryInit(_) => Some(3),
            DataDrop(_) => Some(0),
            MemoryCopy => Some(3),
            MemoryFill => Some(3),
        }
    }

    /// Returns the number of values this instruction pushes onto the stack.
    /// Returns None for instructions with variable results.
    pub fn stack_push_count(&self) -> Option<usize> {
        use Instruction::*;
        match self {
            Nop | Unreachable | End | Else | Return => Some(0),
            Drop => Some(0),
            LocalSet(_) | GlobalSet(_) => Some(0),
            // All loads push 1
            I32Load(_, _) | I64Load(_, _) | F32Load(_, _) | F64Load(_, _) => Some(1),
            I32Load8S(_, _) | I32Load8U(_, _) | I32Load16S(_, _) | I32Load16U(_, _) => Some(1),
            I64Load8S(_, _) | I64Load8U(_, _) | I64Load16S(_, _) | I64Load16U(_, _) => Some(1),
            I64Load32S(_, _) | I64Load32U(_, _) => Some(1),
            // All stores push 0
            I32Store(_, _) | I64Store(_, _) | F32Store(_, _) | F64Store(_, _) => Some(0),
            I32Store8(_, _) | I32Store16(_, _) => Some(0),
            I64Store8(_, _) | I64Store16(_, _) | I64Store32(_, _) => Some(0),
            // Constants push 1
            I32Const(_) | I64Const(_) | F32Const(_) | F64Const(_) => Some(1),
            LocalGet(_) | GlobalGet(_) => Some(1),
            LocalTee(_) => Some(1),
            Select => Some(1),
            MemorySize => Some(1),
            MemoryGrow => Some(1),
            // All comparisons/arithmetic push 1
            I32Eqz | I32Eq | I32Ne | I32LtS | I32LtU | I32GtS | I32GtU |
            I32LeS | I32LeU | I32GeS | I32GeU => Some(1),
            I64Eqz | I64Eq | I64Ne | I64LtS | I64LtU | I64GtS | I64GtU |
            I64LeS | I64LeU | I64GeS | I64GeU => Some(1),
            F32Eq | F32Ne | F32Lt | F32Gt | F32Le | F32Ge => Some(1),
            F64Eq | F64Ne | F64Lt | F64Gt | F64Le | F64Ge => Some(1),
            I32Clz | I32Ctz | I32Popcnt | I32Add | I32Sub | I32Mul |
            I32DivS | I32DivU | I32RemS | I32RemU |
            I32And | I32Or | I32Xor | I32Shl | I32ShrS | I32ShrU | I32Rotl | I32Rotr => Some(1),
            I64Clz | I64Ctz | I64Popcnt | I64Add | I64Sub | I64Mul |
            I64DivS | I64DivU | I64RemS | I64RemU |
            I64And | I64Or | I64Xor | I64Shl | I64ShrS | I64ShrU | I64Rotl | I64Rotr => Some(1),
            F32Abs | F32Neg | F32Ceil | F32Floor | F32Trunc | F32Nearest | F32Sqrt |
            F32Add | F32Sub | F32Mul | F32Div | F32Min | F32Max | F32Copysign => Some(1),
            F64Abs | F64Neg | F64Ceil | F64Floor | F64Trunc | F64Nearest | F64Sqrt |
            F64Add | F64Sub | F64Mul | F64Div | F64Min | F64Max | F64Copysign => Some(1),
            // Conversions push 1
            I32WrapI64 | I32TruncF32S | I32TruncF32U | I32TruncF64S | I32TruncF64U |
            I64ExtendI32S | I64ExtendI32U |
            I64TruncF32S | I64TruncF32U | I64TruncF64S | I64TruncF64U |
            F32ConvertI32S | F32ConvertI32U | F32ConvertI64S | F32ConvertI64U | F32DemoteF64 |
            F64ConvertI32S | F64ConvertI32U | F64ConvertI64S | F64ConvertI64U | F64PromoteF32 |
            I32ReinterpretF32 | I64ReinterpretF64 | F32ReinterpretI32 | F64ReinterpretI64 => Some(1),
            I32Extend8S | I32Extend16S | I64Extend8S | I64Extend16S | I64Extend32S => Some(1),
            I32TruncSatF32S | I32TruncSatF32U | I32TruncSatF64S | I32TruncSatF64U |
            I64TruncSatF32S | I64TruncSatF32U | I64TruncSatF64S | I64TruncSatF64U => Some(1),
            RefNull | RefFunc(_) => Some(1),
            RefIsNull => Some(1),
            TableGet(_) => Some(1),
            TableSet(_) => Some(0),
            TableSize(_) => Some(1),
            TableGrow(_) => Some(1),
            // Variable results
            Block(_) | Loop(_) | If(_) | Br(_) | BrIf(_) | BrTable(_, _) |
            Call(_) | CallIndirect(_, _) => None,
            TableInit(_, _) | ElemDrop(_) | TableCopy(_, _) | TableFill(_) => Some(0),
            MemoryInit(_) | DataDrop(_) | MemoryCopy | MemoryFill => Some(0),
        }
    }

    /// Returns a human-readable name for this instruction.
    pub fn name(&self) -> &'static str {
        use Instruction::*;
        match self {
            Unreachable => "unreachable",
            Nop => "nop",
            Block(_) => "block",
            Loop(_) => "loop",
            If(_) => "if",
            Else => "else",
            End => "end",
            Br(_) => "br",
            BrIf(_) => "br_if",
            BrTable(_, _) => "br_table",
            Return => "return",
            Call(_) => "call",
            CallIndirect(_, _) => "call_indirect",
            RefNull => "ref.null",
            RefIsNull => "ref.is_null",
            RefFunc(_) => "ref.func",
            Drop => "drop",
            Select => "select",
            LocalGet(_) => "local.get",
            LocalSet(_) => "local.set",
            LocalTee(_) => "local.tee",
            GlobalGet(_) => "global.get",
            GlobalSet(_) => "global.set",
            TableGet(_) => "table.get",
            TableSet(_) => "table.set",
            TableInit(_, _) => "table.init",
            ElemDrop(_) => "elem.drop",
            TableCopy(_, _) => "table.copy",
            TableGrow(_) => "table.grow",
            TableSize(_) => "table.size",
            TableFill(_) => "table.fill",
            I32Load(_, _) => "i32.load",
            I64Load(_, _) => "i64.load",
            F32Load(_, _) => "f32.load",
            F64Load(_, _) => "f64.load",
            I32Load8S(_, _) => "i32.load8_s",
            I32Load8U(_, _) => "i32.load8_u",
            I32Load16S(_, _) => "i32.load16_s",
            I32Load16U(_, _) => "i32.load16_u",
            I64Load8S(_, _) => "i64.load8_s",
            I64Load8U(_, _) => "i64.load8_u",
            I64Load16S(_, _) => "i64.load16_s",
            I64Load16U(_, _) => "i64.load16_u",
            I64Load32S(_, _) => "i64.load32_s",
            I64Load32U(_, _) => "i64.load32_u",
            I32Store(_, _) => "i32.store",
            I64Store(_, _) => "i64.store",
            F32Store(_, _) => "f32.store",
            F64Store(_, _) => "f64.store",
            I32Store8(_, _) => "i32.store8",
            I32Store16(_, _) => "i32.store16",
            I64Store8(_, _) => "i64.store8",
            I64Store16(_, _) => "i64.store16",
            I64Store32(_, _) => "i64.store32",
            MemorySize => "memory.size",
            MemoryGrow => "memory.grow",
            MemoryInit(_) => "memory.init",
            DataDrop(_) => "data.drop",
            MemoryCopy => "memory.copy",
            MemoryFill => "memory.fill",
            I32Const(_) => "i32.const",
            I64Const(_) => "i64.const",
            F32Const(_) => "f32.const",
            F64Const(_) => "f64.const",
            I32Eqz => "i32.eqz",
            I32Eq => "i32.eq",
            I32Ne => "i32.ne",
            I32LtS => "i32.lt_s",
            I32LtU => "i32.lt_u",
            I32GtS => "i32.gt_s",
            I32GtU => "i32.gt_u",
            I32LeS => "i32.le_s",
            I32LeU => "i32.le_u",
            I32GeS => "i32.ge_s",
            I32GeU => "i32.ge_u",
            I64Eqz => "i64.eqz",
            I64Eq => "i64.eq",
            I64Ne => "i64.ne",
            I64LtS => "i64.lt_s",
            I64LtU => "i64.lt_u",
            I64GtS => "i64.gt_s",
            I64GtU => "i64.gt_u",
            I64LeS => "i64.le_s",
            I64LeU => "i64.le_u",
            I64GeS => "i64.ge_s",
            I64GeU => "i64.ge_u",
            F32Eq => "f32.eq",
            F32Ne => "f32.ne",
            F32Lt => "f32.lt",
            F32Gt => "f32.gt",
            F32Le => "f32.le",
            F32Ge => "f32.ge",
            F64Eq => "f64.eq",
            F64Ne => "f64.ne",
            F64Lt => "f64.lt",
            F64Gt => "f64.gt",
            F64Le => "f64.le",
            F64Ge => "f64.ge",
            I32Clz => "i32.clz",
            I32Ctz => "i32.ctz",
            I32Popcnt => "i32.popcnt",
            I32Add => "i32.add",
            I32Sub => "i32.sub",
            I32Mul => "i32.mul",
            I32DivS => "i32.div_s",
            I32DivU => "i32.div_u",
            I32RemS => "i32.rem_s",
            I32RemU => "i32.rem_u",
            I32And => "i32.and",
            I32Or => "i32.or",
            I32Xor => "i32.xor",
            I32Shl => "i32.shl",
            I32ShrS => "i32.shr_s",
            I32ShrU => "i32.shr_u",
            I32Rotl => "i32.rotl",
            I32Rotr => "i32.rotr",
            I64Clz => "i64.clz",
            I64Ctz => "i64.ctz",
            I64Popcnt => "i64.popcnt",
            I64Add => "i64.add",
            I64Sub => "i64.sub",
            I64Mul => "i64.mul",
            I64DivS => "i64.div_s",
            I64DivU => "i64.div_u",
            I64RemS => "i64.rem_s",
            I64RemU => "i64.rem_u",
            I64And => "i64.and",
            I64Or => "i64.or",
            I64Xor => "i64.xor",
            I64Shl => "i64.shl",
            I64ShrS => "i64.shr_s",
            I64ShrU => "i64.shr_u",
            I64Rotl => "i64.rotl",
            I64Rotr => "i64.rotr",
            F32Abs => "f32.abs",
            F32Neg => "f32.neg",
            F32Ceil => "f32.ceil",
            F32Floor => "f32.floor",
            F32Trunc => "f32.trunc",
            F32Nearest => "f32.nearest",
            F32Sqrt => "f32.sqrt",
            F32Add => "f32.add",
            F32Sub => "f32.sub",
            F32Mul => "f32.mul",
            F32Div => "f32.div",
            F32Min => "f32.min",
            F32Max => "f32.max",
            F32Copysign => "f32.copysign",
            F64Abs => "f64.abs",
            F64Neg => "f64.neg",
            F64Ceil => "f64.ceil",
            F64Floor => "f64.floor",
            F64Trunc => "f64.trunc",
            F64Nearest => "f64.nearest",
            F64Sqrt => "f64.sqrt",
            F64Add => "f64.add",
            F64Sub => "f64.sub",
            F64Mul => "f64.mul",
            F64Div => "f64.div",
            F64Min => "f64.min",
            F64Max => "f64.max",
            F64Copysign => "f64.copysign",
            I32WrapI64 => "i32.wrap_i64",
            I32TruncF32S => "i32.trunc_f32_s",
            I32TruncF32U => "i32.trunc_f32_u",
            I32TruncF64S => "i32.trunc_f64_s",
            I32TruncF64U => "i32.trunc_f64_u",
            I64ExtendI32S => "i64.extend_i32_s",
            I64ExtendI32U => "i64.extend_i32_u",
            I64TruncF32S => "i64.trunc_f32_s",
            I64TruncF32U => "i64.trunc_f32_u",
            I64TruncF64S => "i64.trunc_f64_s",
            I64TruncF64U => "i64.trunc_f64_u",
            F32ConvertI32S => "f32.convert_i32_s",
            F32ConvertI32U => "f32.convert_i32_u",
            F32ConvertI64S => "f32.convert_i64_s",
            F32ConvertI64U => "f32.convert_i64_u",
            F32DemoteF64 => "f32.demote_f64",
            F64ConvertI32S => "f64.convert_i32_s",
            F64ConvertI32U => "f64.convert_i32_u",
            F64ConvertI64S => "f64.convert_i64_s",
            F64ConvertI64U => "f64.convert_i64_u",
            F64PromoteF32 => "f64.promote_f32",
            I32ReinterpretF32 => "i32.reinterpret_f32",
            I64ReinterpretF64 => "i64.reinterpret_f64",
            F32ReinterpretI32 => "f32.reinterpret_i32",
            F64ReinterpretI64 => "f64.reinterpret_i64",
            I32Extend8S => "i32.extend8_s",
            I32Extend16S => "i32.extend16_s",
            I64Extend8S => "i64.extend8_s",
            I64Extend16S => "i64.extend16_s",
            I64Extend32S => "i64.extend32_s",
            I32TruncSatF32S => "i32.trunc_sat_f32_s",
            I32TruncSatF32U => "i32.trunc_sat_f32_u",
            I32TruncSatF64S => "i32.trunc_sat_f64_s",
            I32TruncSatF64U => "i32.trunc_sat_f64_u",
            I64TruncSatF32S => "i64.trunc_sat_f32_s",
            I64TruncSatF32U => "i64.trunc_sat_f32_u",
            I64TruncSatF64S => "i64.trunc_sat_f64_s",
            I64TruncSatF64U => "i64.trunc_sat_f64_u",
        }
    }
}
