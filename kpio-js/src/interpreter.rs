//! JavaScript interpreter.
//!
//! Tree-walking interpreter for JavaScript AST.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use alloc::rc::Rc;
use core::cell::RefCell;
use libm::trunc;

use crate::ast::*;
use crate::error::{JsError, JsResult};
use crate::object::{
    JsObject, PropertyKey, Environment, Callable, UserFunction, 
    NativeFunction, PropertyDescriptor,
};
use crate::value::{Value, Completion};
use crate::builtin;

/// JavaScript interpreter.
pub struct Interpreter {
    /// Global environment.
    global_env: Rc<RefCell<Environment>>,
    /// Current environment.
    current_env: Rc<RefCell<Environment>>,
    /// Global object.
    global_object: Rc<RefCell<JsObject>>,
    /// Call stack depth.
    call_depth: usize,
    /// Maximum call stack depth.
    max_call_depth: usize,
}

impl Interpreter {
    /// Create a new interpreter.
    pub fn new() -> Self {
        let global_env = Rc::new(RefCell::new(Environment::global()));
        let global_object = Rc::new(RefCell::new(JsObject::new()));
        
        let mut interp = Interpreter {
            global_env: global_env.clone(),
            current_env: global_env,
            global_object,
            call_depth: 0,
            max_call_depth: 1000,
        };
        
        // Initialize built-in objects
        builtin::init(&mut interp);
        
        interp
    }
    
    /// Get the global object.
    pub fn global_object(&self) -> Rc<RefCell<JsObject>> {
        self.global_object.clone()
    }
    
    /// Get the global environment.
    pub fn global_env(&self) -> Rc<RefCell<Environment>> {
        self.global_env.clone()
    }
    
    /// Define a global variable.
    pub fn define_global(&mut self, name: &str, value: Value) {
        self.global_env.borrow_mut().initialize(name, value.clone()).ok();
        self.global_object.borrow_mut().set(PropertyKey::string(name), value).ok();
    }
    
    /// Define a native function.
    pub fn define_native_function(
        &mut self,
        name: &str,
        length: usize,
        func: fn(&Value, &[Value]) -> JsResult<Value>,
    ) {
        let callable = Callable::Native(NativeFunction {
            name: name.into(),
            length,
            func,
        });
        
        let obj = JsObject::function(callable);
        let value = Value::object(obj);
        
        self.define_global(name, value);
    }
    
    /// Execute a program.
    pub fn execute(&mut self, program: &Program) -> JsResult<Value> {
        let mut last_value = Value::undefined();
        
        for statement in &program.body {
            match self.execute_statement(statement)? {
                Completion::Normal(v) => last_value = v,
                Completion::Return(v) => return Ok(v),
                Completion::Throw(v) => {
                    return Err(self.value_to_error(v));
                }
                Completion::Break(_) => {
                    return Err(JsError::syntax("Illegal break statement"));
                }
                Completion::Continue(_) => {
                    return Err(JsError::syntax("Illegal continue statement"));
                }
            }
        }
        
        Ok(last_value)
    }
    
    /// Execute a statement.
    fn execute_statement(&mut self, stmt: &Statement) -> JsResult<Completion> {
        match stmt {
            Statement::Empty(_) => Ok(Completion::empty()),
            Statement::Expression(expr) => {
                let value = self.evaluate(&expr.expression)?;
                Ok(Completion::normal(value))
            }
            Statement::Block(block) => self.execute_block(block),
            Statement::Variable(decl) => self.execute_variable_declaration(decl),
            Statement::If(if_stmt) => self.execute_if(if_stmt),
            Statement::For(for_stmt) => self.execute_for(for_stmt),
            Statement::ForIn(for_in) => self.execute_for_in(for_in),
            Statement::ForOf(for_of) => self.execute_for_of(for_of),
            Statement::While(while_stmt) => self.execute_while(while_stmt),
            Statement::DoWhile(do_while) => self.execute_do_while(do_while),
            Statement::Switch(switch_stmt) => self.execute_switch(switch_stmt),
            Statement::Break(break_stmt) => {
                Ok(Completion::Break(break_stmt.label.as_ref().map(|l| l.name.clone())))
            }
            Statement::Continue(cont_stmt) => {
                Ok(Completion::Continue(cont_stmt.label.as_ref().map(|l| l.name.clone())))
            }
            Statement::Return(ret) => {
                let value = if let Some(expr) = &ret.argument {
                    self.evaluate(expr)?
                } else {
                    Value::undefined()
                };
                Ok(Completion::Return(value))
            }
            Statement::Throw(throw) => {
                let value = self.evaluate(&throw.argument)?;
                Ok(Completion::Throw(value))
            }
            Statement::Try(try_stmt) => self.execute_try(try_stmt),
            Statement::Function(func) => {
                self.execute_function_declaration(func)?;
                Ok(Completion::empty())
            }
            Statement::Class(class) => {
                self.execute_class_declaration(class)?;
                Ok(Completion::empty())
            }
            Statement::Debugger(_) => Ok(Completion::empty()),
            _ => Ok(Completion::empty()),
        }
    }
    
    /// Execute a block.
    fn execute_block(&mut self, block: &BlockStmt) -> JsResult<Completion> {
        let outer = self.current_env.clone();
        self.current_env = Rc::new(RefCell::new(Environment::child(outer.clone())));
        
        let mut result = Completion::empty();
        
        for stmt in &block.body {
            result = self.execute_statement(stmt)?;
            if !result.is_normal() {
                break;
            }
        }
        
        self.current_env = outer;
        Ok(result)
    }
    
