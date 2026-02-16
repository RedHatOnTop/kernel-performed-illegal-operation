//! JavaScript built-in objects and functions.
//!
//! Implements standard JavaScript built-in objects.

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use libm::trunc;

use crate::error::{JsError, JsResult};
use crate::interpreter::Interpreter;
use crate::object::{Callable, JsObject, NativeFunction, PropertyDescriptor, PropertyKey};
use crate::value::Value;

/// Initialize built-in objects.
pub fn init(interp: &mut Interpreter) {
    // Global values
    interp.define_global("undefined", Value::undefined());
    interp.define_global("NaN", Value::number(f64::NAN));
    interp.define_global("Infinity", Value::number(f64::INFINITY));

    // Global functions
    interp.define_native_function("isNaN", 1, is_nan);
    interp.define_native_function("isFinite", 1, is_finite);
    interp.define_native_function("parseInt", 2, parse_int);
    interp.define_native_function("parseFloat", 1, parse_float);
    interp.define_native_function("eval", 1, eval);

    // Console object
    init_console(interp);

    // Object constructor
    init_object(interp);

    // Array constructor
    init_array(interp);

    // String constructor
    init_string(interp);

    // Number constructor
    init_number(interp);

    // Boolean constructor
    init_boolean(interp);

    // Math object
    init_math(interp);

    // JSON object
    init_json(interp);

    // Error constructors
    init_error(interp);
}

// Global functions

fn is_nan(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let num = args.first().unwrap_or(&Value::undefined()).to_number()?;
    Ok(Value::boolean(num.is_nan()))
}

fn is_finite(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let num = args.first().unwrap_or(&Value::undefined()).to_number()?;
    Ok(Value::boolean(num.is_finite()))
}

fn parse_int(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let string = args.first().unwrap_or(&Value::undefined()).to_string()?;
    let radix = if args.len() > 1 {
        args[1].to_number()? as i32
    } else {
        10
    };

    let string = string.trim();

    if radix != 0 && (radix < 2 || radix > 36) {
        return Ok(Value::number(f64::NAN));
    }

    let radix = if radix == 0 { 10 } else { radix } as u32;

    let mut result: i64 = 0;
    let mut negative = false;
    let mut chars = string.chars().peekable();

    // Handle sign
    if chars.peek() == Some(&'-') {
        negative = true;
        chars.next();
    } else if chars.peek() == Some(&'+') {
        chars.next();
    }

    // Handle 0x prefix for radix 16
    if radix == 16 {
        if chars.peek() == Some(&'0') {
            chars.next();
            if chars.peek() == Some(&'x') || chars.peek() == Some(&'X') {
                chars.next();
            }
        }
    }

    let mut has_digits = false;
    for c in chars {
        if let Some(digit) = c.to_digit(radix) {
            has_digits = true;
            result = result * radix as i64 + digit as i64;
        } else {
            break;
        }
    }

    if !has_digits {
        return Ok(Value::number(f64::NAN));
    }

    if negative {
        result = -result;
    }

    Ok(Value::number(result as f64))
}

fn parse_float(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let string = args.first().unwrap_or(&Value::undefined()).to_string()?;
    let string = string.trim();

    // Simple float parsing
    let mut result = 0.0f64;
    let mut negative = false;
    let mut chars = string.chars().peekable();

    // Handle sign
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
        return Ok(Value::number(f64::NAN));
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

        result *= libm::pow(10.0, exp as f64);
    }

    if negative {
        result = -result;
    }

    Ok(Value::number(result))
}

fn eval(_this: &Value, _args: &[Value]) -> JsResult<Value> {
    // eval is not fully supported for security
    Err(JsError::error("EvalError", "eval is not supported"))
}

// Console object

