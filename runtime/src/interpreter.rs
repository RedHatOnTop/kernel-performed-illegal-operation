//! WASM stack machine interpreter.
//!
//! Implements the core stack-based virtual machine for executing WASM bytecode.
//! This serves as both the cold-tier execution path and the reference
//! implementation for correctness verification of the JIT compiler.

use alloc::string::String;
use alloc::vec::Vec;

use crate::module::ValueType;
use crate::parser::BlockType;

/// Maximum value stack depth (in values).
pub const MAX_VALUE_STACK_DEPTH: usize = 16384;

/// Maximum call stack depth (frames).
pub const MAX_CALL_STACK_DEPTH: usize = 1024;

// ============================================================================
// WASM Values
// ============================================================================

/// A runtime WASM value.
#[derive(Debug, Clone, Copy)]
pub enum WasmValue {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    FuncRef(Option<u32>),
    ExternRef(Option<u32>),
}

impl WasmValue {
    /// Get the value type.
    pub fn value_type(&self) -> ValueType {
        match self {
            WasmValue::I32(_) => ValueType::I32,
            WasmValue::I64(_) => ValueType::I64,
            WasmValue::F32(_) => ValueType::F32,
            WasmValue::F64(_) => ValueType::F64,
            WasmValue::FuncRef(_) => ValueType::FuncRef,
            WasmValue::ExternRef(_) => ValueType::ExternRef,
        }
    }

    /// Get as i32.
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            WasmValue::I32(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as i64.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            WasmValue::I64(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as f32.
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            WasmValue::F32(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as f64.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            WasmValue::F64(v) => Some(*v),
            _ => None,
        }
    }

    /// Default value for a given type.
    pub fn default_for(vt: ValueType) -> Self {
        match vt {
            ValueType::I32 => WasmValue::I32(0),
            ValueType::I64 => WasmValue::I64(0),
            ValueType::F32 => WasmValue::F32(0.0),
            ValueType::F64 => WasmValue::F64(0.0),
            ValueType::FuncRef => WasmValue::FuncRef(None),
            ValueType::ExternRef => WasmValue::ExternRef(None),
            ValueType::V128 => WasmValue::I64(0), // fallback
        }
    }
}

impl PartialEq for WasmValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (WasmValue::I32(a), WasmValue::I32(b)) => a == b,
            (WasmValue::I64(a), WasmValue::I64(b)) => a == b,
            (WasmValue::F32(a), WasmValue::F32(b)) => a.to_bits() == b.to_bits(),
            (WasmValue::F64(a), WasmValue::F64(b)) => a.to_bits() == b.to_bits(),
            (WasmValue::FuncRef(a), WasmValue::FuncRef(b)) => a == b,
            (WasmValue::ExternRef(a), WasmValue::ExternRef(b)) => a == b,
            _ => false,
        }
    }
}

// ============================================================================
// Value Stack
// ============================================================================

/// The operand stack for the interpreter.
pub struct ValueStack {
    values: Vec<WasmValue>,
    max_depth: usize,
}

impl ValueStack {
    /// Create a new value stack.
    pub fn new() -> Self {
        ValueStack {
            values: Vec::with_capacity(256),
            max_depth: MAX_VALUE_STACK_DEPTH,
        }
    }

    /// Push a value onto the stack.
    pub fn push(&mut self, value: WasmValue) -> Result<(), TrapError> {
        if self.values.len() >= self.max_depth {
            return Err(TrapError::StackOverflow);
        }
        self.values.push(value);
        Ok(())
    }

    /// Pop a value from the stack.
    pub fn pop(&mut self) -> Result<WasmValue, TrapError> {
        self.values.pop().ok_or(TrapError::StackUnderflow)
    }

    /// Pop an i32 from the stack.
    pub fn pop_i32(&mut self) -> Result<i32, TrapError> {
        match self.pop()? {
            WasmValue::I32(v) => Ok(v),
            other => Err(TrapError::TypeMismatch {
                expected: "i32",
                got: other.value_type(),
            }),
        }
    }