    /// Execute variable declaration.
    fn execute_variable_declaration(&mut self, decl: &VariableDecl) -> JsResult<Completion> {
        let is_const = matches!(decl.kind, VariableKind::Const);
        
        for declarator in &decl.declarations {
            let value = if let Some(init) = &declarator.init {
                self.evaluate(init)?
            } else {
                if is_const {
                    return Err(JsError::syntax("Missing initializer in const declaration"));
                }
                Value::undefined()
            };
            
            self.bind_pattern(&declarator.id, value, !is_const)?;
        }
        
        Ok(Completion::empty())
    }
    
    /// Bind a pattern to a value.
    fn bind_pattern(&mut self, pattern: &Pattern, value: Value, mutable: bool) -> JsResult<()> {
        match pattern {
            Pattern::Identifier(id) => {
                self.current_env.borrow_mut().declare(id.name.clone(), mutable)?;
                self.current_env.borrow_mut().initialize(&id.name, value)?;
            }
            Pattern::Array(arr) => {
                let obj = value.to_object()?;
                for (i, elem) in arr.elements.iter().enumerate() {
                    if let Some(p) = elem {
                        let v = obj.borrow().get(&PropertyKey::Index(i as u32))?;
                        self.bind_pattern(p, v, mutable)?;
                    }
                }
            }
            Pattern::Object(obj) => {
                let val_obj = value.to_object()?;
                for prop in &obj.properties {
                    match prop {
                        ObjectPatternProperty::Property { key, value: pat, .. } => {
                            let key = self.property_key_from_expr(key)?;
                            let v = val_obj.borrow().get(&key)?;
                            self.bind_pattern(pat, v, mutable)?;
                        }
                        ObjectPatternProperty::Rest(rest) => {
                            // Simplified: just bind undefined for rest
                            self.bind_pattern(&rest.argument, Value::undefined(), mutable)?;
                        }
                    }
                }
            }
            Pattern::Assignment(_) => {
                // TODO: Handle default values
            }
            Pattern::Rest(rest) => {
                self.bind_pattern(&rest.argument, value, mutable)?;
            }
        }
        
        Ok(())
    }
    
    /// Execute if statement.
    fn execute_if(&mut self, if_stmt: &IfStmt) -> JsResult<Completion> {
        let test = self.evaluate(&if_stmt.test)?;
        
        if test.to_boolean() {
            self.execute_statement(&if_stmt.consequent)
        } else if let Some(alt) = &if_stmt.alternate {
            self.execute_statement(alt)
        } else {
            Ok(Completion::empty())
        }
    }
    
    /// Execute for loop.
    fn execute_for(&mut self, for_stmt: &ForStmt) -> JsResult<Completion> {
        let outer = self.current_env.clone();
        self.current_env = Rc::new(RefCell::new(Environment::child(outer.clone())));
        
        // Init
        if let Some(init) = &for_stmt.init {
            match init {
                ForInit::Variable(decl) => {
                    self.execute_variable_declaration(decl)?;
                }
                ForInit::Expression(expr) => {
                    self.evaluate(expr)?;
                }
            }
        }
        
        let mut result = Completion::empty();
        
        loop {
            // Test
            if let Some(test) = &for_stmt.test {
                let cond = self.evaluate(test)?;
                if !cond.to_boolean() {
                    break;
                }
            }
            
            // Body
            result = self.execute_statement(&for_stmt.body)?;
            match &result {
                Completion::Break(_) => {
                    result = Completion::empty();
                    break;
                }
                Completion::Continue(_) => {
                    // Continue to update
                }
                Completion::Return(_) | Completion::Throw(_) => break,
                _ => {}
            }
            
            // Update
            if let Some(update) = &for_stmt.update {
                self.evaluate(update)?;
            }
        }
        
        self.current_env = outer;
        Ok(result)
    }
    
    /// Execute for-in loop.
    fn execute_for_in(&mut self, for_in: &ForInStmt) -> JsResult<Completion> {
        let right = self.evaluate(&for_in.right)?;
        
        if right.is_nullish() {
            return Ok(Completion::empty());
        }
        
        let obj = right.to_object()?;
        let keys = obj.borrow().own_enumerable_keys();
        
        let outer = self.current_env.clone();
        self.current_env = Rc::new(RefCell::new(Environment::child(outer.clone())));
        
        let mut result = Completion::empty();
        
        for key in keys {
            let key_value = Value::string(key.to_string());
            
            // Bind the variable
            match &for_in.left {
                ForInLeft::Variable(decl) => {
                    if let Some(declarator) = decl.declarations.first() {
                        self.bind_pattern(&declarator.id, key_value, true)?;
                    }
                }
                ForInLeft::Pattern(pat) => {
                    self.bind_pattern(pat, key_value, true)?;
                }
            }
            
            result = self.execute_statement(&for_in.body)?;
            match &result {
                Completion::Break(_) => {
                    result = Completion::empty();
                    break;
                }
                Completion::Continue(_) => continue,
                Completion::Return(_) | Completion::Throw(_) => break,
                _ => {}
            }
        }
        
        self.current_env = outer;
        Ok(result)
    }
    