fn init_console(interp: &mut Interpreter) {
    let mut console = JsObject::new();

    console.define_property(
        PropertyKey::string("log"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "log".into(),
                length: 0,
                func: console_log,
            }))),
            true,
            false,
            true,
        ),
    );

    console.define_property(
        PropertyKey::string("error"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "error".into(),
                length: 0,
                func: console_log,
            }))),
            true,
            false,
            true,
        ),
    );

    console.define_property(
        PropertyKey::string("warn"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "warn".into(),
                length: 0,
                func: console_log,
            }))),
            true,
            false,
            true,
        ),
    );

    console.define_property(
        PropertyKey::string("info"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "info".into(),
                length: 0,
                func: console_log,
            }))),
            true,
            false,
            true,
        ),
    );

    interp.define_global("console", Value::object(console));
}

fn console_log(_this: &Value, args: &[Value]) -> JsResult<Value> {
    // In a real implementation, this would output to the console
    let parts: Vec<String> = args
        .iter()
        .map(|v| v.to_string().unwrap_or_else(|_| "[error]".into()))
        .collect();

    let _message = parts.join(" ");
    // TODO: Actually log the message somewhere

    Ok(Value::undefined())
}

// Object constructor

fn init_object(interp: &mut Interpreter) {
    let mut obj = JsObject::function(Callable::Native(NativeFunction {
        name: "Object".into(),
        length: 1,
        func: object_constructor,
    }));

    // Object.keys
    obj.define_property(
        PropertyKey::string("keys"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "keys".into(),
                length: 1,
                func: object_keys,
            }))),
            true,
            false,
            true,
        ),
    );

    // Object.values
    obj.define_property(
        PropertyKey::string("values"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "values".into(),
                length: 1,
                func: object_values,
            }))),
            true,
            false,
            true,
        ),
    );

    // Object.entries
    obj.define_property(
        PropertyKey::string("entries"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "entries".into(),
                length: 1,
                func: object_entries,
            }))),
            true,
            false,
            true,
        ),
    );

    // Object.assign
    obj.define_property(
        PropertyKey::string("assign"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "assign".into(),
                length: 2,
                func: object_assign,
            }))),
            true,
            false,
            true,
        ),
    );

    interp.define_global("Object", Value::object(obj));
}

fn object_constructor(_this: &Value, args: &[Value]) -> JsResult<Value> {
    if args.is_empty() || args[0].is_nullish() {
        Ok(Value::object(JsObject::new()))
    } else {
        args[0].to_object().map(Value::Object)
    }
}

fn object_keys(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let obj = args.first().unwrap_or(&Value::undefined()).to_object()?;
    let keys: Vec<Option<Value>> = obj
        .borrow()
        .own_enumerable_keys()
        .into_iter()
        .map(|k| Some(Value::string(k.to_string())))
        .collect();

    Ok(Value::object(JsObject::array(keys)))
}

fn object_values(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let obj = args.first().unwrap_or(&Value::undefined()).to_object()?;
    let values: Vec<Option<Value>> = obj
        .borrow()
        .own_enumerable_keys()
        .into_iter()
        .map(|k| obj.borrow().get(&k).ok())
        .collect();

    Ok(Value::object(JsObject::array(values)))
}

fn object_entries(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let obj = args.first().unwrap_or(&Value::undefined()).to_object()?;
    let entries: Vec<Option<Value>> = obj
        .borrow()
        .own_enumerable_keys()
        .into_iter()
        .map(|k| {
            let value = obj.borrow().get(&k).unwrap_or(Value::undefined());
            Some(Value::object(JsObject::array(vec![
                Some(Value::string(k.to_string())),
                Some(value),
            ])))
        })
        .collect();

    Ok(Value::object(JsObject::array(entries)))
}

fn object_assign(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let target = args.first().unwrap_or(&Value::undefined()).to_object()?;

    for source in args.iter().skip(1) {
        if source.is_nullish() {
            continue;
        }

        let src = source.to_object()?;
        for key in src.borrow().own_enumerable_keys() {
            let value = src.borrow().get(&key)?;
            target.borrow_mut().set(key, value)?;
        }
    }

    Ok(Value::Object(target))
}

// Array constructor

