//! JavaScript Parser Fuzzing
//!
//! Fuzzing harness for JavaScript parsing and execution.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use crate::{FuzzTarget, FuzzResult};

/// JavaScript Parser fuzzer
pub struct JsFuzzer {
    /// Maximum script size
    max_size: usize,
    /// Maximum AST depth
    max_depth: usize,
    /// Enable strict mode
    strict_mode: bool,
    /// Enable module mode
    module_mode: bool,
}

impl JsFuzzer {
    /// Create new JavaScript fuzzer
    pub fn new() -> Self {
        Self {
            max_size: 1024 * 1024,
            max_depth: 256,
            strict_mode: false,
            module_mode: false,
        }
    }

    /// Enable strict mode
    pub fn strict(mut self) -> Self {
        self.strict_mode = true;
        self
    }

    /// Enable module mode
    pub fn module(mut self) -> Self {
        self.module_mode = true;
        self
    }

    /// Parse JavaScript input
    fn parse(&self, input: &[u8]) -> Result<(), JsParseError> {
        if input.len() > self.max_size {
            return Err(JsParseError::TooLarge);
        }

        let text = match core::str::from_utf8(input) {
            Ok(s) => s,
            Err(_) => return Err(JsParseError::InvalidEncoding),
        };

        // Check balanced delimiters
        let mut brace_depth = 0i32;
        let mut paren_depth = 0i32;
        let mut bracket_depth = 0i32;
        let mut in_string = false;
        let mut in_template = false;
        let mut in_regex = false;
        let mut in_comment = false;
        let mut in_line_comment = false;
        let mut string_char = ' ';
        let mut chars = text.chars().peekable();
        let mut last_char = ' ';

        while let Some(ch) = chars.next() {
            if in_line_comment {
                if ch == '\n' {
                    in_line_comment = false;
                }
                continue;
            }

            if in_comment {
                if ch == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    in_comment = false;
                }
                continue;
            }

            if in_string {
                if ch == string_char && last_char != '\\' {
                    in_string = false;
                }
                last_char = ch;
                continue;
            }

            if in_template {
                if ch == '`' && last_char != '\\' {
                    in_template = false;
                } else if ch == '$' && chars.peek() == Some(&'{') {
                    chars.next();
                    brace_depth += 1;
                }
                last_char = ch;
                continue;
            }

            if in_regex {
                if ch == '/' && last_char != '\\' {
                    in_regex = false;
                }
                last_char = ch;
                continue;
            }

            match ch {
                '/' => {
                    if let Some(&next) = chars.peek() {
                        if next == '/' {
                            chars.next();
                            in_line_comment = true;
                        } else if next == '*' {
                            chars.next();
                            in_comment = true;
                        } else if !last_char.is_alphanumeric() && last_char != ')' && last_char != ']' {
                            in_regex = true;
                        }
                    }
                }
                '"' | '\'' => {
                    in_string = true;
                    string_char = ch;
                }
                '`' => {
                    in_template = true;
                }
                '{' => {
                    brace_depth += 1;
                    if brace_depth > self.max_depth as i32 {
                        return Err(JsParseError::TooDeep);
                    }
                }
                '}' => {
                    brace_depth -= 1;
                    if brace_depth < 0 {
                        return Err(JsParseError::UnbalancedBraces);
                    }
                }
                '(' => {
                    paren_depth += 1;
                    if paren_depth > self.max_depth as i32 {
                        return Err(JsParseError::TooDeep);
                    }
                }
                ')' => {
                    paren_depth -= 1;
                    if paren_depth < 0 {
                        return Err(JsParseError::UnbalancedParens);
                    }
                }
                '[' => {
                    bracket_depth += 1;
                }
                ']' => {
                    bracket_depth -= 1;
                    if bracket_depth < 0 {
                        return Err(JsParseError::UnbalancedBrackets);
                    }
                }
                _ => {}
            }

            if !ch.is_whitespace() {
                last_char = ch;
            }
        }

        if in_string {
            return Err(JsParseError::UnterminatedString);
        }
        if in_template {
            return Err(JsParseError::UnterminatedTemplate);
        }
        if in_comment {
            return Err(JsParseError::UnterminatedComment);
        }
        if brace_depth != 0 {
            return Err(JsParseError::UnbalancedBraces);
        }
        if paren_depth != 0 {
            return Err(JsParseError::UnbalancedParens);
        }
        if bracket_depth != 0 {
            return Err(JsParseError::UnbalancedBrackets);
        }

        Ok(())
    }
}

impl Default for JsFuzzer {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzTarget for JsFuzzer {
    fn name(&self) -> &str {
        "js_parser"
    }

    fn fuzz(&mut self, input: &[u8]) -> FuzzResult {
        match self.parse(input) {
            Ok(()) => FuzzResult::Ok,
            Err(e) => FuzzResult::ParseError(e.message()),
        }
    }

