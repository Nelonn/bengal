use crate::lexer::Token;

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

#[derive(Debug, Clone)]
pub enum Stmt {
    Module { path: Vec<String> },
    Import { path: Vec<String> },
    Class(ClassDef),
    Interface(InterfaceDef),
    Enum(EnumDef),
    Function(FunctionDef),
    TypeAlias(TypeAliasDef),
    Let { name: String, type_annotation: Option<String>, expr: Expr },
    Assign { name: String, expr: Expr, span: Span },
    Return(Option<Expr>),
    Expr(Expr),
    If { condition: Expr, then_branch: Block, else_branch: Option<Block> },
    For { var_name: String, range: Box<Expr>, body: Block },
    While { condition: Expr, body: Block },
    Break,
    Continue,
    TryCatch { try_block: Block, catch_var: String, catch_block: Block },
    Throw(Expr),
}

pub type Block = Vec<Stmt>;

#[derive(Debug, Clone)]
pub struct TypeAliasDef {
    pub name: String,
    pub type_params: Vec<String>,
    pub aliased_type: String,
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<EnumVariant>,
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
    pub is_async: bool,
    pub is_native: bool,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub name: String,
    pub type_params: Vec<String>,
    pub parent_interfaces: Vec<String>,
    pub fields: Vec<Field>,
    pub methods: Vec<Method>,
    pub is_native: bool,
}

#[derive(Debug, Clone)]
pub struct InterfaceDef {
    pub name: String,
    pub type_params: Vec<String>,
    pub parent_interfaces: Vec<String>,
    pub methods: Vec<Method>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub type_name: String,
    pub default: Option<Expr>,
    pub private: bool,
}

#[derive(Debug, Clone)]
pub struct Method {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<String>,
    pub return_optional: bool,
    pub body: Block,
    pub private: bool,
    pub is_async: bool,
    pub is_native: bool,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub type_name: Option<String>,
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

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    Variable { name: String, span: Span },
    Binary { left: Box<Expr>, op: BinaryOp, right: Box<Expr>, span: Span },
    Unary { op: UnaryOp, expr: Box<Expr>, span: Span },
    Call { callee: Box<Expr>, args: Vec<Expr>, span: Span },
    Get { object: Box<Expr>, name: String, span: Span },
    Set { object: Box<Expr>, name: String, value: Box<Expr>, span: Span },
    Interpolated { parts: Vec<InterpPart>, span: Span },
    Range { start: Box<Expr>, end: Box<Expr>, span: Span },
    Await { expr: Box<Expr>, span: Span },
    Cast { expr: Box<Expr>, target_type: CastType, span: Span },
    Array { elements: Vec<Expr>, span: Span },
    Index { object: Box<Expr>, index: Box<Expr>, span: Span },
    ObjectLiteral { fields: Vec<ObjectField>, span: Span, inferred_type: Option<String> },
    Lambda { params: Vec<LambdaParam>, return_type: Option<String>, body: Block, span: Span, is_async: bool },
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
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
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
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Not,
    PrefixIncrement,
    PrefixDecrement,
    PostfixIncrement,
    PostfixDecrement,
    Decrement, // Keep for backward compatibility if used elsewhere, but we'll use PostfixDecrement for x--
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
}

impl Parser {
    pub fn new(tokens: Vec<Token>, source: &str, path: &str, token_positions: Vec<usize>) -> Self {
        Self {
            tokens,
            pos: 0,
            source: source.to_string(),
            path: path.to_string(),
            token_positions,
        }
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

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let token = self.peek().clone();
        self.pos += 1;
        token
    }

    fn error(&self, message: &str) -> Result<Stmt, String> {
        let span = self.compute_span(self.pos);
        let filename = std::path::Path::new(&self.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.path);
        Err(format!("{}:{}:{}: error: {}", filename, span.line, span.column, message))
    }

    fn error_expr(&self, message: &str) -> Result<Expr, String> {
        let span = self.compute_span(self.pos);
        let filename = std::path::Path::new(&self.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.path);
        Err(format!("{}:{}:{}: error: {}", filename, span.line, span.column, message))
    }

