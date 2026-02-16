//! KPIO JavaScript Engine
//!
//! A lightweight JavaScript engine for the KPIO operating system.
//! Implements ECMAScript 2020 (ES11) subset suitable for web browsers.
//!
//! # Architecture
//!
//! The engine is organized into:
//!
//! - `lexer`: Tokenization of JavaScript source code
//! - `parser`: Parsing tokens into an Abstract Syntax Tree (AST)
//! - `ast`: AST node definitions
//! - `interpreter`: Tree-walking interpreter for execution
//! - `value`: JavaScript value representation
//! - `object`: Object and property handling
//! - `builtin`: Built-in objects and functions
//! - `gc`: Simple mark-and-sweep garbage collector
//! - `dom`: DOM binding interface for browser integration
//!
//! # Usage
//!
//! ```ignore
//! use kpio_js::{Engine, Value};
//!
//! let mut engine = Engine::new();
//! let result = engine.eval("1 + 2 * 3")?;
//! assert_eq!(result, Value::Number(7.0));
//! ```

#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;

pub mod ast;
pub mod builtin;
pub mod dom;
pub mod error;
pub mod gc;
pub mod interpreter;
pub mod lexer;
pub mod object;
pub mod parser;
pub mod token;
pub mod value;

use alloc::string::String;

pub use error::{JsError, JsResult};
pub use interpreter::Engine;
pub use value::Value;

/// JavaScript engine version.
pub const VERSION: &str = "0.1.0";

/// ECMAScript version supported.
pub const ECMA_VERSION: u32 = 11; // ES2020

/// Initialize the JavaScript engine.
pub fn init() -> Engine {
    Engine::new()
}

/// Quick evaluation of a JavaScript expression.
pub fn eval(source: &str) -> JsResult<Value> {
    let mut engine = Engine::new();
    engine.eval(source)
}

/// Quick evaluation with a custom context.
pub fn eval_with_globals(source: &str, globals: &[(String, Value)]) -> JsResult<Value> {
    let mut engine = Engine::new();
    for (name, value) in globals {
        engine.set_global(name.clone(), value.clone());
    }
    engine.eval(source)
}
