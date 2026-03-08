use crate::lexer::Token;

#[derive(Debug, Clone)]
pub enum Stmt {
    Import { path: Vec<String> },
    Class(ClassDef),
    Let { name: String, expr: Expr },
    Assign { name: String, expr: Expr },
    Return(Option<Expr>),
    Expr(Expr),
    If { condition: Expr, then_branch: Block, else_branch: Option<Block> },
}

pub type Block = Vec<Stmt>;

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

        let stmt = if self.match_token(&Token::Import) {
            self.parse_import()?
        } else if self.match_token(&Token::Class) {
            self.parse_class()?
        } else if self.match_token(&Token::Let) {
            self.parse_let()?
        } else if self.match_token(&Token::Return) {
            self.parse_return()?
        } else if self.match_token(&Token::If) {
            self.parse_if()?
        } else if self.match_token(&Token::Fn) {
            return Err("Unexpected 'fn' outside of class".to_string());
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

            if self.check(&Token::Colon) {
                self.advance();
                if self.check(&Token::Colon) {
                    self.advance();
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if self.match_token(&Token::Semicolon) {}
        self.skip_newlines();

        Ok(Stmt::Import { path })
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
            self.skip_newlines();

            if self.match_token(&Token::Fn) {
                let method = self.parse_method(is_private)?;
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

    fn parse_field(&mut self, private: bool) -> Result<Field, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            t => return Err(format!("Expected field name, got {:?}", t)),
        };

        if !self.match_token(&Token::Colon) {
            return Err(format!("Expected ':' after field name, got {:?}", self.peek()));
        }

        let type_name = match self.advance() {
            Token::TypeInt => "int".to_string(),
            Token::TypeFloat => "float".to_string(),
            Token::TypeStr => "str".to_string(),
            Token::TypeBool => "bool".to_string(),
            Token::Identifier(t) => t,
            t => return Err(format!("Expected type name, got {:?}", t)),
        };

        let default = if self.match_token(&Token::Equal) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        if self.match_token(&Token::Semicolon) {}

        Ok(Field { name, type_name, default, private })
    }

    fn parse_method(&mut self, private: bool) -> Result<Method, String> {
        let name = match self.advance() {
            Token::Identifier(n) => n,
            _ => return Err("Expected method name".to_string()),
        };

        if !self.match_token(&Token::LParen) {
            return Err("Expected '(' after method name".to_string());
        }

        let params = self.parse_params()?;

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

        Ok(Method { name, params, return_type, return_optional, body, private })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, String> {
        let mut params = Vec::new();

        if self.check(&Token::RParen) {
            return Ok(params);
        }

        loop {
            let name = match self.advance() {
                Token::Identifier(n) => n,
                _ => return Err("Expected parameter name".to_string()),
            };

            let type_name = if self.match_token(&Token::Colon) {
                match self.advance() {
                    Token::TypeInt => Some("int".to_string()),
                    Token::TypeFloat => Some("float".to_string()),
                    Token::TypeStr => Some("str".to_string()),
                    Token::TypeBool => Some("bool".to_string()),
                    Token::Identifier(t) => Some(t),
                    _ => return Err("Expected parameter type".to_string()),
                }
            } else {
                None
            };

            params.push(Param { name, type_name });

            if !self.match_token(&Token::Comma) {
                break;
            }
        }

        Ok(params)
    }

    fn parse_type(&mut self) -> Result<(String, bool), String> {
        let type_name = match self.advance() {
            Token::TypeInt => "int".to_string(),
            Token::TypeFloat => "float".to_string(),
            Token::TypeStr => "str".to_string(),
            Token::TypeBool => "bool".to_string(),
            Token::Identifier(t) => t,
            _ => return Err("Expected type name".to_string()),
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
        let expr = if self.check(&Token::Semicolon) || self.check(&Token::RBrace) || self.check(&Token::Eof) {
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
        let mut expr = self.parse_unary()?;

        loop {
            if self.match_token(&Token::BangEqual) {
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::NotEqual,
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

        self.parse_call()
    }

    fn parse_call(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.match_token(&Token::LParen) {
                let args = self.parse_arguments()?;
                if !self.match_token(&Token::RParen) {
                    return Err("Expected ')' after arguments".to_string());
                }
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
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

        if !self.check(&Token::RParen) {
            loop {
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