fn init_array(interp: &mut Interpreter) {
    let mut arr = JsObject::function(Callable::Native(NativeFunction {
        name: "Array".into(),
        length: 1,
        func: array_constructor,
    }));

    // Array.isArray
    arr.define_property(
        PropertyKey::string("isArray"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "isArray".into(),
                length: 1,
                func: array_is_array,
            }))),
            true,
            false,
            true,
        ),
    );

    // Array.from
    arr.define_property(
        PropertyKey::string("from"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "from".into(),
                length: 1,
                func: array_from,
            }))),
            true,
            false,
            true,
        ),
    );

    // Prototype methods
    let mut proto = JsObject::new();

    proto.define_property(
        PropertyKey::string("push"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "push".into(),
                length: 1,
                func: array_push,
            }))),
            true,
            false,
            true,
        ),
    );

    proto.define_property(
        PropertyKey::string("pop"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "pop".into(),
                length: 0,
                func: array_pop,
            }))),
            true,
            false,
            true,
        ),
    );

    proto.define_property(
        PropertyKey::string("join"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "join".into(),
                length: 1,
                func: array_join,
            }))),
            true,
            false,
            true,
        ),
    );

    proto.define_property(
        PropertyKey::string("indexOf"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "indexOf".into(),
                length: 1,
                func: array_index_of,
            }))),
            true,
            false,
            true,
        ),
    );

    proto.define_property(
        PropertyKey::string("includes"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "includes".into(),
                length: 1,
                func: array_includes,
            }))),
            true,
            false,
            true,
        ),
    );

    arr.define_property(
        PropertyKey::string("prototype"),
        PropertyDescriptor::data(Value::object(proto), false, false, false),
    );

    interp.define_global("Array", Value::object(arr));
}

fn array_constructor(_this: &Value, args: &[Value]) -> JsResult<Value> {
    if args.len() == 1 {
        if let Value::Number(n) = &args[0] {
            let len = *n as usize;
            return Ok(Value::object(JsObject::array(vec![None; len])));
        }
    }

    let elements: Vec<Option<Value>> = args.iter().cloned().map(Some).collect();
    Ok(Value::object(JsObject::array(elements)))
}

fn array_is_array(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let is_arr = args.first().map_or(false, |v| v.is_array());
    Ok(Value::boolean(is_arr))
}

fn array_from(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let default_val = Value::undefined();
    let iterable = args.first().unwrap_or(&default_val);

    if iterable.is_array() {
        return Ok(iterable.clone());
    }

    if let Value::String(s) = iterable {
        let chars: Vec<Option<Value>> = s
            .chars()
            .map(|c| Some(Value::string(c.to_string())))
            .collect();
        return Ok(Value::object(JsObject::array(chars)));
    }

    Ok(Value::object(JsObject::array(Vec::new())))
}

fn array_push(this: &Value, args: &[Value]) -> JsResult<Value> {
    if let Value::Object(obj) = this {
        for arg in args {
            obj.borrow_mut().array_push(arg.clone());
        }
        Ok(Value::number(obj.borrow().array_length() as f64))
    } else {
        Err(JsError::type_error(
            "Array.prototype.push called on non-object",
        ))
    }
}

fn array_pop(this: &Value, _args: &[Value]) -> JsResult<Value> {
    if let Value::Object(obj) = this {
        Ok(obj.borrow_mut().array_pop().unwrap_or(Value::undefined()))
    } else {
        Err(JsError::type_error(
            "Array.prototype.pop called on non-object",
        ))
    }
}

fn array_join(this: &Value, args: &[Value]) -> JsResult<Value> {
    let separator = args
        .first()
        .map(|v| v.to_string().unwrap_or_else(|_| ",".into()))
        .unwrap_or_else(|| ",".into());

    if let Value::Object(obj) = this {
        let len = obj.borrow().array_length();
        let mut parts = Vec::new();

        for i in 0..len {
            let value = obj.borrow().get(&PropertyKey::Index(i as u32))?;
            let s = if value.is_nullish() {
                String::new()
            } else {
                value.to_string()?
            };
            parts.push(s);
        }

        Ok(Value::string(parts.join(&separator)))
    } else {
        Err(JsError::type_error(
            "Array.prototype.join called on non-object",
        ))
    }
}

