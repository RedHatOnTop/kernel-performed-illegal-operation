//! JavaScript token definitions.

use alloc::string::String;
use core::fmt;

/// Source location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    /// Start byte offset.
    pub start: usize,
    /// End byte offset.
    pub end: usize,
    /// Line number (1-based).
    pub line: usize,
    /// Column number (1-based).
    pub column: usize,
}

impl Span {
    /// Create a new span.
    pub fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Span {
            start,
            end,
            line,
            column,
        }
    }

    /// Merge two spans.
    pub fn merge(self, other: Span) -> Self {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            line: self.line,
            column: self.column,
        }
    }
}

/// JavaScript token types.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    /// Number literal (42, 3.14, 0xFF, etc.)
    Number(f64),
    /// String literal ("hello", 'world')
    String(String),
    /// Template literal (`hello ${name}`)
    Template(String),
    /// BigInt literal (42n)
    BigInt(String),
    /// Regular expression literal (/pattern/flags)
    RegExp {
        pattern: String,
        flags: String,
    },
    /// Boolean true
    True,
    /// Boolean false
    False,
    /// Null literal
    Null,

    // Identifiers and Keywords
    /// Identifier (variable name, function name, etc.)
    Identifier(String),
    /// Private identifier (#name)
    PrivateIdentifier(String),

    // Keywords
    Await,
    Break,
    Case,
    Catch,
    Class,
    Const,
    Continue,
    Debugger,
    Default,
    Delete,
    Do,
    Else,
    Enum,
    Export,
    Extends,
    Finally,
    For,
    Function,
    If,
    Import,
    In,
    Instanceof,
    Let,
    New,
    Return,
    Static,
    Super,
    Switch,
    This,
    Throw,
    Try,
    Typeof,
    Var,
    Void,
    While,
    With,
    Yield,

    // Future reserved words
    Implements,
    Interface,
    Package,
    Private,
    Protected,
    Public,

    // Async/Generator
    Async,
    Of,
    Get,
    Set,

    // Punctuators
    /// {
    LeftBrace,
    /// }
    RightBrace,
    /// (
    LeftParen,
    /// )
    RightParen,
    /// [
    LeftBracket,
    /// ]
    RightBracket,
    /// .
    Dot,
    /// ...
    Ellipsis,
    /// ;
    Semicolon,
    /// ,
    Comma,
    /// <
    LessThan,
    /// >
    GreaterThan,
    /// <=
    LessEqual,
    /// >=
    GreaterEqual,
    /// ==
    Equal,
    /// !=
    NotEqual,
    /// ===
    StrictEqual,
    /// !==
    StrictNotEqual,
    /// +
    Plus,
    /// -
    Minus,
    /// *
    Star,
    /// /
    Slash,
    /// %
    Percent,
    /// **
    StarStar,
    /// ++
    PlusPlus,
    /// --
    MinusMinus,
    /// <<
    LeftShift,
    /// >>
    RightShift,
    /// >>>
    UnsignedRightShift,
    /// &
    Ampersand,
    /// |
    Pipe,
    /// ^
    Caret,
    /// !
    Bang,
    /// ~
    Tilde,
    /// &&
    AmpersandAmpersand,
    /// ||
    PipePipe,
    /// ??
    QuestionQuestion,
    /// ?
    Question,
    /// ?.
    QuestionDot,
    /// :
    Colon,
    /// =
    Assign,
    /// +=
    PlusAssign,
    /// -=
    MinusAssign,
    /// *=
    StarAssign,
    /// /=
    SlashAssign,
    /// %=
    PercentAssign,
    /// **=
    StarStarAssign,
    /// <<=
    LeftShiftAssign,
    /// >>=
    RightShiftAssign,
    /// >>>=
    UnsignedRightShiftAssign,
    /// &=
    AmpersandAssign,
    /// |=
    PipeAssign,
    /// ^=
    CaretAssign,
    /// &&=
    AmpersandAmpersandAssign,
    /// ||=
    PipePipeAssign,
    /// ??=
    QuestionQuestionAssign,
    /// =>
    Arrow,

    // Special
    /// End of file
    Eof,
    /// Invalid token
    Invalid,
    /// Line terminator (for ASI)
    LineTerminator,
}