    /// Execute for-of loop.
    fn execute_for_of(&mut self, for_of: &ForOfStmt) -> JsResult<Completion> {
        let right = self.evaluate(&for_of.right)?;
        
        // Simplified: only handle arrays
        if !right.is_array() {
            return Err(JsError::type_error("Value is not iterable"));
        }
        
        let obj = right.to_object()?;
        let len = obj.borrow().array_length();
        
        let outer = self.current_env.clone();
        self.current_env = Rc::new(RefCell::new(Environment::child(outer.clone())));
        
        let mut result = Completion::empty();
        
        for i in 0..len {
            let value = obj.borrow().get(&PropertyKey::Index(i as u32))?;
            
            // Bind the variable
            match &for_of.left {
                ForInLeft::Variable(decl) => {
                    if let Some(declarator) = decl.declarations.first() {
                        self.bind_pattern(&declarator.id, value, true)?;
                    }
                }
                ForInLeft::Pattern(pat) => {
                    self.bind_pattern(pat, value, true)?;
                }
            }
            
            result = self.execute_statement(&for_of.body)?;
            match &result {
                Completion::Break(_) => {
                    result = Completion::empty();
                    break;
                }
                Completion::Continue(_) => continue,
                Completion::Return(_) | Completion::Throw(_) => break,
                _ => {}
            }
        }
        
        self.current_env = outer;
        Ok(result)
    }
    
    /// Execute while loop.
    fn execute_while(&mut self, while_stmt: &WhileStmt) -> JsResult<Completion> {
        let mut result = Completion::empty();
        
        loop {
            let test = self.evaluate(&while_stmt.test)?;
            if !test.to_boolean() {
                break;
            }
            
            result = self.execute_statement(&while_stmt.body)?;
            match &result {
                Completion::Break(_) => {
                    result = Completion::empty();
                    break;
                }
                Completion::Continue(_) => continue,
                Completion::Return(_) | Completion::Throw(_) => break,
                _ => {}
            }
        }
        
        Ok(result)
    }
    
    /// Execute do-while loop.
    fn execute_do_while(&mut self, do_while: &DoWhileStmt) -> JsResult<Completion> {
        let mut result;
        
        loop {
            result = self.execute_statement(&do_while.body)?;
            match &result {
                Completion::Break(_) => {
                    result = Completion::empty();
                    break;
                }
                Completion::Continue(_) => {}
                Completion::Return(_) | Completion::Throw(_) => break,
                _ => {}
            }
            
            let test = self.evaluate(&do_while.test)?;
            if !test.to_boolean() {
                break;
            }
        }
        
        Ok(result)
    }
    
    /// Execute switch statement.
    fn execute_switch(&mut self, switch_stmt: &SwitchStmt) -> JsResult<Completion> {
        let discriminant = self.evaluate(&switch_stmt.discriminant)?;
        
        let mut matched = false;
        let mut default_index = None;
        let mut result = Completion::empty();
        
        // Find matching case
        for (i, case) in switch_stmt.cases.iter().enumerate() {
            if case.test.is_none() {
                default_index = Some(i);
                continue;
            }
            
            if !matched {
                let test = self.evaluate(case.test.as_ref().unwrap())?;
                if discriminant.strict_equals(&test) {
                    matched = true;
                }
            }
            
            if matched {
                for stmt in &case.consequent {
                    result = self.execute_statement(stmt)?;
                    if let Completion::Break(_) = result {
                        return Ok(Completion::empty());
                    }
                    if !result.is_normal() {
                        return Ok(result);
                    }
                }
            }
        }
        
        // Default case
        if !matched {
            if let Some(idx) = default_index {
                for stmt in &switch_stmt.cases[idx].consequent {
                    result = self.execute_statement(stmt)?;
                    if let Completion::Break(_) = result {
                        return Ok(Completion::empty());
                    }
                    if !result.is_normal() {
                        return Ok(result);
                    }
                }
            }
        }
        
        Ok(result)
    }
    
    /// Execute try statement.
    fn execute_try(&mut self, try_stmt: &TryStmt) -> JsResult<Completion> {
        let result = self.execute_block(&try_stmt.block);
        
        match &result {
            Ok(Completion::Throw(ref value)) => {
                // Execute catch
                if let Some(handler) = &try_stmt.handler {
                    let outer = self.current_env.clone();
                    self.current_env = Rc::new(RefCell::new(Environment::child(outer.clone())));
                    
                    if let Some(param) = &handler.param {
                        let error_value = value.clone();
                        self.bind_pattern(param, error_value, true)?;
                    }
                    
                    let catch_result = self.execute_block(&handler.body);
                    self.current_env = outer;
                    
                    if let Some(finalizer) = &try_stmt.finalizer {
                        self.execute_block(finalizer)?;
                    }
                    
                    catch_result
                } else if let Some(finalizer) = &try_stmt.finalizer {
                    self.execute_block(finalizer)?;
                    Err(JsError::Error(format!("Uncaught exception")))
                } else {
                    Err(JsError::Error(format!("Uncaught exception")))
                }
            }
            Err(ref e) => {
                // Handle Err case - execute catch with error message
                if let Some(handler) = &try_stmt.handler {
                    let outer = self.current_env.clone();
                    self.current_env = Rc::new(RefCell::new(Environment::child(outer.clone())));
                    
                    if let Some(param) = &handler.param {
                        let error_value = Value::string(e.message());
                        self.bind_pattern(param, error_value, true)?;
                    }
                    
                    let catch_result = self.execute_block(&handler.body);
                    self.current_env = outer;
                    
                    if let Some(finalizer) = &try_stmt.finalizer {
                        self.execute_block(finalizer)?;
                    }
                    
                    catch_result
                } else if let Some(finalizer) = &try_stmt.finalizer {
                    self.execute_block(finalizer)?;
                    result
                } else {
                    result
                }
            }
            Ok(other) => {
                if let Some(finalizer) = &try_stmt.finalizer {
                    self.execute_block(finalizer)?;
                }
                Ok(other.clone())
            }
        }
    }
    
