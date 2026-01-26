//! JavaScript error types.

use alloc::string::String;
use core::fmt;

/// Result type for JavaScript operations.
pub type JsResult<T> = Result<T, JsError>;

/// JavaScript error types.
#[derive(Debug, Clone)]
pub enum JsError {
    /// Syntax error during parsing.
    SyntaxError(String),
    /// Type error during execution.
    TypeError(String),
    /// Reference error (undefined variable).
    ReferenceError(String),
    /// Range error (invalid array index, etc.).
    RangeError(String),
    /// URI error.
    UriError(String),
    /// Internal error.
    InternalError(String),
    /// Eval error.
    EvalError(String),
    /// Generic error.
    Error(String),
}

impl JsError {
    /// Create a syntax error.
    pub fn syntax<S: Into<String>>(msg: S) -> Self {
        JsError::SyntaxError(msg.into())
    }
    
    /// Create a type error.
    pub fn type_error<S: Into<String>>(msg: S) -> Self {
        JsError::TypeError(msg.into())
    }
    
    /// Create a reference error.
    pub fn reference<S: Into<String>>(msg: S) -> Self {
        JsError::ReferenceError(msg.into())
    }
    
    /// Create a range error.
    pub fn range<S: Into<String>>(msg: S) -> Self {
        JsError::RangeError(msg.into())
    }
    
    /// Create an internal error.
    pub fn internal<S: Into<String>>(msg: S) -> Self {
        JsError::InternalError(msg.into())
    }
    
    /// Create a generic error with custom type name.
    pub fn error<S: Into<String>, M: Into<String>>(_name: S, msg: M) -> Self {
        JsError::Error(msg.into())
    }
    
    /// Get error name.
    pub fn name(&self) -> &'static str {
        match self {
            JsError::SyntaxError(_) => "SyntaxError",
            JsError::TypeError(_) => "TypeError",
            JsError::ReferenceError(_) => "ReferenceError",
            JsError::RangeError(_) => "RangeError",
            JsError::UriError(_) => "URIError",
            JsError::InternalError(_) => "InternalError",
            JsError::EvalError(_) => "EvalError",
            JsError::Error(_) => "Error",
        }
    }
    
    /// Get error message.
    pub fn message(&self) -> &str {
        match self {
            JsError::SyntaxError(msg) => msg,
            JsError::TypeError(msg) => msg,
            JsError::ReferenceError(msg) => msg,
            JsError::RangeError(msg) => msg,
            JsError::UriError(msg) => msg,
            JsError::InternalError(msg) => msg,
            JsError::EvalError(msg) => msg,
            JsError::Error(msg) => msg,
        }
    }
}

impl fmt::Display for JsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name(), self.message())
    }
}