    fn reset(&mut self) {
        // No state
    }
}

/// JavaScript parsing error
#[derive(Debug)]
enum JsParseError {
    /// Too large
    TooLarge,
    /// Invalid encoding
    InvalidEncoding,
    /// Nesting too deep
    TooDeep,
    /// Unbalanced braces
    UnbalancedBraces,
    /// Unbalanced parentheses
    UnbalancedParens,
    /// Unbalanced brackets
    UnbalancedBrackets,
    /// Unterminated string
    UnterminatedString,
    /// Unterminated template
    UnterminatedTemplate,
    /// Unterminated comment
    UnterminatedComment,
    /// Invalid syntax
    InvalidSyntax,
}

impl JsParseError {
    fn message(&self) -> String {
        match self {
            Self::TooLarge => String::from("Script too large"),
            Self::InvalidEncoding => String::from("Invalid encoding"),
            Self::TooDeep => String::from("Nesting too deep"),
            Self::UnbalancedBraces => String::from("Unbalanced braces"),
            Self::UnbalancedParens => String::from("Unbalanced parentheses"),
            Self::UnbalancedBrackets => String::from("Unbalanced brackets"),
            Self::UnterminatedString => String::from("Unterminated string"),
            Self::UnterminatedTemplate => String::from("Unterminated template literal"),
            Self::UnterminatedComment => String::from("Unterminated comment"),
            Self::InvalidSyntax => String::from("Invalid syntax"),
        }
    }
}

/// Create JavaScript dictionary for fuzzing
pub fn js_dictionary() -> Vec<Vec<u8>> {
    let entries: &[&[u8]] = &[
        // Keywords
        b"var",
        b"let",
        b"const",
        b"function",
        b"class",
        b"extends",
        b"return",
        b"if",
        b"else",
        b"for",
        b"while",
        b"do",
        b"switch",
        b"case",
        b"break",
        b"continue",
        b"try",
        b"catch",
        b"finally",
        b"throw",
        b"async",
        b"await",
        b"yield",
        b"import",
        b"export",
        b"default",
        b"new",
        b"this",
        b"super",
        b"typeof",
        b"instanceof",
        b"in",
        b"of",
        b"delete",
        b"void",
        // Operators
        b"=>",
        b"...",
        b"?.?",
        b"??",
        b"**",
        b"&&",
        b"||",
        b"===",
        b"!==",
        b"++",
        b"--",
        b"<<",
        b">>",
        b">>>",
        // Built-ins
        b"undefined",
        b"null",
        b"true",
        b"false",
        b"NaN",
        b"Infinity",
        b"globalThis",
        b"console",
        b"Object",
        b"Array",
        b"String",
        b"Number",
        b"Boolean",
        b"Symbol",
        b"BigInt",
        b"Function",
        b"Promise",
        b"Proxy",
        b"Reflect",
        b"Map",
        b"Set",
        b"WeakMap",
        b"WeakSet",
        b"RegExp",
        b"Date",
        b"Error",
        b"JSON",
        b"Math",
        b"Intl",
        b"ArrayBuffer",
        b"DataView",
        b"Uint8Array",
        // Methods
        b".prototype",
        b".constructor",
        b".__proto__",
        b".toString()",
        b".valueOf()",
        b".hasOwnProperty",
        b".call(",
        b".apply(",
        b".bind(",
        // Edge cases
        b"eval(",
        b"Function(",
        b"with(",
        b"arguments",
        b"caller",
        b"callee",
        // Template literals
        b"`",
        b"${",
        b"}",
        // Comments
        b"//",
        b"/*",
        b"*/",
        // Regex
        b"/regex/",
        b"/./g",
        b"/(?:)/",
        // Unicode
        b"\\u0000",
        b"\\x00",
        b"\\0",
    ];

    entries.iter().map(|e| e.to_vec()).collect()
}

/// Generate interesting JavaScript inputs
pub fn generate_js_corpus() -> Vec<Vec<u8>> {
    let mut corpus = Vec::new();

    // Simple expressions
    corpus.push(b"1 + 2".to_vec());
    corpus.push(b"'hello'".to_vec());
    corpus.push(b"true".to_vec());

    // Empty
    corpus.push(Vec::new());

    // Functions
    corpus.push(b"function foo() { return 42; }".to_vec());
    corpus.push(b"const foo = () => 42".to_vec());
    corpus.push(b"async function bar() { await Promise.resolve(); }".to_vec());
    corpus.push(b"function* gen() { yield 1; yield 2; }".to_vec());

    // Classes
    corpus.push(b"class Foo { constructor() {} method() {} }".to_vec());
    corpus.push(b"class Bar extends Foo { constructor() { super(); } }".to_vec());

    // Destructuring
    corpus.push(b"const { a, b: c, ...rest } = obj".to_vec());
    corpus.push(b"const [x, y, ...z] = arr".to_vec());

    // Template literals
    corpus.push(b"`hello ${world}`".to_vec());
    corpus.push(b"`nested ${`template ${x}`}`".to_vec());

    // Regex
    corpus.push(b"/test/gi".to_vec());
    corpus.push(b"/^[a-z]+$/i".to_vec());
    corpus.push(b"/(a+)+$/".to_vec()); // ReDoS pattern

    // Deep nesting
    let mut deep = b"function f(){".to_vec();
    for _ in 0..50 {
        deep.extend_from_slice(b"if(true){");
    }
    for _ in 0..50 {
        deep.extend_from_slice(b"}");
    }
    deep.extend_from_slice(b"}");
    corpus.push(deep);

    // Many parameters
    let mut many_params = b"function f(".to_vec();
    for i in 0..100 {
        if i > 0 {
            many_params.extend_from_slice(b",");
        }
        many_params.extend_from_slice(format!("p{}", i).as_bytes());
    }
    many_params.extend_from_slice(b"){}");
    corpus.push(many_params);

    // Malformed
    corpus.push(b"function { }".to_vec());
    corpus.push(b"if then else".to_vec());
    corpus.push(b"{{{{}}}}".to_vec());

    // Strict mode violations
    corpus.push(b"'use strict'; with({}){}".to_vec());
    corpus.push(b"'use strict'; arguments = 1".to_vec());

    // eval and with
    corpus.push(b"eval('code')".to_vec());
    corpus.push(b"with(obj) { x }".to_vec());

    // Prototype pollution patterns
    corpus.push(b"obj.__proto__.polluted = true".to_vec());
    corpus.push(b"Object.prototype.foo = 'bar'".to_vec());
    corpus.push(b"obj['__proto__']['x'] = 1".to_vec());

    // Unicode
    corpus.push("const ಠ_ಠ = 'disapproval'".as_bytes().to_vec());
    corpus.push("const 変数 = 42".as_bytes().to_vec());

    // Edge case numbers
    corpus.push(b"0x1fffffffffffff".to_vec());
    corpus.push(b"9007199254740993n".to_vec());
    corpus.push(b"1e309".to_vec());

    // Modules
    corpus.push(b"import { foo } from 'bar'".to_vec());
    corpus.push(b"export default function() {}".to_vec());
    corpus.push(b"import * as mod from 'module'".to_vec());

    // Dynamic import
    corpus.push(b"import('module').then(m => m.default)".to_vec());

    // Private fields
    corpus.push(b"class C { #private = 1; get() { return this.#private; } }".to_vec());

    corpus
}

/// JavaScript execution fuzzer
pub struct JsExecutionFuzzer {
    /// Execution timeout
    timeout_ms: u64,
    /// Maximum memory
    max_memory: usize,
    /// Maximum call stack
    max_stack: usize,
}

impl JsExecutionFuzzer {
    /// Create new execution fuzzer
    pub fn new() -> Self {
        Self {
            timeout_ms: 1000,
            max_memory: 64 * 1024 * 1024,
            max_stack: 1000,
        }
    }
}

impl Default for JsExecutionFuzzer {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzTarget for JsExecutionFuzzer {
    fn name(&self) -> &str {
        "js_execution"
    }

