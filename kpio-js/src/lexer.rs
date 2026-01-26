//! JavaScript lexer (tokenizer).

use alloc::string::String;
use alloc::vec::Vec;
use libm::pow;

use crate::token::{Token, TokenKind, Span};
use crate::error::{JsError, JsResult};

/// JavaScript lexer.
pub struct Lexer<'a> {
    /// Source code.
    source: &'a str,
    /// Source bytes.
    bytes: &'a [u8],
    /// Current position.
    pos: usize,
    /// Current line (1-based).
    line: usize,
    /// Current column (1-based).
    column: usize,
    /// Start of current token.
    token_start: usize,
    /// Start line of current token.
    token_line: usize,
    /// Start column of current token.
    token_column: usize,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer.
    pub fn new(source: &'a str) -> Self {
        Lexer {
            source,
            bytes: source.as_bytes(),
            pos: 0,
            line: 1,
            column: 1,
            token_start: 0,
            token_line: 1,
            token_column: 1,
        }
    }
    
    /// Tokenize the entire source.
    pub fn tokenize(&mut self) -> JsResult<Vec<Token>> {
        let mut tokens = Vec::new();
        
        loop {
            let token = self.next_token()?;
            let is_eof = token.is_eof();
            tokens.push(token);
            if is_eof {
                break;
            }
        }
        
        Ok(tokens)
    }
    
    /// Get the next token.
    pub fn next_token(&mut self) -> JsResult<Token> {
        self.skip_whitespace_and_comments();
        
        self.token_start = self.pos;
        self.token_line = self.line;
        self.token_column = self.column;
        
        if self.is_eof() {
            return Ok(self.make_token(TokenKind::Eof));
        }
        
        let ch = self.current();
        
        // Number
        if ch.is_ascii_digit() || (ch == '.' && self.peek().is_ascii_digit()) {
            return self.scan_number();
        }
        
        // String
        if ch == '"' || ch == '\'' {
            return self.scan_string(ch);
        }
        
        // Template literal
        if ch == '`' {
            return self.scan_template();
        }
        
        // Identifier or keyword
        if is_id_start(ch) {
            return self.scan_identifier();
        }
        
        // Private identifier
        if ch == '#' && is_id_start(self.peek()) {
            return self.scan_private_identifier();
        }
        
        // Punctuators
        self.scan_punctuator()
    }
    
    /// Skip whitespace and comments.
    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while !self.is_eof() && is_whitespace(self.current()) {
                if self.current() == '\n' {
                    self.line += 1;
                    self.column = 1;
                } else {
                    self.column += 1;
                }
                self.pos += 1;
            }
            
            // Skip single-line comments
            if self.current() == '/' && self.peek() == '/' {
                while !self.is_eof() && self.current() != '\n' {
                    self.advance();
                }
                continue;
            }
            
            // Skip multi-line comments
            if self.current() == '/' && self.peek() == '*' {
                self.advance(); // /
                self.advance(); // *
                while !self.is_eof() {
                    if self.current() == '*' && self.peek() == '/' {
                        self.advance(); // *
                        self.advance(); // /
                        break;
                    }
                    if self.current() == '\n' {
                        self.line += 1;
                        self.column = 1;
                        self.pos += 1;
                    } else {
                        self.advance();
                    }
                }
                continue;
            }
            
            break;
        }
    }
    
    /// Scan a number literal.
    fn scan_number(&mut self) -> JsResult<Token> {
        let start = self.pos;
        
        // Check for hex, octal, binary
        if self.current() == '0' {
            self.advance();
            match self.current() {
                'x' | 'X' => {
                    self.advance();
                    while !self.is_eof() && self.current().is_ascii_hexdigit() {
                        self.advance();
                    }
                    let hex_str = &self.source[start + 2..self.pos];
                    let value = u64::from_str_radix(hex_str, 16)
                        .map_err(|_| JsError::syntax("Invalid hex number"))? as f64;
                    return Ok(self.make_token(TokenKind::Number(value)));
                }
                'o' | 'O' => {
                    self.advance();
                    while !self.is_eof() && matches!(self.current(), '0'..='7') {
                        self.advance();
                    }
                    let oct_str = &self.source[start + 2..self.pos];
                    let value = u64::from_str_radix(oct_str, 8)
                        .map_err(|_| JsError::syntax("Invalid octal number"))? as f64;
                    return Ok(self.make_token(TokenKind::Number(value)));
                }
                'b' | 'B' => {
                    self.advance();
                    while !self.is_eof() && matches!(self.current(), '0' | '1') {
                        self.advance();
                    }
                    let bin_str = &self.source[start + 2..self.pos];
                    let value = u64::from_str_radix(bin_str, 2)
                        .map_err(|_| JsError::syntax("Invalid binary number"))? as f64;
                    return Ok(self.make_token(TokenKind::Number(value)));
                }
                _ => {}
            }
        }
        
        // Decimal integer part
        while !self.is_eof() && self.current().is_ascii_digit() {
            self.advance();
        }
        
        // Fractional part
        if self.current() == '.' && self.peek().is_ascii_digit() {
            self.advance(); // .
            while !self.is_eof() && self.current().is_ascii_digit() {
                self.advance();
            }
        }
        
        // Exponent
        if matches!(self.current(), 'e' | 'E') {
            self.advance();
            if matches!(self.current(), '+' | '-') {
                self.advance();
            }
            while !self.is_eof() && self.current().is_ascii_digit() {
                self.advance();
            }
        }
        
        // BigInt suffix
        if self.current() == 'n' {
            self.advance();
            let bigint_str = String::from(&self.source[start..self.pos - 1]);
            return Ok(self.make_token(TokenKind::BigInt(bigint_str)));
        }
        
        let num_str = &self.source[start..self.pos];
        let value = parse_float(num_str).unwrap_or(f64::NAN);
        Ok(self.make_token(TokenKind::Number(value)))
    }
    
    /// Scan a string literal.
    fn scan_string(&mut self, quote: char) -> JsResult<Token> {
        self.advance(); // Opening quote
        let mut value = String::new();
        
        while !self.is_eof() && self.current() != quote {
            if self.current() == '\\' {
                self.advance();
                match self.current() {
                    'n' => { value.push('\n'); self.advance(); }
                    'r' => { value.push('\r'); self.advance(); }
                    't' => { value.push('\t'); self.advance(); }
                    '\\' => { value.push('\\'); self.advance(); }
                    '\'' => { value.push('\''); self.advance(); }
                    '"' => { value.push('"'); self.advance(); }
                    '0' => { value.push('\0'); self.advance(); }
                    'x' => {
                        self.advance();
                        let hex = self.scan_hex_digits(2)?;
                        if let Some(ch) = char::from_u32(hex) {
                            value.push(ch);
                        }
                    }
                    'u' => {
                        self.advance();
                        if self.current() == '{' {
                            self.advance();
                            let mut hex_str = String::new();
                            while !self.is_eof() && self.current() != '}' {
                                hex_str.push(self.current());
                                self.advance();
                            }
                            self.advance(); // }
                            let code = u32::from_str_radix(&hex_str, 16)
                                .map_err(|_| JsError::syntax("Invalid unicode escape"))?;
                            if let Some(ch) = char::from_u32(code) {
                                value.push(ch);
                            }
                        } else {
                            let hex = self.scan_hex_digits(4)?;
                            if let Some(ch) = char::from_u32(hex) {
                                value.push(ch);
                            }
                        }
                    }
                    '\n' => {
                        self.line += 1;
                        self.column = 1;
                        self.pos += 1;
                    }
                    ch => {
                        value.push(ch);
                        self.advance();
                    }
                }
            } else if self.current() == '\n' {
                return Err(JsError::syntax("Unterminated string literal"));
            } else {
                value.push(self.current());
                self.advance();
            }
        }
        
        if self.is_eof() {
            return Err(JsError::syntax("Unterminated string literal"));
        }
        
        self.advance(); // Closing quote
        Ok(self.make_token(TokenKind::String(value)))
    }
    
    /// Scan a template literal.
    fn scan_template(&mut self) -> JsResult<Token> {
        self.advance(); // `
        let mut value = String::new();
        
        while !self.is_eof() && self.current() != '`' {
            if self.current() == '$' && self.peek() == '{' {
                // TODO: Handle template expressions
                value.push(self.current());
                self.advance();
            } else if self.current() == '\\' {
                self.advance();
                match self.current() {
                    'n' => { value.push('\n'); self.advance(); }
                    't' => { value.push('\t'); self.advance(); }
                    '`' => { value.push('`'); self.advance(); }
                    '$' => { value.push('$'); self.advance(); }
                    '\\' => { value.push('\\'); self.advance(); }
                    ch => { value.push(ch); self.advance(); }
                }
            } else {
                if self.current() == '\n' {
                    self.line += 1;
                    self.column = 0;
                }
                value.push(self.current());
                self.advance();
            }
        }
        
        if self.is_eof() {
            return Err(JsError::syntax("Unterminated template literal"));
        }
        
        self.advance(); // `
        Ok(self.make_token(TokenKind::Template(value)))
    }
    
    /// Scan an identifier or keyword.
    fn scan_identifier(&mut self) -> JsResult<Token> {
        let start = self.pos;
        
        while !self.is_eof() && is_id_continue(self.current()) {
            self.advance();
        }
        
        let text = &self.source[start..self.pos];
        let kind = TokenKind::keyword_from_str(text)
            .unwrap_or_else(|| TokenKind::Identifier(String::from(text)));
        
        Ok(self.make_token(kind))
    }
    
    /// Scan a private identifier.
    fn scan_private_identifier(&mut self) -> JsResult<Token> {
        self.advance(); // #
        let start = self.pos;
        
        while !self.is_eof() && is_id_continue(self.current()) {
            self.advance();
        }
        
        let text = String::from(&self.source[start..self.pos]);
        Ok(self.make_token(TokenKind::PrivateIdentifier(text)))
    }
    
    /// Scan a punctuator.
    fn scan_punctuator(&mut self) -> JsResult<Token> {
        let ch = self.current();
        self.advance();
        
        let kind = match ch {
            '{' => TokenKind::LeftBrace,
            '}' => TokenKind::RightBrace,
            '(' => TokenKind::LeftParen,
            ')' => TokenKind::RightParen,
            '[' => TokenKind::LeftBracket,
            ']' => TokenKind::RightBracket,
            ';' => TokenKind::Semicolon,
            ',' => TokenKind::Comma,
            ':' => TokenKind::Colon,
            '~' => TokenKind::Tilde,
            
            '.' => {
                if self.current() == '.' && self.peek() == '.' {
                    self.advance();
                    self.advance();
                    TokenKind::Ellipsis
                } else {
                    TokenKind::Dot
                }
            }
            
            '?' => {
                if self.current() == '?' {
                    self.advance();
                    if self.current() == '=' {
                        self.advance();
                        TokenKind::QuestionQuestionAssign
                    } else {
                        TokenKind::QuestionQuestion
                    }
                } else if self.current() == '.' {
                    self.advance();
                    TokenKind::QuestionDot
                } else {
                    TokenKind::Question
                }
            }
            
            '<' => {
                if self.current() == '=' {
                    self.advance();
                    TokenKind::LessEqual
                } else if self.current() == '<' {
                    self.advance();
                    if self.current() == '=' {
                        self.advance();
                        TokenKind::LeftShiftAssign
                    } else {
                        TokenKind::LeftShift
                    }
                } else {
                    TokenKind::LessThan
                }
            }
            
            '>' => {
                if self.current() == '=' {
                    self.advance();
                    TokenKind::GreaterEqual
                } else if self.current() == '>' {
                    self.advance();
                    if self.current() == '>' {
                        self.advance();
                        if self.current() == '=' {
                            self.advance();
                            TokenKind::UnsignedRightShiftAssign
                        } else {
                            TokenKind::UnsignedRightShift
                        }
                    } else if self.current() == '=' {
                        self.advance();
                        TokenKind::RightShiftAssign
                    } else {
                        TokenKind::RightShift
                    }
                } else {
                    TokenKind::GreaterThan
                }
            }
            
            '=' => {
                if self.current() == '=' {
                    self.advance();
                    if self.current() == '=' {
                        self.advance();
                        TokenKind::StrictEqual
                    } else {
                        TokenKind::Equal
                    }
                } else if self.current() == '>' {
                    self.advance();
                    TokenKind::Arrow
                } else {
                    TokenKind::Assign
                }
            }
            
            '!' => {
                if self.current() == '=' {
                    self.advance();
                    if self.current() == '=' {
                        self.advance();
                        TokenKind::StrictNotEqual
                    } else {
                        TokenKind::NotEqual
                    }
                } else {
                    TokenKind::Bang
                }
            }
            
            '+' => {
                if self.current() == '+' {
                    self.advance();
                    TokenKind::PlusPlus
                } else if self.current() == '=' {
                    self.advance();
                    TokenKind::PlusAssign
                } else {
                    TokenKind::Plus
                }
            }
            
            '-' => {
                if self.current() == '-' {
                    self.advance();
                    TokenKind::MinusMinus
                } else if self.current() == '=' {
                    self.advance();
                    TokenKind::MinusAssign
                } else {
                    TokenKind::Minus
                }
            }
            
            '*' => {
                if self.current() == '*' {
                    self.advance();
                    if self.current() == '=' {
                        self.advance();
                        TokenKind::StarStarAssign
                    } else {
                        TokenKind::StarStar
                    }
                } else if self.current() == '=' {
                    self.advance();
                    TokenKind::StarAssign
                } else {
                    TokenKind::Star
                }
            }
            
            '/' => {
                if self.current() == '=' {
                    self.advance();
                    TokenKind::SlashAssign
                } else {
                    TokenKind::Slash
                }
            }
            
            '%' => {
                if self.current() == '=' {
                    self.advance();
                    TokenKind::PercentAssign
                } else {
                    TokenKind::Percent
                }
            }
            
            '&' => {
                if self.current() == '&' {
                    self.advance();
                    if self.current() == '=' {
                        self.advance();
                        TokenKind::AmpersandAmpersandAssign
                    } else {
                        TokenKind::AmpersandAmpersand
                    }
                } else if self.current() == '=' {
                    self.advance();
                    TokenKind::AmpersandAssign
                } else {
                    TokenKind::Ampersand
                }
            }
            
            '|' => {
                if self.current() == '|' {
                    self.advance();
                    if self.current() == '=' {
                        self.advance();
                        TokenKind::PipePipeAssign
                    } else {
                        TokenKind::PipePipe
                    }
                } else if self.current() == '=' {
                    self.advance();
                    TokenKind::PipeAssign
                } else {
                    TokenKind::Pipe
                }
            }
            
            '^' => {
                if self.current() == '=' {
                    self.advance();
                    TokenKind::CaretAssign
                } else {
                    TokenKind::Caret
                }
            }
            
            _ => TokenKind::Invalid,
        };
        
        Ok(self.make_token(kind))
    }
    
    /// Scan hex digits.
    fn scan_hex_digits(&mut self, count: usize) -> JsResult<u32> {
        let mut value: u32 = 0;
        for _ in 0..count {
            if !self.current().is_ascii_hexdigit() {
                return Err(JsError::syntax("Invalid hex escape"));
            }
            value = value * 16 + self.current().to_digit(16).unwrap();
            self.advance();
        }
        Ok(value)
    }
    
    // Helper methods
    
    fn is_eof(&self) -> bool {
        self.pos >= self.bytes.len()
    }
    
    fn current(&self) -> char {
        if self.is_eof() {
            '\0'
        } else {
            self.bytes[self.pos] as char
        }
    }
    
    fn peek(&self) -> char {
        if self.pos + 1 >= self.bytes.len() {
            '\0'
        } else {
            self.bytes[self.pos + 1] as char
        }
    }
    
    fn advance(&mut self) {
        if !self.is_eof() {
            self.pos += 1;
            self.column += 1;
        }
    }
    
    fn make_token(&self, kind: TokenKind) -> Token {
        Token::new(
            kind,
            Span::new(self.token_start, self.pos, self.token_line, self.token_column),
        )
    }
}

