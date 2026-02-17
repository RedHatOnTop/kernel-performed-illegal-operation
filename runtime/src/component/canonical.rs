//! Canonical ABI — lowering (host→WASM) and lifting (WASM→host)
//!
//! Implements the Component Model canonical ABI for converting between
//! high-level `ComponentValue` types and core WASM values (i32/i64/f32/f64)
//! with optional linear memory for strings and lists.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use super::{ComponentError, ComponentType, ComponentValue};

/// Core WASM value — the low-level representation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoreValue {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
}

/// Linear memory interface for string/list lowering and lifting.
pub trait LinearMemory {
    /// Read bytes from memory at the given offset.
    fn read(&self, offset: u32, len: u32) -> Result<&[u8], ComponentError>;
    /// Write bytes to memory at the given offset.
    fn write(&mut self, offset: u32, data: &[u8]) -> Result<(), ComponentError>;
    /// Allocate `size` bytes with the given `align` and return the offset.
    fn alloc(&mut self, size: u32, align: u32) -> Result<u32, ComponentError>;
    /// Current memory size in bytes.
    fn size(&self) -> u32;
}

/// Simple vec-backed linear memory for testing.
pub struct VecMemory {
    data: Vec<u8>,
    next_alloc: u32,
}

impl VecMemory {
    /// Create a new memory with the given capacity.
    pub fn new(capacity: u32) -> Self {
        let mut data = Vec::new();
        data.resize(capacity as usize, 0);
        Self {
            data,
            next_alloc: 0,
        }
    }
}

impl LinearMemory for VecMemory {
    fn read(&self, offset: u32, len: u32) -> Result<&[u8], ComponentError> {
        let start = offset as usize;
        let end = start + len as usize;
        if end > self.data.len() {
            return Err(ComponentError::MemoryOutOfBounds);
        }
        Ok(&self.data[start..end])
    }

    fn write(&mut self, offset: u32, data: &[u8]) -> Result<(), ComponentError> {
        let start = offset as usize;
        let end = start + data.len();
        if end > self.data.len() {
            return Err(ComponentError::MemoryOutOfBounds);
        }
        self.data[start..end].copy_from_slice(data);
        Ok(())
    }

    fn alloc(&mut self, size: u32, align: u32) -> Result<u32, ComponentError> {
        // Align up
        let aligned = (self.next_alloc + align - 1) & !(align - 1);
        let end = aligned + size;
        if end > self.data.len() as u32 {
            return Err(ComponentError::MemoryOutOfBounds);
        }
        self.next_alloc = end;
        Ok(aligned)
    }

    fn size(&self) -> u32 {
        self.data.len() as u32
    }
}

// ── Lowering (ComponentValue → CoreValue[]) ──────────────────────────

