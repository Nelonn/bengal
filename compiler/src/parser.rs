use crate::lexer::Token;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn unknown() -> Self {
        Self { line: 0, column: 0 }
    }
}

/// Parser error with span information
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub file: String,
    pub source_line: Option<String>,
}

impl ParseError {
    pub fn new(message: String, file: String, line: usize, column: usize) -> Self {
        Self { message, file, line, column, source_line: None }
    }

    pub fn with_source(mut self, source_line: String) -> Self {
        self.source_line = Some(source_line);
        self
    }

    pub fn format(&self) -> String {
        let base = format!("{}:{}:{}: error: {}", self.file, self.line, self.column, self.message);
        if let Some(ref line) = self.source_line {
            let pointer = " ".repeat(self.column.saturating_sub(1)) + "^";
            format!("{}\n  {}\n  {}", base, line, pointer)
        } else {
            base
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportKind {
    /// import std.io - brings io into scope, use io.println()
    Simple,
    /// import std - brings std into scope, use std.io.println()
    Module,
    /// import std.io.println - brings println into scope, use println()
    Member,
    /// import std.io as io - brings io into scope as alias, use io.println()
    Aliased(String),
    /// import std.io.* - brings all members of std.io into scope, use println()
    Wildcard,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Module { path: Vec<String>, span: Span },
    Import { path: Vec<String>, kind: ImportKind, span: Span },
    Class(ClassDef),
    Interface(InterfaceDef),
    Enum(EnumDef),
    Function(FunctionDef),
    TypeAlias(TypeAliasDef),
    Let { name: String, type_annotation: Option<String>, expr: Expr, private: bool, span: Span },
    Const { name: String, type_annotation: Option<String>, expr: Expr, private: bool, span: Span },
    Assign { name: String, expr: Expr, span: Span },
    AugAssign { target: AugAssignTarget, op: AugOp, expr: Expr, span: Span },
    Return { expr: Option<Expr>, span: Span },
    Expr(Expr),
    If { condition: Expr, then_branch: Block, else_branch: Option<Block>, span: Span },
    For { var_name: String, range: Box<Expr>, body: Block, span: Span },
    While { condition: Expr, body: Block, span: Span },
    Break(Span),
    Continue(Span),
    TryCatch { try_block: Block, catch_var: String, catch_block: Block, span: Span },
    Throw { expr: Expr, span: Span },
    Spawn { expr: Expr, span: Span },
}

#[derive(Debug, Clone)]
pub enum AugAssignTarget {
    Variable(String),
    Field { object: Expr, name: String },
}

pub type Block = Vec<Stmt>;

#[derive(Debug, Clone)]
pub struct TypeAliasDef {
    pub name: String,
    pub type_params: Vec<String>,
    pub aliased_type: String,
    pub private: bool,
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub private: bool,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub value: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<String>,
    pub return_optional: bool,
    pub body: Block,
    pub is_native: bool,
    pub private: bool,
    pub type_params: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub name: String,
    pub type_params: Vec<String>,
    pub parent_interfaces: Vec<String>,
    pub fields: Vec<Field>,
    pub methods: Vec<Method>,
    pub nested_classes: Vec<ClassDef>,
    pub nested_interfaces: Vec<InterfaceDef>,
    pub is_native: bool,
    pub private: bool,
}

#[derive(Debug, Clone)]
pub struct InterfaceDef {
    pub name: String,
    pub type_params: Vec<String>,
    pub parent_interfaces: Vec<String>,
    pub methods: Vec<Method>,
    pub nested_classes: Vec<ClassDef>,
    pub nested_interfaces: Vec<InterfaceDef>,
    pub private: bool,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub type_name: String,
    pub default: Option<Expr>,
    pub private: bool,
    pub is_static: bool,
}

#[derive(Debug, Clone)]
pub struct Method {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<String>,
    pub return_optional: bool,
    pub body: Block,
    pub private: bool,
    pub is_native: bool,
    pub is_static: bool,
    pub type_params: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub type_name: Option<String>,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct ObjectField {
    pub name: String,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct LambdaParam {
    pub name: String,
    pub type_name: Option<String>,
}

/// Represents a function call argument, either positional or named
#[derive(Debug, Clone)]
pub enum CallArg {
    Positional(Expr),
    Named { name: String, value: Expr, span: Span },
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    Variable { name: String, span: Span },
    Binary { left: Box<Expr>, op: BinaryOp, right: Box<Expr>, span: Span },
    Unary { op: UnaryOp, expr: Box<Expr>, span: Span },
    Call { callee: Box<Expr>, args: Vec<CallArg>, span: Span },
    Get { object: Box<Expr>, name: String, span: Span },
    Set { object: Box<Expr>, name: String, value: Box<Expr>, span: Span },
    Interpolated { parts: Vec<InterpPart>, span: Span },
    Range { start: Box<Expr>, end: Box<Expr>, span: Span },
    Cast { expr: Box<Expr>, target_type: CastType, span: Span },
    Array { elements: Vec<Expr>, span: Span },
    Index { object: Box<Expr>, index: Box<Expr>, span: Span },
    ObjectLiteral { fields: Vec<ObjectField>, span: Span, inferred_type: Option<String> },
    Lambda { params: Vec<LambdaParam>, return_type: Option<String>, body: Block, span: Span },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CastType {
    Int,
    Float,
    Str,
    Bool,
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Int64,
    UInt64,
    Float32,
    Float64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    String(String, Span),
    Int(i64, Span),
    Float(f64, Span),
    Bool(bool, Span),
    Null(Span),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOp {
    Equal,
    NotEqual,
    And,
    Or,
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Pow,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AugOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Not,
    PrefixIncrement,
    PrefixDecrement,
    PostfixIncrement,
    PostfixDecrement,
    Decrement, // Keep for backward compatibility if used elsewhere, but we'll use PostfixDecrement for x--
    Negate,    // Unary minus for negative numbers: -5, -3.14
    BitNot,    // Bitwise NOT: ~x
}

#[derive(Debug, Clone)]
pub enum InterpPart {
    Text(String),
    Expr(Expr),
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    source: String,
    path: String,
    token_positions: Vec<usize>,  // Position in source for each token
    errors: RefCell<Vec<ParseError>>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, source: &str, path: &str, token_positions: Vec<usize>) -> Self {
        Self {
            tokens,
            pos: 0,
            source: source.to_string(),
            path: path.to_string(),
            token_positions,
            errors: RefCell::new(Vec::new()),
        }
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.borrow().is_empty()
    }

    pub fn get_errors(&self) -> Vec<ParseError> {
        self.errors.borrow().clone()
    }

    pub fn format_errors(&self) -> String {
        let errors = self.errors.borrow();
        let error_messages: Vec<String> = errors
            .iter()
            .map(|e| e.format())
            .collect();
        error_messages.join("\n")
    }

    fn compute_span(&self, token_idx: usize) -> Span {
        let pos = self.token_positions.get(token_idx).copied().unwrap_or(0);
        let source_up_to_pos = &self.source[..pos.min(self.source.len())];
        let line = source_up_to_pos.matches('\n').count() + 1;

        // Find column (position after last newline)
        let last_newline = source_up_to_pos.rfind('\n').map(|p| p + 1).unwrap_or(0);
        let column = pos - last_newline + 1;

        Span { line, column }
    }

    fn current_span(&self) -> Span {
        self.compute_span(self.pos)
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn peek_ahead(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.pos + n)
    }

    fn advance(&mut self) -> Token {
        let token = self.peek().clone();
        self.pos += 1;
        token
    }

    fn add_error(&self, message: String) {
        let span = self.compute_span(self.pos);
        let filename = std::path::Path::new(&self.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.path)
            .to_string();
        
        let mut error = ParseError::new(message, filename, span.line, span.column);
        
        // Capture the source line for the error snippet
        let source_line = self.get_source_line(span.line);
        if let Some(line) = source_line {
            error = error.with_source(line);
        }
        
        self.errors.borrow_mut().push(error);
    }

    fn get_source_line(&self, line_num: usize) -> Option<String> {
        self.source.lines().nth(line_num.saturating_sub(1)).map(|s| s.to_string())
    }

    fn error(&self, message: &str) -> Result<Stmt, String> {
        self.add_error(message.to_string());
        Err(message.to_string())
    }

    fn error_expr(&self, message: &str) -> Result<Expr, String> {
        self.add_error(message.to_string());
        Err(message.to_string())
    }

    fn error_generic<T>(&self, message: &str) -> Result<T, String> {
        self.add_error(message.to_string());
        Err(message.to_string())
    }

    fn check(&self, token: &Token) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(token)
    }

    fn match_token(&mut self, token: &Token) -> bool {
        if self.check(token) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn skip_newlines(&mut self) {
        while self.match_token(&Token::Newline) {}
    }

    fn require_statement_terminator(&mut self) -> Result<(), String> {
        // Consume all consecutive semicolons and newlines (allows ;; for empty statements)
        while self.check(&Token::Semicolon) || self.check(&Token::Newline) {
            self.advance();
        }
        
        Ok(())
    }

    pub fn parse(&mut self) -> Result<Vec<Stmt>, String> {
        let mut statements = Vec::new();
        self.skip_newlines();

        while !self.check(&Token::Eof) {
            // Try to parse a statement, but don't stop on errors
            match self.parse_statement() {
                Ok(Some(stmt)) => {
                    match self.maybe_parse_else_if(stmt) {
                        Ok(s) => statements.push(s),
                        Err(_) => {
                            // Error already recorded, try to continue
                            self.synchronize();
                        }
                    }
                }
                Ok(None) => {
                    self.advance(); // Move past EOF or RBrace
                }
                Err(_) => {
                    // Error already recorded, try to synchronize and continue
                    self.synchronize();
                }
            }
            self.skip_newlines();
        }

        // Always return statements, even if there are errors
        // Errors can be retrieved via get_errors() or format_errors()
        Ok(statements)
    }

    /// Get formatted error string
    pub fn get_error_string(&self) -> Option<String> {
        if self.has_errors() {
            Some(self.format_errors())
        } else {
            None
        }
    }

    /// Synchronize parser after an error - skip to next statement boundary
    fn synchronize(&mut self) {
        self.skip_newlines();
        // Skip tokens until we find a statement starter or EOF
        while !self.check(&Token::Eof) {
            if self.check(&Token::Module)
                || self.check(&Token::Import)
                || self.check(&Token::Class)
                || self.check(&Token::Interface)
                || self.check(&Token::Enum)
                || self.check(&Token::Fn)
                || self.check(&Token::Type)
                || self.check(&Token::Let)
                || self.check(&Token::Return)
                || self.check(&Token::If)
                || self.check(&Token::For)
                || self.check(&Token::While)
                || self.check(&Token::Break)
                || self.check(&Token::Continue)
                || self.check(&Token::Try)
                || self.check(&Token::Throw)
            {
                return;
            }
            self.advance();
        }
    }

    fn parse_statement(&mut self) -> Result<Option<Stmt>, String> {
        self.skip_newlines();

        if self.check(&Token::Eof) || self.check(&Token::RBrace) {
            return Ok(None);
        }

        let mut is_native = false;
        let mut is_private = false;

        // Only consume native/private if followed by fn (for function declarations)
        while self.check(&Token::Native) || self.check(&Token::Private) {
            let is_current_native = self.match_token(&Token::Native);
            let is_current_private = if !is_current_native { self.match_token(&Token::Private) } else { false };

            if is_current_native {
                is_native = true;
            } else if is_current_private {
                is_private = true;
            }
            self.skip_newlines();
        }

        self.skip_newlines();
        let stmt = if self.match_token(&Token::Module) {
            self.parse_module()?
        } else if self.match_token(&Token::Import) {
            self.parse_import()?
        } else if self.match_token(&Token::Class) {
            self.parse_class(is_native, is_private)?
        } else if self.match_token(&Token::Interface) {
            self.parse_interface(is_private)?
        } else if self.match_token(&Token::Enum) {
            self.parse_enum(is_private)?
        } else if self.match_token(&Token::Fn) {
            self.parse_function_ext(is_private, is_native)?
        } else if self.match_token(&Token::Type) {
            self.parse_type_alias(is_private)?
        } else if self.match_token(&Token::Let) {
            self.parse_let(is_private)?
        } else if self.match_token(&Token::Const) {
            self.parse_const(is_private)?
        } else if self.match_token(&Token::Return) {
            self.parse_return()?
        } else if self.match_token(&Token::If) {
            self.parse_if()?
        } else if self.match_token(&Token::For) {
            self.parse_for()?
        } else if self.match_token(&Token::While) {
            self.parse_while()?
        } else if self.match_token(&Token::Break) {
            self.parse_break()?
        } else if self.match_token(&Token::Continue) {
            self.parse_continue()?
        } else if self.match_token(&Token::Try) {
            self.parse_try_catch()?
        } else if self.match_token(&Token::Throw) {
            self.parse_throw()?
        } else if self.match_token(&Token::Spawn) {
            self.parse_spawn()?
        } else {
            let expr = self.parse_expression()?;

            if self.match_token(&Token::Equal) {
                self.skip_newlines();
                if let Expr::Variable { name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::Assign { name, expr: value, span }
                } else if let Expr::Get { object, name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::Expr(Expr::Set { object, name, value: Box::new(value), span })
                } else {
                    return self.error_generic("Left side of assignment must be a variable or property access");
                }
            } else if self.match_token(&Token::PlusEqual) {
                self.skip_newlines();
                if let Expr::Variable { name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Variable(name), op: AugOp::Add, expr: value, span }
                } else if let Expr::Get { object, name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Field { object: *object, name }, op: AugOp::Add, expr: value, span }
                } else {
                    return self.error_generic("Left side of += must be a variable or field access");
                }
            } else if self.match_token(&Token::MinusEqual) {
                self.skip_newlines();
                if let Expr::Variable { name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Variable(name), op: AugOp::Subtract, expr: value, span }
                } else if let Expr::Get { object, name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Field { object: *object, name }, op: AugOp::Subtract, expr: value, span }
                } else {
                    return self.error_generic("Left side of -= must be a variable or field access");
                }
            } else if self.match_token(&Token::StarEqual) {
                self.skip_newlines();
                if let Expr::Variable { name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Variable(name), op: AugOp::Multiply, expr: value, span }
                } else if let Expr::Get { object, name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Field { object: *object, name }, op: AugOp::Multiply, expr: value, span }
                } else {
                    return self.error_generic("Left side of *= must be a variable or field access");
                }
            } else if self.match_token(&Token::SlashEqual) {
                self.skip_newlines();
                if let Expr::Variable { name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Variable(name), op: AugOp::Divide, expr: value, span }
                } else if let Expr::Get { object, name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Field { object: *object, name }, op: AugOp::Divide, expr: value, span }
                } else {
                    return self.error_generic("Left side of /= must be a variable or field access");
                }
            } else if self.match_token(&Token::PercentEqual) {
                self.skip_newlines();
                if let Expr::Variable { name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Variable(name), op: AugOp::Modulo, expr: value, span }
                } else if let Expr::Get { object, name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Field { object: *object, name }, op: AugOp::Modulo, expr: value, span }
                } else {
                    return self.error_generic("Left side of %= must be a variable or field access");
                }
            } else if self.match_token(&Token::BitAndEqual) {
                self.skip_newlines();
                if let Expr::Variable { name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Variable(name), op: AugOp::BitAnd, expr: value, span }
                } else if let Expr::Get { object, name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Field { object: *object, name }, op: AugOp::BitAnd, expr: value, span }
                } else {
                    return self.error_generic("Left side of &= must be a variable or field access");
                }
            } else if self.match_token(&Token::BitOrEqual) {
                self.skip_newlines();
                if let Expr::Variable { name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Variable(name), op: AugOp::BitOr, expr: value, span }
                } else if let Expr::Get { object, name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Field { object: *object, name }, op: AugOp::BitOr, expr: value, span }
                } else {
                    return self.error_generic("Left side of |= must be a variable or field access");
                }
            } else if self.match_token(&Token::BitXorEqual) {
                self.skip_newlines();
                if let Expr::Variable { name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Variable(name), op: AugOp::BitXor, expr: value, span }
                } else if let Expr::Get { object, name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Field { object: *object, name }, op: AugOp::BitXor, expr: value, span }
                } else {
                    return self.error_generic("Left side of ^= must be a variable or field access");
                }
            } else if self.match_token(&Token::ShiftLeftEqual) {
                self.skip_newlines();
                if let Expr::Variable { name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Variable(name), op: AugOp::ShiftLeft, expr: value, span }
                } else if let Expr::Get { object, name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Field { object: *object, name }, op: AugOp::ShiftLeft, expr: value, span }
                } else {
                    return self.error_generic("Left side of <<= must be a variable or field access");
                }
            } else if self.match_token(&Token::ShiftRightEqual) {
                self.skip_newlines();
                if let Expr::Variable { name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Variable(name), op: AugOp::ShiftRight, expr: value, span }
                } else if let Expr::Get { object, name, span } = expr {
                    let value = self.parse_expression()?;
                    Stmt::AugAssign { target: AugAssignTarget::Field { object: *object, name }, op: AugOp::ShiftRight, expr: value, span }
                } else {
                    return self.error_generic("Left side of >>= must be a variable or field access");
                }
            } else {
                Stmt::Expr(expr)
            }
        };

        // Require statement terminator: semicolon or newline (or EOF/RBrace)
        self.require_statement_terminator()?;

        Ok(Some(stmt))
    }

    fn parse_import(&mut self) -> Result<Stmt, String> {
        let span = self.current_span();
        let mut path = Vec::new();

        // Parse the import path (e.g., std.io.println)
        loop {
            // Check for wildcard (*)
            if self.match_token(&Token::Star) {
                // Wildcard import (e.g., import std.io.*)
                if path.is_empty() {
                    return self.error("Cannot use wildcard without a module path");
                }
                self.skip_newlines();
                return Ok(Stmt::Import { path, kind: ImportKind::Wildcard, span });
            }

            if let Token::Identifier(part) = self.advance() {
                path.push(part);
            } else {
                return self.error("Expected identifier in import path");
            }

            if self.match_token(&Token::Dot) {
                continue;
            } else {
                break;
            }
        }

        // Check for aliased import (e.g., import std.io as io)
        if self.match_token(&Token::As) {
            if let Token::Identifier(alias) = self.advance() {
                return Ok(Stmt::Import { path, kind: ImportKind::Aliased(alias), span });
            } else {
                return self.error("Expected identifier after 'as'");
            }
        }

        // Determine import kind based on path length and context
        // - Single element: Module (import std -> std.io.println)
        // - Multiple elements: Simple (import std.io -> println())
        // - Three+ elements: Member (import std.io.println -> println())
        let kind = if path.len() == 1 {
            ImportKind::Module
        } else if path.len() >= 3 {
            ImportKind::Member
        } else {
            ImportKind::Simple
        };

        self.skip_newlines();
        Ok(Stmt::Import { path, kind, span })
    }

    fn parse_module(&mut self) -> Result<Stmt, String> {
        let span = self.current_span();
        let mut path = Vec::new();

        loop {
            if let Token::Identifier(part) = self.advance() {
                path.push(part);
            } else {
                return self.error("Expected identifier in module path");
            }

            if self.match_token(&Token::Dot) {
                continue;
            } else {
                break;
            }
        }

        self.skip_newlines();
        Ok(Stmt::Module { path, span })
    }

    fn parse_class(&mut self, is_native_class: bool, is_private: bool) -> Result<Stmt, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return self.error_generic("Expected class name"),
        };

        // Parse generic type parameters if present
        let type_params = if self.match_token(&Token::LAngle) {
            let mut params = Vec::new();
            loop {
                if let Token::Identifier(param) = self.advance() {
                    params.push(param);
                } else {
                    return self.error_generic("Expected type parameter name");
                }
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
            if !self.match_token(&Token::RAngle) {
                return self.error_generic("Expected '>' after type parameters");
            }
            params
        } else {
            Vec::new()
        };

        // Parse parent interfaces (class MyClass : Interface1, Interface2)
        let mut parent_interfaces = Vec::new();
        if self.match_token(&Token::Colon) {
            loop {
                if let Token::Identifier(iface) = self.advance() {
                    parent_interfaces.push(iface);
                } else {
                    return self.error_generic("Expected interface name after ':'");
                }
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
        }

        if !self.match_token(&Token::LBrace) {
            return self.error_generic("Expected '{' after class name");
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut nested_classes = Vec::new();
        let mut nested_interfaces = Vec::new();

        self.skip_newlines();
        while !self.check(&Token::RBrace) && !self.check(&Token::Eof) {
            let mut is_private = false;
            let mut is_native_method = false;
            let mut is_static = false;

            self.skip_newlines();
            while self.check(&Token::Private) || self.check(&Token::Native) || self.check(&Token::Static) {
                if self.match_token(&Token::Private) { is_private = true; }
                else if self.match_token(&Token::Native) {
                    if !is_native_class {
                        return self.error_generic("Class member-functions can't have 'native' modifier. Use 'native class' instead.");
                    }
                    is_native_method = true;
                }
                else if self.match_token(&Token::Static) { is_static = true; }
                self.skip_newlines();
            }

            if self.match_token(&Token::Fn) {
                let method = self.parse_method(is_private, is_native_method || is_native_class, is_static)?;
                methods.push(method);
            } else if self.match_token(&Token::Constructor) {
                if is_static {
                    return self.error_generic("Constructor cannot be static");
                }
                let method = self.parse_method_named("constructor", is_private, is_native_class, is_static)?;
                if is_native_class && !method.body.is_empty() {
                    return self.error_generic("Constructor in native classes cannot have implementation.");
                }
                methods.push(method);
            } else if self.check(&Token::Class) {
                self.advance(); // consume 'class' token
                let nested_class_stmt = self.parse_class(is_native_class, is_private)?;
                if let Stmt::Class(class_def) = nested_class_stmt {
                    nested_classes.push(class_def);
                } else {
                    return self.error_generic("Expected class declaration");
                }
            } else if self.check(&Token::Interface) {
                self.advance(); // consume 'interface' token
                let nested_iface_stmt = self.parse_interface(is_private)?;
                if let Stmt::Interface(iface_def) = nested_iface_stmt {
                    nested_interfaces.push(iface_def);
                } else {
                    return self.error_generic("Expected interface declaration");
                }
            } else {
                if is_native_class {
                    return self.error_generic("Native classes cannot have member-fields.");
                }
                let field = self.parse_field(is_private, is_static)?;
                fields.push(field);
            }
            self.skip_newlines();
        }

        if !self.match_token(&Token::RBrace) {
            return self.error_generic("Expected '}' to close class");
        }

        // Auto-generate constructors if no custom constructor is defined
        let mut final_methods = methods;
        let has_constructor = final_methods.iter().any(|m| m.name == "constructor");
        if !has_constructor && !is_native_class {
            // Empty constructor() - fields will be initialized with defaults
            final_methods.push(Method {
                name: "constructor".to_string(),
                params: vec![],
                return_type: None,
                return_optional: false,
                body: vec![],
                private: false,
                is_native: false,
                is_static: false,
                type_params: Vec::new(),
                span: Span::unknown(),
            });

            // Constructor with all fields as parameters
            if !fields.is_empty() {
                let field_params: Vec<Param> = fields.iter().map(|field| Param {
                    name: field.name.clone(),
                    type_name: Some(field.type_name.clone()),
                    default: field.default.clone(),
                }).collect();
                final_methods.push(Method {
                    name: "constructor".to_string(),
                    params: field_params,
                    return_type: None,
                    return_optional: false,
                    body: vec![],
                    private: false,
                    is_native: false,
                    is_static: false,
                    type_params: Vec::new(),
                    span: Span::unknown(),
                });
            }
        }

        Ok(Stmt::Class(ClassDef { name, type_params, parent_interfaces, fields, methods: final_methods, nested_classes, nested_interfaces, is_native: is_native_class, private: is_private }))
    }

    fn parse_interface(&mut self, is_private: bool) -> Result<Stmt, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return self.error_generic("Expected interface name"),
        };

        // Parse generic type parameters if present
        let type_params = if self.match_token(&Token::LAngle) {
            let mut params = Vec::new();
            loop {
                if let Token::Identifier(param) = self.advance() {
                    params.push(param);
                } else {
                    return self.error_generic("Expected type parameter name");
                }
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
            if !self.match_token(&Token::RAngle) {
                return self.error_generic("Expected '>' after type parameters");
            }
            params
        } else {
            Vec::new()
        };

        // Parse parent interfaces (interface MyInterface : ParentInterface1, ParentInterface2)
        let mut parent_interfaces = Vec::new();
        if self.match_token(&Token::Colon) {
            loop {
                if let Token::Identifier(iface) = self.advance() {
                    parent_interfaces.push(iface);
                } else {
                    return self.error_generic("Expected interface name after ':'");
                }
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
        }

        if !self.match_token(&Token::LBrace) {
            return self.error_generic("Expected '{' after interface name");
        }

        let mut methods = Vec::new();
        let mut nested_classes = Vec::new();
        let mut nested_interfaces = Vec::new();

        self.skip_newlines();
        while !self.check(&Token::RBrace) && !self.check(&Token::Eof) {
            let mut is_private = false;

            self.skip_newlines();
            while self.check(&Token::Private) {
                if self.match_token(&Token::Private) { is_private = true; }
                self.skip_newlines();
            }

            if self.match_token(&Token::Fn) {
                let method = self.parse_interface_method(is_private)?;
                methods.push(method);
            } else if self.check(&Token::Class) {
                self.advance(); // consume 'class' token
                let nested_class_stmt = self.parse_class(false, is_private)?;
                if let Stmt::Class(class_def) = nested_class_stmt {
                    nested_classes.push(class_def);
                } else {
                    return self.error_generic("Expected class declaration");
                }
            } else if self.check(&Token::Interface) {
                self.advance(); // consume 'interface' token
                let nested_iface_stmt = self.parse_interface(is_private)?;
                if let Stmt::Interface(iface_def) = nested_iface_stmt {
                    nested_interfaces.push(iface_def);
                } else {
                    return self.error_generic("Expected interface declaration");
                }
            } else {
                return self.error_generic("Interfaces can only contain method declarations, nested classes, or nested interfaces");
            }
            self.skip_newlines();
        }

        if !self.match_token(&Token::RBrace) {
            return self.error_generic("Expected '}' to close interface");
        }

        Ok(Stmt::Interface(InterfaceDef { name, type_params, parent_interfaces, methods, nested_classes, nested_interfaces, private: is_private }))
    }

    fn parse_interface_method(&mut self, private: bool) -> Result<Method, String> {
        let span = self.current_span();
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return self.error_generic("Expected method name"),
        };

        // Parse generic type parameters if present: fn name<T>(params)
        let type_params = if self.match_token(&Token::LAngle) {
            let mut params = Vec::new();
            loop {
                if let Token::Identifier(param) = self.advance() {
                    params.push(param);
                } else {
                    return self.error_generic("Expected type parameter name");
                }
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
            if !self.match_token(&Token::RAngle) {
                return self.error_generic("Expected '>' after type parameters");
            }
            params
        } else {
            Vec::new()
        };

        if !self.match_token(&Token::LParen) {
            return self.error_generic(&format!("Expected '(' after {} name", name));
        }

        let params = self.parse_params()?;
        self.skip_newlines();

        if !self.match_token(&Token::RParen) {
            return self.error_generic("Expected ')' after parameters");
        }

        let (return_type, return_optional) = if self.match_token(&Token::Colon) {
            let (type_name, optional) = self.parse_type()?;
            (Some(type_name), optional)
        } else {
            (None, false)
        };

        // Interface methods can have optional default implementation
        let body = if self.match_token(&Token::LBrace) {
            let b = self.parse_block()?;
            if !self.match_token(&Token::RBrace) {
                return self.error_generic(&format!("Expected '}}' to close {}", name));
            }
            b
        } else {
            // Abstract method - no body
            if self.match_token(&Token::Semicolon) {}
            Vec::new()
        };

        Ok(Method { name: name.to_string(), params, return_type, return_optional, body, private, is_native: false, is_static: false, type_params, span })
    }

    fn parse_enum(&mut self, is_private: bool) -> Result<Stmt, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return self.error("Expected enum name"),
        };

        if !self.match_token(&Token::LBrace) {
            return self.error("Expected '{' after enum name");
        }

        let mut variants = Vec::new();
        self.skip_newlines();

        while !self.check(&Token::RBrace) && !self.check(&Token::Eof) {
            let variant_name = match self.advance() {
                Token::Identifier(n) => n,
                _ => return self.error_generic("Expected variant name"),
            };

            let value = if self.match_token(&Token::Equal) {
                Some(self.parse_expression()?)
            } else {
                None
            };

            if self.match_token(&Token::Comma) {}
            if self.match_token(&Token::Semicolon) {}
            self.skip_newlines();

            variants.push(EnumVariant { name: variant_name, value });
        }

        if !self.match_token(&Token::RBrace) {
            return self.error("Expected '}' to close enum");
        }

        Ok(Stmt::Enum(EnumDef { name, variants, private: is_private }))
    }

    fn parse_type_alias(&mut self, is_private: bool) -> Result<Stmt, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return self.error("Expected type alias name"),
        };

        // Parse generic type parameters if present
        let type_params = if self.match_token(&Token::LAngle) {
            let mut params = Vec::new();
            loop {
                if let Token::Identifier(param) = self.advance() {
                    params.push(param);
                } else {
                    return self.error_generic("Expected type parameter name");
                }
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
            if !self.match_token(&Token::RAngle) {
                return self.error_generic("Expected '>' after type parameters");
            }
            params
        } else {
            Vec::new()
        };

        if !self.match_token(&Token::Equal) {
            return self.error("Expected '=' after type alias name");
        }

        // Parse the aliased type name
        let (aliased_type, _) = self.parse_type()?;

        Ok(Stmt::TypeAlias(TypeAliasDef { name, type_params, aliased_type, private: is_private }))
    }

    fn parse_function_ext(&mut self, is_private: bool, is_native: bool) -> Result<Stmt, String> {
        let span = self.current_span();
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return self.error("Expected function name"),
        };

        // Parse generic type parameters if present
        let type_params = if self.match_token(&Token::LAngle) {
            let mut params = Vec::new();
            loop {
                if let Token::Identifier(param) = self.advance() {
                    params.push(param);
                } else {
                    return self.error_generic("Expected type parameter name");
                }
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
            if !self.match_token(&Token::RAngle) {
                return self.error_generic("Expected '>' after type parameters");
            }
            params
        } else {
            Vec::new()
        };

        if !self.match_token(&Token::LParen) {
            return self.error("Expected '(' after function name");
        }

        let params = self.parse_params()?;
        self.skip_newlines();

        if !self.match_token(&Token::RParen) {
            return self.error("Expected ')' after parameters");
        }

        let (return_type, return_optional) = if self.match_token(&Token::Colon) {
            let (type_name, optional) = self.parse_type()?;
            (Some(type_name), optional)
        } else {
            (None, false)
        };

        let body = if is_native {
            if self.match_token(&Token::Semicolon) {
                Vec::new()
            } else if self.check(&Token::LBrace) {
                // Record error but still parse the body to continue checking for more errors
                self.add_error("Native functions cannot have a body".to_string());
                self.advance(); // consume '{'
                let b = self.parse_block();
                let _ = self.match_token(&Token::RBrace); // consume '}' if present
                b.unwrap_or_default()
            } else {
                // Also allow no semicolon if it's the end of a block/file
                Vec::new()
            }
        } else {
            if !self.match_token(&Token::LBrace) {
                return self.error(&format!("Expected '{{' to start function body for '{}'", name));
            }

            let body = self.parse_block()?;

            if !self.match_token(&Token::RBrace) {
                return self.error("Expected '}' to close function");
            }
            body
        };

        Ok(Stmt::Function(FunctionDef {
            name,
            params,
            return_type,
            return_optional,
            body,
            is_native,
            private: is_private,
            type_params,
            span
        }))
    }

    fn parse_field(&mut self, private: bool, is_static: bool) -> Result<Field, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            t => return self.error_generic(&format!("Expected field name, got {:?}", t)),
        };

        if !self.match_token(&Token::Colon) {
            return self.error_generic(&format!("Expected ':' after field name, got {:?}", self.peek()));
        }

        let (mut type_name, optional) = self.parse_type()?;

        // Handle optional type marker (?)
        if optional {
            type_name = type_name + "?";
        }

        let default = if self.match_token(&Token::Equal) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        Ok(Field { name, type_name, default, private, is_static })
    }

    fn parse_method(&mut self, private: bool, is_native: bool, is_static: bool) -> Result<Method, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return self.error_generic("Expected method name"),
        };
        self.parse_method_named(&name, private, is_native, is_static)
    }

    fn parse_method_named(&mut self, name: &str, private: bool, is_native: bool, is_static: bool) -> Result<Method, String> {
        let span = self.current_span();
        // Parse generic type parameters if present: fn name<T>(params)
        let type_params = if self.match_token(&Token::LAngle) {
            let mut params = Vec::new();
            loop {
                if let Token::Identifier(param) = self.advance() {
                    params.push(param);
                } else {
                    return self.error_generic("Expected type parameter name");
                }
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
            if !self.match_token(&Token::RAngle) {
                return self.error_generic("Expected '>' after type parameters");
            }
            params
        } else {
            Vec::new()
        };

        if !self.match_token(&Token::LParen) {
            return self.error_generic(&format!("Expected '(' after {} name", name));
        }

        let params = self.parse_params()?;
        self.skip_newlines();

        if !self.match_token(&Token::RParen) {
            return self.error_generic("Expected ')' after parameters");
        }

        let (return_type, return_optional) = if self.match_token(&Token::Colon) {
            let (type_name, optional) = self.parse_type()?;
            (Some(type_name), optional)
        } else {
            (None, false)
        };

        let body = if is_native {
            if self.match_token(&Token::Semicolon) {
                Vec::new()
            } else if self.check(&Token::LBrace) {
                // Record error but still parse the body to continue checking for more errors
                self.add_error(format!("Native {} cannot have a body", name));
                self.advance(); // consume '{'
                let b = self.parse_block();
                let _ = self.match_token(&Token::RBrace); // consume '}' if present
                b.unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            if !self.match_token(&Token::LBrace) {
                return self.error_generic(&format!("Expected '{{' to start {} body", name));
            }

            let body = self.parse_block()?;

            if !self.match_token(&Token::RBrace) {
                return self.error_generic(&format!("Expected '}}' to close {}", name));
            }
            body
        };

        Ok(Method { name: name.to_string(), params, return_type, return_optional, body, private, is_native, is_static, type_params, span })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, String> {
        let mut params = Vec::new();
        self.skip_newlines();

        if self.check(&Token::RParen) {
            return Ok(params);
        }

        loop {
            self.skip_newlines();
            if self.check(&Token::RParen) {
                break;
            }

            let name = match self.advance() {
                Token::Identifier(n) => n,
                t => return self.error_generic(&format!("Expected parameter name, got {:?}", t)),
            };

            if !self.match_token(&Token::Colon) {
                return self.error_generic(&format!("Expected ':' after parameter name '{}'. Use 'name: type' syntax (e.g., 'path: str')", name));
            }

            let (mut t_name, optional) = self.parse_type()?;
            if optional {
                t_name = t_name + "?";
            }

            let default = if self.match_token(&Token::Equal) {
                Some(self.parse_expression()?)
            } else {
                None
            };

            params.push(Param { name, type_name: Some(t_name), default });

            if !self.match_token(&Token::Comma) {
                break;
            }
        }

        Ok(params)
    }

    fn parse_type(&mut self) -> Result<(String, bool), String> {
        self.skip_newlines();

        // Check for function type: (params) -> return_type
        if self.check(&Token::LParen) {
            return self.parse_function_type();
        }

        let token = self.advance();
        let type_name = match &token {
            Token::TypeInt => "int".to_string(),
            Token::TypeFloat => "float".to_string(),
            Token::TypeStr => "str".to_string(),
            Token::TypeBool => "bool".to_string(),
            Token::TypeInt8 => "int8".to_string(),
            Token::TypeUInt8 => "uint8".to_string(),
            Token::TypeInt16 => "int16".to_string(),
            Token::TypeUInt16 => "uint16".to_string(),
            Token::TypeInt32 => "int32".to_string(),
            Token::TypeUInt32 => "uint32".to_string(),
            Token::TypeInt64 => "int64".to_string(),
            Token::TypeUInt64 => "uint64".to_string(),
            Token::TypeFloat32 => "float32".to_string(),
            Token::TypeFloat64 => "float64".to_string(),
            Token::Null => return self.error_generic("'null' is not a valid type. Functions without a return value should not specify a return type"),
            Token::Identifier(t) => t.clone(),
            _ => return self.error_generic(&format!("Expected type name, got {:?}", token)),
        };

        // Handle generic type arguments
        let mut full_type = type_name;
        if self.match_token(&Token::LAngle) {
            full_type.push('<');
            let mut first = true;
            loop {
                if !first {
                    full_type.push_str(", ");
                }
                first = false;
                let (arg_type, _) = self.parse_type()?;
                full_type.push_str(&arg_type);
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
            if !self.match_token(&Token::RAngle) {
                return self.error_generic("Expected '>' after generic type arguments");
            }
            full_type.push('>');
        }

        // Handle array type suffix []
        while self.match_token(&Token::LBracket) {
            if !self.match_token(&Token::RBracket) {
                return self.error_generic("Expected ']' after '[' in array type");
            }
            full_type.push_str("[]");
        }

        let optional = self.match_token(&Token::Question);

        Ok((full_type, optional))
    }

    fn parse_function_type(&mut self) -> Result<(String, bool), String> {
        // Parse function type: (param_types) -> return_type  or  (param_types) for void return
        let mut type_str = String::from("(");

        if !self.match_token(&Token::LParen) {
            return self.error_generic("Expected '(' at start of function type");
        }
        self.skip_newlines();

        let mut first = true;
        while !self.check(&Token::RParen) && !self.check(&Token::Eof) {
            if !first {
                type_str.push_str(", ");
            }
            first = false;

            let (param_type, _) = self.parse_type()?;
            type_str.push_str(&param_type);

            if !self.match_token(&Token::Comma) {
                break;
            }
            self.skip_newlines();
        }

        if !self.match_token(&Token::RParen) {
            return self.error_generic("Expected ')' after function parameter types");
        }
        type_str.push(')');

        // Parse optional return type
        self.skip_newlines();
        if self.match_token(&Token::Arrow) {
            self.skip_newlines();
            let (return_type, optional) = self.parse_type()?;
            type_str.push_str(" -> ");
            type_str.push_str(&return_type);
            Ok((type_str, optional))
        } else {
            // No return type specified - function returns nothing (void)
            Ok((type_str, false))
        }
    }

    fn parse_let(&mut self, is_private: bool) -> Result<Stmt, String> {
        let span = self.current_span();
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return self.error("Expected variable name after 'let'"),
        };

        let type_annotation = if self.check(&Token::Colon) {
            self.advance();
            let (type_name, optional) = self.parse_type()?;
            if optional {
                Some(type_name + "?")
            } else {
                Some(type_name)
            }
        } else {
            None
        };

        self.skip_newlines();
        if !self.match_token(&Token::Equal) {
            return self.error("Expected '=' in let statement");
        }
        self.skip_newlines();

        let expr = self.parse_expression()?;

        Ok(Stmt::Let { name, type_annotation, expr, private: is_private, span })
    }

    fn parse_const(&mut self, is_private: bool) -> Result<Stmt, String> {
        let span = self.current_span();
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return self.error("Expected constant name after 'const'"),
        };

        let type_annotation = if self.check(&Token::Colon) {
            self.advance();
            let (type_name, optional) = self.parse_type()?;
            if optional {
                Some(type_name + "?")
            } else {
                Some(type_name)
            }
        } else {
            None
        };

        self.skip_newlines();
        if !self.match_token(&Token::Equal) {
            return self.error("Expected '=' in const statement");
        }
        self.skip_newlines();

        let expr = self.parse_expression()?;

        Ok(Stmt::Const { name, type_annotation, expr, private: is_private, span })
    }

    fn parse_return(&mut self) -> Result<Stmt, String> {
        let span = self.current_span();
        let expr = if self.check(&Token::Semicolon) || self.check(&Token::Newline) || self.check(&Token::RBrace) || self.check(&Token::Eof) {
            None
        } else {
            Some(self.parse_expression()?)
        };

        Ok(Stmt::Return { expr, span })
    }

    fn parse_if(&mut self) -> Result<Stmt, String> {
        let span = self.current_span();
        if !self.match_token(&Token::LParen) {
            return self.error("Expected '(' after 'if'");
        }

        let condition = self.parse_expression()?;
        self.skip_newlines();

        if !self.match_token(&Token::RParen) {
            return self.error("Expected ')' after condition");
        }

        if !self.match_token(&Token::LBrace) {
            return self.error("Expected '{' for if body");
        }

        let then_branch = self.parse_block()?;

        if !self.match_token(&Token::RBrace) {
            return self.error("Expected '}' to close if body");
        }

        Ok(Stmt::If { condition, then_branch, else_branch: None, span })
    }

    fn parse_for(&mut self) -> Result<Stmt, String> {
        let span = self.current_span();
        if !self.match_token(&Token::LParen) {
            return self.error("Expected '(' after 'for'");
        }

        let var_name = match self.advance() {
            Token::Identifier(name) => name,
            _ => return self.error("Expected variable name in for loop"),
        };

        if !self.match_token(&Token::In) {
            return self.error("Expected 'in' after loop variable");
        }

        let range = self.parse_range()?;

        if !self.match_token(&Token::RParen) {
            return self.error("Expected ')' after range expression");
        }

        if !self.match_token(&Token::LBrace) {
            return self.error("Expected '{' for loop body");
        }

        let body = self.parse_block()?;

        if !self.match_token(&Token::RBrace) {
            return self.error("Expected '}' to close loop body");
        }

        Ok(Stmt::For { var_name, range: Box::new(range), body, span })
    }

    fn parse_while(&mut self) -> Result<Stmt, String> {
        let span = self.current_span();
        if !self.match_token(&Token::LParen) {
            return self.error("Expected '(' after 'while'");
        }

        let condition = self.parse_expression()?;

        if !self.match_token(&Token::RParen) {
            return self.error("Expected ')' after condition");
        }

        if !self.match_token(&Token::LBrace) {
            return self.error("Expected '{' for while body");
        }

        let body = self.parse_block()?;

        if !self.match_token(&Token::RBrace) {
            return self.error("Expected '}' to close while body");
        }

        Ok(Stmt::While { condition, body, span })
    }

    fn parse_break(&mut self) -> Result<Stmt, String> {
        let span = self.current_span();
        Ok(Stmt::Break(span))
    }

    fn parse_continue(&mut self) -> Result<Stmt, String> {
        let span = self.current_span();
        Ok(Stmt::Continue(span))
    }

    fn parse_try_catch(&mut self) -> Result<Stmt, String> {
        let span = self.current_span();
        if !self.match_token(&Token::LBrace) {
            return self.error("Expected '{' for try body");
        }

        let try_block = self.parse_block()?;

        if !self.match_token(&Token::RBrace) {
            return self.error("Expected '}' to close try body");
        }

        self.skip_newlines();

        if !self.match_token(&Token::Catch) {
            return self.error("Expected 'catch' after try block");
        }

        if !self.match_token(&Token::LParen) {
            return self.error("Expected '(' after 'catch'");
        }

        let catch_var = match self.advance() {
            Token::Identifier(n) => n,
            _ => return self.error("Expected identifier for catch variable"),
        };

        if !self.match_token(&Token::RParen) {
            return self.error("Expected ')' after catch variable");
        }

        if !self.match_token(&Token::LBrace) {
            return self.error("Expected '{' for catch body");
        }

        let catch_block = self.parse_block()?;

        if !self.match_token(&Token::RBrace) {
            return self.error("Expected '}' to close catch body");
        }

        Ok(Stmt::TryCatch { try_block, catch_var, catch_block, span })
    }

    fn parse_throw(&mut self) -> Result<Stmt, String> {
        let span = self.current_span();
        let expr = self.parse_expression()?;

        Ok(Stmt::Throw { expr, span })
    }

    fn parse_spawn(&mut self) -> Result<Stmt, String> {
        let span = self.current_span();
        // spawn keyword is followed by an expression (usually a function call)
        let expr = self.parse_expression()?;

        Ok(Stmt::Spawn { expr, span })
    }

    fn parse_block(&mut self) -> Result<Block, String> {
        let mut block = Vec::new();
        self.skip_newlines();

        while !self.check(&Token::RBrace) && !self.check(&Token::Eof) {
            // Skip standalone semicolons (empty statements)
            while self.match_token(&Token::Semicolon) {}
            self.skip_newlines();
            
            if self.check(&Token::RBrace) || self.check(&Token::Eof) {
                break;
            }
            
            if let Some(stmt) = self.parse_statement()? {
                // Check if this is an if statement that might need an else branch
                let stmt = self.maybe_parse_else_if(stmt)?;
                block.push(stmt);
            }
            self.skip_newlines();
        }

        Ok(block)
    }

    fn maybe_parse_else_if(&mut self, stmt: Stmt) -> Result<Stmt, String> {
        // If the statement is an If, check for else/else if
        if let Stmt::If { condition, then_branch, else_branch, span } = stmt {
            self.skip_newlines();

            if self.match_token(&Token::Else) {
                self.skip_newlines();

                // Check for 'else if'
                if self.match_token(&Token::If) {
                    // Parse the nested if statement
                    let nested_if = self.parse_if()?;
                    // Recursively check for more else/else if on the nested if
                    let nested_if = self.maybe_parse_else_if(nested_if)?;
                    // Return the original if with the nested if as its else branch
                    return Ok(Stmt::If {
                        condition,
                        then_branch,
                        else_branch: Some(vec![nested_if]),
                        span,
                    });
                } else {
                    // Regular else block
                    if !self.match_token(&Token::LBrace) {
                        return Err(format!("Expected '{{' for else body"));
                    }
                    let else_block = self.parse_block()?;
                    if !self.match_token(&Token::RBrace) {
                        return Err(format!("Expected '}}' to close else body"));
                    }
                    return Ok(Stmt::If {
                        condition,
                        then_branch,
                        else_branch: Some(else_block),
                        span,
                    });
                }
            }

            // No else, return the if as-is
            return Ok(Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            });
        }

        Ok(stmt)
    }

    fn parse_expression(&mut self) -> Result<Expr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_and()?;

        loop {
            if self.match_token(&Token::DoubleOr) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Or,
                    right: Box::new(self.parse_and()?),
                    span,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_range()?;

        loop {
            if self.match_token(&Token::DoubleAnd) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::And,
                    right: Box::new(self.parse_range()?),
                    span,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_range(&mut self) -> Result<Expr, String> {
        let start = self.parse_bitwise_or()?;
        let span = self.compute_span(self.pos);

        if self.match_token(&Token::Range) {
            let end = self.parse_bitwise_or()?;
            return Ok(Expr::Range {
                start: Box::new(start),
                end: Box::new(end),
                span,
            });
        }

        Ok(start)
    }

    fn parse_bitwise_or(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_bitwise_xor()?;

        loop {
            if self.match_token(&Token::BitOr) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::BitOr,
                    right: Box::new(self.parse_bitwise_xor()?),
                    span,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_bitwise_and()?;

        loop {
            if self.match_token(&Token::BitXor) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::BitXor,
                    right: Box::new(self.parse_bitwise_and()?),
                    span,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_bitwise_and(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_equality()?;

        loop {
            if self.match_token(&Token::BitAnd) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::BitAnd,
                    right: Box::new(self.parse_equality()?),
                    span,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_comparison()?;

        loop {
            if self.match_token(&Token::BangEqual) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::NotEqual,
                    right: Box::new(self.parse_comparison()?),
                    span,
                };
            } else if self.match_token(&Token::DoubleEqual) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Equal,
                    right: Box::new(self.parse_comparison()?),
                    span,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_additive()?;

        loop {
            if self.match_token(&Token::RAngle) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Greater,
                    right: Box::new(self.parse_additive()?),
                    span,
                };
            } else if self.match_token(&Token::GreaterEqual) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::GreaterEqual,
                    right: Box::new(self.parse_additive()?),
                    span,
                };
            } else if self.match_token(&Token::LAngle) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Less,
                    right: Box::new(self.parse_additive()?),
                    span,
                };
            } else if self.match_token(&Token::LessEqual) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::LessEqual,
                    right: Box::new(self.parse_additive()?),
                    span,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_shift()?;

        loop {
            if self.match_token(&Token::Plus) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Add,
                    right: Box::new(self.parse_shift()?),
                    span,
                };
            } else if self.match_token(&Token::Minus) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Subtract,
                    right: Box::new(self.parse_shift()?),
                    span,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_shift(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_multiplicative()?;

        loop {
            if self.match_token(&Token::ShiftLeft) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::ShiftLeft,
                    right: Box::new(self.parse_multiplicative()?),
                    span,
                };
            } else if self.match_token(&Token::ShiftRight) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::ShiftRight,
                    right: Box::new(self.parse_multiplicative()?),
                    span,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_exponential()?;

        loop {
            if self.match_token(&Token::Star) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Multiply,
                    right: Box::new(self.parse_exponential()?),
                    span,
                };
            } else if self.match_token(&Token::Slash) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Divide,
                    right: Box::new(self.parse_exponential()?),
                    span,
                };
            } else if self.match_token(&Token::Percent) {
                self.skip_newlines();
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Modulo,
                    right: Box::new(self.parse_exponential()?),
                    span,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_exponential(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_unary()?;

        // ** is right-associative, so we parse it differently
        if self.match_token(&Token::StarStar) {
            self.skip_newlines();
            let span = self.compute_span(self.pos - 1);
            let right = self.parse_exponential()?;  // Right-associative: recurse for right side
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::Pow,
                right: Box::new(right),
                span,
            };
        }

        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.match_token(&Token::Bang) {
            let span = self.compute_span(self.pos - 1);
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(self.parse_unary()?),
                span,
            });
        }

        if self.match_token(&Token::BitNot) {
            let span = self.compute_span(self.pos - 1);
            return Ok(Expr::Unary {
                op: UnaryOp::BitNot,
                expr: Box::new(self.parse_unary()?),
                span,
            });
        }

        if self.match_token(&Token::Minus) {
            let span = self.compute_span(self.pos - 1);
            return Ok(Expr::Unary {
                op: UnaryOp::Negate,
                expr: Box::new(self.parse_unary()?),
                span,
            });
        }

        if self.match_token(&Token::PlusPlus) {
            let span = self.compute_span(self.pos - 1);
            let expr = self.parse_unary()?;
            if let Expr::Variable { .. } | Expr::Get { .. } = &expr {
                return Ok(Expr::Unary {
                    op: UnaryOp::PrefixIncrement,
                    expr: Box::new(expr),
                    span,
                });
            } else {
                return self.error_expr("Prefix increment operator requires a variable or field access");
            }
        }

        if self.match_token(&Token::MinusMinus) {
            let span = self.compute_span(self.pos - 1);
            let expr = self.parse_unary()?;
            if let Expr::Variable { .. } | Expr::Get { .. } = &expr {
                return Ok(Expr::Unary {
                    op: UnaryOp::PrefixDecrement,
                    expr: Box::new(expr),
                    span,
                });
            } else {
                return self.error_expr("Prefix decrement operator requires a variable or field access");
            }
        }

        self.parse_call()
    }

    fn parse_call(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;

        loop {
            // Check for generic type arguments before function call
            // Only parse as generic if it's an identifier followed by <types> and then (
            if self.check(&Token::LAngle) {
                // Save position and error count in case we need to backtrack
                let saved_pos = self.pos;
                let saved_error_count = self.errors.borrow().len();

                // Build generic type string
                let mut type_str = String::new();
                if let Expr::Variable { name, .. } = &expr {
                    type_str = name.clone();
                }

                self.advance(); // consume <
                type_str.push('<');
                let mut first = true;
                let mut valid_generic = true;

                loop {
                    if !first {
                        type_str.push_str(", ");
                    }
                    first = false;

                    // Try to parse a type
                    match self.parse_type() {
                        Ok((arg_type, _)) => {
                            type_str.push_str(&arg_type);
                        }
                        Err(_) => {
                            valid_generic = false;
                            break;
                        }
                    }

                    if !self.match_token(&Token::Comma) {
                        break;
                    }
                }

                if valid_generic && self.match_token(&Token::RAngle) {
                    // Check if followed by ( for class instantiation
                    self.skip_newlines();
                    if self.check(&Token::LParen) {
                        // This is a generic class instantiation
                        type_str.push('>');

                        // Update expr to be the generic type
                        if let Expr::Variable { span, .. } = &expr {
                            expr = Expr::Variable { name: type_str, span: *span };
                        }
                        continue;
                    }
                }
                // Backtrack - restore position and error count
                self.pos = saved_pos;
                self.errors.borrow_mut().truncate(saved_error_count);
            }

            if self.match_token(&Token::LParen) {
                let args = self.parse_arguments()?;
                let span = self.compute_span(self.pos - 1);
                self.skip_newlines();
                if !self.match_token(&Token::RParen) {
                    return self.error_expr("Expected ')' after arguments");
                }

                // All type conversion functions (str, int, float, bool, etc.) are handled as native function calls
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                    span,
                };
            } else if self.match_token(&Token::LBracket) {
                let index = self.parse_expression()?;
                if !self.match_token(&Token::RBracket) {
                    return self.error_expr("Expected ']' after array index");
                }
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Index {
                    object: Box::new(expr),
                    index: Box::new(index),
                    span,
                };
            } else if self.match_token(&Token::LBrace) {
                // Class instantiation with {} or object literal
                let span = self.compute_span(self.pos - 1);
                let fields = self.parse_object_literal()?;

                // If expr is a Variable (class name), this is class instantiation
                // Otherwise, it's a standalone object literal (type inferred from context)
                if let Expr::Variable { name: class_name, .. } = &expr {
                    // This is class instantiation: ClassName { field: value }
                    // Store as ObjectLiteral with inferred_type set to the class name
                    expr = Expr::ObjectLiteral { fields, span, inferred_type: Some(class_name.clone()) };
                } else {
                    // This is a standalone object literal: { field: value }
                    // Type will be inferred from context (e.g., function parameter)
                    expr = Expr::ObjectLiteral { fields, span, inferred_type: None };
                }
            } else if self.match_token(&Token::Dot) {
                let name = match self.advance() {
                    Token::Identifier(n) => n,
                    _ => return self.error_expr("Expected identifier after '.'"),
                };
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Get {
                    object: Box::new(expr),
                    name,
                    span,
                };
            } else if self.match_token(&Token::PlusPlus) {
                // Postfix increment: var++ or obj.field++
                let span = self.compute_span(self.pos - 1);
                if let Expr::Variable { name, span: var_span } = &expr {
                    expr = Expr::Unary {
                        op: UnaryOp::PostfixIncrement,
                        expr: Box::new(Expr::Variable { name: name.clone(), span: *var_span }),
                        span,
                    };
                } else if let Expr::Get { object, name, span: get_span } = &expr {
                    expr = Expr::Unary {
                        op: UnaryOp::PostfixIncrement,
                        expr: Box::new(Expr::Get {
                            object: object.clone(),
                            name: name.clone(),
                            span: *get_span,
                        }),
                        span,
                    };
                } else {
                    return self.error_expr("Increment operator requires a variable or field access");
                }
            } else if self.match_token(&Token::MinusMinus) {
                // Postfix decrement: var-- or obj.field--
                let span = self.compute_span(self.pos - 1);
                if let Expr::Variable { name, span: var_span } = &expr {
                    expr = Expr::Unary {
                        op: UnaryOp::PostfixDecrement,
                        expr: Box::new(Expr::Variable { name: name.clone(), span: *var_span }),
                        span,
                    };
                } else if let Expr::Get { object, name, span: get_span } = &expr {
                    expr = Expr::Unary {
                        op: UnaryOp::PostfixDecrement,
                        expr: Box::new(Expr::Get {
                            object: object.clone(),
                            name: name.clone(),
                            span: *get_span,
                        }),
                        span,
                    };
                } else {
                    return self.error_expr("Decrement operator requires a variable or field access");
                }
            } else {                break;
            }
        }

        Ok(expr)
    }

    fn parse_arguments(&mut self) -> Result<Vec<CallArg>, String> {
        let mut args = Vec::new();
        self.skip_newlines();

        if !self.check(&Token::RParen) {
            loop {
                self.skip_newlines();
                if self.check(&Token::RParen) {
                    break;
                }

                // Check for named argument: name: value
                let arg = if self.peek_ahead(1) == Some(&Token::Colon) {
                    let span = self.current_span();
                    let name = match self.advance() {
                        Token::Identifier(n) => n,
                        t => return self.error_generic(&format!("Expected parameter name in named argument, got {:?}", t)),
                    };
                    self.match_token(&Token::Colon);
                    let value = self.parse_expression()?;
                    CallArg::Named { name, value, span }
                } else {
                    CallArg::Positional(self.parse_expression()?)
                };

                args.push(arg);

                // Allow trailing comma
                if !self.match_token(&Token::Comma) {
                    break;
                }
                
                // Check for closing paren (trailing comma case)
                if self.check(&Token::RParen) {
                    break;
                }
            }
        }

        Ok(args)
    }

    fn parse_object_literal(&mut self) -> Result<Vec<ObjectField>, String> {
        let mut fields = Vec::new();
        self.skip_newlines();
        
        // Check for empty object literal
        if self.check(&Token::RBrace) {
            self.advance(); // consume }
            return Ok(fields);
        }
        
        loop {
            self.skip_newlines();
            if self.check(&Token::RBrace) {
                break;
            }
            
            // Parse field name
            let field_name = match self.advance() {
                Token::Identifier(name) => name,
                _ => return self.error_generic("Expected field name in object literal"),
            };
            
            // Expect colon
            if !self.match_token(&Token::Colon) {
                return self.error_generic("Expected ':' after field name in object literal");
            }
            
            // Parse field value
            let value = self.parse_expression()?;
            fields.push(ObjectField { name: field_name, value });
            
            // Check for comma/semicolon or end
            if self.match_token(&Token::Comma) || self.match_token(&Token::Semicolon) {
                continue;
            }
            
            self.skip_newlines();
            if self.check(&Token::RBrace) {
                break;
            }
        }
        
        if !self.match_token(&Token::RBrace) {
            return self.error_generic("Expected '}' to close object literal");
        }
        
        Ok(fields)
    }

    fn parse_lambda_params(&mut self) -> Result<Vec<LambdaParam>, String> {
        let mut params = Vec::new();
        self.skip_newlines();

        if self.check(&Token::RParen) {
            return Ok(params);
        }

        loop {
            self.skip_newlines();
            if self.check(&Token::RParen) {
                break;
            }

            // Parse parameter name
            let name = match self.advance() {
                Token::Identifier(n) => n,
                _ => return self.error_generic("Expected parameter name in lambda"),
            };

            // Optional type annotation
            let type_name = if self.match_token(&Token::Colon) {
                let (mut t_name, optional) = self.parse_type()?;
                if optional {
                    t_name = t_name + "?";
                }
                Some(t_name)
            } else {
                None
            };

            params.push(LambdaParam { name, type_name });

            if !self.match_token(&Token::Comma) {
                break;
            }
        }

        Ok(params)
    }

    fn parse_lambda(&mut self) -> Result<Expr, String> {
        let span = self.compute_span(self.pos - 1);

        // Parse parameters (already consumed LParen)
        let params = self.parse_lambda_params()?;
        self.skip_newlines();

        if !self.match_token(&Token::RParen) {
            return self.error_generic("Expected ')' after lambda parameters");
        }

        // Parse optional return type
        let return_type = if self.match_token(&Token::Colon) {
            let (t_name, optional) = self.parse_type()?;
            if optional {
                Some(t_name + "?")
            } else {
                Some(t_name)
            }
        } else {
            None
        };

        // Parse body
        if !self.match_token(&Token::LBrace) {
            return self.error_generic("Expected '{' to start lambda body");
        }

        let body = self.parse_block()?;

        if !self.match_token(&Token::RBrace) {
            return self.error_generic("Expected '}' to close lambda body");
        }

        Ok(Expr::Lambda { params, return_type, body, span })
    }

    /// Check if the current position (after LParen) looks like a lambda
    /// Lambda syntax: (params): ReturnType { body }
    fn is_lambda(&mut self) -> bool {
        // Save position
        let saved_pos = self.pos;

        // Try to parse lambda parameters
        let mut found_params_end = false;

        // Skip potential parameters
        self.skip_newlines();
        
        // Simple case: () { ... }
        if self.match_token(&Token::RParen) {
            self.skip_newlines();
            if self.check(&Token::LBrace) || self.check(&Token::Colon) {
                self.pos = saved_pos;
                return true;
            }
        }
        
        self.pos = saved_pos;
        self.skip_newlines();

        if !self.check(&Token::RParen) {
            // There are parameters - they might have type annotations
            loop {
                // Parameter name - must be identifier
                if !matches!(self.peek(), Token::Identifier(_)) {
                    break;
                }
                self.advance();

                // Optional colon for type annotation
                if self.match_token(&Token::Colon) {
                    // Type name
                    if self.check_type_token() {
                        self.advance();
                        // Optional '?'
                        self.match_token(&Token::Question);
                    }
                }

                // Check for comma or end
                if !self.match_token(&Token::Comma) {
                    break;
                }
                self.skip_newlines();
            }
        }

        // Should have closing paren
        if self.match_token(&Token::RParen) {
            self.skip_newlines();
            // Check for colon (return type) or brace (body without return type)
            if self.check(&Token::Colon) || self.check(&Token::LBrace) {
                found_params_end = true;
            }
        }

        // Restore position
        self.pos = saved_pos;

        found_params_end
    }

    fn check_type_token(&self) -> bool {
        matches!(self.peek(), Token::TypeInt | Token::TypeFloat | Token::TypeStr | Token::TypeBool |
            Token::TypeInt8 | Token::TypeUInt8 | Token::TypeInt16 | Token::TypeUInt16 |
            Token::TypeInt32 | Token::TypeUInt32 | Token::TypeInt64 | Token::TypeUInt64 |
            Token::TypeFloat32 | Token::TypeFloat64 | Token::Identifier(_) | Token::Null)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let token_pos = self.pos;
        match self.advance() {
            Token::String(s) => self.parse_interpolated_text(s, token_pos),
            Token::Int(n) => Ok(Expr::Literal(Literal::Int(n, self.compute_span(token_pos)))),
            Token::Float(n) => Ok(Expr::Literal(Literal::Float(n, self.compute_span(token_pos)))),
            Token::Null => Ok(Expr::Literal(Literal::Null(self.compute_span(token_pos)))),
            Token::LParen => {
                // Check if this is a lambda: (params): ReturnType { body }
                // We need to peek ahead to see if this looks like a lambda
                if self.is_lambda() {
                    return self.parse_lambda();
                }
                // Otherwise it's a parenthesized expression
                let expr = self.parse_expression()?;
                if !self.match_token(&Token::RParen) {
                    return self.error_expr("Expected ')' after expression");
                }
                Ok(expr)
            },
            Token::Identifier(name) => {
                let span = self.compute_span(token_pos);
                if name == "true" {
                    Ok(Expr::Literal(Literal::Bool(true, span)))
                } else if name == "false" {
                    Ok(Expr::Literal(Literal::Bool(false, span)))
                } else {
                    Ok(Expr::Variable { name, span })
                }
            },
            // Handle type keywords as potential cast functions
            Token::LBracket => {
                let span = self.compute_span(token_pos);
                let mut elements = Vec::new();
                self.skip_newlines();
                while !self.check(&Token::RBracket) && !self.check(&Token::Eof) {
                    elements.push(self.parse_expression()?);
                    self.skip_newlines();
                    if !self.match_token(&Token::Comma) {
                        break;
                    }
                    self.skip_newlines();
                }
                if !self.match_token(&Token::RBracket) {
                    return self.error_expr("Expected ']' after array elements");
                }
                Ok(Expr::Array { elements, span })
            },
            Token::LBrace => {
                // Object literal: { field: value, ... }
                let span = self.compute_span(token_pos);
                let fields = self.parse_object_literal()?;
                Ok(Expr::ObjectLiteral { fields, span, inferred_type: None })
            },
            Token::TypeInt => {
                let span = self.compute_span(token_pos);
                Ok(Expr::Variable { name: "int".to_string(), span })
            },
            Token::TypeFloat => {
                let span = self.compute_span(token_pos);
                Ok(Expr::Variable { name: "float".to_string(), span })
            },
            Token::TypeStr => {
                let span = self.compute_span(token_pos);
                Ok(Expr::Variable { name: "str".to_string(), span })
            },
            Token::TypeBool => {
                let span = self.compute_span(token_pos);
                Ok(Expr::Variable { name: "bool".to_string(), span })
            },
            Token::TypeInt8 => {
                let span = self.compute_span(token_pos);
                Ok(Expr::Variable { name: "int8".to_string(), span })
            },
            Token::TypeUInt8 => {
                let span = self.compute_span(token_pos);
                Ok(Expr::Variable { name: "uint8".to_string(), span })
            },
            Token::TypeInt16 => {
                let span = self.compute_span(token_pos);
                Ok(Expr::Variable { name: "int16".to_string(), span })
            },
            Token::TypeUInt16 => {
                let span = self.compute_span(token_pos);
                Ok(Expr::Variable { name: "uint16".to_string(), span })
            },
            Token::TypeInt32 => {
                let span = self.compute_span(token_pos);
                Ok(Expr::Variable { name: "int32".to_string(), span })
            },
            Token::TypeUInt32 => {
                let span = self.compute_span(token_pos);
                Ok(Expr::Variable { name: "uint32".to_string(), span })
            },
            Token::TypeInt64 => {
                let span = self.compute_span(token_pos);
                Ok(Expr::Variable { name: "int64".to_string(), span })
            },
            Token::TypeUInt64 => {
                let span = self.compute_span(token_pos);
                Ok(Expr::Variable { name: "uint64".to_string(), span })
            },
            Token::TypeFloat32 => {
                let span = self.compute_span(token_pos);
                Ok(Expr::Variable { name: "float32".to_string(), span })
            },
            Token::TypeFloat64 => {
                let span = self.compute_span(token_pos);
                Ok(Expr::Variable { name: "float64".to_string(), span })
            },
            Token::Dollar => self.parse_interpolated_string(),
            token => self.error_expr(&format!("Unexpected token: {:?}", token)),
        }
    }

    fn parse_interpolated_string(&mut self) -> Result<Expr, String> {
        if !self.match_token(&Token::LBrace) {
            return self.error_expr("Expected '{' after '$' in interpolated string");
        }

        let span = self.compute_span(self.pos - 1);
        let expr = self.parse_expression()?;

        if !self.match_token(&Token::RBrace) {
            return self.error_expr("Expected '}' to close interpolated expression");
        }

        Ok(Expr::Interpolated {
            parts: vec![InterpPart::Expr(expr)],
            span,
        })
    }

    /// Recursively adjust the span of all expressions to use the given line and column offset
    fn adjust_expr_span(expr: Expr, line: usize, column_offset: usize) -> Expr {
        match expr {
            Expr::Variable { name, span: _ } => {
                Expr::Variable { name, span: Span { line, column: column_offset } }
            }
            Expr::Literal(literal) => {
                Expr::Literal(literal)  // Literals already have their spans
            }
            Expr::Binary { left, op, right, span: _ } => {
                Expr::Binary {
                    left: Box::new(Self::adjust_expr_span(*left, line, column_offset)),
                    op,
                    right: Box::new(Self::adjust_expr_span(*right, line, column_offset)),
                    span: Span { line, column: column_offset },
                }
            }
            Expr::Unary { op, expr: inner, span: _ } => {
                Expr::Unary {
                    op,
                    expr: Box::new(Self::adjust_expr_span(*inner, line, column_offset)),
                    span: Span { line, column: column_offset },
                }
            }
            Expr::Call { callee, args, span: _ } => {
                Expr::Call {
                    callee: Box::new(Self::adjust_expr_span(*callee, line, column_offset)),
                    args: args.into_iter().map(|arg| {
                        match arg {
                            CallArg::Positional(expr) => CallArg::Positional(Self::adjust_expr_span(expr, line, column_offset)),
                            CallArg::Named { name, value, span: _ } => CallArg::Named {
                                name,
                                value: Self::adjust_expr_span(value, line, column_offset),
                                span: Span { line, column: column_offset },
                            }
                        }
                    }).collect(),
                    span: Span { line, column: column_offset },
                }
            }
            Expr::Get { object, name, span: _ } => {
                Expr::Get {
                    object: Box::new(Self::adjust_expr_span(*object, line, column_offset)),
                    name,
                    span: Span { line, column: column_offset },
                }
            }
            Expr::Set { object, name, value, span: _ } => {
                Expr::Set {
                    object: Box::new(Self::adjust_expr_span(*object, line, column_offset)),
                    name,
                    value: Box::new(Self::adjust_expr_span(*value, line, column_offset)),
                    span: Span { line, column: column_offset },
                }
            }
            Expr::Interpolated { parts, span: _ } => {
                let new_parts = parts.into_iter().map(|part| {
                    match part {
                        InterpPart::Text(t) => InterpPart::Text(t),
                        InterpPart::Expr(e) => InterpPart::Expr(Self::adjust_expr_span(e, line, column_offset)),
                    }
                }).collect();
                Expr::Interpolated {
                    parts: new_parts,
                    span: Span { line, column: column_offset },
                }
            }
            Expr::Range { start, end, span: _ } => {
                Expr::Range {
                    start: Box::new(Self::adjust_expr_span(*start, line, column_offset)),
                    end: Box::new(Self::adjust_expr_span(*end, line, column_offset)),
                    span: Span { line, column: column_offset },
                }
            }
            Expr::Cast { expr: inner, target_type, span: _ } => {
                Expr::Cast {
                    expr: Box::new(Self::adjust_expr_span(*inner, line, column_offset)),
                    target_type,
                    span: Span { line, column: column_offset },
                }
            }
            Expr::Array { elements, span: _ } => {
                Expr::Array {
                    elements: elements.into_iter().map(|e| Self::adjust_expr_span(e, line, column_offset)).collect(),
                    span: Span { line, column: column_offset },
                }
            }
            Expr::Index { object, index, span: _ } => {
                Expr::Index {
                    object: Box::new(Self::adjust_expr_span(*object, line, column_offset)),
                    index: Box::new(Self::adjust_expr_span(*index, line, column_offset)),
                    span: Span { line, column: column_offset },
                }
            }
            Expr::ObjectLiteral { fields, span: _, inferred_type } => {
                Expr::ObjectLiteral {
                    fields,
                    span: Span { line, column: column_offset },
                    inferred_type,
                }
            }
            Expr::Lambda { params, return_type, body, span: _ } => {
                Expr::Lambda {
                    params,
                    return_type,
                    body,  // Block contains statements, not expressions
                    span: Span { line, column: column_offset },
                }
            }
        }
    }

    fn parse_interpolated_text(&mut self, s: String, token_pos: usize) -> Result<Expr, String> {
        let mut parts = Vec::new();
        let mut last_pos = 0;
        let span = self.compute_span(token_pos);

        while let Some(interp_start) = s[last_pos..].find("${") {
            let abs_start = last_pos + interp_start;
            
            // Check if this ${ is preceded by our escape marker (\x00)
            if abs_start > 0 && s.as_bytes()[abs_start - 1] == 0 {
                // This is an escaped ${, skip it
                // Add text before and including the escaped sequence
                if abs_start + 2 > last_pos {
                    parts.push(InterpPart::Text(s[last_pos..abs_start + 2].to_string()));
                }
                last_pos = abs_start + 2;
                continue;
            }
            
            if abs_start > last_pos {
                parts.push(InterpPart::Text(s[last_pos..abs_start].to_string()));
            }

            let rest = &s[abs_start + 2..];
            if let Some(interp_end) = rest.find('}') {
                let expr_str = &rest[..interp_end];
                let trimmed_expr = expr_str.trim();

                // Calculate the column offset for the expression inside ${...}
                // span.column points to the opening quote, so we need:
                // +1 for the quote, +abs_start for position within string, +2 for "${", +whitespace
                let whitespace_trimmed = expr_str.len() - trimmed_expr.len();
                let expr_column = span.column + 1 + abs_start + 2 + whitespace_trimmed;

                let mut sub_lexer = crate::lexer::Lexer::new(trimmed_expr, &self.path);
                let (tokens, token_positions) = sub_lexer.tokenize()?;
                let mut sub_parser = Parser::new(tokens, trimmed_expr, &self.path, token_positions);
                let sub_stmts = sub_parser.parse()?;

                // Create expression with proper span based on the string's position
                let expr = if let Some(Stmt::Expr(e)) = sub_stmts.first() {
                    Self::adjust_expr_span(e.clone(), span.line, expr_column)
                } else if let Some(Stmt::Assign { name, .. }) = sub_stmts.first() {
                    Expr::Variable {
                        name: name.clone(),
                        span: Span { line: span.line, column: expr_column }
                    }
                } else {
                    Expr::Variable {
                        name: trimmed_expr.to_string(),
                        span: Span { line: span.line, column: expr_column }
                    }
                };

                parts.push(InterpPart::Expr(expr));
                last_pos = abs_start + 2 + interp_end + 1;
            } else {
                return self.error_expr("Unclosed interpolation ${...}");
            }
        }

        if last_pos < s.len() {
            parts.push(InterpPart::Text(s[last_pos..].to_string()));
        }

        // Clean up escape markers from text parts
        for part in &mut parts {
            if let InterpPart::Text(text) = part {
                *text = text.replace("\x00$", "$");
            }
        }

        if parts.iter().any(|p| matches!(p, InterpPart::Expr(_))) {
            Ok(Expr::Interpolated { parts, span })
        } else {
            Ok(Expr::Literal(Literal::String(s.replace("\x00$", "$"), span)))
        }
    }
}