fn array_index_of(this: &Value, args: &[Value]) -> JsResult<Value> {
    let search = args.first().cloned().unwrap_or(Value::undefined());

    if let Value::Object(obj) = this {
        let len = obj.borrow().array_length();

        for i in 0..len {
            let value = obj.borrow().get(&PropertyKey::Index(i as u32))?;
            if value.strict_equals(&search) {
                return Ok(Value::number(i as f64));
            }
        }

        Ok(Value::number(-1.0))
    } else {
        Err(JsError::type_error(
            "Array.prototype.indexOf called on non-object",
        ))
    }
}

fn array_includes(this: &Value, args: &[Value]) -> JsResult<Value> {
    let search = args.first().cloned().unwrap_or(Value::undefined());

    if let Value::Object(obj) = this {
        let len = obj.borrow().array_length();

        for i in 0..len {
            let value = obj.borrow().get(&PropertyKey::Index(i as u32))?;
            if value.strict_equals(&search) {
                return Ok(Value::boolean(true));
            }
        }

        Ok(Value::boolean(false))
    } else {
        Err(JsError::type_error(
            "Array.prototype.includes called on non-object",
        ))
    }
}

// String constructor

fn init_string(interp: &mut Interpreter) {
    let str_obj = JsObject::function(Callable::Native(NativeFunction {
        name: "String".into(),
        length: 1,
        func: string_constructor,
    }));

    interp.define_global("String", Value::object(str_obj));
}

fn string_constructor(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let s = args
        .first()
        .map(|v| v.to_string())
        .transpose()?
        .unwrap_or_default();

    Ok(Value::string(s))
}

// Number constructor

fn init_number(interp: &mut Interpreter) {
    let mut num = JsObject::function(Callable::Native(NativeFunction {
        name: "Number".into(),
        length: 1,
        func: number_constructor,
    }));

    num.define_property(
        PropertyKey::string("isNaN"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "isNaN".into(),
                length: 1,
                func: number_is_nan,
            }))),
            true,
            false,
            true,
        ),
    );

    num.define_property(
        PropertyKey::string("isFinite"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "isFinite".into(),
                length: 1,
                func: number_is_finite,
            }))),
            true,
            false,
            true,
        ),
    );

    num.define_property(
        PropertyKey::string("isInteger"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "isInteger".into(),
                length: 1,
                func: number_is_integer,
            }))),
            true,
            false,
            true,
        ),
    );

    num.define_property(
        PropertyKey::string("MAX_VALUE"),
        PropertyDescriptor::data(Value::number(f64::MAX), false, false, false),
    );

    num.define_property(
        PropertyKey::string("MIN_VALUE"),
        PropertyDescriptor::data(Value::number(f64::MIN_POSITIVE), false, false, false),
    );

    num.define_property(
        PropertyKey::string("NaN"),
        PropertyDescriptor::data(Value::number(f64::NAN), false, false, false),
    );

    num.define_property(
        PropertyKey::string("POSITIVE_INFINITY"),
        PropertyDescriptor::data(Value::number(f64::INFINITY), false, false, false),
    );

    num.define_property(
        PropertyKey::string("NEGATIVE_INFINITY"),
        PropertyDescriptor::data(Value::number(f64::NEG_INFINITY), false, false, false),
    );

    interp.define_global("Number", Value::object(num));
}

fn number_constructor(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let n = args
        .first()
        .map(|v| v.to_number())
        .transpose()?
        .unwrap_or(0.0);

    Ok(Value::number(n))
}

fn number_is_nan(_this: &Value, args: &[Value]) -> JsResult<Value> {
    if let Some(Value::Number(n)) = args.first() {
        Ok(Value::boolean(n.is_nan()))
    } else {
        Ok(Value::boolean(false))
    }
}

