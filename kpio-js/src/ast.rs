//! JavaScript Abstract Syntax Tree definitions.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::token::Span;

/// Program node - the root of the AST.
#[derive(Debug, Clone)]
pub struct Program {
    /// Program body (statements).
    pub body: Vec<Statement>,
    /// Source type (script or module).
    pub source_type: SourceType,
    /// Source span.
    pub span: Span,
}

/// Source type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Script,
    Module,
}

/// Statement node.
#[derive(Debug, Clone)]
pub enum Statement {
    /// Empty statement (;)
    Empty(Span),
    /// Expression statement
    Expression(ExpressionStmt),
    /// Block statement
    Block(BlockStmt),
    /// Variable declaration
    Variable(VariableDecl),
    /// If statement
    If(IfStmt),
    /// For statement
    For(ForStmt),
    /// For-in statement
    ForIn(ForInStmt),
    /// For-of statement
    ForOf(ForOfStmt),
    /// While statement
    While(WhileStmt),
    /// Do-while statement
    DoWhile(DoWhileStmt),
    /// Switch statement
    Switch(SwitchStmt),
    /// Break statement
    Break(BreakStmt),
    /// Continue statement
    Continue(ContinueStmt),
    /// Return statement
    Return(ReturnStmt),
    /// Throw statement
    Throw(ThrowStmt),
    /// Try statement
    Try(TryStmt),
    /// With statement
    With(WithStmt),
    /// Labeled statement
    Labeled(LabeledStmt),
    /// Debugger statement
    Debugger(Span),
    /// Function declaration
    Function(FunctionDecl),
    /// Class declaration
    Class(ClassDecl),
    /// Import declaration
    Import(ImportDecl),
    /// Export declaration
    Export(ExportDecl),
}

/// Expression statement.
#[derive(Debug, Clone)]
pub struct ExpressionStmt {
    pub expression: Expression,
    pub span: Span,
}

/// Block statement.
#[derive(Debug, Clone)]
pub struct BlockStmt {
    pub body: Vec<Statement>,
    pub span: Span,
}

/// Variable declaration.
#[derive(Debug, Clone)]
pub struct VariableDecl {
    pub kind: VariableKind,
    pub declarations: Vec<VariableDeclarator>,
    pub span: Span,
}

/// Variable kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableKind {
    Var,
    Let,
    Const,
}

/// Variable declarator.
#[derive(Debug, Clone)]
pub struct VariableDeclarator {
    pub id: Pattern,
    pub init: Option<Expression>,
    pub span: Span,
}

/// If statement.
#[derive(Debug, Clone)]
pub struct IfStmt {
    pub test: Expression,
    pub consequent: Box<Statement>,
    pub alternate: Option<Box<Statement>>,
    pub span: Span,
}

/// For statement.
#[derive(Debug, Clone)]
pub struct ForStmt {
    pub init: Option<ForInit>,
    pub test: Option<Expression>,
    pub update: Option<Expression>,
    pub body: Box<Statement>,
    pub span: Span,
}

/// For loop initializer.
#[derive(Debug, Clone)]
pub enum ForInit {
    Variable(VariableDecl),
    Expression(Expression),
}

/// For-in statement.
#[derive(Debug, Clone)]
pub struct ForInStmt {
    pub left: ForInLeft,
    pub right: Expression,
    pub body: Box<Statement>,
    pub span: Span,
}

/// For-of statement.
#[derive(Debug, Clone)]
pub struct ForOfStmt {
    pub left: ForInLeft,
    pub right: Expression,
    pub body: Box<Statement>,
    pub is_await: bool,
    pub span: Span,
}

/// For-in/of left side.
#[derive(Debug, Clone)]
pub enum ForInLeft {
    Variable(VariableDecl),
    Pattern(Pattern),
}

/// While statement.
#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub test: Expression,
    pub body: Box<Statement>,
    pub span: Span,
}

/// Do-while statement.
#[derive(Debug, Clone)]
pub struct DoWhileStmt {
    pub body: Box<Statement>,
    pub test: Expression,
    pub span: Span,
}

/// Switch statement.
#[derive(Debug, Clone)]
pub struct SwitchStmt {
    pub discriminant: Expression,
    pub cases: Vec<SwitchCase>,
    pub span: Span,
}

/// Switch case.
#[derive(Debug, Clone)]
pub struct SwitchCase {
    pub test: Option<Expression>,
    pub consequent: Vec<Statement>,
    pub span: Span,
}

/// Break statement.
#[derive(Debug, Clone)]
pub struct BreakStmt {
    pub label: Option<Identifier>,
    pub span: Span,
}