/// Lower a `ComponentValue` to core WASM values.
///
/// For scalar types, this produces a single `CoreValue`.
/// For strings/lists, the data is written to linear memory and the
/// lowered result is (pointer, length) as two i32 values.
pub fn lower(
    value: &ComponentValue,
    memory: Option<&mut dyn LinearMemory>,
) -> Result<Vec<CoreValue>, ComponentError> {
    match value {
        ComponentValue::Bool(b) => Ok(alloc::vec![CoreValue::I32(if *b { 1 } else { 0 })]),
        ComponentValue::U8(v) => Ok(alloc::vec![CoreValue::I32(*v as i32)]),
        ComponentValue::U16(v) => Ok(alloc::vec![CoreValue::I32(*v as i32)]),
        ComponentValue::U32(v) => Ok(alloc::vec![CoreValue::I32(*v as i32)]),
        ComponentValue::U64(v) => Ok(alloc::vec![CoreValue::I64(*v as i64)]),
        ComponentValue::S8(v) => Ok(alloc::vec![CoreValue::I32(*v as i32)]),
        ComponentValue::S16(v) => Ok(alloc::vec![CoreValue::I32(*v as i32)]),
        ComponentValue::S32(v) => Ok(alloc::vec![CoreValue::I32(*v)]),
        ComponentValue::S64(v) => Ok(alloc::vec![CoreValue::I64(*v)]),
        ComponentValue::F32(v) => Ok(alloc::vec![CoreValue::F32(*v)]),
        ComponentValue::F64(v) => Ok(alloc::vec![CoreValue::F64(*v)]),
        ComponentValue::Char(c) => Ok(alloc::vec![CoreValue::I32(*c as i32)]),
        ComponentValue::String(s) => {
            let mem = memory.ok_or_else(|| {
                ComponentError::TypeMismatch(String::from("string lowering requires memory"))
            })?;
            let bytes = s.as_bytes();
            let ptr = mem.alloc(bytes.len() as u32, 1)?;
            mem.write(ptr, bytes)?;
            Ok(alloc::vec![
                CoreValue::I32(ptr as i32),
                CoreValue::I32(bytes.len() as i32),
            ])
        }
        ComponentValue::List(items) => {
            let mem = memory.ok_or_else(|| {
                ComponentError::TypeMismatch(String::from("list lowering requires memory"))
            })?;
            // Flatten items: lower each element and store in memory.
            // For simplicity in MVP, each element occupies 8 bytes (padded).
            let elem_size = 8u32;
            let total_size = (items.len() as u32) * elem_size;
            let base = mem.alloc(total_size, 4)?;
            for (i, item) in items.iter().enumerate() {
                let lowered = lower_scalar(item)?;
                let offset = base + (i as u32) * elem_size;
                match lowered {
                    CoreValue::I32(v) => {
                        mem.write(offset, &v.to_le_bytes())?;
                    }
                    CoreValue::I64(v) => {
                        mem.write(offset, &v.to_le_bytes())?;
                    }
                    CoreValue::F32(v) => {
                        mem.write(offset, &v.to_le_bytes())?;
                    }
                    CoreValue::F64(v) => {
                        mem.write(offset, &v.to_le_bytes())?;
                    }
                }
            }
            Ok(alloc::vec![
                CoreValue::I32(base as i32),
                CoreValue::I32(items.len() as i32),
            ])
        }
        ComponentValue::Record(fields) => {
            let mut result = Vec::new();
            // Each field is lowered inline (flattened).
            for (_name, val) in fields {
                let lowered = lower_scalar(val)?;
                result.push(lowered);
            }
            Ok(result)
        }
        ComponentValue::Variant {
            discriminant, value, ..
        } => {
            let mut result = alloc::vec![CoreValue::I32(*discriminant as i32)];
            if let Some(payload) = value {
                let lowered = lower_scalar(payload)?;
                result.push(lowered);
            }
            Ok(result)
        }
        ComponentValue::Enum { discriminant, .. } => {
            Ok(alloc::vec![CoreValue::I32(*discriminant as i32)])
        }
        ComponentValue::Flags(bits) => Ok(alloc::vec![CoreValue::I32(*bits as i32)]),
        ComponentValue::Option(opt) => match opt {
            None => Ok(alloc::vec![CoreValue::I32(0)]),
            Some(val) => {
                let mut result = alloc::vec![CoreValue::I32(1)];
                let lowered = lower_scalar(val)?;
                result.push(lowered);
                Ok(result)
            }
        },
        ComponentValue::Result(res) => match res {
            Ok(ok_val) => {
                let mut result = alloc::vec![CoreValue::I32(0)]; // discriminant 0 = Ok
                if let Some(val) = ok_val {
                    let lowered = lower_scalar(val)?;
                    result.push(lowered);
                }
                Ok(result)
            }
            Err(err_val) => {
                let mut result = alloc::vec![CoreValue::I32(1)]; // discriminant 1 = Err
                if let Some(val) = err_val {
                    let lowered = lower_scalar(val)?;
                    result.push(lowered);
                }
                Ok(result)
            }
        },
    }
}

