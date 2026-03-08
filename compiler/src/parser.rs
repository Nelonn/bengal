use crate::lexer::Token;

#[derive(Debug, Clone)]
pub enum Stmt {
    Module { path: Vec<String> },
    Import { path: Vec<String> },
    Class(ClassDef),
    Enum(EnumDef),
    Function(FunctionDef),
    Let { name: String, expr: Expr },
    Assign { name: String, expr: Expr },
    Return(Option<Expr>),
    Expr(Expr),
    If { condition: Expr, then_branch: Block, else_branch: Option<Block> },
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
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub type_name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    Variable(String),
    Binary { left: Box<Expr>, op: BinaryOp, right: Box<Expr> },
    Unary { op: UnaryOp, expr: Box<Expr> },
    Call { callee: Box<Expr>, args: Vec<Expr> },
    Get { object: Box<Expr>, name: String },
    Set { object: Box<Expr>, name: String, value: Box<Expr> },
    Interpolated { parts: Vec<InterpPart> },
    Await { expr: Box<Expr> },
}

#[derive(Debug, Clone)]
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
    Greater,
    Less,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Not,
}

#[derive(Debug, Clone)]
pub enum InterpPart {
    Text(String),
    Expr(Expr),
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
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

        let stmt = if self.match_token(&Token::Module) {
            self.parse_module()?
        } else if self.match_token(&Token::Import) {
            self.parse_import()?
        } else if self.match_token(&Token::Class) {
            self.parse_class()?
        } else if self.match_token(&Token::Enum) {
            self.parse_enum()?
        } else if self.match_token(&Token::Async) {
            if self.match_token(&Token::Fn) {
                self.parse_async_function()?
            } else {
                return Err("Expected 'fn' after 'async'".to_string());
            }
        } else if self.match_token(&Token::Fn) {
            self.parse_function()?
        } else if self.match_token(&Token::Let) {
            self.parse_let()?
        } else if self.match_token(&Token::Return) {
            self.parse_return()?
        } else if self.match_token(&Token::If) {
            self.parse_if()?
        } else if self.match_token(&Token::Private) {
            return Err("Unexpected 'private' keyword".to_string());
        } else {
            let expr = self.parse_expression()?;

            if self.match_token(&Token::Equal) {
                if let Expr::Variable(name) = expr {
                    let value = self.parse_expression()?;
                    if self.match_token(&Token::Semicolon) {}
                    Stmt::Assign { name, expr: value }
                } else {
                    return Err("Left side of assignment must be a variable".to_string());
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

            if self.match_token(&Token::Dot) {
                continue;
            } else if self.check(&Token::Colon) {
                self.advance();
                if self.match_token(&Token::Colon) {
                    continue;
                } else {
                    return Err("Expected ':' after ':' in import path".to_string());
                }
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

            if self.match_token(&Token::Dot) {
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
            let is_private = self.match_token(&Token::Private);
            let is_async = self.match_token(&Token::Async);
            self.skip_newlines();

            if self.match_token(&Token::Fn) {
                let method = self.parse_method(is_private, is_async)?;
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

    fn parse_function(&mut self) -> Result<Stmt, String> {
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

        if !self.match_token(&Token::LBrace) {
            return Err("Expected '{' to start function body".to_string());
        }

        let body = self.parse_block()?;

        if !self.match_token(&Token::RBrace) {
            return Err("Expected '}' to close function".to_string());
        }

        Ok(Stmt::Function(FunctionDef { name, params, return_type, return_optional, body, is_async: false }))
    }

    fn parse_async_function(&mut self) -> Result<Stmt, String> {
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

        if !self.match_token(&Token::LBrace) {
            return Err("Expected '{' to start function body".to_string());
        }

        let body = self.parse_block()?;

        if !self.match_token(&Token::RBrace) {
            return Err("Expected '}' to close function".to_string());
        }

        Ok(Stmt::Function(FunctionDef { name, params, return_type, return_optional, body, is_async: true }))
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

    fn parse_method(&mut self, private: bool, is_async: bool) -> Result<Method, String> {
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

        if !self.match_token(&Token::LBrace) {
            return Err("Expected '{' to start method body".to_string());
        }

        let body = self.parse_block()?;

        if !self.match_token(&Token::RBrace) {
            return Err("Expected '}' to close method".to_string());
        }

        Ok(Method { name, params, return_type, return_optional, body, private, is_async })
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
        let expr = self.parse_equality()?;

        loop {
            break;
        }

        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_comparison()?;

        loop {
            if self.match_token(&Token::BangEqual) {
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::NotEqual,
                    right: Box::new(self.parse_comparison()?),
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
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Greater,
                    right: Box::new(self.parse_additive()?),
                };
            } else if self.match_token(&Token::Less) {
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Less,
                    right: Box::new(self.parse_additive()?),
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
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Add,
                    right: Box::new(self.parse_multiplicative()?),
                };
            } else if self.match_token(&Token::Minus) {
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Subtract,
                    right: Box::new(self.parse_multiplicative()?),
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
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Multiply,
                    right: Box::new(self.parse_unary()?),
                };
            } else if self.match_token(&Token::Slash) {
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Divide,
                    right: Box::new(self.parse_unary()?),
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.match_token(&Token::Bang) {
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(self.parse_unary()?),
            });
        }

        if self.match_token(&Token::Await) {
            return Ok(Expr::Await {
                expr: Box::new(self.parse_unary()?),
            });
        }

        self.parse_call()
    }

    fn parse_call(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.match_token(&Token::LParen) {
                let args = self.parse_arguments()?;
                self.skip_newlines();
                if !self.match_token(&Token::RParen) {
                    return Err("Expected ')' after arguments".to_string());
                }
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
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
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args: vec![],
                };
            } else if self.match_token(&Token::Dot) {
                let name = match self.advance() {
                    Token::Identifier(n) => n,
                    _ => return Err("Expected identifier after '.'".to_string()),
                };
                expr = Expr::Get {
                    object: Box::new(expr),
                    name,
                };
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
        match self.advance() {
            Token::String(s) => {
                if let Some(interp_pos) = s.find("${") {
                    let before = &s[..interp_pos];
                    let rest = &s[interp_pos + 2..];

                    if let Some(end_pos) = rest.find('}') {
                        let expr_str = &rest[..end_pos];
                        let after = &rest[end_pos + 1..];

                        let mut sub_lexer = crate::lexer::Lexer::new(expr_str.trim());
                        let tokens = sub_lexer.tokenize()?;
                        let mut sub_parser = Parser::new(tokens);
                        let sub_stmts = sub_parser.parse()?;

                        let expr = if let Some(Stmt::Expr(e)) = sub_stmts.first() {
                            e.clone()
                        } else if let Some(Stmt::Assign { name, expr: _ }) = sub_stmts.first() {
                            Expr::Variable(name.clone())
                        } else {
                            Expr::Variable(expr_str.trim().to_string())
                        };

                        let mut parts = vec![InterpPart::Text(before.to_string()), InterpPart::Expr(expr)];

                        if !after.is_empty() {
                            parts.push(InterpPart::Text(after.to_string()));
                        }

                        Ok(Expr::Interpolated { parts })
                    } else {
                        Err("Unclosed interpolation ${...}".to_string())
                    }
                } else {
                    Ok(Expr::Literal(Literal::String(s)))
                }
            }
            Token::Int(n) => Ok(Expr::Literal(Literal::Int(n))),
            Token::Float(n) => Ok(Expr::Literal(Literal::Float(n))),
            Token::TypeBool => Ok(Expr::Literal(Literal::Bool(true))),
            Token::Identifier(name) if name == "true" => Ok(Expr::Literal(Literal::Bool(true))),
            Token::Identifier(name) if name == "false" => Ok(Expr::Literal(Literal::Bool(false))),
            Token::Null => Ok(Expr::Literal(Literal::Null)),
            Token::Identifier(name) => Ok(Expr::Variable(name)),
            Token::Dollar => self.parse_interpolated_string(),
            token => Err(format!("Unexpected token: {:?}", token)),
        }
    }

    fn parse_interpolated_string(&mut self) -> Result<Expr, String> {
        if !self.match_token(&Token::LBrace) {
            return Err("Expected '{' after '$' in interpolated string".to_string());
        }

        let expr = self.parse_expression()?;

        if !self.match_token(&Token::RBrace) {
            return Err("Expected '}' to close interpolated expression".to_string());
        }

        Ok(Expr::Interpolated {
            parts: vec![InterpPart::Expr(expr)]
        })
    }
}