fn number_is_finite(_this: &Value, args: &[Value]) -> JsResult<Value> {
    if let Some(Value::Number(n)) = args.first() {
        Ok(Value::boolean(n.is_finite()))
    } else {
        Ok(Value::boolean(false))
    }
}

fn number_is_integer(_this: &Value, args: &[Value]) -> JsResult<Value> {
    if let Some(Value::Number(n)) = args.first() {
        Ok(Value::boolean(n.is_finite() && trunc(*n) == *n))
    } else {
        Ok(Value::boolean(false))
    }
}

// Boolean constructor

fn init_boolean(interp: &mut Interpreter) {
    let bool_obj = JsObject::function(Callable::Native(NativeFunction {
        name: "Boolean".into(),
        length: 1,
        func: boolean_constructor,
    }));

    interp.define_global("Boolean", Value::object(bool_obj));
}

fn boolean_constructor(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let b = args.first().map(|v| v.to_boolean()).unwrap_or(false);

    Ok(Value::boolean(b))
}

// Math object

fn init_math(interp: &mut Interpreter) {
    let mut math = JsObject::new();

    // Constants
    math.define_property(
        PropertyKey::string("E"),
        PropertyDescriptor::data(Value::number(core::f64::consts::E), false, false, false),
    );
    math.define_property(
        PropertyKey::string("PI"),
        PropertyDescriptor::data(Value::number(core::f64::consts::PI), false, false, false),
    );
    math.define_property(
        PropertyKey::string("LN2"),
        PropertyDescriptor::data(Value::number(core::f64::consts::LN_2), false, false, false),
    );
    math.define_property(
        PropertyKey::string("LN10"),
        PropertyDescriptor::data(Value::number(core::f64::consts::LN_10), false, false, false),
    );
    math.define_property(
        PropertyKey::string("LOG2E"),
        PropertyDescriptor::data(
            Value::number(core::f64::consts::LOG2_E),
            false,
            false,
            false,
        ),
    );
    math.define_property(
        PropertyKey::string("LOG10E"),
        PropertyDescriptor::data(
            Value::number(core::f64::consts::LOG10_E),
            false,
            false,
            false,
        ),
    );
    math.define_property(
        PropertyKey::string("SQRT2"),
        PropertyDescriptor::data(
            Value::number(core::f64::consts::SQRT_2),
            false,
            false,
            false,
        ),
    );

    // Functions
    math.define_property(
        PropertyKey::string("abs"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "abs".into(),
                length: 1,
                func: math_abs,
            }))),
            true,
            false,
            true,
        ),
    );

    math.define_property(
        PropertyKey::string("floor"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "floor".into(),
                length: 1,
                func: math_floor,
            }))),
            true,
            false,
            true,
        ),
    );

    math.define_property(
        PropertyKey::string("ceil"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "ceil".into(),
                length: 1,
                func: math_ceil,
            }))),
            true,
            false,
            true,
        ),
    );

    math.define_property(
        PropertyKey::string("round"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "round".into(),
                length: 1,
                func: math_round,
            }))),
            true,
            false,
            true,
        ),
    );

    math.define_property(
        PropertyKey::string("sqrt"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "sqrt".into(),
                length: 1,
                func: math_sqrt,
            }))),
            true,
            false,
            true,
        ),
    );

    math.define_property(
        PropertyKey::string("pow"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "pow".into(),
                length: 2,
                func: math_pow,
            }))),
            true,
            false,
            true,
        ),
    );

    math.define_property(
        PropertyKey::string("min"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "min".into(),
                length: 2,
                func: math_min,
            }))),
            true,
            false,
            true,
        ),
    );

    math.define_property(
        PropertyKey::string("max"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "max".into(),
                length: 2,
                func: math_max,
            }))),
            true,
            false,
            true,
        ),
    );

    math.define_property(
        PropertyKey::string("sin"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "sin".into(),
                length: 1,
                func: math_sin,
            }))),
            true,
            false,
            true,
        ),
    );

    math.define_property(
        PropertyKey::string("cos"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "cos".into(),
                length: 1,
                func: math_cos,
            }))),
            true,
            false,
            true,
        ),
    );

    math.define_property(
        PropertyKey::string("tan"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "tan".into(),
                length: 1,
                func: math_tan,
            }))),
            true,
            false,
            true,
        ),
    );

    math.define_property(
        PropertyKey::string("log"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "log".into(),
                length: 1,
                func: math_log,
            }))),
            true,
            false,
            true,
        ),
    );

    math.define_property(
        PropertyKey::string("exp"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "exp".into(),
                length: 1,
                func: math_exp,
            }))),
            true,
            false,
            true,
        ),
    );

    math.define_property(
        PropertyKey::string("random"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "random".into(),
                length: 0,
                func: math_random,
            }))),
            true,
            false,
            true,
        ),
    );

    interp.define_global("Math", Value::object(math));
}

