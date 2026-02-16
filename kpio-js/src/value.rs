//! JavaScript value types.
//!
//! Implements JavaScript runtime values.

use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::RefCell;
use core::fmt;
use libm::{fabs, trunc};

use crate::error::{JsError, JsResult};
use crate::object::{JsObject, PropertyKey};

/// A JavaScript value.
#[derive(Clone)]
pub enum Value {
    /// The undefined value.
    Undefined,
    /// The null value.
    Null,
    /// A boolean value.
    Boolean(bool),
    /// A numeric value.
    Number(f64),
    /// A string value.
    String(String),
    /// A symbol value.
    Symbol(Symbol),
    /// A BigInt value.
    BigInt(i64),
    /// An object value.
    Object(Rc<RefCell<JsObject>>),
}

impl Value {
    /// Create undefined.
    pub fn undefined() -> Self {
        Value::Undefined
    }

    /// Create null.
    pub fn null() -> Self {
        Value::Null
    }

    /// Create a boolean.
    pub fn boolean(b: bool) -> Self {
        Value::Boolean(b)
    }

    /// Create a number.
    pub fn number(n: f64) -> Self {
        Value::Number(n)
    }

    /// Create a string.
    pub fn string<S: Into<String>>(s: S) -> Self {
        Value::String(s.into())
    }

    /// Create an object.
    pub fn object(obj: JsObject) -> Self {
        Value::Object(Rc::new(RefCell::new(obj)))
    }

    /// Check if value is undefined.
    pub fn is_undefined(&self) -> bool {
        matches!(self, Value::Undefined)
    }

    /// Check if value is null.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Check if value is nullish (undefined or null).
    pub fn is_nullish(&self) -> bool {
        matches!(self, Value::Undefined | Value::Null)
    }

    /// Check if value is a boolean.
    pub fn is_boolean(&self) -> bool {
        matches!(self, Value::Boolean(_))
    }

    /// Check if value is a number.
    pub fn is_number(&self) -> bool {
        matches!(self, Value::Number(_))
    }

    /// Check if value is a string.
    pub fn is_string(&self) -> bool {
        matches!(self, Value::String(_))
    }

    /// Check if value is a symbol.
    pub fn is_symbol(&self) -> bool {
        matches!(self, Value::Symbol(_))
    }