impl TokenKind {
    /// Check if this token is a keyword.
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::Await
                | TokenKind::Break
                | TokenKind::Case
                | TokenKind::Catch
                | TokenKind::Class
                | TokenKind::Const
                | TokenKind::Continue
                | TokenKind::Debugger
                | TokenKind::Default
                | TokenKind::Delete
                | TokenKind::Do
                | TokenKind::Else
                | TokenKind::Enum
                | TokenKind::Export
                | TokenKind::Extends
                | TokenKind::Finally
                | TokenKind::For
                | TokenKind::Function
                | TokenKind::If
                | TokenKind::Import
                | TokenKind::In
                | TokenKind::Instanceof
                | TokenKind::Let
                | TokenKind::New
                | TokenKind::Return
                | TokenKind::Static
                | TokenKind::Super
                | TokenKind::Switch
                | TokenKind::This
                | TokenKind::Throw
                | TokenKind::Try
                | TokenKind::Typeof
                | TokenKind::Var
                | TokenKind::Void
                | TokenKind::While
                | TokenKind::With
                | TokenKind::Yield
                | TokenKind::Async
                | TokenKind::Of
                | TokenKind::Get
                | TokenKind::Set
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Null
        )
    }

    /// Check if this is an assignment operator.
    pub fn is_assignment(&self) -> bool {
        matches!(
            self,
            TokenKind::Assign
                | TokenKind::PlusAssign
                | TokenKind::MinusAssign
                | TokenKind::StarAssign
                | TokenKind::SlashAssign
                | TokenKind::PercentAssign
                | TokenKind::StarStarAssign
                | TokenKind::LeftShiftAssign
                | TokenKind::RightShiftAssign
                | TokenKind::UnsignedRightShiftAssign
                | TokenKind::AmpersandAssign
                | TokenKind::PipeAssign
                | TokenKind::CaretAssign
                | TokenKind::AmpersandAmpersandAssign
                | TokenKind::PipePipeAssign
                | TokenKind::QuestionQuestionAssign
        )
    }

    /// Get keyword from string.
    pub fn keyword_from_str(s: &str) -> Option<TokenKind> {
        match s {
            "await" => Some(TokenKind::Await),
            "break" => Some(TokenKind::Break),
            "case" => Some(TokenKind::Case),
            "catch" => Some(TokenKind::Catch),
            "class" => Some(TokenKind::Class),
            "const" => Some(TokenKind::Const),
            "continue" => Some(TokenKind::Continue),
            "debugger" => Some(TokenKind::Debugger),
            "default" => Some(TokenKind::Default),
            "delete" => Some(TokenKind::Delete),
            "do" => Some(TokenKind::Do),
            "else" => Some(TokenKind::Else),
            "enum" => Some(TokenKind::Enum),
            "export" => Some(TokenKind::Export),
            "extends" => Some(TokenKind::Extends),
            "finally" => Some(TokenKind::Finally),
            "for" => Some(TokenKind::For),
            "function" => Some(TokenKind::Function),
            "if" => Some(TokenKind::If),
            "import" => Some(TokenKind::Import),
            "in" => Some(TokenKind::In),
            "instanceof" => Some(TokenKind::Instanceof),
            "let" => Some(TokenKind::Let),
            "new" => Some(TokenKind::New),
            "return" => Some(TokenKind::Return),
            "static" => Some(TokenKind::Static),
            "super" => Some(TokenKind::Super),
            "switch" => Some(TokenKind::Switch),
            "this" => Some(TokenKind::This),
            "throw" => Some(TokenKind::Throw),
            "try" => Some(TokenKind::Try),
            "typeof" => Some(TokenKind::Typeof),
            "var" => Some(TokenKind::Var),
            "void" => Some(TokenKind::Void),
            "while" => Some(TokenKind::While),
            "with" => Some(TokenKind::With),
            "yield" => Some(TokenKind::Yield),
            "async" => Some(TokenKind::Async),
            "of" => Some(TokenKind::Of),
            "get" => Some(TokenKind::Get),
            "set" => Some(TokenKind::Set),
            "true" => Some(TokenKind::True),
            "false" => Some(TokenKind::False),
            "null" => Some(TokenKind::Null),
            "implements" => Some(TokenKind::Implements),
            "interface" => Some(TokenKind::Interface),
            "package" => Some(TokenKind::Package),
            "private" => Some(TokenKind::Private),
            "protected" => Some(TokenKind::Protected),
            "public" => Some(TokenKind::Public),
            _ => None,
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Number(n) => write!(f, "{}", n),
            TokenKind::String(s) => write!(f, "\"{}\"", s),
            TokenKind::Identifier(s) => write!(f, "{}", s),
            TokenKind::LeftBrace => write!(f, "{{"),
            TokenKind::RightBrace => write!(f, "}}"),
            TokenKind::LeftParen => write!(f, "("),
            TokenKind::RightParen => write!(f, ")"),
            TokenKind::Semicolon => write!(f, ";"),
            TokenKind::Comma => write!(f, ","),
            TokenKind::Plus => write!(f, "+"),
            TokenKind::Minus => write!(f, "-"),
            TokenKind::Star => write!(f, "*"),
            TokenKind::Slash => write!(f, "/"),
            TokenKind::Assign => write!(f, "="),
            TokenKind::Equal => write!(f, "=="),
            TokenKind::StrictEqual => write!(f, "==="),
            TokenKind::Eof => write!(f, "EOF"),
            _ => write!(f, "{:?}", self),
        }
    }
}

/// A token with source location.
#[derive(Debug, Clone)]
pub struct Token {
    /// Token kind.
    pub kind: TokenKind,
    /// Source location.
    pub span: Span,
}

impl Token {
    /// Create a new token.
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Token { kind, span }
    }

    /// Check if this is EOF.
    pub fn is_eof(&self) -> bool {
        matches!(self.kind, TokenKind::Eof)
    }
}