/// Continue statement.
#[derive(Debug, Clone)]
pub struct ContinueStmt {
    pub label: Option<Identifier>,
    pub span: Span,
}

/// Return statement.
#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub argument: Option<Expression>,
    pub span: Span,
}

/// Throw statement.
#[derive(Debug, Clone)]
pub struct ThrowStmt {
    pub argument: Expression,
    pub span: Span,
}

/// Try statement.
#[derive(Debug, Clone)]
pub struct TryStmt {
    pub block: BlockStmt,
    pub handler: Option<CatchClause>,
    pub finalizer: Option<BlockStmt>,
    pub span: Span,
}

/// Catch clause.
#[derive(Debug, Clone)]
pub struct CatchClause {
    pub param: Option<Pattern>,
    pub body: BlockStmt,
    pub span: Span,
}

/// With statement.
#[derive(Debug, Clone)]
pub struct WithStmt {
    pub object: Expression,
    pub body: Box<Statement>,
    pub span: Span,
}

/// Labeled statement.
#[derive(Debug, Clone)]
pub struct LabeledStmt {
    pub label: Identifier,
    pub body: Box<Statement>,
    pub span: Span,
}

/// Function declaration.
#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub id: Option<Identifier>,
    pub params: Vec<Pattern>,
    pub body: BlockStmt,
    pub is_async: bool,
    pub is_generator: bool,
    pub span: Span,
}

/// Class declaration.
#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub id: Option<Identifier>,
    pub super_class: Option<Expression>,
    pub body: ClassBody,
    pub span: Span,
}

/// Class body.
#[derive(Debug, Clone)]
pub struct ClassBody {
    pub body: Vec<ClassElement>,
    pub span: Span,
}

/// Class element.
#[derive(Debug, Clone)]
pub enum ClassElement {
    Method(MethodDef),
    Property(PropertyDef),
    StaticBlock(StaticBlock),
}

/// Method definition.
#[derive(Debug, Clone)]
pub struct MethodDef {
    pub key: Expression,
    pub value: FunctionExpr,
    pub kind: MethodKind,
    pub computed: bool,
    pub is_static: bool,
    pub span: Span,
}

/// Method kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    Method,
    Get,
    Set,
    Constructor,
}

/// Property definition.
#[derive(Debug, Clone)]
pub struct PropertyDef {
    pub key: Expression,
    pub value: Option<Expression>,
    pub computed: bool,
    pub is_static: bool,
    pub span: Span,
}

/// Static block.
#[derive(Debug, Clone)]
pub struct StaticBlock {
    pub body: Vec<Statement>,
    pub span: Span,
}

/// Import declaration.
#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub specifiers: Vec<ImportSpecifier>,
    pub source: StringLiteral,
    pub span: Span,
}

/// Import specifier.
#[derive(Debug, Clone)]
pub enum ImportSpecifier {
    Default(Identifier),
    Named { imported: Identifier, local: Identifier },
    Namespace(Identifier),
}

/// Export declaration.
#[derive(Debug, Clone)]
pub enum ExportDecl {
    Named {
        specifiers: Vec<ExportSpecifier>,
        source: Option<StringLiteral>,
        span: Span,
    },
    Default {
        declaration: Box<ExportDefault>,
        span: Span,
    },
    All {
        source: StringLiteral,
        exported: Option<Identifier>,
        span: Span,
    },
    Declaration {
        declaration: Box<Statement>,
        span: Span,
    },
}

/// Export specifier.
#[derive(Debug, Clone)]
pub struct ExportSpecifier {
    pub local: Identifier,
    pub exported: Identifier,
    pub span: Span,
}

/// Export default.
#[derive(Debug, Clone)]
pub enum ExportDefault {
    Function(FunctionDecl),
    Class(ClassDecl),
    Expression(Expression),
}

/// Expression node.
#[derive(Debug, Clone)]
pub enum Expression {
    /// Identifier
    Identifier(Identifier),
    /// Literal
    Literal(Literal),
    /// This expression
    This(Span),
    /// Array expression
    Array(ArrayExpr),
    /// Object expression
    Object(ObjectExpr),
    /// Function expression
    Function(FunctionExpr),
    /// Arrow function
    Arrow(ArrowFunctionExpr),
    /// Class expression
    Class(ClassExpr),
    /// Template literal
    Template(TemplateLiteral),
    /// Tagged template
    TaggedTemplate(TaggedTemplateExpr),
    /// Member expression (a.b or a[b])
    Member(MemberExpr),
    /// Call expression
    Call(CallExpr),
    /// New expression
    New(NewExpr),
    /// Update expression (++x, x--)
    Update(UpdateExpr),
    /// Unary expression (!x, -x, etc.)
    Unary(UnaryExpr),
    /// Binary expression (a + b)
    Binary(BinaryExpr),
    /// Logical expression (a && b, a || b)
    Logical(LogicalExpr),
    /// Conditional expression (a ? b : c)
    Conditional(ConditionalExpr),
    /// Assignment expression
    Assignment(AssignmentExpr),
    /// Sequence expression (a, b, c)
    Sequence(SequenceExpr),
    /// Spread element
    Spread(SpreadElement),
    /// Yield expression
    Yield(YieldExpr),
    /// Await expression
    Await(AwaitExpr),
    /// Optional chaining
    OptionalChain(OptionalChainExpr),
}