/// Lower a scalar component value to a single core value.
fn lower_scalar(value: &ComponentValue) -> Result<CoreValue, ComponentError> {
    match value {
        ComponentValue::Bool(b) => Ok(CoreValue::I32(if *b { 1 } else { 0 })),
        ComponentValue::U8(v) => Ok(CoreValue::I32(*v as i32)),
        ComponentValue::U16(v) => Ok(CoreValue::I32(*v as i32)),
        ComponentValue::U32(v) => Ok(CoreValue::I32(*v as i32)),
        ComponentValue::U64(v) => Ok(CoreValue::I64(*v as i64)),
        ComponentValue::S8(v) => Ok(CoreValue::I32(*v as i32)),
        ComponentValue::S16(v) => Ok(CoreValue::I32(*v as i32)),
        ComponentValue::S32(v) => Ok(CoreValue::I32(*v)),
        ComponentValue::S64(v) => Ok(CoreValue::I64(*v)),
        ComponentValue::F32(v) => Ok(CoreValue::F32(*v)),
        ComponentValue::F64(v) => Ok(CoreValue::F64(*v)),
        ComponentValue::Char(c) => Ok(CoreValue::I32(*c as i32)),
        ComponentValue::Flags(bits) => Ok(CoreValue::I32(*bits as i32)),
        ComponentValue::Enum { discriminant, .. } => Ok(CoreValue::I32(*discriminant as i32)),
        _ => Err(ComponentError::TypeMismatch(String::from(
            "non-scalar value in scalar position",
        ))),
    }
}

// ── Lifting (CoreValue[] → ComponentValue) ───────────────────────────

/// Lift core WASM values to a `ComponentValue` according to the given type.
///
/// For strings/lists, the core values represent (pointer, length) and
/// the actual data is read from linear memory.
pub fn lift(
    values: &[CoreValue],
    ty: &ComponentType,
    memory: Option<&dyn LinearMemory>,
) -> Result<ComponentValue, ComponentError> {
    match ty {
        ComponentType::Bool => {
            let v = expect_i32(values, 0)?;
            Ok(ComponentValue::Bool(v != 0))
        }
        ComponentType::U8 => {
            let v = expect_i32(values, 0)?;
            Ok(ComponentValue::U8(v as u8))
        }
        ComponentType::U16 => {
            let v = expect_i32(values, 0)?;
            Ok(ComponentValue::U16(v as u16))
        }
        ComponentType::U32 => {
            let v = expect_i32(values, 0)?;
            Ok(ComponentValue::U32(v as u32))
        }
        ComponentType::U64 => {
            let v = expect_i64(values, 0)?;
            Ok(ComponentValue::U64(v as u64))
        }
        ComponentType::S8 => {
            let v = expect_i32(values, 0)?;
            Ok(ComponentValue::S8(v as i8))
        }
        ComponentType::S16 => {
            let v = expect_i32(values, 0)?;
            Ok(ComponentValue::S16(v as i16))
        }
        ComponentType::S32 => {
            let v = expect_i32(values, 0)?;
            Ok(ComponentValue::S32(v))
        }
        ComponentType::S64 => {
            let v = expect_i64(values, 0)?;
            Ok(ComponentValue::S64(v))
        }
        ComponentType::F32 => {
            let v = expect_f32(values, 0)?;
            Ok(ComponentValue::F32(v))
        }
        ComponentType::F64 => {
            let v = expect_f64(values, 0)?;
            Ok(ComponentValue::F64(v))
        }
        ComponentType::Char => {
            let v = expect_i32(values, 0)?;
            let c = char::from_u32(v as u32).ok_or_else(|| {
                ComponentError::TypeMismatch(String::from("invalid unicode codepoint"))
            })?;
            Ok(ComponentValue::Char(c))
        }
        ComponentType::String => {
            let mem = memory.ok_or_else(|| {
                ComponentError::TypeMismatch(String::from("string lifting requires memory"))
            })?;
            let ptr = expect_i32(values, 0)? as u32;
            let len = expect_i32(values, 1)? as u32;
            let bytes = mem.read(ptr, len)?;
            let s = core::str::from_utf8(bytes).map_err(|_| {
                ComponentError::TypeMismatch(String::from("invalid UTF-8 in string"))
            })?;
            Ok(ComponentValue::String(String::from(s)))
        }
        ComponentType::List(elem_ty) => {
            let mem = memory.ok_or_else(|| {
                ComponentError::TypeMismatch(String::from("list lifting requires memory"))
            })?;
            let ptr = expect_i32(values, 0)? as u32;
            let len = expect_i32(values, 1)? as u32;
            let elem_size = 8u32;
            let mut items = Vec::new();
            for i in 0..len {
                let offset = ptr + i * elem_size;
                let core_val = read_core_value_from_memory(mem, offset, elem_ty)?;
                let lifted = lift(&[core_val], elem_ty, memory)?;
                items.push(lifted);
            }
            Ok(ComponentValue::List(items))
        }
        ComponentType::Record(fields) => {
            let mut result = Vec::new();
            let mut idx = 0;
            for (name, field_ty) in fields {
                let core_count = core_value_count(field_ty);
                let field_values = &values[idx..idx + core_count];
                let lifted = lift(field_values, field_ty, memory)?;
                result.push((name.clone(), lifted));
                idx += core_count;
            }
            Ok(ComponentValue::Record(result))
        }
        ComponentType::Variant(cases) => {
            let disc = expect_i32(values, 0)? as u32;
            if disc as usize >= cases.len() {
                return Err(ComponentError::InvalidDiscriminant(disc));
            }
            let (name, payload_ty) = &cases[disc as usize];
            let payload = if let Some(ty) = payload_ty {
                let lifted = lift(&values[1..], ty, memory)?;
                Some(Box::new(lifted))
            } else {
                None
            };
            Ok(ComponentValue::Variant {
                discriminant: disc,
                name: name.clone(),
                value: payload,
            })
        }
        ComponentType::Enum(names) => {
            let disc = expect_i32(values, 0)? as u32;
            if disc as usize >= names.len() {
                return Err(ComponentError::InvalidDiscriminant(disc));
            }
            Ok(ComponentValue::Enum {
                discriminant: disc,
                name: names[disc as usize].clone(),
            })
        }
        ComponentType::Flags(_flag_names) => {
            let bits = expect_i32(values, 0)? as u32;
            Ok(ComponentValue::Flags(bits))
        }
        ComponentType::Option(inner_ty) => {
            let disc = expect_i32(values, 0)?;
            if disc == 0 {
                Ok(ComponentValue::Option(None))
            } else {
                let lifted = lift(&values[1..], inner_ty, memory)?;
                Ok(ComponentValue::Option(Some(Box::new(lifted))))
            }
        }
        ComponentType::Result { ok, err } => {
            let disc = expect_i32(values, 0)?;
            if disc == 0 {
                // Ok
                let ok_val = if let Some(ok_ty) = ok {
                    let lifted = lift(&values[1..], ok_ty, memory)?;
                    Some(Box::new(lifted))
                } else {
                    None
                };
                Ok(ComponentValue::Result(Ok(ok_val)))
            } else {
                // Err
                let err_val = if let Some(err_ty) = err {
                    let lifted = lift(&values[1..], err_ty, memory)?;
                    Some(Box::new(lifted))
                } else {
                    None
                };
                Ok(ComponentValue::Result(Err(err_val)))
            }
        }
    }
}