fn math_abs(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let n = args.first().unwrap_or(&Value::undefined()).to_number()?;
    Ok(Value::number(libm::fabs(n)))
}

fn math_floor(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let n = args.first().unwrap_or(&Value::undefined()).to_number()?;
    Ok(Value::number(libm::floor(n)))
}

fn math_ceil(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let n = args.first().unwrap_or(&Value::undefined()).to_number()?;
    Ok(Value::number(libm::ceil(n)))
}

fn math_round(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let n = args.first().unwrap_or(&Value::undefined()).to_number()?;
    Ok(Value::number(libm::round(n)))
}

fn math_sqrt(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let n = args.first().unwrap_or(&Value::undefined()).to_number()?;
    Ok(Value::number(libm::sqrt(n)))
}

fn math_pow(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let base = args.first().unwrap_or(&Value::undefined()).to_number()?;
    let exp = args.get(1).unwrap_or(&Value::undefined()).to_number()?;
    Ok(Value::number(libm::pow(base, exp)))
}

fn math_min(_this: &Value, args: &[Value]) -> JsResult<Value> {
    if args.is_empty() {
        return Ok(Value::number(f64::INFINITY));
    }

    let mut min = f64::INFINITY;
    for arg in args {
        let n = arg.to_number()?;
        if n.is_nan() {
            return Ok(Value::number(f64::NAN));
        }
        if n < min {
            min = n;
        }
    }

    Ok(Value::number(min))
}

fn math_max(_this: &Value, args: &[Value]) -> JsResult<Value> {
    if args.is_empty() {
        return Ok(Value::number(f64::NEG_INFINITY));
    }

    let mut max = f64::NEG_INFINITY;
    for arg in args {
        let n = arg.to_number()?;
        if n.is_nan() {
            return Ok(Value::number(f64::NAN));
        }
        if n > max {
            max = n;
        }
    }

    Ok(Value::number(max))
}

fn math_sin(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let n = args.first().unwrap_or(&Value::undefined()).to_number()?;
    Ok(Value::number(libm::sin(n)))
}

fn math_cos(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let n = args.first().unwrap_or(&Value::undefined()).to_number()?;
    Ok(Value::number(libm::cos(n)))
}

fn math_tan(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let n = args.first().unwrap_or(&Value::undefined()).to_number()?;
    Ok(Value::number(libm::tan(n)))
}

fn math_log(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let n = args.first().unwrap_or(&Value::undefined()).to_number()?;
    Ok(Value::number(libm::log(n)))
}

fn math_exp(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let n = args.first().unwrap_or(&Value::undefined()).to_number()?;
    Ok(Value::number(libm::exp(n)))
}

fn math_random(_this: &Value, _args: &[Value]) -> JsResult<Value> {
    // Simple pseudo-random number generator
    static mut SEED: u64 = 12345;
    unsafe {
        SEED = SEED.wrapping_mul(6364136223846793005).wrapping_add(1);
        let val = (SEED >> 33) as f64 / (u32::MAX as f64);
        Ok(Value::number(val))
    }
}

