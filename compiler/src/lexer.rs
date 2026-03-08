#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Identifier(String),
    String(String),
    Int(i64),
    Float(f64),

    Import,
    Module,
    Class,
    Enum,
    Fn,
    Let,
    If,
    Else,
    For,
    While,
    In,
    Return,
    Private,
    Null,
    Native,
    Async,
    Await,
    Try,
    Catch,
    Throw,

    TypeInt,
    TypeFloat,
    TypeStr,
    TypeBool,

    Equal,
    Bang,
    BangEqual,
    Question,
    Range,

    Plus,
    Minus,
    Star,
    Slash,

    Greater,
    Less,

    LParen,
    RParen,
    LBrace,
    RBrace,
    Colon,
    DoubleColon,
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
        // Line comment: //
        if self.peek() == Some('/') && self.peek_next() == Some('/') {
            self.advance(); // skip first /
            self.advance(); // skip second /
            while let Some(ch) = self.peek() {
                if ch == '\n' {
                    break;
                }
                self.advance();
            }
        }
        // Block comment: /* */
        else if self.peek() == Some('/') && self.peek_next() == Some('*') {
            self.advance(); // skip /
            self.advance(); // skip *
            let mut depth = 1;
            while let Some(ch) = self.peek() {
                if ch == '/' && self.peek_next() == Some('*') {
                    // Nested block comment
                    self.advance();
                    self.advance();
                    depth += 1;
                } else if ch == '*' && self.peek_next() == Some('/') {
                    // End of block comment
                    self.advance();
                    self.advance();
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                } else {
                    self.advance();
                }
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
            ':' => {
                self.advance();
                if self.peek() == Some(':') {
                    self.advance();
                    Ok(Token::DoubleColon)
                } else {
                    Ok(Token::Colon)
                }
            }
            ',' => { self.advance(); Ok(Token::Comma) }
            ';' => { self.advance(); Ok(Token::Semicolon) }
            '.' => {
                self.advance();
                if self.peek() == Some('.') {
                    self.advance();
                    Ok(Token::Range)
                } else {
                    Ok(Token::Dot)
                }
            }
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
            '>' => { self.advance(); Ok(Token::Greater) }
            '<' => { self.advance(); Ok(Token::Less) }
            c if c.is_alphabetic() || c == '_' => self.read_identifier(),
            c if c.is_ascii_digit() => self.read_number(),
            _ => Err(format!("Unexpected character: '{}'", ch)),
        }
    }

    fn read_string(&mut self) -> Result<Token, String> {
        self.advance(); // consume first "
        
        if self.peek() == Some('"') && self.peek_next() == Some('"') {
            self.advance(); // consume second "
            self.advance(); // consume third "
            return self.read_multiline_string();
        }

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

    fn read_multiline_string(&mut self) -> Result<Token, String> {
        let mut s = String::new();
        while let Some(ch) = self.peek() {
            if ch == '"' && self.peek_next() == Some('"') && self.chars.get(self.pos + 2) == Some(&'"') {
                self.advance();
                self.advance();
                self.advance();
                
                let processed = self.process_multiline_string(&s);
                return Ok(Token::String(processed));
            }
            s.push(ch);
            self.advance();
        }
        Err("Unterminated multiline string".to_string())
    }

    fn process_multiline_string(&self, s: &str) -> String {
        let mut lines: Vec<&str> = s.split('\n').collect();
        
        if lines.is_empty() {
            return String::new();
        }

        if let Some(first) = lines.first() {
            if first.trim().is_empty() {
                lines.remove(0);
            }
        }

        if let Some(last) = lines.last() {
            if last.trim().is_empty() {
                lines.pop();
            }
        }

        if lines.is_empty() {
            return String::new();
        }

        let min_indent = lines.iter()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.chars().take_while(|c| c.is_whitespace()).count())
            .min()
            .unwrap_or(0);

        let processed_lines: Vec<String> = lines.into_iter()
            .map(|line| {
                if line.len() <= min_indent {
                    if line.trim().is_empty() {
                        String::new()
                    } else {
                        line.to_string()
                    }
                } else {
                    line.chars().skip(min_indent).collect()
                }
            })
            .collect();

        processed_lines.join("\n")
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
            "enum" => Token::Enum,
            "fn" => Token::Fn,
            "let" => Token::Let,
            "if" => Token::If,
            "else" => Token::Else,
            "for" => Token::For,
            "while" => Token::While,
            "in" => Token::In,
            "return" => Token::Return,
            "private" => Token::Private,
            "null" => Token::Null,
            "native" => Token::Native,
            "async" => Token::Async,
            "await" => Token::Await,
            "try" => Token::Try,
            "catch" => Token::Catch,
            "throw" => Token::Throw,
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
                // Check if this is a range operator (..)
                if self.peek_next() == Some('.') {
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