    /// Pop an i64 from the stack.
    pub fn pop_i64(&mut self) -> Result<i64, TrapError> {
        match self.pop()? {
            WasmValue::I64(v) => Ok(v),
            other => Err(TrapError::TypeMismatch {
                expected: "i64",
                got: other.value_type(),
            }),
        }
    }

    /// Pop an f32 from the stack.
    pub fn pop_f32(&mut self) -> Result<f32, TrapError> {
        match self.pop()? {
            WasmValue::F32(v) => Ok(v),
            other => Err(TrapError::TypeMismatch {
                expected: "f32",
                got: other.value_type(),
            }),
        }
    }

    /// Pop an f64 from the stack.
    pub fn pop_f64(&mut self) -> Result<f64, TrapError> {
        match self.pop()? {
            WasmValue::F64(v) => Ok(v),
            other => Err(TrapError::TypeMismatch {
                expected: "f64",
                got: other.value_type(),
            }),
        }
    }

    /// Peek at the top value.
    pub fn peek(&self) -> Result<&WasmValue, TrapError> {
        self.values.last().ok_or(TrapError::StackUnderflow)
    }

    /// Get current stack depth.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check if stack is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Truncate stack to given length.
    pub fn truncate(&mut self, len: usize) {
        self.values.truncate(len);
    }

    /// Get values from a given position.
    pub fn split_off(&mut self, at: usize) -> Vec<WasmValue> {
        self.values.split_off(at)
    }

    /// Extend with values.
    pub fn extend(&mut self, values: impl IntoIterator<Item = WasmValue>) {
        self.values.extend(values);
    }
}

// ============================================================================
// Call Frame
// ============================================================================

/// A function call frame on the call stack.
#[derive(Debug, Clone)]
pub struct CallFrame {
    /// Function index (in the module).
    pub func_idx: u32,
    /// Local variables for this frame.
    pub locals: Vec<WasmValue>,
    /// Instruction pointer (index into function body instructions).
    pub pc: usize,
    /// The stack depth when this frame was entered (for cleanup on return).
    pub stack_base: usize,
    /// Number of return values expected.
    pub return_arity: usize,
    /// Block stack for structured control flow.
    pub block_stack: Vec<BlockFrame>,
    /// Whether this is a host function call.
    pub is_host: bool,
}

/// A structured control flow block.
#[derive(Debug, Clone)]
pub struct BlockFrame {
    /// Block kind.
    pub kind: BlockKind,
    /// Block type (return arity).
    pub block_type: BlockType,
    /// Stack depth when block was entered.
    pub stack_depth: usize,
    /// Instruction index where the block starts.
    pub start_pc: usize,
    /// Instruction index of the end of the block.
    pub end_pc: usize,
    /// For `if` blocks: the instruction index of the `else` branch.
    pub else_pc: Option<usize>,
    /// Number of values the block produces.
    pub arity: usize,
    /// Number of parameters the block consumes.
    pub param_arity: usize,
}

/// Block kinds for structured control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// A `block` instruction.
    Block,
    /// A `loop` instruction.
    Loop,
    /// An `if` instruction.
    If,
    /// An `else` branch.
    Else,
    /// Function body (implicit outermost block).
    Function,
}

// ============================================================================
// Trap (Runtime Error)
// ============================================================================

/// Runtime trap errors during WASM execution.
#[derive(Debug, Clone)]
pub enum TrapError {
    /// Division by zero.
    DivisionByZero,
    /// Integer overflow in division.
    IntegerOverflow,
    /// Invalid conversion to integer.
    InvalidConversionToInteger,
    /// Memory access out of bounds.
    MemoryOutOfBounds {
        offset: usize,
        size: usize,
        memory_size: usize,
    },
    /// Value stack overflow.
    StackOverflow,
    /// Value stack underflow.
    StackUnderflow,
    /// Call stack overflow (too deep recursion).
    CallStackOverflow,
    /// Unreachable instruction executed.
    Unreachable,
    /// Type mismatch.
    TypeMismatch {
        expected: &'static str,
        got: ValueType,
    },
    /// Undefined element in table.
    UndefinedElement {
        index: u32,
    },
    /// Indirect call type mismatch.
    IndirectCallTypeMismatch {
        expected_type: u32,
        actual_type: u32,
    },
    /// Uninitialized table element.
    UninitializedElement {
        index: u32,
    },
    /// Export not found.
    ExportNotFound(String),
    /// Function not found.
    FunctionNotFound(u32),
    /// Fuel exhausted.
    FuelExhausted,
    /// Process exit with code.
    ProcessExit(i32),
    /// Host function error.
    HostError(String),
    /// Generic execution error.
    ExecutionError(String),
}