/// Read a core value from linear memory at the given offset.
fn read_core_value_from_memory(
    mem: &dyn LinearMemory,
    offset: u32,
    ty: &ComponentType,
) -> Result<CoreValue, ComponentError> {
    match ty {
        ComponentType::Bool
        | ComponentType::U8
        | ComponentType::U16
        | ComponentType::U32
        | ComponentType::S8
        | ComponentType::S16
        | ComponentType::S32
        | ComponentType::Char
        | ComponentType::Enum(_)
        | ComponentType::Flags(_) => {
            let bytes = mem.read(offset, 4)?;
            let v = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            Ok(CoreValue::I32(v))
        }
        ComponentType::U64 | ComponentType::S64 => {
            let bytes = mem.read(offset, 8)?;
            let v = i64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]);
            Ok(CoreValue::I64(v))
        }
        ComponentType::F32 => {
            let bytes = mem.read(offset, 4)?;
            let v = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            Ok(CoreValue::F32(v))
        }
        ComponentType::F64 => {
            let bytes = mem.read(offset, 8)?;
            let v = f64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]);
            Ok(CoreValue::F64(v))
        }
        _ => {
            // For compound types in a list, default to i32 for MVP
            let bytes = mem.read(offset, 4)?;
            let v = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            Ok(CoreValue::I32(v))
        }
    }
}