/// Identifier.
#[derive(Debug, Clone)]
pub struct Identifier {
    pub name: String,
    pub span: Span,
}

/// Literal.
#[derive(Debug, Clone)]
pub enum Literal {
    Null(Span),
    Boolean(bool, Span),
    Number(f64, Span),
    String(StringLiteral),
    BigInt(String, Span),
    RegExp { pattern: String, flags: String, span: Span },
}

/// String literal.
#[derive(Debug, Clone)]
pub struct StringLiteral {
    pub value: String,
    pub span: Span,
}

/// Array expression.
#[derive(Debug, Clone)]
pub struct ArrayExpr {
    pub elements: Vec<Option<Expression>>,
    pub span: Span,
}

/// Object expression.
#[derive(Debug, Clone)]
pub struct ObjectExpr {
    pub properties: Vec<ObjectProperty>,
    pub span: Span,
}

/// Object property.
#[derive(Debug, Clone)]
pub enum ObjectProperty {
    Property {
        key: Expression,
        value: Expression,
        computed: bool,
        shorthand: bool,
        method: bool,
        span: Span,
    },
    Spread(SpreadElement),
}

/// Function expression.
#[derive(Debug, Clone)]
pub struct FunctionExpr {
    pub id: Option<Identifier>,
    pub params: Vec<Pattern>,
    pub body: BlockStmt,
    pub is_async: bool,
    pub is_generator: bool,
    pub span: Span,
}

/// Arrow function expression.
#[derive(Debug, Clone)]
pub struct ArrowFunctionExpr {
    pub params: Vec<Pattern>,
    pub body: ArrowFunctionBody,
    pub is_async: bool,
    pub span: Span,
}

/// Arrow function body.
#[derive(Debug, Clone)]
pub enum ArrowFunctionBody {
    Expression(Box<Expression>),
    Block(BlockStmt),
}

/// Class expression.
#[derive(Debug, Clone)]
pub struct ClassExpr {
    pub id: Option<Identifier>,
    pub super_class: Option<Box<Expression>>,
    pub body: ClassBody,
    pub span: Span,
}

/// Template literal.
#[derive(Debug, Clone)]
pub struct TemplateLiteral {
    pub quasis: Vec<TemplateElement>,
    pub expressions: Vec<Expression>,
    pub span: Span,
}

/// Template element.
#[derive(Debug, Clone)]
pub struct TemplateElement {
    pub raw: String,
    pub cooked: Option<String>,
    pub tail: bool,
    pub span: Span,
}

/// Tagged template expression.
#[derive(Debug, Clone)]
pub struct TaggedTemplateExpr {
    pub tag: Box<Expression>,
    pub quasi: TemplateLiteral,
    pub span: Span,
}

/// Member expression.
#[derive(Debug, Clone)]
pub struct MemberExpr {
    pub object: Box<Expression>,
    pub property: Box<Expression>,
    pub computed: bool,
    pub optional: bool,
    pub span: Span,
}

/// Call expression.
#[derive(Debug, Clone)]
pub struct CallExpr {
    pub callee: Box<Expression>,
    pub arguments: Vec<Expression>,
    pub optional: bool,
    pub span: Span,
}

/// New expression.
#[derive(Debug, Clone)]
pub struct NewExpr {
    pub callee: Box<Expression>,
    pub arguments: Vec<Expression>,
    pub span: Span,
}

/// Update expression.
#[derive(Debug, Clone)]
pub struct UpdateExpr {
    pub operator: UpdateOp,
    pub argument: Box<Expression>,
    pub prefix: bool,
    pub span: Span,
}

/// Update operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOp {
    Increment, // ++
    Decrement, // --
}

/// Unary expression.
#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub operator: UnaryOp,
    pub argument: Box<Expression>,
    pub span: Span,
}

/// Unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Minus,    // -
    Plus,     // +
    Not,      // !
    BitNot,   // ~
    Typeof,   // typeof
    Void,     // void
    Delete,   // delete
}

