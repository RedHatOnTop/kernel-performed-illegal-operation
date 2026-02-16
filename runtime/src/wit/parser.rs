//! Minimal WIT text format parser.
//!
//! Parses a subset of the WIT IDL that is sufficient for KPIO's
//! `kpio:gui`, `kpio:system`, and `kpio:net` interface definitions.
//!
//! Grammar subset handled:
//!
//! ```text
//! package <ns>:<name>[@<ver>]
//! interface <name> { <items> }
//! world <name> { import/export <ref> }
//! record <name> { <fields> }
//! enum <name> { <cases> }
//! flags <name> { <flags> }
//! variant <name> { <cases> }
//! type <name> = <ref>
//! <name>: func(<params>) -> <results>
//! use <path>.{<names>}
//! ```

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use super::types::*;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitParseError {
    pub message: String,
    pub line: usize,
}

impl WitParseError {
    fn new(msg: &str, line: usize) -> Self {
        WitParseError {
            message: String::from(msg),
            line,
        }
    }
}

// ---------------------------------------------------------------------------
// Tokenizer (simplified)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Ident(String),
    Colon,
    Semicolon,
    Comma,
    LBrace,
    RBrace,
    LParen,
    RParen,
    Arrow, // ->
    Eq,    // =
    Dot,
    At,
    Star,
    Lt,
    Gt,
    Eof,
}

struct Tokenizer<'a> {
    input: &'a [u8],
    pos: usize,
    line: usize,
}

impl<'a> Tokenizer<'a> {
    fn new(input: &'a str) -> Self {
        Tokenizer {
            input: input.as_bytes(),
            pos: 0,
            line: 1,
        }
    }