    fn error_generic<T>(&self, message: &str) -> Result<T, String> {
        let span = self.compute_span(self.pos);
        let filename = std::path::Path::new(&self.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.path);
        Err(format!("{}:{}:{}: error: {}", filename, span.line, span.column, message))
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

    pub fn parse(&mut self) -> Result<Vec<Stmt>, String> {
        let mut statements = Vec::new();
        self.skip_newlines();

        while !self.check(&Token::Eof) {
            let stmt = self.parse_statement()?;
            if let Some(s) = stmt {
                statements.push(s);
            }
            self.skip_newlines();
        }
        Ok(statements)
    }

    fn parse_statement(&mut self) -> Result<Option<Stmt>, String> {
        self.skip_newlines();

        if self.check(&Token::Eof) || self.check(&Token::RBrace) {
            return Ok(None);
        }

        let mut is_native = false;
        let mut is_async = false;

        // Only consume async/native if followed by fn (for function declarations)
        // This allows async to be used for lambdas in expressions
        while self.check(&Token::Native) || self.check(&Token::Async) {
            let is_current_async = self.match_token(&Token::Async);
            let is_current_native = if !is_current_async { self.match_token(&Token::Native) } else { false };

            if is_current_async {
                // Check if followed by fn (possibly with native in between: async native fn)
                self.skip_newlines();
                if self.check(&Token::Fn) {
                    is_async = true;
                } else if self.check(&Token::Native) {
                    // async native fn pattern
                    is_async = true;
                    // Continue loop to consume native on next iteration
                } else {
                    // Not followed by fn, put async back by not consuming it
                    self.pos -= 1; // Go back to async token
                    break;
                }
            } else if is_current_native {
                is_native = true;
            }
            self.skip_newlines();
        }

        let stmt = if self.match_token(&Token::Module) {
            self.parse_module()?
        } else if self.match_token(&Token::Import) {
            self.parse_import()?
        } else if self.match_token(&Token::Class) {
            self.parse_class(is_native)?
        } else if self.match_token(&Token::Interface) {
            self.parse_interface()?
        } else if self.match_token(&Token::Enum) {
            self.parse_enum()?
        } else if self.match_token(&Token::Fn) {
            self.parse_function_ext(false, is_async, is_native)?
        } else if self.match_token(&Token::Type) {
            self.parse_type_alias()?
        } else if self.match_token(&Token::Let) {
            self.parse_let()?
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
        } else {
            let expr = self.parse_expression()?;

            if self.match_token(&Token::Equal) {
                if let Expr::Variable { name, span } = expr {
                    let value = self.parse_expression()?;
                    if self.match_token(&Token::Semicolon) {}
                    Stmt::Assign { name, expr: value, span }
                } else if let Expr::Get { object, name, span } = expr {
                    let value = self.parse_expression()?;
                    if self.match_token(&Token::Semicolon) {}
                    Stmt::Expr(Expr::Set { object, name, value: Box::new(value), span })
                } else {
                    return self.error_generic("Left side of assignment must be a variable or property access");
                }
            } else {
                if self.match_token(&Token::Semicolon) {}
                Stmt::Expr(expr)
            }
        };

        Ok(Some(stmt))
    }

    fn parse_import(&mut self) -> Result<Stmt, String> {
        let mut path = Vec::new();

        loop {
            if let Token::Identifier(part) = self.advance() {
                path.push(part);
            } else {
                return self.error("Expected identifier in import path");
            }

            if self.match_token(&Token::DoubleColon) {
                continue;
            } else {
                break;
            }
        }

        if self.match_token(&Token::Semicolon) {}
        self.skip_newlines();

        Ok(Stmt::Import { path })
    }

    fn parse_module(&mut self) -> Result<Stmt, String> {
        let mut path = Vec::new();

        loop {
            if let Token::Identifier(part) = self.advance() {
                path.push(part);
            } else {
                return self.error("Expected identifier in module path");
            }

            if self.match_token(&Token::DoubleColon) {
                continue;
            } else {
                break;
            }
        }

        if self.match_token(&Token::Semicolon) {}
        self.skip_newlines();

        Ok(Stmt::Module { path })
    }

    fn parse_class(&mut self, is_native_class: bool) -> Result<Stmt, String> {
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

        self.skip_newlines();
        while !self.check(&Token::RBrace) && !self.check(&Token::Eof) {
            let mut is_private = false;
            let mut is_native_method = false;
            let mut is_async = false;

            self.skip_newlines();
            while self.check(&Token::Private) || self.check(&Token::Native) || self.check(&Token::Async) {
                if self.match_token(&Token::Private) { is_private = true; }
                else if self.match_token(&Token::Native) {
                    if !is_native_class {
                        return self.error_generic("Class member-functions can't have 'native' modifier. Use 'native class' instead.");
                    }
                    is_native_method = true;
                }
                else if self.match_token(&Token::Async) { is_async = true; }
                self.skip_newlines();
            }

            if self.match_token(&Token::Fn) {
                let method = self.parse_method(is_private, is_async, is_native_method || is_native_class)?;
                methods.push(method);
            } else if self.match_token(&Token::Constructor) {
                let method = self.parse_method_named("constructor", is_private, false, is_native_class)?;
                if is_native_class && !method.body.is_empty() {
                    return self.error_generic("Constructor in native classes cannot have implementation.");
                }
                methods.push(method);
            } else {
                if is_native_class {
                    return self.error_generic("Native classes cannot have member-fields.");
                }
                let field = self.parse_field(is_private)?;
                fields.push(field);
            }
            self.skip_newlines();
        }

        if !self.match_token(&Token::RBrace) {
            return self.error_generic("Expected '}' to close class");
        }

        Ok(Stmt::Class(ClassDef { name, type_params, parent_interfaces, fields, methods, is_native: is_native_class }))
    }

    fn parse_interface(&mut self) -> Result<Stmt, String> {
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

        self.skip_newlines();
        while !self.check(&Token::RBrace) && !self.check(&Token::Eof) {
            let mut is_private = false;
            let mut is_async = false;

            self.skip_newlines();
            while self.check(&Token::Private) || self.check(&Token::Async) {
                if self.match_token(&Token::Private) { is_private = true; }
                else if self.match_token(&Token::Async) { is_async = true; }
                self.skip_newlines();
            }

            if self.match_token(&Token::Fn) {
                let method = self.parse_interface_method(is_private, is_async)?;
                methods.push(method);
            } else {
                return self.error_generic("Interfaces can only contain method declarations");
            }
            self.skip_newlines();
        }

        if !self.match_token(&Token::RBrace) {
            return self.error_generic("Expected '}' to close interface");
        }

        Ok(Stmt::Interface(InterfaceDef { name, type_params, parent_interfaces, methods }))
    }

    fn parse_interface_method(&mut self, private: bool, is_async: bool) -> Result<Method, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return self.error_generic("Expected method name"),
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

        Ok(Method { name: name.to_string(), params, return_type, return_optional, body, private, is_async, is_native: false })
    }