// JSON object

fn init_json(interp: &mut Interpreter) {
    let mut json = JsObject::new();

    json.define_property(
        PropertyKey::string("parse"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "parse".into(),
                length: 2,
                func: json_parse,
            }))),
            true,
            false,
            true,
        ),
    );

    json.define_property(
        PropertyKey::string("stringify"),
        PropertyDescriptor::data(
            Value::object(JsObject::function(Callable::Native(NativeFunction {
                name: "stringify".into(),
                length: 3,
                func: json_stringify,
            }))),
            true,
            false,
            true,
        ),
    );

    interp.define_global("JSON", Value::object(json));
}

fn json_parse(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let text = args.first().unwrap_or(&Value::undefined()).to_string()?;

    // Simplified JSON parsing
    let text = text.trim();

    if text == "null" {
        return Ok(Value::null());
    }
    if text == "true" {
        return Ok(Value::boolean(true));
    }
    if text == "false" {
        return Ok(Value::boolean(false));
    }

    // Try as number
    if let Ok(n) = text.parse::<f64>() {
        return Ok(Value::number(n));
    }

    // String
    if text.starts_with('"') && text.ends_with('"') {
        let s = &text[1..text.len() - 1];
        return Ok(Value::string(s.to_string()));
    }

    // For complex JSON, return error
    Err(JsError::syntax("JSON.parse not fully implemented"))
}

fn json_stringify(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let default_val = Value::undefined();
    let value = args.first().unwrap_or(&default_val);

    match value {
        Value::Undefined => Ok(Value::undefined()),
        Value::Null => Ok(Value::string("null")),
        Value::Boolean(b) => Ok(Value::string(if *b { "true" } else { "false" })),
        Value::Number(n) => {
            if n.is_nan() || n.is_infinite() {
                Ok(Value::string("null"))
            } else {
                Ok(Value::string(alloc::format!("{}", n)))
            }
        }
        Value::String(s) => Ok(Value::string(alloc::format!("\"{}\"", s))),
        Value::Object(_) => {
            // Simplified - just return [object Object] for now
            Ok(Value::string("{}"))
        }
        _ => Ok(Value::undefined()),
    }
}

// Error constructors

fn init_error(interp: &mut Interpreter) {
    interp.define_native_function("Error", 1, error_constructor);
    interp.define_native_function("TypeError", 1, type_error_constructor);
    interp.define_native_function("RangeError", 1, range_error_constructor);
    interp.define_native_function("ReferenceError", 1, reference_error_constructor);
    interp.define_native_function("SyntaxError", 1, syntax_error_constructor);
}

fn error_constructor(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let message = args
        .first()
        .map(|v| v.to_string())
        .transpose()?
        .unwrap_or_default();

    Ok(Value::object(JsObject::error("Error".into(), message)))
}

fn type_error_constructor(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let message = args
        .first()
        .map(|v| v.to_string())
        .transpose()?
        .unwrap_or_default();

    Ok(Value::object(JsObject::error("TypeError".into(), message)))
}

fn range_error_constructor(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let message = args
        .first()
        .map(|v| v.to_string())
        .transpose()?
        .unwrap_or_default();

    Ok(Value::object(JsObject::error("RangeError".into(), message)))
}

fn reference_error_constructor(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let message = args
        .first()
        .map(|v| v.to_string())
        .transpose()?
        .unwrap_or_default();

    Ok(Value::object(JsObject::error(
        "ReferenceError".into(),
        message,
    )))
}

fn syntax_error_constructor(_this: &Value, args: &[Value]) -> JsResult<Value> {
    let message = args
        .first()
        .map(|v| v.to_string())
        .transpose()?
        .unwrap_or_default();

    Ok(Value::object(JsObject::error(
        "SyntaxError".into(),
        message,
    )))
}
