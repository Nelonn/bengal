#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Identifier(String),
    String(String),
    Int(i64),
    Float(f64),

    Import,
    Module,
    Class,
    Fn,
    Let,
    If,
    Else,
    Return,
    Private,
    Null,

    TypeInt,
    TypeFloat,
    TypeStr,
    TypeBool,

    Equal,
    Bang,
    BangEqual,
    Question,

    Plus,
    Minus,
    Star,
    Slash,

    LParen,
    RParen,
    LBrace,
    RBrace,
    Colon,
    Comma,
    Semicolon,
    Dot,
    Dollar,

    Newline,
    Eof,
}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek();
        self.pos += 1;
        ch
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() && ch != '\n' {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_comment(&mut self) {
        if self.peek() == Some('/') && self.peek_next() == Some('/') {
            while let Some(ch) = self.peek() {
                if ch == '\n' {
                    break;
                }
                self.advance();
            }
        }
    }

    pub fn next_token(&mut self) -> Result<Token, String> {
        self.skip_whitespace();
        self.skip_comment();
        self.skip_whitespace();

        let ch = match self.peek() {
            Some(c) => c,
            None => return Ok(Token::Eof),
        };

        match ch {
            '\n' => {
                self.advance();
                Ok(Token::Newline)
            }
            '(' => { self.advance(); Ok(Token::LParen) }
            ')' => { self.advance(); Ok(Token::RParen) }
            '{' => { self.advance(); Ok(Token::LBrace) }
            '}' => { self.advance(); Ok(Token::RBrace) }
            ':' => { self.advance(); Ok(Token::Colon) }
            ',' => { self.advance(); Ok(Token::Comma) }
            ';' => { self.advance(); Ok(Token::Semicolon) }
            '.' => { self.advance(); Ok(Token::Dot) }
            '$' => { self.advance(); Ok(Token::Dollar) }
            '=' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::BangEqual)
                } else {
                    Ok(Token::Equal)
                }
            }
            '!' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::BangEqual)
                } else {
                    Ok(Token::Bang)
                }
            }
            '?' => { self.advance(); Ok(Token::Question) }
            '+' => { self.advance(); Ok(Token::Plus) }
            '-' => { self.advance(); Ok(Token::Minus) }
            '*' => { self.advance(); Ok(Token::Star) }
            '/' => {
                self.advance();
                if self.peek() == Some('/') {
                    // This is a comment, skip it
                    self.skip_comment();
                    self.next_token()
                } else {
                    Ok(Token::Slash)
                }
            }
            '"' => self.read_string(),
            c if c.is_alphabetic() || c == '_' => self.read_identifier(),
            c if c.is_ascii_digit() => self.read_number(),
            _ => Err(format!("Unexpected character: '{}'", ch)),
        }
    }

    fn read_string(&mut self) -> Result<Token, String> {
        self.advance();
        let mut s = String::new();

        while let Some(ch) = self.peek() {
            if ch == '"' {
                self.advance();
                return Ok(Token::String(s));
            }
            s.push(ch);
            self.advance();
        }
        Err("Unterminated string".to_string())
    }

    fn read_identifier(&mut self) -> Result<Token, String> {
        let mut s = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                s.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        let token = match s.as_str() {
            "import" => Token::Import,
            "module" => Token::Module,
            "class" => Token::Class,
            "fn" => Token::Fn,
            "let" => Token::Let,
            "if" => Token::If,
            "else" => Token::Else,
            "return" => Token::Return,
            "private" => Token::Private,
            "null" => Token::Null,
            "int" => Token::TypeInt,
            "float" => Token::TypeFloat,
            "str" => Token::TypeStr,
            "bool" => Token::TypeBool,
            _ => Token::Identifier(s),
        };
        Ok(token)
    }

    fn read_number(&mut self) -> Result<Token, String> {
        let mut s = String::new();
        let mut is_float = false;

        while let Some(ch) = self.peek() {
            if ch == '.' {
                if is_float {
                    break;
                }
                is_float = true;
                s.push(ch);
                self.advance();
            } else if ch.is_ascii_digit() {
                s.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        if is_float {
            s.parse::<f64>()
                .map(Token::Float)
                .map_err(|e| format!("Invalid float: {}", e))
        } else {
            s.parse::<i64>()
                .map(Token::Int)
                .map_err(|e| format!("Invalid int: {}", e))
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token()?;
            if token == Token::Eof {
                tokens.push(token);
                break;
            }
            tokens.push(token);
        }
        Ok(tokens)
    }
}