    fn parse_enum(&mut self) -> Result<Stmt, String> {
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

        Ok(Stmt::Enum(EnumDef { name, variants }))
    }

    fn parse_type_alias(&mut self) -> Result<Stmt, String> {
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

        if self.match_token(&Token::Semicolon) {}

        Ok(Stmt::TypeAlias(TypeAliasDef { name, type_params, aliased_type }))
    }

    fn parse_function_ext(&mut self, _is_private: bool, is_async: bool, is_native: bool) -> Result<Stmt, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return self.error("Expected function name"),
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
                self.advance();
                let b = self.parse_block()?;
                if !self.match_token(&Token::RBrace) {
                    return self.error_generic("Expected '}' to close native function body");
                }
                b
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
            is_async, 
            is_native 
        }))
    }

    fn parse_field(&mut self, private: bool) -> Result<Field, String> {
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

        if self.match_token(&Token::Semicolon) {}

        Ok(Field { name, type_name, default, private })
    }

    fn parse_method(&mut self, private: bool, is_async: bool, is_native: bool) -> Result<Method, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return self.error_generic("Expected method name"),
        };
        self.parse_method_named(&name, private, is_async, is_native)
    }

    fn parse_method_named(&mut self, name: &str, private: bool, is_async: bool, is_native: bool) -> Result<Method, String> {
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
                self.advance();
                let b = self.parse_block()?;
                if !self.match_token(&Token::RBrace) {
                    return self.error_generic(&format!("Expected '}}' to close native {} body", name));
                }
                b
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

        Ok(Method { name: name.to_string(), params, return_type, return_optional, body, private, is_async, is_native })
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

            params.push(Param { name, type_name: Some(t_name) });

            if !self.match_token(&Token::Comma) {
                break;
            }
        }

        Ok(params)
    }

    fn parse_type(&mut self) -> Result<(String, bool), String> {
        self.skip_newlines();
        
        // Check for function type: [async] (params) -> return_type
        if self.check(&Token::LParen) || self.check(&Token::Async) {
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

        let optional = self.match_token(&Token::Question);

        Ok((full_type, optional))
    }

    fn parse_function_type(&mut self) -> Result<(String, bool), String> {
        // Check for async function type: async (params) -> return_type
        let is_async = self.match_token(&Token::Async);
        if is_async {
            self.skip_newlines();
        }
        
        // Parse function type: (param_types) -> return_type
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
        
        // Parse return type
        self.skip_newlines();
        if !self.match_token(&Token::Arrow) {
            return self.error_generic("Expected '->' in function type");
        }
        self.skip_newlines();
        
        let (return_type, optional) = self.parse_type()?;
        
        if is_async {
            type_str.insert_str(0, "async ");
        }
        type_str.push_str(" -> ");
        type_str.push_str(&return_type);
        
        Ok((type_str, optional))
    }

    fn parse_let(&mut self) -> Result<Stmt, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return self.error("Expected variable name after 'let'"),
        };

        let type_annotation = if self.match_token(&Token::Colon) {
            let (type_name, optional) = self.parse_type()?;
            if optional {
                Some(type_name + "?")
            } else {
                Some(type_name)
            }
        } else {
            None
        };

        if !self.match_token(&Token::Equal) {
            return self.error("Expected '=' in let statement");
        }

        let expr = self.parse_expression()?;

        if self.match_token(&Token::Semicolon) {}

        Ok(Stmt::Let { name, type_annotation, expr })
    }

    fn parse_return(&mut self) -> Result<Stmt, String> {
        let expr = if self.check(&Token::Semicolon) || self.check(&Token::Newline) || self.check(&Token::RBrace) || self.check(&Token::Eof) {
            None
        } else {
            Some(self.parse_expression()?)
        };

        if self.match_token(&Token::Semicolon) {}

        Ok(Stmt::Return(expr))
    }

    fn parse_if(&mut self) -> Result<Stmt, String> {
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

        Ok(Stmt::If { condition, then_branch, else_branch: None })
    }

    fn parse_for(&mut self) -> Result<Stmt, String> {
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

        Ok(Stmt::For { var_name, range: Box::new(range), body })
    }

    fn parse_while(&mut self) -> Result<Stmt, String> {
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

        Ok(Stmt::While { condition, body })
    }

    fn parse_break(&mut self) -> Result<Stmt, String> {
        if self.match_token(&Token::Semicolon) {}
        Ok(Stmt::Break)
    }

    fn parse_continue(&mut self) -> Result<Stmt, String> {
        if self.match_token(&Token::Semicolon) {}
        Ok(Stmt::Continue)
    }

    fn parse_try_catch(&mut self) -> Result<Stmt, String> {
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

        Ok(Stmt::TryCatch { try_block, catch_var, catch_block })
    }

    fn parse_throw(&mut self) -> Result<Stmt, String> {
        let expr = self.parse_expression()?;

        if self.match_token(&Token::Semicolon) {}

        Ok(Stmt::Throw(expr))
    }

    fn parse_block(&mut self) -> Result<Block, String> {
        let mut block = Vec::new();
        self.skip_newlines();

        while !self.check(&Token::RBrace) && !self.check(&Token::Eof) {
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
        if let Stmt::If { condition, then_branch, else_branch } = stmt {
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
                    });
                }
            }
            
            // No else, return the if as-is
            return Ok(Stmt::If {
                condition,
                then_branch,
                else_branch,
            });
        }
        
        Ok(stmt)
    }

    fn parse_expression(&mut self) -> Result<Expr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let expr = self.parse_and()?;

        loop {
            break;
        }

        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let expr = self.parse_range()?;

        loop {
            break;
        }

        Ok(expr)
    }

    fn parse_range(&mut self) -> Result<Expr, String> {
        let start = self.parse_equality()?;
        let span = self.compute_span(self.pos);

        if self.match_token(&Token::Range) {
            let end = self.parse_equality()?;
            return Ok(Expr::Range {
                start: Box::new(start),
                end: Box::new(end),
                span,
            });
        }

        Ok(start)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_comparison()?;

        loop {
            if self.match_token(&Token::BangEqual) {
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::NotEqual,
                    right: Box::new(self.parse_comparison()?),
                    span,
                };
            } else if self.match_token(&Token::DoubleEqual) {
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
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Greater,
                    right: Box::new(self.parse_additive()?),
                    span,
                };
            } else if self.match_token(&Token::GreaterEqual) {
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::GreaterEqual,
                    right: Box::new(self.parse_additive()?),
                    span,
                };
            } else if self.match_token(&Token::LAngle) {
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Less,
                    right: Box::new(self.parse_additive()?),
                    span,
                };
            } else if self.match_token(&Token::LessEqual) {
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
        let mut expr = self.parse_multiplicative()?;

        loop {
            if self.match_token(&Token::Plus) {
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Add,
                    right: Box::new(self.parse_multiplicative()?),
                    span,
                };
            } else if self.match_token(&Token::Minus) {
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Subtract,
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
        let mut expr = self.parse_unary()?;

        loop {
            if self.match_token(&Token::Star) {
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Multiply,
                    right: Box::new(self.parse_unary()?),
                    span,
                };
            } else if self.match_token(&Token::Slash) {
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Divide,
                    right: Box::new(self.parse_unary()?),
                    span,
                };
            } else if self.match_token(&Token::Percent) {
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Modulo,
                    right: Box::new(self.parse_unary()?),
                    span,
                };
            } else {
                break;
            }
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

        if self.match_token(&Token::PlusPlus) {
            let span = self.compute_span(self.pos - 1);
            let expr = self.parse_unary()?;
            if let Expr::Variable { .. } = &expr {
                return Ok(Expr::Unary {
                    op: UnaryOp::PrefixIncrement,
                    expr: Box::new(expr),
                    span,
                });
            } else {
                return self.error_expr("Prefix increment operator requires a variable");
            }
        }

        if self.match_token(&Token::MinusMinus) {
            let span = self.compute_span(self.pos - 1);
            let expr = self.parse_unary()?;
            if let Expr::Variable { .. } = &expr {
                return Ok(Expr::Unary {
                    op: UnaryOp::PrefixDecrement,
                    expr: Box::new(expr),
                    span,
                });
            } else {
                return self.error_expr("Prefix decrement operator requires a variable");
            }
        }

        if self.match_token(&Token::Await) {
            let span = self.compute_span(self.pos - 1);
            return Ok(Expr::Await {
                expr: Box::new(self.parse_unary()?),
                span,
            });
        }

        self.parse_call()
    }

    fn parse_call(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;

        loop {
            // Check for generic type arguments before function call
            // Only parse as generic if it's an identifier followed by <types> and then (
            if self.check(&Token::LAngle) {
                // Save position in case we need to backtrack
                let saved_pos = self.pos;
                
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
                // Backtrack - restore position
                self.pos = saved_pos;
            }

            if self.match_token(&Token::LParen) {
                let args = self.parse_arguments()?;
                let span = self.compute_span(self.pos - 1);
                self.skip_newlines();
                if !self.match_token(&Token::RParen) {
                    return self.error_expr("Expected ')' after arguments");
                }
                
                // Check if this is a cast expression (str(x), int(x), float(x), bool(x))
                if args.len() == 1 {
                    if let Expr::Variable { name, .. } = &expr {
                        let cast_type = match name.as_str() {
                            "str" => Some(CastType::Str),
                            "int" => Some(CastType::Int),
                            "float" => Some(CastType::Float),
                            "bool" => Some(CastType::Bool),
                            "int8" => Some(CastType::Int8),
                            "uint8" => Some(CastType::UInt8),
                            "int16" => Some(CastType::Int16),
                            "uint16" => Some(CastType::UInt16),
                            "int32" => Some(CastType::Int32),
                            "uint32" => Some(CastType::UInt32),
                            "int64" => Some(CastType::Int64),
                            "uint64" => Some(CastType::UInt64),
                            "float32" => Some(CastType::Float32),
                            "float64" => Some(CastType::Float64),
                            _ => None,
                        };
                        if let Some(target_type) = cast_type {
                            expr = Expr::Cast {
                                expr: Box::new(args.into_iter().next().unwrap()),
                                target_type,
                                span,
                            };
                            continue;
                        }
                    }
                }
                
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
                // Postfix increment: var++
                let span = self.compute_span(self.pos - 1);
                if let Expr::Variable { name, span: var_span } = &expr {
                    expr = Expr::Unary {
                    op: UnaryOp::PostfixIncrement,
                    expr: Box::new(Expr::Variable { name: name.clone(), span: *var_span }),
                    span,
                    };
                    } else {
                    return self.error_expr("Increment operator requires a variable");
                    }
                    } else if self.match_token(&Token::MinusMinus) {
                    // Postfix decrement: var--
                    let span = self.compute_span(self.pos - 1);
                    if let Expr::Variable { name, span: var_span } = &expr {
                    expr = Expr::Unary {
                    op: UnaryOp::PostfixDecrement,
                    expr: Box::new(Expr::Variable { name: name.clone(), span: *var_span }),
                    span,
                    };
                    } else {
                    return self.error_expr("Decrement operator requires a variable");
                    }
                    } else {                break;
            }
        }

        Ok(expr)
    }

    fn parse_arguments(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        self.skip_newlines();

        if !self.check(&Token::RParen) {
            loop {
                self.skip_newlines();
                if self.check(&Token::RParen) {
                    break;
                }
                args.push(self.parse_expression()?);

                if !self.match_token(&Token::Comma) {
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

            // Lambda parameters require type annotations
            if !self.match_token(&Token::Colon) {
                return self.error_generic(&format!("Expected ':' after parameter name '{}' in lambda. Lambda parameters require type annotations.", name));
            }

            let (mut t_name, optional) = self.parse_type()?;
            if optional {
                t_name = t_name + "?";
            }

            params.push(LambdaParam { name, type_name: Some(t_name) });

            if !self.match_token(&Token::Comma) {
                break;
            }
        }

        Ok(params)
    }

    fn parse_lambda(&mut self, is_async: bool) -> Result<Expr, String> {
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

        Ok(Expr::Lambda { params, return_type, body, span, is_async })
    }

    /// Check if the current position (after LParen) looks like a lambda
    /// Lambda syntax: [async] (params): ReturnType { body }
    fn is_lambda(&mut self) -> bool {
        // Save position
        let saved_pos = self.pos;

        // Try to parse lambda parameters
        let mut is_lambda = true;
        let mut found_colon_after_params = false;

        // Skip potential parameters
        self.skip_newlines();
        if !self.check(&Token::RParen) {
            // There are parameters - they should have type annotations
            loop {
                // Parameter name - must be identifier
                if !matches!(self.peek(), Token::Identifier(_)) {
                    is_lambda = false;
                    break;
                }
                self.advance();

                // Must have colon for type annotation
                if !self.match_token(&Token::Colon) {
                    is_lambda = false;
                    break;
                }

                // Type name
                if !self.check_type_token() {
                    is_lambda = false;
                    break;
                }
                self.advance();

                // Optional '?'
                self.match_token(&Token::Question);

                // Check for comma or end
                if !self.match_token(&Token::Comma) {
                    break;
                }
                self.skip_newlines();
            }
        }

        if is_lambda {
            // Should have closing paren
            if self.match_token(&Token::RParen) {
                self.skip_newlines();
                // Check for colon (return type) or brace (body without return type)
                if self.check(&Token::Colon) || self.check(&Token::LBrace) {
                    found_colon_after_params = true;
                }
            }
        }

        // Restore position
        self.pos = saved_pos;

        found_colon_after_params
    }

    /// Check if current position looks like an async lambda (called after 'async' was consumed)
    /// Async lambda syntax: (params): ReturnType { body }
    fn is_async_lambda(&mut self) -> bool {
        // Save position - we're already past 'async'
        let saved_pos = self.pos;
        let mut result = false;

        // Must have '(' at current position
        if self.match_token(&Token::LParen) {
            // Use is_lambda to check the rest
            result = self.is_lambda();
        }

        // Restore position
        self.pos = saved_pos;

        result
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
            Token::String(s) => self.parse_interpolated_text(s),
            Token::Int(n) => Ok(Expr::Literal(Literal::Int(n))),
            Token::Float(n) => Ok(Expr::Literal(Literal::Float(n))),
            Token::Null => Ok(Expr::Literal(Literal::Null)),
            Token::LParen => {
                // Check if this is a lambda: (params): ReturnType { body }
                // We need to peek ahead to see if this looks like a lambda
                if self.is_lambda() {
                    return self.parse_lambda(false);
                }
                // Otherwise it's a parenthesized expression
                let expr = self.parse_expression()?;
                if !self.match_token(&Token::RParen) {
                    return self.error_expr("Expected ')' after expression");
                }
                Ok(expr)
            },
            Token::Async => {
                // Check if this is an async lambda: async (params): ReturnType { body }
                // Note: self.advance() already consumed 'async', so we're now at the next token
                if self.is_async_lambda() {
                    // We're already past 'async', just need to match '(' and parse lambda
                    if self.match_token(&Token::LParen) {
                        return self.parse_lambda(true);
                    }
                }
                // Not an async lambda, treat as identifier
                Ok(Expr::Variable { name: "async".to_string(), span: self.compute_span(token_pos) })
            },
            Token::Identifier(name) => {
                let mut full_name = name;
                while self.match_token(&Token::DoubleColon) {
                    full_name.push_str("::");
                    if let Token::Identifier(part) = self.advance() {
                        full_name.push_str(&part);
                    } else {
                        return self.error_expr("Expected identifier after ::");
                    }
                }

                if full_name == "true" {
                    Ok(Expr::Literal(Literal::Bool(true)))
                } else if full_name == "false" {
                    Ok(Expr::Literal(Literal::Bool(false)))
                } else {
                    let span = self.compute_span(token_pos);
                    Ok(Expr::Variable { name: full_name, span })
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

    fn parse_interpolated_text(&mut self, s: String) -> Result<Expr, String> {
        let mut parts = Vec::new();
        let mut last_pos = 0;
        let span = Span::unknown();  // Approximate span for interpolated strings

        while let Some(interp_start) = s[last_pos..].find("${") {
            let abs_start = last_pos + interp_start;
            if abs_start > last_pos {
                parts.push(InterpPart::Text(s[last_pos..abs_start].to_string()));
            }

            let rest = &s[abs_start + 2..];
            if let Some(interp_end) = rest.find('}') {
                let expr_str = &rest[..interp_end];

                let mut sub_lexer = crate::lexer::Lexer::new(expr_str.trim(), &self.path);
                let (tokens, token_positions) = sub_lexer.tokenize()?;
                let mut sub_parser = Parser::new(tokens, expr_str.trim(), &self.path, token_positions);
                let sub_stmts = sub_parser.parse()?;

                let expr = if let Some(Stmt::Expr(e)) = sub_stmts.first() {
                    e.clone()
                } else if let Some(Stmt::Assign { name, .. }) = sub_stmts.first() {
                    let span = Span::unknown();
                    Expr::Variable { name: name.clone(), span }
                } else {
                    let span = Span::unknown();
                    Expr::Variable { name: expr_str.trim().to_string(), span }
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

        if parts.iter().any(|p| matches!(p, InterpPart::Expr(_))) {
            Ok(Expr::Interpolated { parts, span })
        } else {
            Ok(Expr::Literal(Literal::String(s)))
        }
    }
}
