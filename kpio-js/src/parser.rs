//! JavaScript parser.
//!
//! Parses tokens into an Abstract Syntax Tree.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use crate::ast::*;
use crate::error::{JsError, JsResult};
use crate::lexer::Lexer;
use crate::token::{Token, TokenKind, Span};

/// JavaScript parser.
pub struct Parser<'a> {
    /// Tokens.
    tokens: Vec<Token>,
    /// Current position.
    pos: usize,
    /// Source code (for error messages).
    source: &'a str,
}

impl<'a> Parser<'a> {
    /// Create a new parser.
    pub fn new(source: &'a str) -> JsResult<Self> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize()?;
        
        Ok(Parser {
            tokens,
            pos: 0,
            source,
        })
    }
    
    /// Parse the source as a script.
    pub fn parse_script(&mut self) -> JsResult<Program> {
        let start = self.current_span();
        let mut body = Vec::new();
        
        while !self.is_eof() {
            body.push(self.parse_statement()?);
        }
        
        let end = if body.is_empty() { start } else { self.prev_span() };
        
        Ok(Program {
            body,
            source_type: SourceType::Script,
            span: start.merge(end),
        })
    }
    
    /// Parse a statement.
    pub fn parse_statement(&mut self) -> JsResult<Statement> {
        match &self.current().kind {
            TokenKind::Semicolon => {
                let span = self.current_span();
                self.advance();
                Ok(Statement::Empty(span))
            }
            TokenKind::LeftBrace => self.parse_block_statement(),
            TokenKind::Var | TokenKind::Let | TokenKind::Const => self.parse_variable_declaration(),
            TokenKind::If => self.parse_if_statement(),
            TokenKind::For => self.parse_for_statement(),
            TokenKind::While => self.parse_while_statement(),
            TokenKind::Do => self.parse_do_while_statement(),
            TokenKind::Switch => self.parse_switch_statement(),
            TokenKind::Break => self.parse_break_statement(),
            TokenKind::Continue => self.parse_continue_statement(),
            TokenKind::Return => self.parse_return_statement(),
            TokenKind::Throw => self.parse_throw_statement(),
            TokenKind::Try => self.parse_try_statement(),
            TokenKind::Debugger => {
                let span = self.current_span();
                self.advance();
                self.consume_semicolon()?;
                Ok(Statement::Debugger(span))
            }
            TokenKind::Function => self.parse_function_declaration(),
            TokenKind::Class => self.parse_class_declaration(),
            TokenKind::Async if self.peek_is(&TokenKind::Function) => {
                self.parse_async_function_declaration()
            }
            _ => self.parse_expression_statement(),
        }
    }
    
    /// Parse a block statement.
    fn parse_block_statement(&mut self) -> JsResult<Statement> {
        let block = self.parse_block()?;
        Ok(Statement::Block(block))
    }
    
    /// Parse a block.
    fn parse_block(&mut self) -> JsResult<BlockStmt> {
        let start = self.current_span();
        self.expect(&TokenKind::LeftBrace)?;
        
        let mut body = Vec::new();
        while !self.check(&TokenKind::RightBrace) && !self.is_eof() {
            body.push(self.parse_statement()?);
        }
        
        self.expect(&TokenKind::RightBrace)?;
        
        Ok(BlockStmt {
            body,
            span: start.merge(self.prev_span()),
        })
    }
    