    /// Execute function declaration.
    fn execute_function_declaration(&mut self, func: &FunctionDecl) -> JsResult<()> {
        if let Some(id) = &func.id {
            let callable = Callable::UserDefined(UserFunction {
                name: Some(id.name.clone()),
                params: func.params.iter()
                    .filter_map(|p| match p {
                        Pattern::Identifier(id) => Some(id.name.clone()),
                        _ => None,
                    })
                    .collect(),
                body: func.body.clone(),
                environment: self.current_env.clone(),
                is_async: func.is_async,
                is_generator: func.is_generator,
            });
            
            let obj = JsObject::function(callable);
            let value = Value::object(obj);
            
            self.current_env.borrow_mut().declare(id.name.clone(), true)?;
            self.current_env.borrow_mut().initialize(&id.name, value)?;
        }
        
        Ok(())
    }
    
    /// Execute class declaration.
    fn execute_class_declaration(&mut self, class: &ClassDecl) -> JsResult<()> {
        if let Some(id) = &class.id {
            // Create constructor function
            let constructor = self.create_class_constructor(class)?;
            
            self.current_env.borrow_mut().declare(id.name.clone(), false)?;
            self.current_env.borrow_mut().initialize(&id.name, constructor)?;
        }
        
        Ok(())
    }
    
    /// Create a class constructor.
    fn create_class_constructor(&mut self, class: &ClassDecl) -> JsResult<Value> {
        // Find constructor method
        let mut constructor_body = None;
        let mut constructor_params = Vec::new();
        
        for elem in &class.body.body {
            if let ClassElement::Method(method) = elem {
                if method.kind == MethodKind::Constructor {
                    constructor_params = method.value.params.iter()
                        .filter_map(|p| match p {
                            Pattern::Identifier(id) => Some(id.name.clone()),
                            _ => None,
                        })
                        .collect();
                    constructor_body = Some(method.value.body.clone());
                    break;
                }
            }
        }
        
        let body = constructor_body.unwrap_or_else(|| BlockStmt {
            body: Vec::new(),
            span: class.span,
        });
        
        let callable = Callable::UserDefined(UserFunction {
            name: class.id.as_ref().map(|id| id.name.clone()),
            params: constructor_params,
            body,
            environment: self.current_env.clone(),
            is_async: false,
            is_generator: false,
        });
        
        let mut obj = JsObject::function(callable);
        
        // Add prototype
        let proto = Rc::new(RefCell::new(JsObject::new()));
        
        // Add methods to prototype
        for elem in &class.body.body {
            if let ClassElement::Method(method) = elem {
                if method.kind != MethodKind::Constructor {
                    let method_func = self.create_function_from_expr(&method.value)?;
                    let key = self.property_key_from_expr(&method.key)?;
                    proto.borrow_mut().set(key, method_func)?;
                }
            }
        }
        
        obj.define_property(
            PropertyKey::string("prototype"),
            PropertyDescriptor::data(Value::Object(proto), false, false, false),
        );
        
        Ok(Value::object(obj))
    }
    
    /// Evaluate an expression.
    pub fn evaluate(&mut self, expr: &Expression) -> JsResult<Value> {
        match expr {
            Expression::Identifier(id) => self.current_env.borrow().get(&id.name),
            Expression::Literal(lit) => self.evaluate_literal(lit),
            Expression::This(_) => Ok(self.current_env.borrow().get_this()),
            Expression::Array(arr) => self.evaluate_array(arr),
            Expression::Object(obj) => self.evaluate_object(obj),
            Expression::Function(func) => self.create_function_from_expr(func),
            Expression::Arrow(arrow) => self.evaluate_arrow(arrow),
            Expression::Class(class) => self.evaluate_class_expr(class),
            Expression::Member(member) => self.evaluate_member(member),
            Expression::Call(call) => self.evaluate_call(call),
            Expression::New(new) => self.evaluate_new(new),
            Expression::Update(update) => self.evaluate_update(update),
            Expression::Unary(unary) => self.evaluate_unary(unary),
            Expression::Binary(binary) => self.evaluate_binary(binary),
            Expression::Logical(logical) => self.evaluate_logical(logical),
            Expression::Conditional(cond) => self.evaluate_conditional(cond),
            Expression::Assignment(assign) => self.evaluate_assignment(assign),
            Expression::Sequence(seq) => self.evaluate_sequence(seq),
            Expression::Spread(spread) => self.evaluate(&spread.argument),
            Expression::Template(template) => self.evaluate_template(template),
            Expression::Await(await_expr) => self.evaluate(&await_expr.argument),
            Expression::Yield(_) => Ok(Value::undefined()),
            Expression::OptionalChain(_) => Ok(Value::undefined()),
            Expression::TaggedTemplate(_) => Ok(Value::undefined()),
        }
    }
    
    /// Evaluate a literal.
    fn evaluate_literal(&mut self, lit: &Literal) -> JsResult<Value> {
        match lit {
            Literal::Null(_) => Ok(Value::null()),
            Literal::Boolean(b, _) => Ok(Value::boolean(*b)),
            Literal::Number(n, _) => Ok(Value::number(*n)),
            Literal::String(s) => Ok(Value::string(s.value.clone())),
            Literal::BigInt(s, _) => {
                // Parse BigInt string to i64
                let n = s.parse::<i64>().unwrap_or(0);
                Ok(Value::BigInt(n))
            }
            Literal::RegExp { pattern, flags, .. } => {
                // Simplified: return string representation
                Ok(Value::string(alloc::format!("/{}/{}", pattern, flags)))
            }
        }
    }
    