/// Check if character is whitespace.
fn is_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | '\x0B' | '\x0C')
}

/// Check if character can start an identifier.
fn is_id_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch == '$'
}

/// Check if character can continue an identifier.
fn is_id_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'
}

/// Parse a float from string (simple implementation).
fn parse_float(s: &str) -> Option<f64> {
    // Simple float parser for no_std
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    
    let mut result: f64 = 0.0;
    let mut fraction: f64 = 0.0;
    let mut fraction_digits = 0;
    let mut exponent: i32 = 0;
    let mut exp_sign = 1;
    let mut in_fraction = false;
    let mut in_exponent = false;
    let mut negative = false;
    
    let mut chars = s.chars().peekable();
    
    // Sign
    if chars.peek() == Some(&'-') {
        negative = true;
        chars.next();
    } else if chars.peek() == Some(&'+') {
        chars.next();
    }
    
    for ch in chars {
        if ch == '.' && !in_fraction && !in_exponent {
            in_fraction = true;
        } else if (ch == 'e' || ch == 'E') && !in_exponent {
            in_exponent = true;
        } else if in_exponent {
            if ch == '-' {
                exp_sign = -1;
            } else if ch == '+' {
                // skip
            } else if let Some(d) = ch.to_digit(10) {
                exponent = exponent * 10 + d as i32;
            }
        } else if let Some(d) = ch.to_digit(10) {
            if in_fraction {
                fraction = fraction * 10.0 + d as f64;
                fraction_digits += 1;
            } else {
                result = result * 10.0 + d as f64;
            }
        }
    }
    
    // Combine parts
    if fraction_digits > 0 {
        let mut divisor = 1.0f64;
        for _ in 0..fraction_digits {
            divisor *= 10.0;
        }
        result += fraction / divisor;
    }
    
    // Apply exponent
    if in_exponent {
        let exp_value = (exp_sign * exponent) as f64;
        result *= pow(10.0, exp_value);
    }
    
    if negative {
        result = -result;
    }
    
    Some(result)
}