    /// Check if value is an object (or function).
    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }

    /// Check if value is a function.
    pub fn is_function(&self) -> bool {
        if let Value::Object(obj) = self {
            obj.borrow().is_callable()
        } else {
            false
        }
    }

    /// Check if value is an array.
    pub fn is_array(&self) -> bool {
        if let Value::Object(obj) = self {
            obj.borrow().is_array()
        } else {
            false
        }
    }

    /// Get the type of value as a string.
    pub fn type_of(&self) -> &'static str {
        match self {
            Value::Undefined => "undefined",
            Value::Null => "object", // Historical JavaScript quirk
            Value::Boolean(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Symbol(_) => "symbol",
            Value::BigInt(_) => "bigint",
            Value::Object(obj) => {
                if obj.borrow().is_callable() {
                    "function"
                } else {
                    "object"
                }
            }
        }
    }

    /// Convert to boolean (ToBoolean).
    pub fn to_boolean(&self) -> bool {
        match self {
            Value::Undefined | Value::Null => false,
            Value::Boolean(b) => *b,
            Value::Number(n) => *n != 0.0 && !n.is_nan(),
            Value::String(s) => !s.is_empty(),
            Value::Symbol(_) => true,
            Value::BigInt(n) => *n != 0,
            Value::Object(_) => true,
        }
    }

    /// Convert to number (ToNumber).
    pub fn to_number(&self) -> JsResult<f64> {
        match self {
            Value::Undefined => Ok(f64::NAN),
            Value::Null => Ok(0.0),
            Value::Boolean(b) => Ok(if *b { 1.0 } else { 0.0 }),
            Value::Number(n) => Ok(*n),
            Value::String(s) => {
                let s = s.trim();
                if s.is_empty() {
                    return Ok(0.0);
                }
                // Try to parse as number
                parse_number(s).ok_or_else(|| JsError::type_error("Cannot convert to number"))
            }
            Value::Symbol(_) => Err(JsError::type_error("Cannot convert symbol to number")),
            Value::BigInt(_) => Err(JsError::type_error("Cannot convert BigInt to number")),
            Value::Object(_) => {
                // Should call ToPrimitive first
                Ok(f64::NAN)
            }
        }
    }

    /// Convert to integer.
    pub fn to_integer(&self) -> JsResult<i64> {
        let n = self.to_number()?;
        if n.is_nan() {
            return Ok(0);
        }
        if n.is_infinite() {
            return Ok(if n > 0.0 { i64::MAX } else { i64::MIN });
        }
        Ok(trunc(n) as i64)
    }

    /// Convert to unsigned 32-bit integer.
    pub fn to_u32(&self) -> JsResult<u32> {
        let n = self.to_number()?;
        if n.is_nan() || n.is_infinite() || n == 0.0 {
            return Ok(0);
        }
        Ok(trunc(n) as i64 as u32)
    }

    /// Convert to signed 32-bit integer.
    pub fn to_i32(&self) -> JsResult<i32> {
        let n = self.to_number()?;
        if n.is_nan() || n.is_infinite() || n == 0.0 {
            return Ok(0);
        }
        Ok(trunc(n) as i64 as i32)
    }

    /// Convert to string (ToString).
    pub fn to_string(&self) -> JsResult<String> {
        match self {
            Value::Undefined => Ok("undefined".into()),
            Value::Null => Ok("null".into()),
            Value::Boolean(b) => Ok(if *b { "true".into() } else { "false".into() }),
            Value::Number(n) => Ok(number_to_string(*n)),
            Value::String(s) => Ok(s.clone()),
            Value::Symbol(_) => Err(JsError::type_error("Cannot convert symbol to string")),
            Value::BigInt(n) => Ok(format_number(*n)),
            Value::Object(_) => {
                // Should call ToPrimitive first
                Ok("[object Object]".into())
            }
        }
    }

    /// Convert to object (ToObject).
    pub fn to_object(&self) -> JsResult<Rc<RefCell<JsObject>>> {
        match self {
            Value::Undefined | Value::Null => Err(JsError::type_error(
                "Cannot convert null or undefined to object",
            )),
            Value::Boolean(b) => {
                let obj = JsObject::boolean_object(*b);
                Ok(Rc::new(RefCell::new(obj)))
            }
            Value::Number(n) => {
                let obj = JsObject::number_object(*n);
                Ok(Rc::new(RefCell::new(obj)))
            }
            Value::String(s) => {
                let obj = JsObject::string_object(s.clone());
                Ok(Rc::new(RefCell::new(obj)))
            }
            Value::Symbol(s) => {
                let obj = JsObject::symbol_object(s.clone());
                Ok(Rc::new(RefCell::new(obj)))
            }
            Value::BigInt(n) => {
                let obj = JsObject::bigint_object(*n);
                Ok(Rc::new(RefCell::new(obj)))
            }
            Value::Object(obj) => Ok(obj.clone()),
        }
    }

    /// Get property from object.
    pub fn get(&self, key: &PropertyKey) -> JsResult<Value> {
        match self {
            Value::Object(obj) => obj.borrow().get(key),
            Value::String(s) => {
                // String indexing
                if let PropertyKey::Index(i) = key {
                    if (*i as usize) < s.len() {
                        if let Some(ch) = s.chars().nth(*i as usize) {
                            return Ok(Value::string(ch.to_string()));
                        }
                    }
                    return Ok(Value::undefined());
                }
                if let PropertyKey::String(k) = key {
                    if k == "length" {
                        return Ok(Value::number(s.len() as f64));
                    }
                }
                Ok(Value::undefined())
            }
            _ => Ok(Value::undefined()),
        }
    }

    /// Set property on object.
    pub fn set(&self, key: PropertyKey, value: Value) -> JsResult<()> {
        match self {
            Value::Object(obj) => obj.borrow_mut().set(key, value),
            _ => Err(JsError::type_error("Cannot set property on primitive")),
        }
    }

    /// Strict equality (===).
    pub fn strict_equals(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Undefined, Value::Undefined) => true,
            (Value::Null, Value::Null) => true,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => {
                if a.is_nan() || b.is_nan() {
                    false
                } else {
                    a == b
                }
            }
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Symbol(a), Value::Symbol(b)) => a == b,
            (Value::BigInt(a), Value::BigInt(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// Abstract equality (==).
    pub fn abstract_equals(&self, other: &Value) -> JsResult<bool> {
        // Same type
        if core::mem::discriminant(self) == core::mem::discriminant(other) {
            return Ok(self.strict_equals(other));
        }

        // Null and undefined
        match (self, other) {
            (Value::Null, Value::Undefined) | (Value::Undefined, Value::Null) => return Ok(true),
            _ => {}
        }

        // Number comparisons
        match (self, other) {
            (Value::Number(a), Value::String(_)) => {
                let b = other.to_number()?;
                return Ok(*a == b);
            }
            (Value::String(_), Value::Number(b)) => {
                let a = self.to_number()?;
                return Ok(a == *b);
            }
            (Value::Boolean(_), _) => {
                let a = self.to_number()?;
                return Value::number(a).abstract_equals(other);
            }
            (_, Value::Boolean(_)) => {
                let b = other.to_number()?;
                return self.abstract_equals(&Value::number(b));
            }
            _ => {}
        }

        Ok(false)
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Undefined
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Undefined => write!(f, "undefined"),
            Value::Null => write!(f, "null"),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{:?}", s),
            Value::Symbol(s) => write!(f, "{:?}", s),
            Value::BigInt(n) => write!(f, "{}n", n),
            Value::Object(_) => write!(f, "[object Object]"),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_string() {
            Ok(s) => write!(f, "{}", s),
            Err(_) => write!(f, "[conversion error]"),
        }
    }
}

/// A JavaScript Symbol.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Symbol {
    /// Symbol description.
    pub description: Option<String>,
    /// Unique identifier.
    id: u64,
}