/// Return the number of core values a component type flattens to.
pub fn core_value_count(ty: &ComponentType) -> usize {
    match ty {
        ComponentType::Bool
        | ComponentType::U8
        | ComponentType::U16
        | ComponentType::U32
        | ComponentType::S8
        | ComponentType::S16
        | ComponentType::S32
        | ComponentType::F32
        | ComponentType::Char
        | ComponentType::Enum(_)
        | ComponentType::Flags(_) => 1,
        ComponentType::U64 | ComponentType::S64 | ComponentType::F64 => 1,
        ComponentType::String | ComponentType::List(_) => 2, // (ptr, len)
        ComponentType::Record(fields) => fields.iter().map(|(_, t)| core_value_count(t)).sum(),
        ComponentType::Variant(_) => 2, // discriminant + max payload (simplified)
        ComponentType::Option(_) => 2,  // discriminant + payload
        ComponentType::Result { .. } => 2, // discriminant + payload
    }
}

// ── Helper extractors ────────────────────────────────────────────────

fn expect_i32(values: &[CoreValue], idx: usize) -> Result<i32, ComponentError> {
    match values.get(idx) {
        Some(CoreValue::I32(v)) => Ok(*v),
        _ => Err(ComponentError::TypeMismatch(String::from(
            "expected i32 core value",
        ))),
    }
}

fn expect_i64(values: &[CoreValue], idx: usize) -> Result<i64, ComponentError> {
    match values.get(idx) {
        Some(CoreValue::I64(v)) => Ok(*v),
        _ => Err(ComponentError::TypeMismatch(String::from(
            "expected i64 core value",
        ))),
    }
}

fn expect_f32(values: &[CoreValue], idx: usize) -> Result<f32, ComponentError> {
    match values.get(idx) {
        Some(CoreValue::F32(v)) => Ok(*v),
        _ => Err(ComponentError::TypeMismatch(String::from(
            "expected f32 core value",
        ))),
    }
}