    /// Evaluate array expression.
    fn evaluate_array(&mut self, arr: &ArrayExpr) -> JsResult<Value> {
        let mut elements = Vec::new();
        
        for elem in &arr.elements {
            if let Some(e) = elem {
                if let Expression::Spread(spread) = e {
                    let value = self.evaluate(&spread.argument)?;
                    if let Value::Object(obj) = value {
                        let len = obj.borrow().array_length();
                        for i in 0..len {
                            let v = obj.borrow().get(&PropertyKey::Index(i as u32))?;
                            elements.push(Some(v));
                        }
                    }
                } else {
                    elements.push(Some(self.evaluate(e)?));
                }
            } else {
                elements.push(None);
            }
        }
        
        Ok(Value::object(JsObject::array(elements)))
    }
    
    /// Evaluate object expression.
    fn evaluate_object(&mut self, obj_expr: &ObjectExpr) -> JsResult<Value> {
        let mut obj = JsObject::new();
        
        for prop in &obj_expr.properties {
            match prop {
                ObjectProperty::Property { key, value, shorthand, method, .. } => {
                    let key = if *shorthand || *method {
                        if let Expression::Identifier(id) = key {
                            PropertyKey::string(id.name.clone())
                        } else {
                            self.property_key_from_expr(key)?
                        }
                    } else {
                        self.property_key_from_expr(key)?
                    };
                    
                    let val = self.evaluate(value)?;
                    obj.set(key, val)?;
                }
                ObjectProperty::Spread(spread) => {
                    let value = self.evaluate(&spread.argument)?;
                    if let Value::Object(src) = value {
                        for key in src.borrow().own_enumerable_keys() {
                            let v = src.borrow().get(&key)?;
                            obj.set(key, v)?;
                        }
                    }
                }
            }
        }
        
        Ok(Value::object(obj))
    }
    
    /// Create a function from expression.
    fn create_function_from_expr(&mut self, func: &FunctionExpr) -> JsResult<Value> {
        let callable = Callable::UserDefined(UserFunction {
            name: func.id.as_ref().map(|id| id.name.clone()),
            params: func.params.iter()
                .filter_map(|p| match p {
                    Pattern::Identifier(id) => Some(id.name.clone()),
                    _ => None,
                })
                .collect(),
            body: func.body.clone(),
            environment: self.current_env.clone(),
            is_async: func.is_async,
            is_generator: func.is_generator,
        });
        
        Ok(Value::object(JsObject::function(callable)))
    }
    
    /// Evaluate arrow function.
    fn evaluate_arrow(&mut self, arrow: &ArrowFunctionExpr) -> JsResult<Value> {
        let body = match &arrow.body {
            ArrowFunctionBody::Expression(expr) => {
                BlockStmt {
                    body: vec![Statement::Return(ReturnStmt {
                        argument: Some(expr.as_ref().clone()),
                        span: arrow.span,
                    })],
                    span: arrow.span,
                }
            }
            ArrowFunctionBody::Block(block) => block.clone(),
        };
        
        let callable = Callable::UserDefined(UserFunction {
            name: None,
            params: arrow.params.iter()
                .filter_map(|p| match p {
                    Pattern::Identifier(id) => Some(id.name.clone()),
                    _ => None,
                })
                .collect(),
            body,
            environment: self.current_env.clone(),
            is_async: arrow.is_async,
            is_generator: false,
        });
        
        Ok(Value::object(JsObject::function(callable)))
    }
    
    /// Evaluate class expression.
    fn evaluate_class_expr(&mut self, class: &ClassExpr) -> JsResult<Value> {
        let decl = ClassDecl {
            id: class.id.clone(),
            super_class: class.super_class.as_ref().map(|e| e.as_ref().clone()),
            body: class.body.clone(),
            span: class.span,
        };
        
        self.create_class_constructor(&decl)
    }
    
    /// Evaluate member expression.
    fn evaluate_member(&mut self, member: &MemberExpr) -> JsResult<Value> {
        let object = self.evaluate(&member.object)?;
        
        if member.optional && object.is_nullish() {
            return Ok(Value::undefined());
        }
        
        let key = if member.computed {
            let prop = self.evaluate(&member.property)?;
            self.value_to_property_key(&prop)?
        } else if let Expression::Identifier(id) = member.property.as_ref() {
            PropertyKey::string(id.name.clone())
        } else {
            return Err(JsError::syntax("Invalid member expression"));
        };
        
        object.get(&key)
    }
    
    /// Evaluate call expression.
    fn evaluate_call(&mut self, call: &CallExpr) -> JsResult<Value> {
        // Check call depth
        if self.call_depth >= self.max_call_depth {
            return Err(JsError::range("Maximum call stack size exceeded"));
        }
        
        // Get the function and this value
        let (func, this_value) = if let Expression::Member(member) = call.callee.as_ref() {
            let obj = self.evaluate(&member.object)?;
            let key = if member.computed {
                let prop = self.evaluate(&member.property)?;
                self.value_to_property_key(&prop)?
            } else if let Expression::Identifier(id) = member.property.as_ref() {
                PropertyKey::string(id.name.clone())
            } else {
                return Err(JsError::syntax("Invalid member expression"));
            };
            
            let func = obj.get(&key)?;
            (func, obj)
        } else {
            let func = self.evaluate(&call.callee)?;
            (func, Value::undefined())
        };
        
        if call.optional && func.is_nullish() {
            return Ok(Value::undefined());
        }
        
        // Evaluate arguments
        let args: Vec<Value> = call.arguments.iter()
            .map(|arg| self.evaluate(arg))
            .collect::<JsResult<Vec<_>>>()?;
        
        self.call_function(&func, &this_value, &args)
    }
    