impl Symbol {
    /// Create a new symbol.
    pub fn new(description: Option<String>) -> Self {
        static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
        Symbol {
            description,
            id: COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst),
        }
    }

    /// Get the symbol description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// Well-known symbols.
pub struct WellKnownSymbols {
    pub iterator: Symbol,
    pub async_iterator: Symbol,
    pub has_instance: Symbol,
    pub is_concat_spreadable: Symbol,
    pub species: Symbol,
    pub to_primitive: Symbol,
    pub to_string_tag: Symbol,
    pub unscopables: Symbol,
}

impl Default for WellKnownSymbols {
    fn default() -> Self {
        Self::new()
    }
}

impl WellKnownSymbols {
    /// Create well-known symbols.
    pub fn new() -> Self {
        WellKnownSymbols {
            iterator: Symbol::new(Some("Symbol.iterator".into())),
            async_iterator: Symbol::new(Some("Symbol.asyncIterator".into())),
            has_instance: Symbol::new(Some("Symbol.hasInstance".into())),
            is_concat_spreadable: Symbol::new(Some("Symbol.isConcatSpreadable".into())),
            species: Symbol::new(Some("Symbol.species".into())),
            to_primitive: Symbol::new(Some("Symbol.toPrimitive".into())),
            to_string_tag: Symbol::new(Some("Symbol.toStringTag".into())),
            unscopables: Symbol::new(Some("Symbol.unscopables".into())),
        }
    }
}

// Helper functions

/// Parse a number from string.
fn parse_number(s: &str) -> Option<f64> {
    let s = s.trim();

    // Handle special values
    if s == "Infinity" || s == "+Infinity" {
        return Some(f64::INFINITY);
    }
    if s == "-Infinity" {
        return Some(f64::NEG_INFINITY);
    }
    if s == "NaN" {
        return Some(f64::NAN);
    }

    // Handle hex
    if s.starts_with("0x") || s.starts_with("0X") {
        let hex = &s[2..];
        let mut result = 0u64;
        for c in hex.chars() {
            let digit = c.to_digit(16)?;
            result = result * 16 + digit as u64;
        }
        return Some(result as f64);
    }

    // Handle octal
    if s.starts_with("0o") || s.starts_with("0O") {
        let oct = &s[2..];
        let mut result = 0u64;
        for c in oct.chars() {
            let digit = c.to_digit(8)?;
            result = result * 8 + digit as u64;
        }
        return Some(result as f64);
    }

    // Handle binary
    if s.starts_with("0b") || s.starts_with("0B") {
        let bin = &s[2..];
        let mut result = 0u64;
        for c in bin.chars() {
            let digit = c.to_digit(2)?;
            result = result * 2 + digit as u64;
        }
        return Some(result as f64);
    }

    // Regular number parsing
    parse_decimal(s)
}