fn expect_f64(values: &[CoreValue], idx: usize) -> Result<f64, ComponentError> {
    match values.get(idx) {
        Some(CoreValue::F64(v)) => Ok(*v),
        _ => Err(ComponentError::TypeMismatch(String::from(
            "expected f64 core value",
        ))),
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;

    // ── Round-trip scalar tests ──────────────────────────────────────

    #[test]
    fn test_roundtrip_bool_true() {
        let val = ComponentValue::Bool(true);
        let lowered = lower(&val, None).unwrap();
        assert_eq!(lowered, alloc::vec![CoreValue::I32(1)]);
        let lifted = lift(&lowered, &ComponentType::Bool, None).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_bool_false() {
        let val = ComponentValue::Bool(false);
        let lowered = lower(&val, None).unwrap();
        assert_eq!(lowered, alloc::vec![CoreValue::I32(0)]);
        let lifted = lift(&lowered, &ComponentType::Bool, None).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_u8() {
        let val = ComponentValue::U8(255);
        let lowered = lower(&val, None).unwrap();
        let lifted = lift(&lowered, &ComponentType::U8, None).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_u16() {
        let val = ComponentValue::U16(65535);
        let lowered = lower(&val, None).unwrap();
        let lifted = lift(&lowered, &ComponentType::U16, None).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_u32() {
        let val = ComponentValue::U32(0xDEADBEEF);
        let lowered = lower(&val, None).unwrap();
        let lifted = lift(&lowered, &ComponentType::U32, None).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_u64() {
        let val = ComponentValue::U64(0x123456789ABCDEF0);
        let lowered = lower(&val, None).unwrap();
        let lifted = lift(&lowered, &ComponentType::U64, None).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_s8() {
        let val = ComponentValue::S8(-128);
        let lowered = lower(&val, None).unwrap();
        let lifted = lift(&lowered, &ComponentType::S8, None).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_s16() {
        let val = ComponentValue::S16(-32768);
        let lowered = lower(&val, None).unwrap();
        let lifted = lift(&lowered, &ComponentType::S16, None).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_s32() {
        let val = ComponentValue::S32(-42);
        let lowered = lower(&val, None).unwrap();
        let lifted = lift(&lowered, &ComponentType::S32, None).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_s64() {
        let val = ComponentValue::S64(i64::MIN);
        let lowered = lower(&val, None).unwrap();
        let lifted = lift(&lowered, &ComponentType::S64, None).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_f32() {
        let val = ComponentValue::F32(3.14);
        let lowered = lower(&val, None).unwrap();
        let lifted = lift(&lowered, &ComponentType::F32, None).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_f64() {
        let val = ComponentValue::F64(2.718281828);
        let lowered = lower(&val, None).unwrap();
        let lifted = lift(&lowered, &ComponentType::F64, None).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_char() {
        let val = ComponentValue::Char('한');
        let lowered = lower(&val, None).unwrap();
        let lifted = lift(&lowered, &ComponentType::Char, None).unwrap();
        assert_eq!(lifted, val);
    }

    // ── String tests (requires memory) ──────────────────────────────

    #[test]
    fn test_roundtrip_string() {
        let val = ComponentValue::String(String::from("hello world"));
        let mut mem = VecMemory::new(1024);
        let lowered = lower(&val, Some(&mut mem)).unwrap();
        assert_eq!(lowered.len(), 2); // ptr + len
        let lifted = lift(&lowered, &ComponentType::String, Some(&mem)).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_string_empty() {
        let val = ComponentValue::String(String::from(""));
        let mut mem = VecMemory::new(1024);
        let lowered = lower(&val, Some(&mut mem)).unwrap();
        let lifted = lift(&lowered, &ComponentType::String, Some(&mem)).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_string_unicode() {
        let val = ComponentValue::String(String::from("안녕하세요 🌍"));
        let mut mem = VecMemory::new(1024);
        let lowered = lower(&val, Some(&mut mem)).unwrap();
        let lifted = lift(&lowered, &ComponentType::String, Some(&mem)).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_string_no_memory_error() {
        let val = ComponentValue::String(String::from("test"));
        let result = lower(&val, None);
        assert!(result.is_err());
    }

    // ── List tests ──────────────────────────────────────────────────

    #[test]
    fn test_roundtrip_list_u32() {
        let val = ComponentValue::List(alloc::vec![
            ComponentValue::U32(10),
            ComponentValue::U32(20),
            ComponentValue::U32(30),
        ]);
        let mut mem = VecMemory::new(4096);
        let lowered = lower(&val, Some(&mut mem)).unwrap();
        assert_eq!(lowered.len(), 2); // ptr + len
        let lifted = lift(
            &lowered,
            &ComponentType::List(Box::new(ComponentType::U32)),
            Some(&mem),
        )
        .unwrap();
        assert_eq!(lifted, val);
    }

    // ── Record tests ────────────────────────────────────────────────

    #[test]
    fn test_roundtrip_record() {
        let val = ComponentValue::Record(alloc::vec![
            (String::from("x"), ComponentValue::S32(10)),
            (String::from("y"), ComponentValue::S32(20)),
        ]);
        let lowered = lower(&val, None).unwrap();
        assert_eq!(lowered.len(), 2);
        let ty = ComponentType::Record(alloc::vec![
            (String::from("x"), ComponentType::S32),
            (String::from("y"), ComponentType::S32),
        ]);
        let lifted = lift(&lowered, &ty, None).unwrap();
        assert_eq!(lifted, val);
    }

    // ── Variant tests ───────────────────────────────────────────────

    #[test]
    fn test_roundtrip_variant() {
        let val = ComponentValue::Variant {
            discriminant: 1,
            name: String::from("b"),
            value: Some(Box::new(ComponentValue::U32(42))),
        };
        let lowered = lower(&val, None).unwrap();
        let ty = ComponentType::Variant(alloc::vec![
            (String::from("a"), None),
            (String::from("b"), Some(ComponentType::U32)),
        ]);
        let lifted = lift(&lowered, &ty, None).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_variant_invalid_discriminant() {
        let values = alloc::vec![CoreValue::I32(5)];
        let ty = ComponentType::Variant(alloc::vec![
            (String::from("a"), None),
            (String::from("b"), None),
        ]);
        let result = lift(&values, &ty, None);
        assert!(matches!(result, Err(ComponentError::InvalidDiscriminant(5))));
    }

    // ── Enum tests ──────────────────────────────────────────────────

    #[test]
    fn test_roundtrip_enum() {
        let val = ComponentValue::Enum {
            discriminant: 2,
            name: String::from("blue"),
        };
        let lowered = lower(&val, None).unwrap();
        let ty = ComponentType::Enum(alloc::vec![
            String::from("red"),
            String::from("green"),
            String::from("blue"),
        ]);
        let lifted = lift(&lowered, &ty, None).unwrap();
        assert_eq!(lifted, val);
    }

    // ── Flags tests ─────────────────────────────────────────────────

    #[test]
    fn test_roundtrip_flags() {
        let val = ComponentValue::Flags(0b1010);
        let lowered = lower(&val, None).unwrap();
        let ty = ComponentType::Flags(alloc::vec![
            String::from("a"),
            String::from("b"),
            String::from("c"),
            String::from("d"),
        ]);
        let lifted = lift(&lowered, &ty, None).unwrap();
        assert_eq!(lifted, val);
    }

    // ── Option tests ────────────────────────────────────────────────

    #[test]
    fn test_roundtrip_option_some() {
        let val = ComponentValue::Option(Some(Box::new(ComponentValue::U32(42))));
        let lowered = lower(&val, None).unwrap();
        let ty = ComponentType::Option(Box::new(ComponentType::U32));
        let lifted = lift(&lowered, &ty, None).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_option_none() {
        let val = ComponentValue::Option(None);
        let lowered = lower(&val, None).unwrap();
        let ty = ComponentType::Option(Box::new(ComponentType::U32));
        let lifted = lift(&lowered, &ty, None).unwrap();
        assert_eq!(lifted, val);
    }

    // ── Result tests ────────────────────────────────────────────────

    #[test]
    fn test_roundtrip_result_ok() {
        let val = ComponentValue::Result(Ok(Some(Box::new(ComponentValue::U32(99)))));
        let lowered = lower(&val, None).unwrap();
        let ty = ComponentType::Result {
            ok: Some(Box::new(ComponentType::U32)),
            err: None,
        };
        let lifted = lift(&lowered, &ty, None).unwrap();
        assert_eq!(lifted, val);
    }

    #[test]
    fn test_roundtrip_result_err() {
        let val = ComponentValue::Result(Err(Some(Box::new(ComponentValue::S32(-1)))));
        let lowered = lower(&val, None).unwrap();
        let ty = ComponentType::Result {
            ok: None,
            err: Some(Box::new(ComponentType::S32)),
        };
        let lifted = lift(&lowered, &ty, None).unwrap();
        assert_eq!(lifted, val);
    }

    // ── core_value_count tests ──────────────────────────────────────

    #[test]
    fn test_core_value_count_scalars() {
        assert_eq!(core_value_count(&ComponentType::Bool), 1);
        assert_eq!(core_value_count(&ComponentType::U32), 1);
        assert_eq!(core_value_count(&ComponentType::S64), 1);
        assert_eq!(core_value_count(&ComponentType::F64), 1);
        assert_eq!(core_value_count(&ComponentType::String), 2);
    }

    #[test]
    fn test_core_value_count_record() {
        let ty = ComponentType::Record(alloc::vec![
            (String::from("a"), ComponentType::U32),
            (String::from("b"), ComponentType::U64),
            (String::from("c"), ComponentType::F32),
        ]);
        assert_eq!(core_value_count(&ty), 3);
    }

    // ── VecMemory tests ────────────────────────────────────────────

    #[test]
    fn test_vec_memory_basic() {
        let mut mem = VecMemory::new(256);
        assert_eq!(mem.size(), 256);
        mem.write(0, &[1, 2, 3, 4]).unwrap();
        let data = mem.read(0, 4).unwrap();
        assert_eq!(data, &[1, 2, 3, 4]);
    }

    #[test]
    fn test_vec_memory_out_of_bounds() {
        let mem = VecMemory::new(10);
        assert!(mem.read(8, 4).is_err());
    }

    #[test]
    fn test_vec_memory_alloc_alignment() {
        let mut mem = VecMemory::new(256);
        let a = mem.alloc(3, 1).unwrap();
        assert_eq!(a, 0);
        let b = mem.alloc(4, 4).unwrap();
        assert_eq!(b, 4); // aligned to 4
    }
}