    /// Parse variable declaration.
    fn parse_variable_declaration(&mut self) -> JsResult<Statement> {
        let start = self.current_span();
        let kind = match &self.current().kind {
            TokenKind::Var => VariableKind::Var,
            TokenKind::Let => VariableKind::Let,
            TokenKind::Const => VariableKind::Const,
            _ => return Err(JsError::syntax("Expected variable declaration")),
        };
        self.advance();
        
        let mut declarations = Vec::new();
        
        loop {
            let decl_start = self.current_span();
            let id = self.parse_pattern()?;
            
            let init = if self.check(&TokenKind::Assign) {
                self.advance();
                Some(self.parse_assignment_expression()?)
            } else {
                None
            };
            
            declarations.push(VariableDeclarator {
                id,
                init,
                span: decl_start.merge(self.prev_span()),
            });
            
            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        
        self.consume_semicolon()?;
        
        Ok(Statement::Variable(VariableDecl {
            kind,
            declarations,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse if statement.
    fn parse_if_statement(&mut self) -> JsResult<Statement> {
        let start = self.current_span();
        self.expect(&TokenKind::If)?;
        self.expect(&TokenKind::LeftParen)?;
        
        let test = self.parse_expression()?;
        
        self.expect(&TokenKind::RightParen)?;
        
        let consequent = Box::new(self.parse_statement()?);
        
        let alternate = if self.check(&TokenKind::Else) {
            self.advance();
            Some(Box::new(self.parse_statement()?))
        } else {
            None
        };
        
        Ok(Statement::If(IfStmt {
            test,
            consequent,
            alternate,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse for statement.
    fn parse_for_statement(&mut self) -> JsResult<Statement> {
        let start = self.current_span();
        self.expect(&TokenKind::For)?;
        self.expect(&TokenKind::LeftParen)?;
        
        // Parse init
        let init = if self.check(&TokenKind::Semicolon) {
            None
        } else if matches!(self.current().kind, TokenKind::Var | TokenKind::Let | TokenKind::Const) {
            let var_decl = self.parse_variable_declaration_no_semi()?;
            
            // Check for for-in/of
            if self.check(&TokenKind::In) {
                return self.parse_for_in(start, ForInLeft::Variable(var_decl));
            }
            if self.check(&TokenKind::Of) {
                return self.parse_for_of(start, ForInLeft::Variable(var_decl), false);
            }
            
            Some(ForInit::Variable(var_decl))
        } else {
            let expr = self.parse_expression()?;
            Some(ForInit::Expression(expr))
        };
        
        self.expect(&TokenKind::Semicolon)?;
        
        // Parse test
        let test = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        
        self.expect(&TokenKind::Semicolon)?;
        
        // Parse update
        let update = if self.check(&TokenKind::RightParen) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        
        self.expect(&TokenKind::RightParen)?;
        
        let body = Box::new(self.parse_statement()?);
        
        Ok(Statement::For(ForStmt {
            init,
            test,
            update,
            body,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse for-in statement.
    fn parse_for_in(&mut self, start: Span, left: ForInLeft) -> JsResult<Statement> {
        self.expect(&TokenKind::In)?;
        let right = self.parse_expression()?;
        self.expect(&TokenKind::RightParen)?;
        let body = Box::new(self.parse_statement()?);
        
        Ok(Statement::ForIn(ForInStmt {
            left,
            right,
            body,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse for-of statement.
    fn parse_for_of(&mut self, start: Span, left: ForInLeft, is_await: bool) -> JsResult<Statement> {
        self.expect(&TokenKind::Of)?;
        let right = self.parse_assignment_expression()?;
        self.expect(&TokenKind::RightParen)?;
        let body = Box::new(self.parse_statement()?);
        
        Ok(Statement::ForOf(ForOfStmt {
            left,
            right,
            body,
            is_await,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse variable declaration without semicolon.
    fn parse_variable_declaration_no_semi(&mut self) -> JsResult<VariableDecl> {
        let start = self.current_span();
        let kind = match &self.current().kind {
            TokenKind::Var => VariableKind::Var,
            TokenKind::Let => VariableKind::Let,
            TokenKind::Const => VariableKind::Const,
            _ => return Err(JsError::syntax("Expected variable declaration")),
        };
        self.advance();
        
        let decl_start = self.current_span();
        let id = self.parse_pattern()?;
        
        let init = if self.check(&TokenKind::Assign) {
            self.advance();
            Some(self.parse_assignment_expression()?)
        } else {
            None
        };
        
        Ok(VariableDecl {
            kind,
            declarations: vec![VariableDeclarator {
                id,
                init,
                span: decl_start.merge(self.prev_span()),
            }],
            span: start.merge(self.prev_span()),
        })
    }
    
    /// Parse while statement.
    fn parse_while_statement(&mut self) -> JsResult<Statement> {
        let start = self.current_span();
        self.expect(&TokenKind::While)?;
        self.expect(&TokenKind::LeftParen)?;
        
        let test = self.parse_expression()?;
        
        self.expect(&TokenKind::RightParen)?;
        
        let body = Box::new(self.parse_statement()?);
        
        Ok(Statement::While(WhileStmt {
            test,
            body,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse do-while statement.
    fn parse_do_while_statement(&mut self) -> JsResult<Statement> {
        let start = self.current_span();
        self.expect(&TokenKind::Do)?;
        
        let body = Box::new(self.parse_statement()?);
        
        self.expect(&TokenKind::While)?;
        self.expect(&TokenKind::LeftParen)?;
        
        let test = self.parse_expression()?;
        
        self.expect(&TokenKind::RightParen)?;
        self.consume_semicolon()?;
        
        Ok(Statement::DoWhile(DoWhileStmt {
            body,
            test,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse switch statement.
    fn parse_switch_statement(&mut self) -> JsResult<Statement> {
        let start = self.current_span();
        self.expect(&TokenKind::Switch)?;
        self.expect(&TokenKind::LeftParen)?;
        
        let discriminant = self.parse_expression()?;
        
        self.expect(&TokenKind::RightParen)?;
        self.expect(&TokenKind::LeftBrace)?;
        
        let mut cases = Vec::new();
        
        while !self.check(&TokenKind::RightBrace) && !self.is_eof() {
            cases.push(self.parse_switch_case()?);
        }
        
        self.expect(&TokenKind::RightBrace)?;
        
        Ok(Statement::Switch(SwitchStmt {
            discriminant,
            cases,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse switch case.
    fn parse_switch_case(&mut self) -> JsResult<SwitchCase> {
        let start = self.current_span();
        
        let test = if self.check(&TokenKind::Case) {
            self.advance();
            Some(self.parse_expression()?)
        } else if self.check(&TokenKind::Default) {
            self.advance();
            None
        } else {
            return Err(JsError::syntax("Expected 'case' or 'default'"));
        };
        
        self.expect(&TokenKind::Colon)?;
        
        let mut consequent = Vec::new();
        while !self.check(&TokenKind::Case) 
            && !self.check(&TokenKind::Default) 
            && !self.check(&TokenKind::RightBrace) 
            && !self.is_eof() 
        {
            consequent.push(self.parse_statement()?);
        }
        
        Ok(SwitchCase {
            test,
            consequent,
            span: start.merge(self.prev_span()),
        })
    }
    
    /// Parse break statement.
    fn parse_break_statement(&mut self) -> JsResult<Statement> {
        let start = self.current_span();
        self.expect(&TokenKind::Break)?;
        
        let label = if let TokenKind::Identifier(name) = &self.current().kind {
            let span = self.current_span();
            let name = name.clone();
            self.advance();
            Some(Identifier { name, span })
        } else {
            None
        };
        
        self.consume_semicolon()?;
        
        Ok(Statement::Break(BreakStmt {
            label,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse continue statement.
    fn parse_continue_statement(&mut self) -> JsResult<Statement> {
        let start = self.current_span();
        self.expect(&TokenKind::Continue)?;
        
        let label = if let TokenKind::Identifier(name) = &self.current().kind {
            let span = self.current_span();
            let name = name.clone();
            self.advance();
            Some(Identifier { name, span })
        } else {
            None
        };
        
        self.consume_semicolon()?;
        
        Ok(Statement::Continue(ContinueStmt {
            label,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse return statement.
    fn parse_return_statement(&mut self) -> JsResult<Statement> {
        let start = self.current_span();
        self.expect(&TokenKind::Return)?;
        
        let argument = if self.check(&TokenKind::Semicolon) || self.is_eof() {
            None
        } else {
            Some(self.parse_expression()?)
        };
        
        self.consume_semicolon()?;
        
        Ok(Statement::Return(ReturnStmt {
            argument,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse throw statement.
    fn parse_throw_statement(&mut self) -> JsResult<Statement> {
        let start = self.current_span();
        self.expect(&TokenKind::Throw)?;
        
        let argument = self.parse_expression()?;
        self.consume_semicolon()?;
        
        Ok(Statement::Throw(ThrowStmt {
            argument,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse try statement.
    fn parse_try_statement(&mut self) -> JsResult<Statement> {
        let start = self.current_span();
        self.expect(&TokenKind::Try)?;
        
        let block = self.parse_block()?;
        
        let handler = if self.check(&TokenKind::Catch) {
            Some(self.parse_catch_clause()?)
        } else {
            None
        };
        
        let finalizer = if self.check(&TokenKind::Finally) {
            self.advance();
            Some(self.parse_block()?)
        } else {
            None
        };
        
        if handler.is_none() && finalizer.is_none() {
            return Err(JsError::syntax("Missing catch or finally after try"));
        }
        
        Ok(Statement::Try(TryStmt {
            block,
            handler,
            finalizer,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse catch clause.
    fn parse_catch_clause(&mut self) -> JsResult<CatchClause> {
        let start = self.current_span();
        self.expect(&TokenKind::Catch)?;
        
        let param = if self.check(&TokenKind::LeftParen) {
            self.advance();
            let param = self.parse_pattern()?;
            self.expect(&TokenKind::RightParen)?;
            Some(param)
        } else {
            None
        };
        
        let body = self.parse_block()?;
        
        Ok(CatchClause {
            param,
            body,
            span: start.merge(self.prev_span()),
        })
    }
    
    /// Parse function declaration.
    fn parse_function_declaration(&mut self) -> JsResult<Statement> {
        let func = self.parse_function(false)?;
        Ok(Statement::Function(func))
    }
    
    /// Parse async function declaration.
    fn parse_async_function_declaration(&mut self) -> JsResult<Statement> {
        self.expect(&TokenKind::Async)?;
        let func = self.parse_function(true)?;
        Ok(Statement::Function(FunctionDecl {
            is_async: true,
            ..func
        }))
    }
    
    /// Parse function.
    fn parse_function(&mut self, is_async: bool) -> JsResult<FunctionDecl> {
        let start = self.current_span();
        self.expect(&TokenKind::Function)?;
        
        let is_generator = if self.check(&TokenKind::Star) {
            self.advance();
            true
        } else {
            false
        };
        
        let id = if let TokenKind::Identifier(name) = &self.current().kind {
            let span = self.current_span();
            let name = name.clone();
            self.advance();
            Some(Identifier { name, span })
        } else {
            None
        };
        
        self.expect(&TokenKind::LeftParen)?;
        let params = self.parse_function_params()?;
        self.expect(&TokenKind::RightParen)?;
        
        let body = self.parse_block()?;
        
        Ok(FunctionDecl {
            id,
            params,
            body,
            is_async,
            is_generator,
            span: start.merge(self.prev_span()),
        })
    }
    
    /// Parse function parameters.
    fn parse_function_params(&mut self) -> JsResult<Vec<Pattern>> {
        let mut params = Vec::new();
        
        while !self.check(&TokenKind::RightParen) && !self.is_eof() {
            params.push(self.parse_pattern()?);
            
            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        
        Ok(params)
    }
    
    /// Parse class declaration.
    fn parse_class_declaration(&mut self) -> JsResult<Statement> {
        let start = self.current_span();
        self.expect(&TokenKind::Class)?;
        
        let id = if let TokenKind::Identifier(name) = &self.current().kind {
            let span = self.current_span();
            let name = name.clone();
            self.advance();
            Some(Identifier { name, span })
        } else {
            None
        };
        
        let super_class = if self.check(&TokenKind::Extends) {
            self.advance();
            Some(self.parse_left_hand_side_expression()?)
        } else {
            None
        };
        
        let body = self.parse_class_body()?;
        
        Ok(Statement::Class(ClassDecl {
            id,
            super_class,
            body,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse class body.
    fn parse_class_body(&mut self) -> JsResult<ClassBody> {
        let start = self.current_span();
        self.expect(&TokenKind::LeftBrace)?;
        
        let mut body = Vec::new();
        
        while !self.check(&TokenKind::RightBrace) && !self.is_eof() {
            // Skip semicolons
            if self.check(&TokenKind::Semicolon) {
                self.advance();
                continue;
            }
            
            body.push(self.parse_class_element()?);
        }
        
        self.expect(&TokenKind::RightBrace)?;
        
        Ok(ClassBody {
            body,
            span: start.merge(self.prev_span()),
        })
    }
    
    /// Parse class element.
    fn parse_class_element(&mut self) -> JsResult<ClassElement> {
        let start = self.current_span();
        
        let is_static = if self.check(&TokenKind::Static) {
            self.advance();
            true
        } else {
            false
        };
        
        // Static block
        if is_static && self.check(&TokenKind::LeftBrace) {
            let mut body = Vec::new();
            self.advance();
            while !self.check(&TokenKind::RightBrace) && !self.is_eof() {
                body.push(self.parse_statement()?);
            }
            self.expect(&TokenKind::RightBrace)?;
            return Ok(ClassElement::StaticBlock(StaticBlock {
                body,
                span: start.merge(self.prev_span()),
            }));
        }
        
        // Method kind
        let method_kind = if self.check(&TokenKind::Get) && !self.peek_is(&TokenKind::LeftParen) {
            self.advance();
            MethodKind::Get
        } else if self.check(&TokenKind::Set) && !self.peek_is(&TokenKind::LeftParen) {
            self.advance();
            MethodKind::Set
        } else {
            MethodKind::Method
        };
        
        // Computed key
        let computed = self.check(&TokenKind::LeftBracket);
        
        // Key
        let key = if computed {
            self.advance();
            let expr = self.parse_assignment_expression()?;
            self.expect(&TokenKind::RightBracket)?;
            expr
        } else {
            self.parse_property_name()?
        };
        
        // Check if constructor
        let final_kind = if !is_static && matches!(&key, Expression::Identifier(id) if id.name == "constructor") {
            MethodKind::Constructor
        } else {
            method_kind
        };
        
        // Method or property
        if self.check(&TokenKind::LeftParen) {
            // Method
            self.expect(&TokenKind::LeftParen)?;
            let params = self.parse_function_params()?;
            self.expect(&TokenKind::RightParen)?;
            let body = self.parse_block()?;
            
            Ok(ClassElement::Method(MethodDef {
                key,
                value: FunctionExpr {
                    id: None,
                    params,
                    body,
                    is_async: false,
                    is_generator: false,
                    span: start.merge(self.prev_span()),
                },
                kind: final_kind,
                computed,
                is_static,
                span: start.merge(self.prev_span()),
            }))
        } else {
            // Property
            let value = if self.check(&TokenKind::Assign) {
                self.advance();
                Some(self.parse_assignment_expression()?)
            } else {
                None
            };
            
            self.consume_semicolon()?;
            
            Ok(ClassElement::Property(PropertyDef {
                key,
                value,
                computed,
                is_static,
                span: start.merge(self.prev_span()),
            }))
        }
    }
    
    /// Parse expression statement.
    fn parse_expression_statement(&mut self) -> JsResult<Statement> {
        let start = self.current_span();
        let expression = self.parse_expression()?;
        self.consume_semicolon()?;
        
        Ok(Statement::Expression(ExpressionStmt {
            expression,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse expression.
    pub fn parse_expression(&mut self) -> JsResult<Expression> {
        self.parse_assignment_expression()
    }
    
    /// Parse assignment expression.
    fn parse_assignment_expression(&mut self) -> JsResult<Expression> {
        let start = self.current_span();
        let left = self.parse_conditional_expression()?;
        
        if self.current().kind.is_assignment() {
            let op = self.parse_assignment_operator()?;
            let right = self.parse_assignment_expression()?;
            
            return Ok(Expression::Assignment(AssignmentExpr {
                operator: op,
                left: AssignmentTarget::Simple(Box::new(left)),
                right: Box::new(right),
                span: start.merge(self.prev_span()),
            }));
        }
        
        Ok(left)
    }
    
    /// Parse assignment operator.
    fn parse_assignment_operator(&mut self) -> JsResult<AssignmentOp> {
        let op = match &self.current().kind {
            TokenKind::Assign => AssignmentOp::Assign,
            TokenKind::PlusAssign => AssignmentOp::AddAssign,
            TokenKind::MinusAssign => AssignmentOp::SubAssign,
            TokenKind::StarAssign => AssignmentOp::MulAssign,
            TokenKind::SlashAssign => AssignmentOp::DivAssign,
            TokenKind::PercentAssign => AssignmentOp::ModAssign,
            TokenKind::StarStarAssign => AssignmentOp::ExpAssign,
            TokenKind::LeftShiftAssign => AssignmentOp::LeftShiftAssign,
            TokenKind::RightShiftAssign => AssignmentOp::RightShiftAssign,
            TokenKind::UnsignedRightShiftAssign => AssignmentOp::UnsignedRightShiftAssign,
            TokenKind::AmpersandAssign => AssignmentOp::BitAndAssign,
            TokenKind::PipeAssign => AssignmentOp::BitOrAssign,
            TokenKind::CaretAssign => AssignmentOp::BitXorAssign,
            TokenKind::AmpersandAmpersandAssign => AssignmentOp::AndAssign,
            TokenKind::PipePipeAssign => AssignmentOp::OrAssign,
            TokenKind::QuestionQuestionAssign => AssignmentOp::NullishAssign,
            _ => return Err(JsError::syntax("Expected assignment operator")),
        };
        self.advance();
        Ok(op)
    }
    
    /// Parse conditional expression.
    fn parse_conditional_expression(&mut self) -> JsResult<Expression> {
        let start = self.current_span();
        let test = self.parse_binary_expression(0)?;
        
        if self.check(&TokenKind::Question) {
            self.advance();
            let consequent = self.parse_assignment_expression()?;
            self.expect(&TokenKind::Colon)?;
            let alternate = self.parse_assignment_expression()?;
            
            return Ok(Expression::Conditional(ConditionalExpr {
                test: Box::new(test),
                consequent: Box::new(consequent),
                alternate: Box::new(alternate),
                span: start.merge(self.prev_span()),
            }));
        }
        
        Ok(test)
    }
    
    /// Parse binary expression with precedence climbing.
    fn parse_binary_expression(&mut self, min_prec: u8) -> JsResult<Expression> {
        let start = self.current_span();
        let mut left = self.parse_unary_expression()?;
        
        loop {
            let (op, prec, is_logical) = match &self.current().kind {
                TokenKind::PipePipe => (BinaryOp::BitOr, 4, true),
                TokenKind::AmpersandAmpersand => (BinaryOp::BitAnd, 5, true),
                TokenKind::QuestionQuestion => (BinaryOp::BitOr, 4, true),
                TokenKind::Pipe => (BinaryOp::BitOr, 6, false),
                TokenKind::Caret => (BinaryOp::BitXor, 7, false),
                TokenKind::Ampersand => (BinaryOp::BitAnd, 8, false),
                TokenKind::Equal => (BinaryOp::Equal, 9, false),
                TokenKind::NotEqual => (BinaryOp::NotEqual, 9, false),
                TokenKind::StrictEqual => (BinaryOp::StrictEqual, 9, false),
                TokenKind::StrictNotEqual => (BinaryOp::StrictNotEqual, 9, false),
                TokenKind::LessThan => (BinaryOp::LessThan, 10, false),
                TokenKind::LessEqual => (BinaryOp::LessEqual, 10, false),
                TokenKind::GreaterThan => (BinaryOp::GreaterThan, 10, false),
                TokenKind::GreaterEqual => (BinaryOp::GreaterEqual, 10, false),
                TokenKind::In => (BinaryOp::In, 10, false),
                TokenKind::Instanceof => (BinaryOp::Instanceof, 10, false),
                TokenKind::LeftShift => (BinaryOp::LeftShift, 11, false),
                TokenKind::RightShift => (BinaryOp::RightShift, 11, false),
                TokenKind::UnsignedRightShift => (BinaryOp::UnsignedRightShift, 11, false),
                TokenKind::Plus => (BinaryOp::Add, 12, false),
                TokenKind::Minus => (BinaryOp::Sub, 12, false),
                TokenKind::Star => (BinaryOp::Mul, 13, false),
                TokenKind::Slash => (BinaryOp::Div, 13, false),
                TokenKind::Percent => (BinaryOp::Mod, 13, false),
                TokenKind::StarStar => (BinaryOp::Exp, 14, false),
                _ => break,
            };
            
            if prec < min_prec {
                break;
            }
            
            self.advance();
            let right = self.parse_binary_expression(prec + 1)?;
            
            left = if is_logical {
                let logical_op = match op {
                    BinaryOp::BitOr => LogicalOp::Or,
                    BinaryOp::BitAnd => LogicalOp::And,
                    _ => LogicalOp::Nullish,
                };
                Expression::Logical(LogicalExpr {
                    operator: logical_op,
                    left: Box::new(left),
                    right: Box::new(right),
                    span: start.merge(self.prev_span()),
                })
            } else {
                Expression::Binary(BinaryExpr {
                    operator: op,
                    left: Box::new(left),
                    right: Box::new(right),
                    span: start.merge(self.prev_span()),
                })
            };
        }
        
        Ok(left)
    }
    
    /// Parse unary expression.
    fn parse_unary_expression(&mut self) -> JsResult<Expression> {
        let start = self.current_span();
        
        match &self.current().kind {
            TokenKind::Bang => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression::Unary(UnaryExpr {
                    operator: UnaryOp::Not,
                    argument: Box::new(argument),
                    span: start.merge(self.prev_span()),
                }))
            }
            TokenKind::Tilde => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression::Unary(UnaryExpr {
                    operator: UnaryOp::BitNot,
                    argument: Box::new(argument),
                    span: start.merge(self.prev_span()),
                }))
            }
            TokenKind::Plus => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression::Unary(UnaryExpr {
                    operator: UnaryOp::Plus,
                    argument: Box::new(argument),
                    span: start.merge(self.prev_span()),
                }))
            }
            TokenKind::Minus => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression::Unary(UnaryExpr {
                    operator: UnaryOp::Minus,
                    argument: Box::new(argument),
                    span: start.merge(self.prev_span()),
                }))
            }
            TokenKind::Typeof => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression::Unary(UnaryExpr {
                    operator: UnaryOp::Typeof,
                    argument: Box::new(argument),
                    span: start.merge(self.prev_span()),
                }))
            }
            TokenKind::Void => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression::Unary(UnaryExpr {
                    operator: UnaryOp::Void,
                    argument: Box::new(argument),
                    span: start.merge(self.prev_span()),
                }))
            }
            TokenKind::Delete => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression::Unary(UnaryExpr {
                    operator: UnaryOp::Delete,
                    argument: Box::new(argument),
                    span: start.merge(self.prev_span()),
                }))
            }
            TokenKind::PlusPlus => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression::Update(UpdateExpr {
                    operator: UpdateOp::Increment,
                    argument: Box::new(argument),
                    prefix: true,
                    span: start.merge(self.prev_span()),
                }))
            }
            TokenKind::MinusMinus => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression::Update(UpdateExpr {
                    operator: UpdateOp::Decrement,
                    argument: Box::new(argument),
                    prefix: true,
                    span: start.merge(self.prev_span()),
                }))
            }
            TokenKind::Await => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression::Await(AwaitExpr {
                    argument: Box::new(argument),
                    span: start.merge(self.prev_span()),
                }))
            }
            _ => self.parse_update_expression(),
        }
    }
    
    /// Parse update expression.
    fn parse_update_expression(&mut self) -> JsResult<Expression> {
        let start = self.current_span();
        let argument = self.parse_left_hand_side_expression()?;
        
        match &self.current().kind {
            TokenKind::PlusPlus => {
                self.advance();
                Ok(Expression::Update(UpdateExpr {
                    operator: UpdateOp::Increment,
                    argument: Box::new(argument),
                    prefix: false,
                    span: start.merge(self.prev_span()),
                }))
            }
            TokenKind::MinusMinus => {
                self.advance();
                Ok(Expression::Update(UpdateExpr {
                    operator: UpdateOp::Decrement,
                    argument: Box::new(argument),
                    prefix: false,
                    span: start.merge(self.prev_span()),
                }))
            }
            _ => Ok(argument),
        }
    }
    
    /// Parse left-hand side expression.
    fn parse_left_hand_side_expression(&mut self) -> JsResult<Expression> {
        let start = self.current_span();
        
        // New expression
        if self.check(&TokenKind::New) {
            self.advance();
            let callee = self.parse_left_hand_side_expression()?;
            
            let arguments = if self.check(&TokenKind::LeftParen) {
                self.advance();
                let args = self.parse_arguments()?;
                self.expect(&TokenKind::RightParen)?;
                args
            } else {
                Vec::new()
            };
            
            return Ok(Expression::New(NewExpr {
                callee: Box::new(callee),
                arguments,
                span: start.merge(self.prev_span()),
            }));
        }
        
        let mut expr = self.parse_primary_expression()?;
        
        loop {
            match &self.current().kind {
                TokenKind::Dot => {
                    self.advance();
                    let property = self.parse_identifier()?;
                    expr = Expression::Member(MemberExpr {
                        object: Box::new(expr),
                        property: Box::new(Expression::Identifier(property)),
                        computed: false,
                        optional: false,
                        span: start.merge(self.prev_span()),
                    });
                }
                TokenKind::LeftBracket => {
                    self.advance();
                    let property = self.parse_expression()?;
                    self.expect(&TokenKind::RightBracket)?;
                    expr = Expression::Member(MemberExpr {
                        object: Box::new(expr),
                        property: Box::new(property),
                        computed: true,
                        optional: false,
                        span: start.merge(self.prev_span()),
                    });
                }
                TokenKind::LeftParen => {
                    self.advance();
                    let arguments = self.parse_arguments()?;
                    self.expect(&TokenKind::RightParen)?;
                    expr = Expression::Call(CallExpr {
                        callee: Box::new(expr),
                        arguments,
                        optional: false,
                        span: start.merge(self.prev_span()),
                    });
                }
                TokenKind::QuestionDot => {
                    self.advance();
                    if self.check(&TokenKind::LeftBracket) {
                        self.advance();
                        let property = self.parse_expression()?;
                        self.expect(&TokenKind::RightBracket)?;
                        expr = Expression::Member(MemberExpr {
                            object: Box::new(expr),
                            property: Box::new(property),
                            computed: true,
                            optional: true,
                            span: start.merge(self.prev_span()),
                        });
                    } else if self.check(&TokenKind::LeftParen) {
                        self.advance();
                        let arguments = self.parse_arguments()?;
                        self.expect(&TokenKind::RightParen)?;
                        expr = Expression::Call(CallExpr {
                            callee: Box::new(expr),
                            arguments,
                            optional: true,
                            span: start.merge(self.prev_span()),
                        });
                    } else {
                        let property = self.parse_identifier()?;
                        expr = Expression::Member(MemberExpr {
                            object: Box::new(expr),
                            property: Box::new(Expression::Identifier(property)),
                            computed: false,
                            optional: true,
                            span: start.merge(self.prev_span()),
                        });
                    }
                }
                _ => break,
            }
        }
        
        Ok(expr)
    }
    
    /// Parse primary expression.
    fn parse_primary_expression(&mut self) -> JsResult<Expression> {
        let start = self.current_span();
        
        match &self.current().kind {
            TokenKind::This => {
                self.advance();
                Ok(Expression::This(start))
            }
            TokenKind::Null => {
                self.advance();
                Ok(Expression::Literal(Literal::Null(start)))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expression::Literal(Literal::Boolean(true, start)))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expression::Literal(Literal::Boolean(false, start)))
            }
            TokenKind::Number(n) => {
                let n = *n;
                self.advance();
                Ok(Expression::Literal(Literal::Number(n, start)))
            }
            TokenKind::String(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expression::Literal(Literal::String(StringLiteral { value: s, span: start })))
            }
            TokenKind::Template(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expression::Template(TemplateLiteral {
                    quasis: vec![TemplateElement {
                        raw: s.clone(),
                        cooked: Some(s),
                        tail: true,
                        span: start,
                    }],
                    expressions: Vec::new(),
                    span: start,
                }))
            }
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(Expression::Identifier(Identifier { name, span: start }))
            }
            TokenKind::LeftParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(&TokenKind::RightParen)?;
                Ok(expr)
            }
            TokenKind::LeftBracket => self.parse_array_expression(),
            TokenKind::LeftBrace => self.parse_object_expression(),
            TokenKind::Function => {
                self.advance();
                let func = self.parse_function_expression()?;
                Ok(Expression::Function(func))
            }
            TokenKind::Class => self.parse_class_expression(),
            TokenKind::Async if self.peek_is(&TokenKind::Function) => {
                self.advance();
                self.advance();
                let mut func = self.parse_function_expression()?;
                func.is_async = true;
                Ok(Expression::Function(func))
            }
            _ => Err(JsError::syntax("Unexpected token")),
        }
    }
    
    /// Parse array expression.
    fn parse_array_expression(&mut self) -> JsResult<Expression> {
        let start = self.current_span();
        self.expect(&TokenKind::LeftBracket)?;
        
        let mut elements = Vec::new();
        
        while !self.check(&TokenKind::RightBracket) && !self.is_eof() {
            if self.check(&TokenKind::Comma) {
                elements.push(None);
            } else if self.check(&TokenKind::Ellipsis) {
                self.advance();
                let argument = self.parse_assignment_expression()?;
                elements.push(Some(Expression::Spread(SpreadElement {
                    argument: Box::new(argument),
                    span: start.merge(self.prev_span()),
                })));
            } else {
                elements.push(Some(self.parse_assignment_expression()?));
            }
            
            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        
        self.expect(&TokenKind::RightBracket)?;
        
        Ok(Expression::Array(ArrayExpr {
            elements,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse object expression.
    fn parse_object_expression(&mut self) -> JsResult<Expression> {
        let start = self.current_span();
        self.expect(&TokenKind::LeftBrace)?;
        
        let mut properties = Vec::new();
        
        while !self.check(&TokenKind::RightBrace) && !self.is_eof() {
            let prop_start = self.current_span();
            
            // Spread
            if self.check(&TokenKind::Ellipsis) {
                self.advance();
                let argument = self.parse_assignment_expression()?;
                properties.push(ObjectProperty::Spread(SpreadElement {
                    argument: Box::new(argument),
                    span: prop_start.merge(self.prev_span()),
                }));
            } else {
                // Computed key
                let computed = self.check(&TokenKind::LeftBracket);
                
                // Key
                let key = if computed {
                    self.advance();
                    let expr = self.parse_assignment_expression()?;
                    self.expect(&TokenKind::RightBracket)?;
                    expr
                } else {
                    self.parse_property_name()?
                };
                
                // Shorthand or method or normal
                if self.check(&TokenKind::LeftParen) {
                    // Method
                    self.advance();
                    let params = self.parse_function_params()?;
                    self.expect(&TokenKind::RightParen)?;
                    let body = self.parse_block()?;
                    
                    properties.push(ObjectProperty::Property {
                        key: key.clone(),
                        value: Expression::Function(FunctionExpr {
                            id: None,
                            params,
                            body,
                            is_async: false,
                            is_generator: false,
                            span: prop_start.merge(self.prev_span()),
                        }),
                        computed,
                        shorthand: false,
                        method: true,
                        span: prop_start.merge(self.prev_span()),
                    });
                } else if self.check(&TokenKind::Colon) {
                    // Normal property
                    self.advance();
                    let value = self.parse_assignment_expression()?;
                    
                    properties.push(ObjectProperty::Property {
                        key,
                        value,
                        computed,
                        shorthand: false,
                        method: false,
                        span: prop_start.merge(self.prev_span()),
                    });
                } else {
                    // Shorthand
                    properties.push(ObjectProperty::Property {
                        key: key.clone(),
                        value: key,
                        computed: false,
                        shorthand: true,
                        method: false,
                        span: prop_start.merge(self.prev_span()),
                    });
                }
            }
            
            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        
        self.expect(&TokenKind::RightBrace)?;
        
        Ok(Expression::Object(ObjectExpr {
            properties,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse property name.
    fn parse_property_name(&mut self) -> JsResult<Expression> {
        let span = self.current_span();
        
        match &self.current().kind {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(Expression::Identifier(Identifier { name, span }))
            }
            TokenKind::String(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expression::Literal(Literal::String(StringLiteral { value: s, span })))
            }
            TokenKind::Number(n) => {
                let n = *n;
                self.advance();
                Ok(Expression::Literal(Literal::Number(n, span)))
            }
            _ => Err(JsError::syntax("Expected property name")),
        }
    }
    
    /// Parse function expression.
    fn parse_function_expression(&mut self) -> JsResult<FunctionExpr> {
        let start = self.current_span();
        
        let is_generator = if self.check(&TokenKind::Star) {
            self.advance();
            true
        } else {
            false
        };
        
        let id = if let TokenKind::Identifier(name) = &self.current().kind {
            let span = self.current_span();
            let name = name.clone();
            self.advance();
            Some(Identifier { name, span })
        } else {
            None
        };
        
        self.expect(&TokenKind::LeftParen)?;
        let params = self.parse_function_params()?;
        self.expect(&TokenKind::RightParen)?;
        
        let body = self.parse_block()?;
        
        Ok(FunctionExpr {
            id,
            params,
            body,
            is_async: false,
            is_generator,
            span: start.merge(self.prev_span()),
        })
    }
    
    /// Parse class expression.
    fn parse_class_expression(&mut self) -> JsResult<Expression> {
        let start = self.current_span();
        self.expect(&TokenKind::Class)?;
        
        let id = if let TokenKind::Identifier(name) = &self.current().kind {
            let span = self.current_span();
            let name = name.clone();
            self.advance();
            Some(Identifier { name, span })
        } else {
            None
        };
        
        let super_class = if self.check(&TokenKind::Extends) {
            self.advance();
            Some(Box::new(self.parse_left_hand_side_expression()?))
        } else {
            None
        };
        
        let body = self.parse_class_body()?;
        
        Ok(Expression::Class(ClassExpr {
            id,
            super_class,
            body,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse arguments.
    fn parse_arguments(&mut self) -> JsResult<Vec<Expression>> {
        let mut args = Vec::new();
        
        while !self.check(&TokenKind::RightParen) && !self.is_eof() {
            if self.check(&TokenKind::Ellipsis) {
                self.advance();
                let argument = self.parse_assignment_expression()?;
                let span = self.prev_span();
                args.push(Expression::Spread(SpreadElement {
                    argument: Box::new(argument),
                    span,
                }));
            } else {
                args.push(self.parse_assignment_expression()?);
            }
            
            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        
        Ok(args)
    }
    
    /// Parse pattern.
    fn parse_pattern(&mut self) -> JsResult<Pattern> {
        match &self.current().kind {
            TokenKind::Identifier(name) => {
                let span = self.current_span();
                let name = name.clone();
                self.advance();
                Ok(Pattern::Identifier(Identifier { name, span }))
            }
            TokenKind::LeftBracket => self.parse_array_pattern(),
            TokenKind::LeftBrace => self.parse_object_pattern(),
            TokenKind::Ellipsis => {
                let start = self.current_span();
                self.advance();
                let argument = self.parse_pattern()?;
                Ok(Pattern::Rest(RestElement {
                    argument: Box::new(argument),
                    span: start.merge(self.prev_span()),
                }))
            }
            _ => Err(JsError::syntax("Expected pattern")),
        }
    }
    
    /// Parse array pattern.
    fn parse_array_pattern(&mut self) -> JsResult<Pattern> {
        let start = self.current_span();
        self.expect(&TokenKind::LeftBracket)?;
        
        let mut elements = Vec::new();
        
        while !self.check(&TokenKind::RightBracket) && !self.is_eof() {
            if self.check(&TokenKind::Comma) {
                elements.push(None);
            } else {
                elements.push(Some(self.parse_pattern()?));
            }
            
            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        
        self.expect(&TokenKind::RightBracket)?;
        
        Ok(Pattern::Array(ArrayPattern {
            elements,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse object pattern.
    fn parse_object_pattern(&mut self) -> JsResult<Pattern> {
        let start = self.current_span();
        self.expect(&TokenKind::LeftBrace)?;
        
        let mut properties = Vec::new();
        
        while !self.check(&TokenKind::RightBrace) && !self.is_eof() {
            let prop_start = self.current_span();
            
            if self.check(&TokenKind::Ellipsis) {
                self.advance();
                let argument = self.parse_pattern()?;
                properties.push(ObjectPatternProperty::Rest(RestElement {
                    argument: Box::new(argument),
                    span: prop_start.merge(self.prev_span()),
                }));
            } else {
                let computed = self.check(&TokenKind::LeftBracket);
                
                let key = if computed {
                    self.advance();
                    let expr = self.parse_assignment_expression()?;
                    self.expect(&TokenKind::RightBracket)?;
                    expr
                } else {
                    self.parse_property_name()?
                };
                
                if self.check(&TokenKind::Colon) {
                    self.advance();
                    let value = self.parse_pattern()?;
                    properties.push(ObjectPatternProperty::Property {
                        key,
                        value,
                        computed,
                        shorthand: false,
                        span: prop_start.merge(self.prev_span()),
                    });
                } else {
                    // Shorthand
                    if let Expression::Identifier(id) = &key {
                        properties.push(ObjectPatternProperty::Property {
                            key: key.clone(),
                            value: Pattern::Identifier(id.clone()),
                            computed: false,
                            shorthand: true,
                            span: prop_start.merge(self.prev_span()),
                        });
                    } else {
                        return Err(JsError::syntax("Expected identifier in shorthand pattern"));
                    }
                }
            }
            
            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        
        self.expect(&TokenKind::RightBrace)?;
        
        Ok(Pattern::Object(ObjectPattern {
            properties,
            span: start.merge(self.prev_span()),
        }))
    }
    
    /// Parse identifier.
    fn parse_identifier(&mut self) -> JsResult<Identifier> {
        if let TokenKind::Identifier(name) = &self.current().kind {
            let span = self.current_span();
            let name = name.clone();
            self.advance();
            Ok(Identifier { name, span })
        } else {
            Err(JsError::syntax("Expected identifier"))
        }
    }
    
    // Helper methods
    
    fn current(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }
    
    fn current_span(&self) -> Span {
        self.current().span
    }
    
    fn prev_span(&self) -> Span {
        if self.pos > 0 {
            self.tokens[self.pos - 1].span
        } else {
            Span::default()
        }
    }
    
    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len() || matches!(self.current().kind, TokenKind::Eof)
    }
    
    fn advance(&mut self) {
        if !self.is_eof() {
            self.pos += 1;
        }
    }
    
    fn check(&self, kind: &TokenKind) -> bool {
        core::mem::discriminant(&self.current().kind) == core::mem::discriminant(kind)
    }
    
    fn peek_is(&self, kind: &TokenKind) -> bool {
        if self.pos + 1 >= self.tokens.len() {
            return false;
        }
        core::mem::discriminant(&self.tokens[self.pos + 1].kind) == core::mem::discriminant(kind)
    }
    
    fn expect(&mut self, kind: &TokenKind) -> JsResult<()> {
        if self.check(kind) {
            self.advance();
            Ok(())
        } else {
            Err(JsError::syntax(alloc::format!(
                "Expected {:?}, got {:?}",
                kind,
                self.current().kind
            )))
        }
    }
    
    fn consume_semicolon(&mut self) -> JsResult<()> {
        if self.check(&TokenKind::Semicolon) {
            self.advance();
        }
        // ASI - automatic semicolon insertion (simplified)
        Ok(())
    }
}

/// Parse JavaScript source code into an AST.
pub fn parse(source: &str) -> JsResult<Program> {
    let mut parser = Parser::new(source)?;
    parser.parse_script()
}