/// Parse decimal number.
fn parse_decimal(s: &str) -> Option<f64> {
    let mut chars = s.chars().peekable();
    let mut result: f64 = 0.0;
    let mut negative = false;

    // Sign
    if chars.peek() == Some(&'-') {
        negative = true;
        chars.next();
    } else if chars.peek() == Some(&'+') {
        chars.next();
    }

    // Integer part
    let mut has_digits = false;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            has_digits = true;
            result = result * 10.0 + (c as u32 - '0' as u32) as f64;
            chars.next();
        } else {
            break;
        }
    }

    // Decimal part
    if chars.peek() == Some(&'.') {
        chars.next();
        let mut frac = 0.1;
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                has_digits = true;
                result += (c as u32 - '0' as u32) as f64 * frac;
                frac *= 0.1;
                chars.next();
            } else {
                break;
            }
        }
    }

    if !has_digits {
        return None;
    }

    // Exponent
    if chars.peek() == Some(&'e') || chars.peek() == Some(&'E') {
        chars.next();
        let mut exp_negative = false;
        if chars.peek() == Some(&'-') {
            exp_negative = true;
            chars.next();
        } else if chars.peek() == Some(&'+') {
            chars.next();
        }

        let mut exp: i32 = 0;
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                exp = exp * 10 + (c as u32 - '0' as u32) as i32;
                chars.next();
            } else {
                break;
            }
        }

        if exp_negative {
            exp = -exp;
        }

        // Apply exponent
        if exp > 0 {
            for _ in 0..exp {
                result *= 10.0;
            }
        } else if exp < 0 {
            for _ in 0..(-exp) {
                result /= 10.0;
            }
        }
    }

    // Check for trailing characters
    if chars.peek().is_some() {
        return None;
    }

    if negative {
        result = -result;
    }

    Some(result)
}

/// Convert number to string.
fn number_to_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".into();
    }
    if n.is_infinite() {
        if n > 0.0 {
            return "Infinity".into();
        } else {
            return "-Infinity".into();
        }
    }
    if n == 0.0 {
        return "0".into();
    }

    // For integers
    if trunc(n) == n && fabs(n) < 1e15 {
        return format_number(n as i64);
    }

    // General case - simplified formatting
    let s = alloc::format!("{}", n);
    s
}

/// Format an integer.
fn format_number(n: i64) -> String {
    if n == 0 {
        return "0".into();
    }

    let mut result = Vec::new();
    let mut n = n;
    let negative = n < 0;
    if negative {
        n = -n;
    }

    while n > 0 {
        result.push((b'0' + (n % 10) as u8) as char);
        n /= 10;
    }

    if negative {
        result.push('-');
    }

    result.reverse();
    result.into_iter().collect()
}

/// Completion value for statements.
#[derive(Clone, Debug)]
pub enum Completion {
    /// Normal completion.
    Normal(Value),
    /// Return completion.
    Return(Value),
    /// Break completion.
    Break(Option<String>),
    /// Continue completion.
    Continue(Option<String>),
    /// Throw completion.
    Throw(Value),
}

impl Completion {
    /// Create a normal completion.
    pub fn normal(value: Value) -> Self {
        Completion::Normal(value)
    }

    /// Create an empty normal completion.
    pub fn empty() -> Self {
        Completion::Normal(Value::undefined())
    }

    /// Check if this is a normal completion.
    pub fn is_normal(&self) -> bool {
        matches!(self, Completion::Normal(_))
    }

    /// Get the value from a normal completion.
    pub fn value(self) -> Value {
        match self {
            Completion::Normal(v) | Completion::Return(v) | Completion::Throw(v) => v,
            _ => Value::undefined(),
        }
    }
}