    fn fuzz(&mut self, input: &[u8]) -> FuzzResult {
        let text = match core::str::from_utf8(input) {
            Ok(s) => s,
            Err(_) => return FuzzResult::ParseError(String::from("invalid utf8")),
        };

        // Check for infinite loop patterns
        if text.contains("while(true)") || text.contains("for(;;)") {
            return FuzzResult::Interesting(String::from("potential infinite loop"));
        }

        // Check for recursion bombs
        if text.contains("f(f)") || (text.contains("function f") && text.matches("f()").count() > 5) {
            return FuzzResult::Interesting(String::from("potential recursion bomb"));
        }

        // Check for memory bombs
        if text.contains("Array(1e9)") || text.contains("'x'.repeat(1e9)") {
            return FuzzResult::Interesting(String::from("potential memory bomb"));
        }

        FuzzResult::Ok
    }

    fn reset(&mut self) {}
}

/// Regex fuzzer (for ReDoS detection)
pub struct RegexFuzzer {
    /// Maximum pattern length
    max_length: usize,
}

impl RegexFuzzer {
    /// Create new regex fuzzer
    pub fn new() -> Self {
        Self { max_length: 1000 }
    }
}

impl Default for RegexFuzzer {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzTarget for RegexFuzzer {
    fn name(&self) -> &str {
        "js_regex"
    }

    fn fuzz(&mut self, input: &[u8]) -> FuzzResult {
        let text = match core::str::from_utf8(input) {
            Ok(s) => s,
            Err(_) => return FuzzResult::ParseError(String::from("invalid utf8")),
        };

        // Check for ReDoS patterns
        // Patterns like (a+)+, (a|a)+, (a|b|a)+ can cause catastrophic backtracking
        if text.contains("(a+)+") 
            || text.contains("(a*)*")
            || text.contains("(a|a)+")
            || text.contains("(.*)+") {
            return FuzzResult::Interesting(String::from("potential ReDoS pattern"));
        }

        FuzzResult::Ok
    }

    fn reset(&mut self) {}
}