impl core::fmt::Display for TrapError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TrapError::DivisionByZero => write!(f, "integer divide by zero"),
            TrapError::IntegerOverflow => write!(f, "integer overflow"),
            TrapError::InvalidConversionToInteger => write!(f, "invalid conversion to integer"),
            TrapError::MemoryOutOfBounds { offset, size, memory_size } => {
                write!(f, "out of bounds memory access: {} + {} > {}", offset, size, memory_size)
            }
            TrapError::StackOverflow => write!(f, "call stack exhausted"),
            TrapError::StackUnderflow => write!(f, "stack underflow"),
            TrapError::CallStackOverflow => write!(f, "call stack overflow"),
            TrapError::Unreachable => write!(f, "unreachable"),
            TrapError::TypeMismatch { expected, got } => {
                write!(f, "type mismatch: expected {}, got {:?}", expected, got)
            }
            TrapError::UndefinedElement { index } => {
                write!(f, "undefined element: table[{}]", index)
            }
            TrapError::IndirectCallTypeMismatch { expected_type, actual_type } => {
                write!(f, "indirect call type mismatch: expected type {}, got {}", expected_type, actual_type)
            }
            TrapError::UninitializedElement { index } => {
                write!(f, "uninitialized element: {}", index)
            }
            TrapError::ExportNotFound(name) => write!(f, "export not found: {}", name),
            TrapError::FunctionNotFound(idx) => write!(f, "function not found: {}", idx),
            TrapError::FuelExhausted => write!(f, "fuel exhausted"),
            TrapError::ProcessExit(code) => write!(f, "process exit with code {}", code),
            TrapError::HostError(msg) => write!(f, "host error: {}", msg),
            TrapError::ExecutionError(msg) => write!(f, "execution error: {}", msg),
        }
    }
}

/// Table for indirect function calls.
#[derive(Debug, Clone)]
pub struct Table {
    /// Table elements (function indices, None = uninitialized).
    pub elements: Vec<Option<u32>>,
    /// Maximum size.
    pub max: Option<u32>,
}

impl Table {
    /// Create a new table.
    pub fn new(min: u32, max: Option<u32>) -> Self {
        let mut elements = Vec::with_capacity(min as usize);
        elements.resize(min as usize, None);
        Table { elements, max }
    }

    /// Get a table element.
    pub fn get(&self, index: u32) -> Result<Option<u32>, TrapError> {
        self.elements
            .get(index as usize)
            .copied()
            .ok_or(TrapError::UndefinedElement { index })
    }

    /// Set a table element.
    pub fn set(&mut self, index: u32, value: Option<u32>) -> Result<(), TrapError> {
        if index as usize >= self.elements.len() {
            return Err(TrapError::UndefinedElement { index });
        }
        self.elements[index as usize] = value;
        Ok(())
    }

    /// Grow the table by delta elements.
    pub fn grow(&mut self, delta: u32, init: Option<u32>) -> Result<u32, TrapError> {
        let old_size = self.elements.len() as u32;
        let new_size = old_size.checked_add(delta).ok_or(TrapError::ExecutionError(
            String::from("table grow overflow"),
        ))?;

        if let Some(max) = self.max {
            if new_size > max {
                return Err(TrapError::ExecutionError(String::from(
                    "table grow exceeds max",
                )));
            }
        }

        self.elements.resize(new_size as usize, init);
        Ok(old_size)
    }

    /// Current size.
    pub fn size(&self) -> u32 {
        self.elements.len() as u32
    }
}

/// Global variable value (mutable or immutable).
#[derive(Debug, Clone)]
pub struct GlobalValue {
    pub value: WasmValue,
    pub mutable: bool,
}