    fn skip_ws_and_comments(&mut self) {
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            if ch == b'\n' {
                self.line += 1;
                self.pos += 1;
            } else if ch == b' ' || ch == b'\t' || ch == b'\r' {
                self.pos += 1;
            } else if ch == b'/'
                && self.pos + 1 < self.input.len()
                && self.input[self.pos + 1] == b'/'
            {
                // Line comment
                while self.pos < self.input.len() && self.input[self.pos] != b'\n' {
                    self.pos += 1;
                }
            } else if ch == b'/'
                && self.pos + 1 < self.input.len()
                && self.input[self.pos + 1] == b'*'
            {
                // Block comment
                self.pos += 2;
                while self.pos + 1 < self.input.len() {
                    if self.input[self.pos] == b'\n' {
                        self.line += 1;
                    }
                    if self.input[self.pos] == b'*' && self.input[self.pos + 1] == b'/' {
                        self.pos += 2;
                        break;
                    }
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn next(&mut self) -> Token {
        self.skip_ws_and_comments();
        if self.pos >= self.input.len() {
            return Token::Eof;
        }
        let ch = self.input[self.pos];
        match ch {
            b':' => {
                self.pos += 1;
                Token::Colon
            }
            b';' => {
                self.pos += 1;
                Token::Semicolon
            }
            b',' => {
                self.pos += 1;
                Token::Comma
            }
            b'{' => {
                self.pos += 1;
                Token::LBrace
            }
            b'}' => {
                self.pos += 1;
                Token::RBrace
            }
            b'(' => {
                self.pos += 1;
                Token::LParen
            }
            b')' => {
                self.pos += 1;
                Token::RParen
            }
            b'=' => {
                self.pos += 1;
                Token::Eq
            }
            b'.' => {
                self.pos += 1;
                Token::Dot
            }
            b'@' => {
                self.pos += 1;
                Token::At
            }
            b'*' => {
                self.pos += 1;
                Token::Star
            }
            b'<' => {
                self.pos += 1;
                Token::Lt
            }
            b'>' => {
                self.pos += 1;
                Token::Gt
            }
            b'-' if self.pos + 1 < self.input.len() && self.input[self.pos + 1] == b'>' => {
                self.pos += 2;
                Token::Arrow
            }
            _ if ch.is_ascii_alphabetic() || ch == b'_' || ch == b'%' => {
                let start = self.pos;
                self.pos += 1;
                while self.pos < self.input.len() {
                    let c = self.input[self.pos];
                    if c.is_ascii_alphanumeric() || c == b'_' || c == b'-' {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                let s = core::str::from_utf8(&self.input[start..self.pos]).unwrap_or("");
                Token::Ident(String::from(s))
            }
            _ if ch.is_ascii_digit() => {
                // Version number segments etc.
                let start = self.pos;
                while self.pos < self.input.len()
                    && (self.input[self.pos].is_ascii_alphanumeric()
                        || self.input[self.pos] == b'.'
                        || self.input[self.pos] == b'-')
                {
                    self.pos += 1;
                }
                let s = core::str::from_utf8(&self.input[start..self.pos]).unwrap_or("");
                Token::Ident(String::from(s))
            }
            _ => {
                self.pos += 1;
                // Skip unknown char, retry
                self.next()
            }
        }
    }

    fn peek(&mut self) -> Token {
        let save_pos = self.pos;
        let save_line = self.line;
        let tok = self.next();
        self.pos = save_pos;
        self.line = save_line;
        tok
    }

    fn expect_ident(&mut self) -> Result<String, WitParseError> {
        match self.next() {
            Token::Ident(s) => Ok(s),
            other => Err(WitParseError::new(
                "expected identifier",
                self.line,
            )),
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), WitParseError> {
        let tok = self.next();
        if tok == expected {
            Ok(())
        } else {
            Err(WitParseError::new("unexpected token", self.line))
        }
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parse a WIT source string into a `WitDocument`.
pub fn parse_wit(source: &str) -> Result<WitDocument, WitParseError> {
    let mut doc = WitDocument::new();
    let mut tz = Tokenizer::new(source);

    loop {
        let tok = tz.peek();
        match tok {
            Token::Eof => break,
            Token::Ident(ref kw) => {
                let kw_owned = kw.clone();
                match kw_owned.as_str() {
                    "package" => {
                        doc.package = Some(parse_package(&mut tz)?);
                    }
                    "interface" => {
                        doc.interfaces.push(parse_interface(&mut tz)?);
                    }
                    "world" => {
                        doc.worlds.push(parse_world(&mut tz)?);
                    }
                    _ => {
                        // Skip unknown top-level token
                        tz.next();
                    }
                }
            }
            _ => {
                tz.next(); // skip
            }
        }
    }

    Ok(doc)
}

fn parse_package(tz: &mut Tokenizer) -> Result<WitPackage, WitParseError> {
    tz.expect_ident()?; // "package"
    let namespace = tz.expect_ident()?;
    tz.expect(Token::Colon)?;
    let name = tz.expect_ident()?;

    let version = if tz.peek() == Token::At {
        tz.next(); // @
        let v = tz.expect_ident()?;
        Some(v)
    } else {
        None
    };

    // Optional semicolon
    if tz.peek() == Token::Semicolon {
        tz.next();
    }

    Ok(WitPackage {
        namespace,
        name,
        version,
    })
}

fn parse_interface(tz: &mut Tokenizer) -> Result<WitInterface, WitParseError> {
    tz.expect_ident()?; // "interface"
    let name = tz.expect_ident()?;
    tz.expect(Token::LBrace)?;
    let items = parse_interface_items(tz)?;
    tz.expect(Token::RBrace)?;

    Ok(WitInterface { name, items })
}

fn parse_interface_items(tz: &mut Tokenizer) -> Result<Vec<WitItem>, WitParseError> {
    let mut items = Vec::new();

    loop {
        match tz.peek() {
            Token::RBrace | Token::Eof => break,
            Token::Ident(ref kw) => {
                let kw_owned = kw.clone();
                match kw_owned.as_str() {
                    "record" => items.push(WitItem::Record(parse_record(tz)?)),
                    "enum" => items.push(WitItem::Enum(parse_enum(tz)?)),
                    "flags" => items.push(WitItem::Flags(parse_flags(tz)?)),
                    "variant" => items.push(WitItem::Variant(parse_variant(tz)?)),
                    "resource" => items.push(WitItem::Resource(parse_resource(tz)?)),
                    "type" => items.push(WitItem::TypeAlias(parse_type_alias(tz)?)),
                    "use" => items.push(WitItem::Use(parse_use(tz)?)),
                    _ => {
                        // Must be a function name
                        items.push(WitItem::Function(parse_function(tz)?));
                    }
                }
            }
            _ => {
                tz.next(); // skip
            }
        }
    }

    Ok(items)
}

fn parse_record(tz: &mut Tokenizer) -> Result<WitRecord, WitParseError> {
    tz.expect_ident()?; // "record"
    let name = tz.expect_ident()?;
    tz.expect(Token::LBrace)?;

    let mut fields = Vec::new();
    loop {
        if tz.peek() == Token::RBrace {
            break;
        }
        let fname = tz.expect_ident()?;
        tz.expect(Token::Colon)?;
        let fty = parse_type_ref(tz)?;
        fields.push(WitField { name: fname, ty: fty });
        // Comma or semicolon separator (both accepted)
        match tz.peek() {
            Token::Comma | Token::Semicolon => {
                tz.next();
            }
            _ => {}
        }
    }
    tz.expect(Token::RBrace)?;

    Ok(WitRecord { name, fields })
}

fn parse_enum(tz: &mut Tokenizer) -> Result<WitEnum, WitParseError> {
    tz.expect_ident()?; // "enum"
    let name = tz.expect_ident()?;
    tz.expect(Token::LBrace)?;

    let mut cases = Vec::new();
    loop {
        if tz.peek() == Token::RBrace {
            break;
        }
        cases.push(tz.expect_ident()?);
        match tz.peek() {
            Token::Comma | Token::Semicolon => {
                tz.next();
            }
            _ => {}
        }
    }
    tz.expect(Token::RBrace)?;

    Ok(WitEnum { name, cases })
}

fn parse_flags(tz: &mut Tokenizer) -> Result<WitFlags, WitParseError> {
    tz.expect_ident()?; // "flags"
    let name = tz.expect_ident()?;
    tz.expect(Token::LBrace)?;

    let mut flags = Vec::new();
    loop {
        if tz.peek() == Token::RBrace {
            break;
        }
        flags.push(tz.expect_ident()?);
        match tz.peek() {
            Token::Comma | Token::Semicolon => {
                tz.next();
            }
            _ => {}
        }
    }
    tz.expect(Token::RBrace)?;

    Ok(WitFlags { name, flags })
}

fn parse_variant(tz: &mut Tokenizer) -> Result<WitVariant, WitParseError> {
    tz.expect_ident()?; // "variant"
    let name = tz.expect_ident()?;
    tz.expect(Token::LBrace)?;

    let mut cases = Vec::new();
    loop {
        if tz.peek() == Token::RBrace {
            break;
        }
        let cname = tz.expect_ident()?;
        let ty = if tz.peek() == Token::LParen {
            tz.next(); // (
            let t = parse_type_ref(tz)?;
            tz.expect(Token::RParen)?;
            Some(t)
        } else {
            None
        };
        cases.push(WitVariantCase { name: cname, ty });
        match tz.peek() {
            Token::Comma | Token::Semicolon => {
                tz.next();
            }
            _ => {}
        }
    }
    tz.expect(Token::RBrace)?;

    Ok(WitVariant { name, cases })
}

fn parse_resource(tz: &mut Tokenizer) -> Result<WitResource, WitParseError> {
    tz.expect_ident()?; // "resource"
    let name = tz.expect_ident()?;

    let mut methods = Vec::new();
    if tz.peek() == Token::LBrace {
        tz.next();
        loop {
            if tz.peek() == Token::RBrace {
                break;
            }
            methods.push(parse_function(tz)?);
        }
        tz.expect(Token::RBrace)?;
    }

    Ok(WitResource { name, methods })
}

fn parse_type_alias(tz: &mut Tokenizer) -> Result<WitTypeAlias, WitParseError> {
    tz.expect_ident()?; // "type"
    let name = tz.expect_ident()?;
    tz.expect(Token::Eq)?;
    let target = parse_type_ref(tz)?;
    // Optional semicolon
    if tz.peek() == Token::Semicolon {
        tz.next();
    }
    Ok(WitTypeAlias { name, target })
}

fn parse_use(tz: &mut Tokenizer) -> Result<WitUse, WitParseError> {
    tz.expect_ident()?; // "use"
    // Collect path segments until {
    let mut path = tz.expect_ident()?;
    while tz.peek() == Token::Dot || tz.peek() == Token::Colon {
        let sep = tz.next();
        let segment = tz.expect_ident()?;
        if sep == Token::Colon {
            path.push(':');
        } else {
            path.push('.');
        }
        path.push_str(&segment);
    }

    let mut names = Vec::new();
    if tz.peek() == Token::Dot {
        tz.next(); // .
        tz.expect(Token::LBrace)?;
        loop {
            if tz.peek() == Token::RBrace {
                break;
            }
            names.push(tz.expect_ident()?);
            if tz.peek() == Token::Comma {
                tz.next();
            }
        }
        tz.expect(Token::RBrace)?;
    }

    // Optional semicolon
    if tz.peek() == Token::Semicolon {
        tz.next();
    }

    Ok(WitUse { path, names })
}

fn parse_function(tz: &mut Tokenizer) -> Result<WitFunction, WitParseError> {
    let name = tz.expect_ident()?;
    tz.expect(Token::Colon)?;

    // Optional "func" keyword
    if let Token::Ident(ref kw) = tz.peek() {
        if kw == "func" {
            tz.next();
        }
    }

    tz.expect(Token::LParen)?;
    let params = parse_params(tz)?;
    tz.expect(Token::RParen)?;

    let results = if tz.peek() == Token::Arrow {
        tz.next(); // ->
        // Results can be a single type or (named1: type1, ...)
        if tz.peek() == Token::LParen {
            tz.next();
            let r = parse_params(tz)?;
            tz.expect(Token::RParen)?;
            r
        } else {
            let ty = parse_type_ref(tz)?;
            alloc::vec![WitParam {
                name: String::from("_"),
                ty,
            }]
        }
    } else {
        Vec::new()
    };

    // Optional semicolon
    if tz.peek() == Token::Semicolon {
        tz.next();
    }

    Ok(WitFunction {
        name,
        params,
        results,
    })
}

fn parse_params(tz: &mut Tokenizer) -> Result<Vec<WitParam>, WitParseError> {
    let mut params = Vec::new();
    loop {
        match tz.peek() {
            Token::RParen | Token::Eof => break,
            _ => {
                let pname = tz.expect_ident()?;
                tz.expect(Token::Colon)?;
                let pty = parse_type_ref(tz)?;
                params.push(WitParam {
                    name: pname,
                    ty: pty,
                });
                if tz.peek() == Token::Comma {
                    tz.next();
                }
            }
        }
    }
    Ok(params)
}

fn parse_type_ref(tz: &mut Tokenizer) -> Result<WitTypeRef, WitParseError> {
    let ident = tz.expect_ident()?;

    match ident.as_str() {
        "u8" => Ok(WitTypeRef::Primitive(WitPrimitive::U8)),
        "u16" => Ok(WitTypeRef::Primitive(WitPrimitive::U16)),
        "u32" => Ok(WitTypeRef::Primitive(WitPrimitive::U32)),
        "u64" => Ok(WitTypeRef::Primitive(WitPrimitive::U64)),
        "s8" => Ok(WitTypeRef::Primitive(WitPrimitive::S8)),
        "s16" => Ok(WitTypeRef::Primitive(WitPrimitive::S16)),
        "s32" => Ok(WitTypeRef::Primitive(WitPrimitive::S32)),
        "s64" => Ok(WitTypeRef::Primitive(WitPrimitive::S64)),
        "f32" | "float32" => Ok(WitTypeRef::Primitive(WitPrimitive::F32)),
        "f64" | "float64" => Ok(WitTypeRef::Primitive(WitPrimitive::F64)),
        "bool" => Ok(WitTypeRef::Primitive(WitPrimitive::Bool)),
        "char" => Ok(WitTypeRef::Primitive(WitPrimitive::Char)),
        "string" => Ok(WitTypeRef::Primitive(WitPrimitive::StringType)),
        "list" => {
            tz.expect(Token::Lt)?;
            let inner = parse_type_ref(tz)?;
            tz.expect(Token::Gt)?;
            Ok(WitTypeRef::List(Box::new(inner)))
        }
        "option" => {
            tz.expect(Token::Lt)?;
            let inner = parse_type_ref(tz)?;
            tz.expect(Token::Gt)?;
            Ok(WitTypeRef::Option(Box::new(inner)))
        }
        "result" => {
            if tz.peek() == Token::Lt {
                tz.next(); // <
                let ok_ty = if tz.peek() == Token::Comma {
                    None
                } else {
                    let t = parse_type_ref(tz)?;
                    Some(Box::new(t))
                };
                let err_ty = if tz.peek() == Token::Comma {
                    tz.next(); // ,
                    let t = parse_type_ref(tz)?;
                    Some(Box::new(t))
                } else {
                    None
                };
                tz.expect(Token::Gt)?;
                Ok(WitTypeRef::Result {
                    ok: ok_ty,
                    err: err_ty,
                })
            } else {
                Ok(WitTypeRef::Result {
                    ok: None,
                    err: None,
                })
            }
        }
        "tuple" => {
            tz.expect(Token::Lt)?;
            let mut tys = Vec::new();
            loop {
                if tz.peek() == Token::Gt {
                    break;
                }
                tys.push(parse_type_ref(tz)?);
                if tz.peek() == Token::Comma {
                    tz.next();
                }
            }
            tz.expect(Token::Gt)?;
            Ok(WitTypeRef::Tuple(tys))
        }
        other => Ok(WitTypeRef::Named(String::from(other))),
    }
}

fn parse_world(tz: &mut Tokenizer) -> Result<WitWorld, WitParseError> {
    tz.expect_ident()?; // "world"
    let name = tz.expect_ident()?;
    tz.expect(Token::LBrace)?;

    let mut imports = Vec::new();
    let mut exports = Vec::new();

    loop {
        match tz.peek() {
            Token::RBrace | Token::Eof => break,
            Token::Ident(ref kw) => {
                let kw_owned = kw.clone();
                match kw_owned.as_str() {
                    "import" => {
                        tz.next(); // "import"
                        let iname = tz.expect_ident()?;
                        // Optional semicolon
                        if tz.peek() == Token::Semicolon {
                            tz.next();
                        }
                        imports.push((iname.clone(), WorldRef::InterfaceName(iname)));
                    }
                    "export" => {
                        tz.next(); // "export"
                        let ename = tz.expect_ident()?;
                        if tz.peek() == Token::Semicolon {
                            tz.next();
                        }
                        exports.push((ename.clone(), WorldRef::InterfaceName(ename)));
                    }
                    _ => {
                        tz.next(); // skip
                    }
                }
            }
            _ => {
                tz.next(); // skip
            }
        }
    }

    tz.expect(Token::RBrace)?;

    Ok(WitWorld {
        name,
        imports,
        exports,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_parse_package() {
        let input = "package kpio:gui@0.1.0;";
        let doc = parse_wit(input).unwrap();
        let pkg = doc.package.unwrap();
        assert_eq!(pkg.namespace, "kpio");
        assert_eq!(pkg.name, "gui");
        assert_eq!(pkg.version, Some(String::from("0.1.0")));
    }

    #[test]
    fn test_parse_interface_with_function() {
        let input = r#"
            interface canvas {
                draw-rect: func(x: u32, y: u32, w: u32, h: u32) -> bool;
            }
        "#;
        let doc = parse_wit(input).unwrap();
        assert_eq!(doc.interfaces.len(), 1);
        let iface = &doc.interfaces[0];
        assert_eq!(iface.name, "canvas");
        assert_eq!(iface.items.len(), 1);
        if let WitItem::Function(f) = &iface.items[0] {
            assert_eq!(f.name, "draw-rect");
            assert_eq!(f.params.len(), 4);
            assert_eq!(f.results.len(), 1);
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_parse_record() {
        let input = r#"
            interface types {
                record point {
                    x: s32,
                    y: s32,
                }
            }
        "#;
        let doc = parse_wit(input).unwrap();
        let items = &doc.interfaces[0].items;
        assert_eq!(items.len(), 1);
        if let WitItem::Record(r) = &items[0] {
            assert_eq!(r.name, "point");
            assert_eq!(r.fields.len(), 2);
            assert_eq!(r.fields[0].name, "x");
        } else {
            panic!("expected record");
        }
    }

    #[test]
    fn test_parse_enum() {
        let input = r#"
            interface events {
                enum mouse-button {
                    left,
                    right,
                    middle,
                }
            }
        "#;
        let doc = parse_wit(input).unwrap();
        if let WitItem::Enum(e) = &doc.interfaces[0].items[0] {
            assert_eq!(e.name, "mouse-button");
            assert_eq!(e.cases, vec!["left", "right", "middle"]);
        } else {
            panic!("expected enum");
        }
    }

    #[test]
    fn test_parse_world() {
        let input = r#"
            world kpio-app {
                import gui;
                import system;
                export run;
            }
        "#;
        let doc = parse_wit(input).unwrap();
        assert_eq!(doc.worlds.len(), 1);
        let w = &doc.worlds[0];
        assert_eq!(w.name, "kpio-app");
        assert_eq!(w.imports.len(), 2);
        assert_eq!(w.exports.len(), 1);
    }

    #[test]
    fn test_parse_flags() {
        let input = r#"
            interface perms {
                flags permissions {
                    read,
                    write,
                    execute,
                }
            }
        "#;
        let doc = parse_wit(input).unwrap();
        if let WitItem::Flags(f) = &doc.interfaces[0].items[0] {
            assert_eq!(f.name, "permissions");
            assert_eq!(f.flags.len(), 3);
        } else {
            panic!("expected flags");
        }
    }

    #[test]
    fn test_parse_list_type() {
        let input = r#"
            interface data {
                get-bytes: func() -> list<u8>;
            }
        "#;
        let doc = parse_wit(input).unwrap();
        if let WitItem::Function(f) = &doc.interfaces[0].items[0] {
            assert_eq!(f.results.len(), 1);
            assert_eq!(
                f.results[0].ty,
                WitTypeRef::List(Box::new(WitTypeRef::Primitive(WitPrimitive::U8)))
            );
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_parse_full_document() {
        let input = r#"
            package kpio:gui@0.1.0;

            interface window {
                record window-config {
                    width: u32,
                    height: u32,
                    title: string,
                }

                enum cursor-style {
                    default,
                    pointer,
                    text,
                }

                create-window: func(config: window-config) -> u32;
                close-window: func(id: u32);
            }

            world gui-app {
                import window;
            }
        "#;
        let doc = parse_wit(input).unwrap();
        assert!(doc.package.is_some());
        assert_eq!(doc.interfaces.len(), 1);
        assert_eq!(doc.worlds.len(), 1);
        let iface = &doc.interfaces[0];
        assert_eq!(iface.items.len(), 4); // record + enum + 2 funcs
    }
}