    /// Call a function.
    pub fn call_function(&mut self, func: &Value, this_value: &Value, args: &[Value]) -> JsResult<Value> {
        if let Value::Object(obj) = func {
            if let Some(callable) = obj.borrow().callable() {
                self.call_depth += 1;
                let result = self.call_callable(callable.clone(), this_value, args);
                self.call_depth -= 1;
                return result;
            }
        }
        
        Err(JsError::type_error("Value is not a function"))
    }
    
    /// Call a callable.
    fn call_callable(&mut self, callable: Callable, this_value: &Value, args: &[Value]) -> JsResult<Value> {
        match callable {
            Callable::Native(native) => {
                (native.func)(this_value, args)
            }
            Callable::UserDefined(user_func) => {
                self.call_user_function(&user_func, this_value, args)
            }
            Callable::Bound(bound) => {
                let mut all_args = bound.bound_args.clone();
                all_args.extend_from_slice(args);
                self.call_callable(*bound.target, &bound.bound_this, &all_args)
            }
        }
    }
    
    /// Call a user-defined function.
    fn call_user_function(&mut self, func: &UserFunction, this_value: &Value, args: &[Value]) -> JsResult<Value> {
        let outer = self.current_env.clone();
        self.current_env = Rc::new(RefCell::new(
            Environment::function(func.environment.clone(), this_value.clone())
        ));
        
        // Bind parameters
        for (i, param) in func.params.iter().enumerate() {
            let value = args.get(i).cloned().unwrap_or(Value::undefined());
            self.current_env.borrow_mut().declare(param.clone(), true)?;
            self.current_env.borrow_mut().initialize(param, value)?;
        }
        
        // Create arguments object
        let args_array = JsObject::array(args.iter().cloned().map(Some).collect());
        self.current_env.borrow_mut().declare("arguments".into(), true)?;
        self.current_env.borrow_mut().initialize("arguments", Value::object(args_array))?;
        
        // Execute body
        let result = self.execute_block(&func.body);
        
        self.current_env = outer;
        
        match result? {
            Completion::Return(v) => Ok(v),
            Completion::Throw(v) => Err(self.value_to_error(v)),
            _ => Ok(Value::undefined()),
        }
    }
    
    /// Evaluate new expression.
    fn evaluate_new(&mut self, new: &NewExpr) -> JsResult<Value> {
        let constructor = self.evaluate(&new.callee)?;
        
        if let Value::Object(obj) = &constructor {
            if !obj.borrow().is_constructable() {
                return Err(JsError::type_error("Value is not a constructor"));
            }
        } else {
            return Err(JsError::type_error("Value is not a constructor"));
        }
        
        // Create new object
        let new_obj = Rc::new(RefCell::new(JsObject::new()));
        
        // Set prototype
        if let Value::Object(func_obj) = &constructor {
            let proto = func_obj.borrow().get(&PropertyKey::string("prototype"))?;
            if let Value::Object(proto_obj) = proto {
                new_obj.borrow_mut().set_prototype(Some(proto_obj));
            }
        }
        
        // Call constructor
        let args: Vec<Value> = new.arguments.iter()
            .map(|arg| self.evaluate(arg))
            .collect::<JsResult<Vec<_>>>()?;
        
        let this = Value::Object(new_obj.clone());
        let result = self.call_function(&constructor, &this, &args)?;
        
        // Return the result if it's an object, otherwise return the new object
        if result.is_object() {
            Ok(result)
        } else {
            Ok(this)
        }
    }
    
    /// Evaluate update expression.
    fn evaluate_update(&mut self, update: &UpdateExpr) -> JsResult<Value> {
        let current = self.evaluate(&update.argument)?;
        let num = current.to_number()?;
        
        let new_value = match update.operator {
            UpdateOp::Increment => Value::number(num + 1.0),
            UpdateOp::Decrement => Value::number(num - 1.0),
        };
        
        self.assign_to_expr(&update.argument, new_value.clone())?;
        
        if update.prefix {
            Ok(new_value)
        } else {
            Ok(Value::number(num))
        }
    }
    
    /// Evaluate unary expression.
    fn evaluate_unary(&mut self, unary: &UnaryExpr) -> JsResult<Value> {
        match unary.operator {
            UnaryOp::Not => {
                let val = self.evaluate(&unary.argument)?;
                Ok(Value::boolean(!val.to_boolean()))
            }
            UnaryOp::BitNot => {
                let val = self.evaluate(&unary.argument)?;
                let n = val.to_i32()?;
                Ok(Value::number(!n as f64))
            }
            UnaryOp::Plus => {
                let val = self.evaluate(&unary.argument)?;
                Ok(Value::number(val.to_number()?))
            }
            UnaryOp::Minus => {
                let val = self.evaluate(&unary.argument)?;
                Ok(Value::number(-val.to_number()?))
            }
            UnaryOp::Typeof => {
                let val = self.evaluate(&unary.argument)?;
                Ok(Value::string(val.type_of()))
            }
            UnaryOp::Void => {
                self.evaluate(&unary.argument)?;
                Ok(Value::undefined())
            }
            UnaryOp::Delete => {
                // Simplified: just return true
                Ok(Value::boolean(true))
            }
        }
    }
    
