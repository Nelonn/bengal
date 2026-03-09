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
    Enum(EnumDef),
    Function(FunctionDef),
    Let { name: String, expr: Expr },
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
    pub fields: Vec<Field>,
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
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CastType {
    Int,
    Float,
    Str,
    Bool,
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
    token_positions: Vec<usize>,  // Position in source for each token
}

impl Parser {
    pub fn new(tokens: Vec<Token>, source: &str, token_positions: Vec<usize>) -> Self {
        Self {
            tokens,
            pos: 0,
            source: source.to_string(),
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

    fn peek_next(&self) -> &Token {
        self.tokens.get(self.pos + 1).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let token = self.peek().clone();
        self.pos += 1;
        token
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

        while self.check(&Token::Native) || self.check(&Token::Async) {
            if self.match_token(&Token::Native) { is_native = true; }
            else if self.match_token(&Token::Async) { is_async = true; }
            self.skip_newlines();
        }

        let stmt = if self.match_token(&Token::Module) {
            self.parse_module()?
        } else if self.match_token(&Token::Import) {
            self.parse_import()?
        } else if self.match_token(&Token::Class) {
            self.parse_class()?
        } else if self.match_token(&Token::Enum) {
            self.parse_enum()?
        } else if self.match_token(&Token::Fn) {
            self.parse_function_ext(false, is_async, is_native)?
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
                    return Err("Left side of assignment must be a variable or property access".to_string());
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
                return Err("Expected identifier in import path".to_string());
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
                return Err("Expected identifier in module path".to_string());
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

    fn parse_class(&mut self) -> Result<Stmt, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return Err("Expected class name".to_string()),
        };

        if !self.match_token(&Token::LBrace) {
            return Err("Expected '{' after class name".to_string());
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        self.skip_newlines();
        while !self.check(&Token::RBrace) && !self.check(&Token::Eof) {
            let mut is_private = false;
            let mut is_native = false;
            let mut is_async = false;

            self.skip_newlines();
            while self.check(&Token::Private) || self.check(&Token::Native) || self.check(&Token::Async) {
                if self.match_token(&Token::Private) { is_private = true; }
                else if self.match_token(&Token::Native) { is_native = true; }
                else if self.match_token(&Token::Async) { is_async = true; }
                self.skip_newlines();
            }

            if self.match_token(&Token::Fn) {
                let method = self.parse_method(is_private, is_async, is_native)?;
                methods.push(method);
            } else {
                let field = self.parse_field(is_private)?;
                fields.push(field);
            }
            self.skip_newlines();
        }

        if !self.match_token(&Token::RBrace) {
            return Err("Expected '}' to close class".to_string());
        }

        Ok(Stmt::Class(ClassDef { name, fields, methods }))
    }

    fn parse_enum(&mut self) -> Result<Stmt, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return Err("Expected enum name".to_string()),
        };

        if !self.match_token(&Token::LBrace) {
            return Err("Expected '{' after enum name".to_string());
        }

        let mut variants = Vec::new();
        self.skip_newlines();

        while !self.check(&Token::RBrace) && !self.check(&Token::Eof) {
            let variant_name = match self.advance() {
                Token::Identifier(n) => n,
                _ => return Err("Expected variant name".to_string()),
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
            return Err("Expected '}' to close enum".to_string());
        }

        Ok(Stmt::Enum(EnumDef { name, variants }))
    }

    fn parse_function_ext(&mut self, _is_private: bool, is_async: bool, is_native: bool) -> Result<Stmt, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return Err("Expected function name".to_string()),
        };

        if !self.match_token(&Token::LParen) {
            return Err("Expected '(' after function name".to_string());
        }

        let params = self.parse_params()?;
        self.skip_newlines();

        if !self.match_token(&Token::RParen) {
            return Err("Expected ')' after parameters".to_string());
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
                    return Err("Expected '}' to close native function body".to_string());
                }
                b
            } else {
                // Also allow no semicolon if it's the end of a block/file
                Vec::new()
            }
        } else {
            if !self.match_token(&Token::LBrace) {
                return Err(format!("Expected '{{' to start function body for '{}'", name));
            }

            let body = self.parse_block()?;

            if !self.match_token(&Token::RBrace) {
                return Err("Expected '}' to close function".to_string());
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
            t => return Err(format!("Expected field name, got {:?}", t)),
        };

        if !self.match_token(&Token::Colon) {
            return Err(format!("Expected ':' after field name, got {:?}", self.peek()));
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
            _ => return Err("Expected method name".to_string()),
        };

        if !self.match_token(&Token::LParen) {
            return Err("Expected '(' after method name".to_string());
        }

        let params = self.parse_params()?;
        self.skip_newlines();

        if !self.match_token(&Token::RParen) {
            return Err("Expected ')' after parameters".to_string());
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
                    return Err("Expected '}' to close native method body".to_string());
                }
                b
            } else {
                Vec::new()
            }
        } else {
            if !self.match_token(&Token::LBrace) {
                return Err("Expected '{' to start method body".to_string());
            }

            let body = self.parse_block()?;

            if !self.match_token(&Token::RBrace) {
                return Err("Expected '}' to close method".to_string());
            }
            body
        };

        Ok(Method { name, params, return_type, return_optional, body, private, is_async, is_native })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, String> {
        let mut params = Vec::new();
        self.skip_newlines();

        if self.check(&Token::RParen) {
            return Ok(params);
        }

        let mut current_type: Option<String> = None;

        loop {
            self.skip_newlines();
            if self.check(&Token::RParen) {
                break;
            }

            let mut param_type: Option<String> = None;

            let is_type_start = (self.check(&Token::TypeInt) || self.check(&Token::TypeFloat) ||
               self.check(&Token::TypeStr) || self.check(&Token::TypeBool)) &&
               matches!(self.peek_next(), Token::Identifier(_));

            let is_class_type = !is_type_start && matches!(self.peek(), Token::Identifier(_)) &&
               matches!(self.peek_next(), Token::Identifier(_));

            if is_type_start || is_class_type {
                let (mut type_name, optional) = self.parse_type()?;
                if optional {
                    type_name = type_name + "?";
                }
                let potential_type = Some(type_name);

                if potential_type.is_some() && matches!(self.peek(), Token::Identifier(_)) {
                    param_type = potential_type;
                    current_type = param_type.clone();
                } else {
                    // This was actually a name if we can't find another identifier
                    // But with our grammar (type name), this should be the name
                    // and we use the current_type
                    if let Some(name) = potential_type {
                        param_type = current_type.clone();
                        params.push(Param { name, type_name: param_type });

                        if !self.match_token(&Token::Comma) {
                            break;
                        }
                        continue;
                    }
                }
            }

            let name = match self.advance() {
                Token::Identifier(n) => n,
                t => return Err(format!("Expected parameter name, got {:?}", t)),
            };

            let type_name = if self.match_token(&Token::Colon) {
                let (mut t_name, optional) = self.parse_type()?;
                if optional {
                    t_name = t_name + "?";
                }
                Some(t_name)
            } else {
                param_type.or_else(|| current_type.clone())
            };

            params.push(Param { name, type_name });

            if !self.match_token(&Token::Comma) {
                break;
            }
        }

        Ok(params)
    }

    fn parse_type(&mut self) -> Result<(String, bool), String> {
        self.skip_newlines();
        let token = self.advance();
        let type_name = match &token {
            Token::TypeInt => "int".to_string(),
            Token::TypeFloat => "float".to_string(),
            Token::TypeStr => "str".to_string(),
            Token::TypeBool => "bool".to_string(),
            Token::Null => "null".to_string(),
            Token::Identifier(t) => t.clone(),
            _ => return Err(format!("Expected type name, got {:?}", token)),
        };

        let optional = self.match_token(&Token::Question);

        Ok((type_name, optional))
    }

    fn parse_let(&mut self) -> Result<Stmt, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return Err("Expected variable name after 'let'".to_string()),
        };

        if !self.match_token(&Token::Equal) {
            return Err("Expected '=' in let statement".to_string());
        }

        let expr = self.parse_expression()?;

        if self.match_token(&Token::Semicolon) {}

        Ok(Stmt::Let { name, expr })
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
            return Err("Expected '(' after 'if'".to_string());
        }

        let condition = self.parse_expression()?;
        self.skip_newlines();

        if !self.match_token(&Token::RParen) {
            return Err("Expected ')' after condition".to_string());
        }

        if !self.match_token(&Token::LBrace) {
            return Err("Expected '{' for if body".to_string());
        }

        let then_branch = self.parse_block()?;

        if !self.match_token(&Token::RBrace) {
            return Err("Expected '}' to close if body".to_string());
        }

        let else_branch = if self.match_token(&Token::Else) {
            if !self.match_token(&Token::LBrace) {
                return Err("Expected '{' for else body".to_string());
            }
            Some(self.parse_block()?)
        } else {
            None
        };

        Ok(Stmt::If { condition, then_branch, else_branch })
    }

    fn parse_for(&mut self) -> Result<Stmt, String> {
        if !self.match_token(&Token::LParen) {
            return Err("Expected '(' after 'for'".to_string());
        }

        let var_name = match self.advance() {
            Token::Identifier(name) => name,
            _ => return Err("Expected variable name in for loop".to_string()),
        };

        if !self.match_token(&Token::In) {
            return Err("Expected 'in' after loop variable".to_string());
        }

        let range = self.parse_range()?;

        if !self.match_token(&Token::RParen) {
            return Err("Expected ')' after range expression".to_string());
        }

        if !self.match_token(&Token::LBrace) {
            return Err("Expected '{' for loop body".to_string());
        }

        let body = self.parse_block()?;

        if !self.match_token(&Token::RBrace) {
            return Err("Expected '}' to close loop body".to_string());
        }

        Ok(Stmt::For { var_name, range: Box::new(range), body })
    }

    fn parse_while(&mut self) -> Result<Stmt, String> {
        if !self.match_token(&Token::LParen) {
            return Err("Expected '(' after 'while'".to_string());
        }

        let condition = self.parse_expression()?;

        if !self.match_token(&Token::RParen) {
            return Err("Expected ')' after condition".to_string());
        }

        if !self.match_token(&Token::LBrace) {
            return Err("Expected '{' for while body".to_string());
        }

        let body = self.parse_block()?;

        if !self.match_token(&Token::RBrace) {
            return Err("Expected '}' to close while body".to_string());
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
            return Err("Expected '{' for try body".to_string());
        }

        let try_block = self.parse_block()?;

        if !self.match_token(&Token::RBrace) {
            return Err("Expected '}' to close try body".to_string());
        }

        self.skip_newlines();

        if !self.match_token(&Token::Catch) {
            return Err("Expected 'catch' after try block".to_string());
        }

        if !self.match_token(&Token::LParen) {
            return Err("Expected '(' after 'catch'".to_string());
        }

        let catch_var = match self.advance() {
            Token::Identifier(n) => n,
            _ => return Err("Expected identifier for catch variable".to_string()),
        };

        if !self.match_token(&Token::RParen) {
            return Err("Expected ')' after catch variable".to_string());
        }

        if !self.match_token(&Token::LBrace) {
            return Err("Expected '{' for catch body".to_string());
        }

        let catch_block = self.parse_block()?;

        if !self.match_token(&Token::RBrace) {
            return Err("Expected '}' to close catch body".to_string());
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
                block.push(stmt);
            }
            self.skip_newlines();
        }

        Ok(block)
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
            if self.match_token(&Token::Greater) {
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
            } else if self.match_token(&Token::Less) {
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
                return Err("Prefix increment operator requires a variable".to_string());
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
                return Err("Prefix decrement operator requires a variable".to_string());
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
            if self.match_token(&Token::LParen) {
                let args = self.parse_arguments()?;
                let span = self.compute_span(self.pos - 1);
                self.skip_newlines();
                if !self.match_token(&Token::RParen) {
                    return Err("Expected ')' after arguments".to_string());
                }
                
                // Check if this is a cast expression (str(x), int(x), float(x), bool(x))
                if args.len() == 1 {
                    if let Expr::Variable { name, .. } = &expr {
                        let cast_type = match name.as_str() {
                            "str" => Some(CastType::Str),
                            "int" => Some(CastType::Int),
                            "float" => Some(CastType::Float),
                            "bool" => Some(CastType::Bool),
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
            } else if self.match_token(&Token::LBrace) {
                // Class instantiation with {} - for now we just consume the braces
                // Full field initialization support would go here

                // Check for empty braces first
                if !self.check(&Token::RBrace) {
                    // Try to parse field initializers (name: value pairs)
                    loop {
                        self.skip_newlines();
                        if self.check(&Token::RBrace) {
                            break;
                        }
                        // Skip field name
                        self.advance();
                        // Skip colon
                        if self.match_token(&Token::Colon) {
                            // Skip value expression
                            self.parse_expression()?;
                        }
                        // Skip comma or semicolon
                        if self.match_token(&Token::Comma) || self.match_token(&Token::Semicolon) {
                            continue;
                        }
                        if self.check(&Token::RBrace) {
                            break;
                        }
                    }
                }
                self.skip_newlines();
                if !self.match_token(&Token::RBrace) {
                    return Err("Expected '}' to close class instantiation".to_string());
                }
                // For now, class instantiation is just a Call with no args
                let span = self.compute_span(self.pos - 1);
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args: vec![],
                    span,
                };
            } else if self.match_token(&Token::Dot) {
                let name = match self.advance() {
                    Token::Identifier(n) => n,
                    _ => return Err("Expected identifier after '.'".to_string()),
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
                    return Err("Increment operator requires a variable".to_string());
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
                    return Err("Decrement operator requires a variable".to_string());
                }
            } else {
                break;
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

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let token_pos = self.pos;
        match self.advance() {
            Token::String(s) => self.parse_interpolated_text(s),
            Token::Int(n) => Ok(Expr::Literal(Literal::Int(n))),
            Token::Float(n) => Ok(Expr::Literal(Literal::Float(n))),
            Token::Null => Ok(Expr::Literal(Literal::Null)),
            Token::LParen => {
                let expr = self.parse_expression()?;
                if !self.match_token(&Token::RParen) {
                    return Err("Expected ')' after expression".to_string());
                }
                Ok(expr)
            },
            Token::Identifier(name) => {
                let mut full_name = name;
                while self.match_token(&Token::DoubleColon) {
                    full_name.push_str("::");
                    if let Token::Identifier(part) = self.advance() {
                        full_name.push_str(&part);
                    } else {
                        return Err("Expected identifier after ::".to_string());
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
            Token::Dollar => self.parse_interpolated_string(),
            token => Err(format!("Unexpected token: {:?}", token)),
        }
    }

    fn parse_interpolated_string(&mut self) -> Result<Expr, String> {
        if !self.match_token(&Token::LBrace) {
            return Err("Expected '{' after '$' in interpolated string".to_string());
        }

        let span = self.compute_span(self.pos - 1);
        let expr = self.parse_expression()?;

        if !self.match_token(&Token::RBrace) {
            return Err("Expected '}' to close interpolated expression".to_string());
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

                let mut sub_lexer = crate::lexer::Lexer::new(expr_str.trim());
                let (tokens, token_positions) = sub_lexer.tokenize()?;
                let mut sub_parser = Parser::new(tokens, expr_str.trim(), token_positions);
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
                return Err("Unclosed interpolation ${...}".to_string());
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