/// Binary expression.
#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub operator: BinaryOp,
    pub left: Box<Expression>,
    pub right: Box<Expression>,
    pub span: Span,
}

/// Binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,              // +
    Sub,              // -
    Mul,              // *
    Div,              // /
    Mod,              // %
    Exp,              // **
    Equal,            // ==
    NotEqual,         // !=
    StrictEqual,      // ===
    StrictNotEqual,   // !==
    LessThan,         // <
    LessEqual,        // <=
    GreaterThan,      // >
    GreaterEqual,     // >=
    LeftShift,        // <<
    RightShift,       // >>
    UnsignedRightShift, // >>>
    BitAnd,           // &
    BitOr,            // |
    BitXor,           // ^
    In,               // in
    Instanceof,       // instanceof
}

/// Logical expression.
#[derive(Debug, Clone)]
pub struct LogicalExpr {
    pub operator: LogicalOp,
    pub left: Box<Expression>,
    pub right: Box<Expression>,
    pub span: Span,
}

/// Logical operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    And,     // &&
    Or,      // ||
    Nullish, // ??
}

/// Conditional expression.
#[derive(Debug, Clone)]
pub struct ConditionalExpr {
    pub test: Box<Expression>,
    pub consequent: Box<Expression>,
    pub alternate: Box<Expression>,
    pub span: Span,
}

/// Assignment expression.
#[derive(Debug, Clone)]
pub struct AssignmentExpr {
    pub operator: AssignmentOp,
    pub left: AssignmentTarget,
    pub right: Box<Expression>,
    pub span: Span,
}

/// Assignment target.
#[derive(Debug, Clone)]
pub enum AssignmentTarget {
    Simple(Box<Expression>),
    Pattern(Pattern),
}

/// Assignment operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentOp {
    Assign,            // =
    AddAssign,         // +=
    SubAssign,         // -=
    MulAssign,         // *=
    DivAssign,         // /=
    ModAssign,         // %=
    ExpAssign,         // **=
    LeftShiftAssign,   // <<=
    RightShiftAssign,  // >>=
    UnsignedRightShiftAssign, // >>>=
    BitAndAssign,      // &=
    BitOrAssign,       // |=
    BitXorAssign,      // ^=
    AndAssign,         // &&=
    OrAssign,          // ||=
    NullishAssign,     // ??=
}

/// Sequence expression.
#[derive(Debug, Clone)]
pub struct SequenceExpr {
    pub expressions: Vec<Expression>,
    pub span: Span,
}

/// Spread element.
#[derive(Debug, Clone)]
pub struct SpreadElement {
    pub argument: Box<Expression>,
    pub span: Span,
}

/// Yield expression.
#[derive(Debug, Clone)]
pub struct YieldExpr {
    pub argument: Option<Box<Expression>>,
    pub delegate: bool,
    pub span: Span,
}

/// Await expression.
#[derive(Debug, Clone)]
pub struct AwaitExpr {
    pub argument: Box<Expression>,
    pub span: Span,
}

/// Optional chain expression.
#[derive(Debug, Clone)]
pub struct OptionalChainExpr {
    pub expression: Box<Expression>,
    pub span: Span,
}

/// Pattern (for destructuring).
#[derive(Debug, Clone)]
pub enum Pattern {
    /// Identifier pattern
    Identifier(Identifier),
    /// Array pattern
    Array(ArrayPattern),
    /// Object pattern
    Object(ObjectPattern),
    /// Assignment pattern (default value)
    Assignment(AssignmentPattern),
    /// Rest element
    Rest(RestElement),
}

/// Array pattern.
#[derive(Debug, Clone)]
pub struct ArrayPattern {
    pub elements: Vec<Option<Pattern>>,
    pub span: Span,
}

/// Object pattern.
#[derive(Debug, Clone)]
pub struct ObjectPattern {
    pub properties: Vec<ObjectPatternProperty>,
    pub span: Span,
}

/// Object pattern property.
#[derive(Debug, Clone)]
pub enum ObjectPatternProperty {
    Property {
        key: Expression,
        value: Pattern,
        computed: bool,
        shorthand: bool,
        span: Span,
    },
    Rest(RestElement),
}

/// Assignment pattern.
#[derive(Debug, Clone)]
pub struct AssignmentPattern {
    pub left: Box<Pattern>,
    pub right: Box<Expression>,
    pub span: Span,
}

/// Rest element.
#[derive(Debug, Clone)]
pub struct RestElement {
    pub argument: Box<Pattern>,
    pub span: Span,
}