    /// Evaluate binary expression.
    fn evaluate_binary(&mut self, binary: &BinaryExpr) -> JsResult<Value> {
        let left = self.evaluate(&binary.left)?;
        let right = self.evaluate(&binary.right)?;
        
        match binary.operator {
            BinaryOp::Add => {
                // String concatenation or numeric addition
                if left.is_string() || right.is_string() {
                    let l = left.to_string()?;
                    let r = right.to_string()?;
                    Ok(Value::string(l + &r))
                } else {
                    let l = left.to_number()?;
                    let r = right.to_number()?;
                    Ok(Value::number(l + r))
                }
            }
            BinaryOp::Sub => {
                let l = left.to_number()?;
                let r = right.to_number()?;
                Ok(Value::number(l - r))
            }
            BinaryOp::Mul => {
                let l = left.to_number()?;
                let r = right.to_number()?;
                Ok(Value::number(l * r))
            }
            BinaryOp::Div => {
                let l = left.to_number()?;
                let r = right.to_number()?;
                Ok(Value::number(l / r))
            }
            BinaryOp::Mod => {
                let l = left.to_number()?;
                let r = right.to_number()?;
                Ok(Value::number(l % r))
            }
            BinaryOp::Exp => {
                let l = left.to_number()?;
                let r = right.to_number()?;
                Ok(Value::number(libm::pow(l, r)))
            }
            BinaryOp::LeftShift => {
                let l = left.to_i32()?;
                let r = right.to_u32()? & 0x1f;
                Ok(Value::number((l << r) as f64))
            }
            BinaryOp::RightShift => {
                let l = left.to_i32()?;
                let r = right.to_u32()? & 0x1f;
                Ok(Value::number((l >> r) as f64))
            }
            BinaryOp::UnsignedRightShift => {
                let l = left.to_u32()?;
                let r = right.to_u32()? & 0x1f;
                Ok(Value::number((l >> r) as f64))
            }
            BinaryOp::BitAnd => {
                let l = left.to_i32()?;
                let r = right.to_i32()?;
                Ok(Value::number((l & r) as f64))
            }
            BinaryOp::BitOr => {
                let l = left.to_i32()?;
                let r = right.to_i32()?;
                Ok(Value::number((l | r) as f64))
            }
            BinaryOp::BitXor => {
                let l = left.to_i32()?;
                let r = right.to_i32()?;
                Ok(Value::number((l ^ r) as f64))
            }
            BinaryOp::Equal => Ok(Value::boolean(left.abstract_equals(&right)?)),
            BinaryOp::NotEqual => Ok(Value::boolean(!left.abstract_equals(&right)?)),
            BinaryOp::StrictEqual => Ok(Value::boolean(left.strict_equals(&right))),
            BinaryOp::StrictNotEqual => Ok(Value::boolean(!left.strict_equals(&right))),
            BinaryOp::LessThan => {
                let l = left.to_number()?;
                let r = right.to_number()?;
                Ok(Value::boolean(l < r))
            }
            BinaryOp::LessEqual => {
                let l = left.to_number()?;
                let r = right.to_number()?;
                Ok(Value::boolean(l <= r))
            }
            BinaryOp::GreaterThan => {
                let l = left.to_number()?;
                let r = right.to_number()?;
                Ok(Value::boolean(l > r))
            }
            BinaryOp::GreaterEqual => {
                let l = left.to_number()?;
                let r = right.to_number()?;
                Ok(Value::boolean(l >= r))
            }
            BinaryOp::In => {
                if let Value::Object(obj) = &right {
                    let key = self.value_to_property_key(&left)?;
                    Ok(Value::boolean(obj.borrow().has(&key)))
                } else {
                    Err(JsError::type_error("Cannot use 'in' operator with non-object"))
                }
            }
            BinaryOp::Instanceof => {
                // Simplified
                Ok(Value::boolean(false))
            }
        }
    }
    
    /// Evaluate logical expression.
    fn evaluate_logical(&mut self, logical: &LogicalExpr) -> JsResult<Value> {
        let left = self.evaluate(&logical.left)?;
        
        match logical.operator {
            LogicalOp::And => {
                if !left.to_boolean() {
                    Ok(left)
                } else {
                    self.evaluate(&logical.right)
                }
            }
            LogicalOp::Or => {
                if left.to_boolean() {
                    Ok(left)
                } else {
                    self.evaluate(&logical.right)
                }
            }
            LogicalOp::Nullish => {
                if left.is_nullish() {
                    self.evaluate(&logical.right)
                } else {
                    Ok(left)
                }
            }
        }
    }
    
    /// Evaluate conditional expression.
    fn evaluate_conditional(&mut self, cond: &ConditionalExpr) -> JsResult<Value> {
        let test = self.evaluate(&cond.test)?;
        
        if test.to_boolean() {
            self.evaluate(&cond.consequent)
        } else {
            self.evaluate(&cond.alternate)
        }
    }
    
    /// Evaluate assignment expression.
    fn evaluate_assignment(&mut self, assign: &AssignmentExpr) -> JsResult<Value> {
        let target_expr = match &assign.left {
            AssignmentTarget::Simple(expr) => expr.as_ref().clone(),
            AssignmentTarget::Pattern(pat) => {
                let value = self.evaluate(&assign.right)?;
                self.bind_pattern(pat, value.clone(), true)?;
                return Ok(value);
            }
        };
        
        let value = match assign.operator {
            AssignmentOp::Assign => self.evaluate(&assign.right)?,
            _ => {
                let current = self.evaluate(&target_expr)?;
                let right = self.evaluate(&assign.right)?;
                
                match assign.operator {
                    AssignmentOp::AddAssign => {
                        if current.is_string() || right.is_string() {
                            let l = current.to_string()?;
                            let r = right.to_string()?;
                            Value::string(l + &r)
                        } else {
                            Value::number(current.to_number()? + right.to_number()?)
                        }
                    }
                    AssignmentOp::SubAssign => Value::number(current.to_number()? - right.to_number()?),
                    AssignmentOp::MulAssign => Value::number(current.to_number()? * right.to_number()?),
                    AssignmentOp::DivAssign => Value::number(current.to_number()? / right.to_number()?),
                    AssignmentOp::ModAssign => Value::number(current.to_number()? % right.to_number()?),
                    _ => self.evaluate(&assign.right)?,
                }
            }
        };
        
        self.assign_to_expr(&target_expr, value.clone())?;
        Ok(value)
    }
    
    /// Assign a value to an expression.
    fn assign_to_expr(&mut self, expr: &Expression, value: Value) -> JsResult<()> {
        match expr {
            Expression::Identifier(id) => {
                self.current_env.borrow_mut().set(&id.name, value)?;
            }
            Expression::Member(member) => {
                let object = self.evaluate(&member.object)?;
                let key = if member.computed {
                    let prop = self.evaluate(&member.property)?;
                    self.value_to_property_key(&prop)?
                } else if let Expression::Identifier(id) = member.property.as_ref() {
                    PropertyKey::string(id.name.clone())
                } else {
                    return Err(JsError::syntax("Invalid assignment target"));
                };
                
                object.set(key, value)?;
            }
            _ => return Err(JsError::syntax("Invalid assignment target")),
        }
        
        Ok(())
    }
    
    /// Evaluate sequence expression.
    fn evaluate_sequence(&mut self, seq: &SequenceExpr) -> JsResult<Value> {
        let mut result = Value::undefined();
        
        for expr in &seq.expressions {
            result = self.evaluate(expr)?;
        }
        
        Ok(result)
    }
    
    /// Evaluate template literal.
    fn evaluate_template(&mut self, template: &TemplateLiteral) -> JsResult<Value> {
        let mut result = String::new();
        
        for (i, quasi) in template.quasis.iter().enumerate() {
            if let Some(cooked) = &quasi.cooked {
                result.push_str(cooked);
            }
            
            if i < template.expressions.len() {
                let value = self.evaluate(&template.expressions[i])?;
                result.push_str(&value.to_string()?);
            }
        }
        
        Ok(Value::string(result))
    }
    
    // Helper methods
    
    /// Convert an expression to a property key.
    fn property_key_from_expr(&mut self, expr: &Expression) -> JsResult<PropertyKey> {
        match expr {
            Expression::Identifier(id) => Ok(PropertyKey::string(id.name.clone())),
            Expression::Literal(Literal::String(s)) => Ok(PropertyKey::string(s.value.clone())),
            Expression::Literal(Literal::Number(n, _)) => {
                if *n >= 0.0 && *n < u32::MAX as f64 && trunc(*n) == *n {
                    Ok(PropertyKey::Index(*n as u32))
                } else {
                    Ok(PropertyKey::string(alloc::format!("{}", n)))
                }
            }
            _ => {
                let value = self.evaluate(expr)?;
                self.value_to_property_key(&value)
            }
        }
    }
    
    /// Convert a value to a property key.
    fn value_to_property_key(&self, value: &Value) -> JsResult<PropertyKey> {
        match value {
            Value::String(s) => Ok(PropertyKey::string(s.clone())),
            Value::Number(n) => {
                if *n >= 0.0 && *n < u32::MAX as f64 && trunc(*n) == *n {
                    Ok(PropertyKey::Index(*n as u32))
                } else {
                    Ok(PropertyKey::string(alloc::format!("{}", n)))
                }
            }
            Value::Symbol(s) => Ok(PropertyKey::Symbol(s.clone())),
            _ => {
                let s = value.to_string()?;
                Ok(PropertyKey::string(s))
            }
        }
    }
    
    /// Convert a value to an error.
    fn value_to_error(&self, value: Value) -> JsError {
        if let Value::Object(obj) = &value {
            if let ObjectKind::Error { name, message } = obj.borrow().kind() {
                return JsError::error(name.clone(), message.clone());
            }
        }
        
        let message = value.to_string().unwrap_or_else(|_| "Unknown error".into());
        JsError::error("Error", message)
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

use crate::object::ObjectKind;

/// JavaScript engine - alias for Interpreter.
pub type Engine = Interpreter;

impl Engine {
    /// Evaluate JavaScript source code.
    pub fn eval(&mut self, source: &str) -> JsResult<Value> {
        let program = crate::parser::parse(source)?;
        self.execute(&program)
    }
    
    /// Set a global variable.
    pub fn set_global(&mut self, name: String, value: Value) {
        self.define_global(&name, value);
    }
    
    /// Get a global variable.
    pub fn get_global(&self, name: &str) -> JsResult<Value> {
        self.global_env.borrow().get(name)
    }
}
