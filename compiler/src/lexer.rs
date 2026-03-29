#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Identifier(String),
    String(String),
    Int(i64),
    Float(f64),

    Import,
    Module,
    Class,
    Interface,
    Enum,
    Fn,
    Type,
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
    Try,
    Catch,
    Throw,
    Break,
    Continue,
    Constructor,
    Static,
    As,

    TypeInt,
    TypeFloat,
    TypeStr,
    TypeBool,
    TypeInt8,
    TypeUInt8,
    TypeInt16,
    TypeUInt16,
    TypeInt32,
    TypeUInt32,
    TypeInt64,
    TypeUInt64,
    TypeFloat32,
    TypeFloat64,

    Equal,
    DoubleEqual,
    Bang,
    BangEqual,
    Question,
    Range,

    DoubleAnd,
    DoubleOr,

    BitAnd,
    BitOr,
    BitXor,
    BitNot,
    ShiftLeft,
    ShiftRight,
    ShiftLeftEqual,
    ShiftRightEqual,
    BitAndEqual,
    BitOrEqual,
    BitXorEqual,

    Plus,
    PlusPlus,
    PlusEqual,
    Minus,
    MinusMinus,
    MinusEqual,
    Star,
    StarStar,
    StarEqual,
    Slash,
    SlashEqual,
    Percent,
    PercentEqual,

    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Arrow,  // ->

    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LAngle,
    RAngle,
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
    path: String,
}

impl Lexer {
    pub fn new(source: &str, path: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            pos: 0,
            path: path.to_string(),
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

    fn get_pos(&self) -> (usize, usize) {
        let source_up_to_pos = &self.chars[..self.pos.min(self.chars.len())];
        let line = source_up_to_pos.iter().filter(|&&c| c == '\n').count() + 1;

        let mut last_newline = 0;
        for (i, &c) in source_up_to_pos.iter().enumerate() {
            if c == '\n' {
                last_newline = i + 1;
            }
        }
        let column = self.pos - last_newline + 1;

        (line, column)
    }

    fn error(&self, message: &str) -> String {
        let (line, col) = self.get_pos();
        let filename = std::path::Path::new(&self.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.path);
        format!("{}:{}:{}: error: {}", filename, line, col, message)
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

    fn skip_comment(&mut self) -> bool {
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
            true
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
            true
        } else {
            false
        }
    }

    fn next_token_inner(&mut self) -> Result<Token, String> {
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
            '[' => { self.advance(); Ok(Token::LBracket) }
            ']' => { self.advance(); Ok(Token::RBracket) }
            ':' => {
                self.advance();
                Ok(Token::Colon)
            }
            ',' => { self.advance(); Ok(Token::Comma) }
            ';' => { self.advance(); Ok(Token::Semicolon) }
            '$' => { self.advance(); Ok(Token::Dollar) }
            '=' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::DoubleEqual)
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
            '+' => {
                self.advance();
                if self.peek() == Some('+') {
                    self.advance();
                    Ok(Token::PlusPlus)
                } else if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::PlusEqual)
                } else {
                    Ok(Token::Plus)
                }
            }
            '-' => {
                self.advance();
                if self.peek() == Some('-') {
                    self.advance();
                    Ok(Token::MinusMinus)
                } else if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::MinusEqual)
                } else if self.peek() == Some('>') {
                    self.advance();
                    Ok(Token::Arrow)
                } else {
                    Ok(Token::Minus)
                }
            }
            '*' => {
                self.advance();
                if self.peek() == Some('*') {
                    self.advance();
                    Ok(Token::StarStar)
                } else if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::StarEqual)
                } else {
                    Ok(Token::Star)
                }
            }
            '/' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::SlashEqual)
                } else {
                    Ok(Token::Slash)
                }
            }
            '%' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::PercentEqual)
                } else {
                    Ok(Token::Percent)
                }
            }
            '&' => {
                self.advance();
                if self.peek() == Some('&') {
                    self.advance();
                    Ok(Token::DoubleAnd)
                } else if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::BitAndEqual)
                } else {
                    Ok(Token::BitAnd)
                }
            }
            '|' => {
                self.advance();
                if self.peek() == Some('|') {
                    self.advance();
                    Ok(Token::DoubleOr)
                } else if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::BitOrEqual)
                } else {
                    Ok(Token::BitOr)
                }
            }
            '^' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::BitXorEqual)
                } else {
                    Ok(Token::BitXor)
                }
            }
            '~' => {
                self.advance();
                Ok(Token::BitNot)
            }
            '<' => {
                self.advance();
                if self.peek() == Some('<') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        Ok(Token::ShiftLeftEqual)
                    } else {
                        Ok(Token::ShiftLeft)
                    }
                } else if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::LessEqual)
                } else if self.peek() == Some('.') {
                    // This is a range operator <..
                    Ok(Token::Less)
                } else {
                    Ok(Token::LAngle)
                }
            }
            '>' => {
                self.advance();
                if self.peek() == Some('>') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        Ok(Token::ShiftRightEqual)
                    } else {
                        Ok(Token::ShiftRight)
                    }
                } else if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::GreaterEqual)
                } else {
                    Ok(Token::RAngle)
                }
            }
            '"' => self.read_string(),
            c if c.is_alphabetic() || c == '_' => self.read_identifier(),
            c if c.is_ascii_digit() => self.read_number(),
            '.' => {
                // Check if this is a float starting with a dot (e.g., .05)
                if self.peek_next().map_or(false, |c| c.is_ascii_digit()) {
                    self.read_number_starting_with_dot()
                } else {
                    self.advance();
                    if self.peek() == Some('.') {
                        self.advance();
                        Ok(Token::Range)
                    } else {
                        Ok(Token::Dot)
                    }
                }
            }
            _ => Err(self.error(&format!("Unexpected character: '{}'", ch))),
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
            if ch == '\\' {
                self.advance();
                match self.peek() {
                    Some('n') => s.push('\n'),
                    Some('r') => s.push('\r'),
                    Some('t') => s.push('\t'),
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    Some('s') => s.push(' '),      // \s = space
                    Some('b') => s.push('\x08'),  // \b = backspace
                    Some('0') => s.push('\0'),    // \0 = null byte
                    Some('a') => s.push('\x07'),  // \a = bell/alert
                    Some('f') => s.push('\x0C'),  // \f = form feed
                    Some('v') => s.push('\x0B'),  // \v = vertical tab
                    Some('$') => {
                        s.push('\x00');  // Use null byte as marker for escaped dollar
                        s.push('$');
                    }  // \$ = literal dollar sign (not interpolation)
                    Some(c) => s.push(c),
                    None => return Err(self.error("Unterminated string escape")),
                }
                self.advance();
            } else {
                s.push(ch);
                self.advance();
            }
        }
        Err(self.error("Unterminated string"))
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
            if ch == '\\' {
                self.advance();
                match self.peek() {
                    Some('n') => s.push('\n'),
                    Some('r') => s.push('\r'),
                    Some('t') => s.push('\t'),
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    Some('s') => s.push(' '),      // \s = space
                    Some('b') => s.push('\x08'),  // \b = backspace
                    Some('0') => s.push('\0'),    // \0 = null byte
                    Some('a') => s.push('\x07'),  // \a = bell/alert
                    Some('f') => s.push('\x0C'),  // \f = form feed
                    Some('v') => s.push('\x0B'),  // \v = vertical tab
                    Some('$') => {
                        s.push('\x00');  // Use null byte as marker for escaped dollar
                        s.push('$');
                    }  // \$ = literal dollar sign (not interpolation)
                    Some(c) => s.push(c),
                    None => return Err(self.error("Unterminated multiline string escape")),
                }
                self.advance();
            } else {
                s.push(ch);
                self.advance();
            }
        }
        Err(self.error("Unterminated multiline string"))
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
                // Strip trailing \r (CRLF -> LF normalization)
                let line = line.strip_suffix('\r').unwrap_or(line);
                
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
            "interface" => Token::Interface,
            "enum" => Token::Enum,
            "fn" => Token::Fn,
            "type" => Token::Type,
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
            "try" => Token::Try,
            "catch" => Token::Catch,
            "throw" => Token::Throw,
            "break" => Token::Break,
            "continue" => Token::Continue,
            "constructor" => Token::Constructor,
            "static" => Token::Static,
            "as" => Token::As,
            "int" => Token::TypeInt,
            "float" => Token::TypeFloat,
            "str" => Token::TypeStr,
            "bool" => Token::TypeBool,
            "int8" => Token::TypeInt8,
            "uint8" => Token::TypeUInt8,
            "int16" => Token::TypeInt16,
            "uint16" => Token::TypeUInt16,
            "int32" => Token::TypeInt32,
            "uint32" => Token::TypeUInt32,
            "int64" => Token::TypeInt64,
            "uint64" => Token::TypeUInt64,
            "float32" => Token::TypeFloat32,
            "float64" => Token::TypeFloat64,
            _ => Token::Identifier(s),
        };
        Ok(token)
    }

    fn read_number(&mut self) -> Result<Token, String> {
        let mut s = String::new();
        let mut is_float = false;

        // Check for number base prefix (0x, 0o, 0b)
        if self.peek() == Some('0') {
            if let Some(prefix_ch) = self.peek_next() {
                match prefix_ch {
                    'x' | 'X' => {
                        // Hexadecimal number
                        self.advance(); // consume '0'
                        self.advance(); // consume 'x' or 'X'
                        return self.read_hex_number();
                    }
                    'o' | 'O' => {
                        // Octal number
                        self.advance(); // consume '0'
                        self.advance(); // consume 'o' or 'O'
                        return self.read_octal_number();
                    }
                    'b' | 'B' => {
                        // Binary number
                        self.advance(); // consume '0'
                        self.advance(); // consume 'b' or 'B'
                        return self.read_binary_number();
                    }
                    _ => {}
                }
            }
        }

        while let Some(ch) = self.peek() {
            if ch == '\'' {
                // Digit separator (e.g., 1'000'000)
                self.advance();
            } else if ch == '.' {
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
            let (line, col) = self.get_pos();
            s.parse::<f64>()
                .map(Token::Float)
                .map_err(|e| format!("[{}:{}] Invalid float: {}", line, col, e))
        } else {
            let (line, col) = self.get_pos();
            s.parse::<i64>()
                .map(Token::Int)
                .map_err(|e| format!("[{}:{}] Invalid int: {}", line, col, e))
        }
    }

    fn read_number_starting_with_dot(&mut self) -> Result<Token, String> {
        let mut s = String::from(".");
        self.advance(); // consume '.'

        while let Some(ch) = self.peek() {
            if ch == '\'' {
                // Digit separator (e.g., .1'000'000)
                self.advance();
            } else if ch.is_ascii_digit() {
                s.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        let (line, col) = self.get_pos();
        s.parse::<f64>()
            .map(Token::Float)
            .map_err(|e| format!("[{}:{}] Invalid float: {}", line, col, e))
    }

    fn read_hex_number(&mut self) -> Result<Token, String> {
        let mut s = String::new();
        while let Some(ch) = self.peek() {
            if ch == '\'' {
                // Digit separator (e.g., 0x1A'FF)
                self.advance();
            } else if ch.is_ascii_hexdigit() {
                s.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        if s.is_empty() {
            let (line, col) = self.get_pos();
            return Err(format!("[{}:{}] Invalid hex number: expected digits after 0x", line, col));
        }

        let (line, col) = self.get_pos();
        i64::from_str_radix(&s, 16)
            .map(Token::Int)
            .map_err(|e| format!("[{}:{}] Invalid hex number: {}", line, col, e))
    }

    fn read_octal_number(&mut self) -> Result<Token, String> {
        let mut s = String::new();
        while let Some(ch) = self.peek() {
            if ch == '\'' {
                // Digit separator (e.g., 0o7'55)
                self.advance();
            } else if ch.is_ascii_digit() && ch != '8' && ch != '9' {
                s.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        if s.is_empty() {
            let (line, col) = self.get_pos();
            return Err(format!("[{}:{}] Invalid octal number: expected digits after 0o", line, col));
        }

        let (line, col) = self.get_pos();
        i64::from_str_radix(&s, 8)
            .map(Token::Int)
            .map_err(|e| format!("[{}:{}] Invalid octal number: {}", line, col, e))
    }

    fn read_binary_number(&mut self) -> Result<Token, String> {
        let mut s = String::new();
        while let Some(ch) = self.peek() {
            if ch == '\'' {
                // Digit separator (e.g., 0b1010'1100)
                self.advance();
            } else if ch == '0' || ch == '1' {
                s.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        if s.is_empty() {
            let (line, col) = self.get_pos();
            return Err(format!("[{}:{}] Invalid binary number: expected digits after 0b", line, col));
        }

        let (line, col) = self.get_pos();
        i64::from_str_radix(&s, 2)
            .map(Token::Int)
            .map_err(|e| format!("[{}:{}] Invalid binary number: {}", line, col, e))
    }

    pub fn tokenize(&mut self) -> Result<(Vec<Token>, Vec<usize>), String> {
        // Skip shebang line if it exists at the very beginning
        if self.pos == 0 && self.peek() == Some('#') && self.peek_next() == Some('!') {
            while let Some(ch) = self.peek() {
                if ch == '\n' {
                    break;
                }
                self.advance();
            }
        }

        let mut tokens = Vec::new();
        let mut token_positions = Vec::new();
        loop {
            // Skip all whitespace and comments
            loop {
                self.skip_whitespace();
                if !self.skip_comment() {
                    break;
                }
            }

            let token_pos = self.pos;
            let token = self.next_token_inner()?;
            if token == Token::Eof {
                tokens.push(token);
                token_positions.push(token_pos);
                break;
            }
            tokens.push(token);
            token_positions.push(token_pos);
        }
        Ok((tokens, token_positions))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(source: &str) -> Result<Vec<Token>, String> {
        let mut lexer = Lexer::new(source, "test");
        let (tokens, _) = lexer.tokenize()?;
        Ok(tokens)
    }

    #[test]
    fn test_hex_numbers() {
        assert_eq!(tokenize("0xFF").unwrap(), vec![Token::Int(255), Token::Eof]);
        assert_eq!(tokenize("0x1A").unwrap(), vec![Token::Int(26), Token::Eof]);
        assert_eq!(tokenize("0x100").unwrap(), vec![Token::Int(256), Token::Eof]);
        assert_eq!(tokenize("0x0").unwrap(), vec![Token::Int(0), Token::Eof]);
        assert_eq!(tokenize("0XFF").unwrap(), vec![Token::Int(255), Token::Eof]);
    }

    #[test]
    fn test_octal_numbers() {
        assert_eq!(tokenize("0o77").unwrap(), vec![Token::Int(63), Token::Eof]);
        assert_eq!(tokenize("0o10").unwrap(), vec![Token::Int(8), Token::Eof]);
        assert_eq!(tokenize("0o0").unwrap(), vec![Token::Int(0), Token::Eof]);
        assert_eq!(tokenize("0O77").unwrap(), vec![Token::Int(63), Token::Eof]);
    }

    #[test]
    fn test_binary_numbers() {
        assert_eq!(tokenize("0b1010").unwrap(), vec![Token::Int(10), Token::Eof]);
        assert_eq!(tokenize("0b1111").unwrap(), vec![Token::Int(15), Token::Eof]);
        assert_eq!(tokenize("0b0").unwrap(), vec![Token::Int(0), Token::Eof]);
        assert_eq!(tokenize("0B1010").unwrap(), vec![Token::Int(10), Token::Eof]);
    }

    #[test]
    fn test_number_with_separators() {
        assert_eq!(tokenize("0x1A'FF").unwrap(), vec![Token::Int(6911), Token::Eof]);
        assert_eq!(tokenize("0b1010'1100").unwrap(), vec![Token::Int(172), Token::Eof]);
        assert_eq!(tokenize("0o7'55").unwrap(), vec![Token::Int(493), Token::Eof]);
    }

    #[test]
    fn test_decimal_still_works() {
        assert_eq!(tokenize("123").unwrap(), vec![Token::Int(123), Token::Eof]);
        assert_eq!(tokenize("0").unwrap(), vec![Token::Int(0), Token::Eof]);
        assert_eq!(tokenize("1'000'000").unwrap(), vec![Token::Int(1000000), Token::Eof]);
    }

    #[test]
    fn test_bitwise_compound_assignment() {
        assert_eq!(tokenize("x <<= 1").unwrap(), vec![
            Token::Identifier("x".to_string()),
            Token::ShiftLeftEqual,
            Token::Int(1),
            Token::Eof
        ]);
        assert_eq!(tokenize("x >>= 2").unwrap(), vec![
            Token::Identifier("x".to_string()),
            Token::ShiftRightEqual,
            Token::Int(2),
            Token::Eof
        ]);
        assert_eq!(tokenize("x &= 0xFF").unwrap(), vec![
            Token::Identifier("x".to_string()),
            Token::BitAndEqual,
            Token::Int(255),
            Token::Eof
        ]);
        assert_eq!(tokenize("x |= 0b1010").unwrap(), vec![
            Token::Identifier("x".to_string()),
            Token::BitOrEqual,
            Token::Int(10),
            Token::Eof
        ]);
        assert_eq!(tokenize("x ^= 0o77").unwrap(), vec![
            Token::Identifier("x".to_string()),
            Token::BitXorEqual,
            Token::Int(63),
            Token::Eof
        ]);
    }

    #[test]
    fn test_string_escapes() {
        // Basic escapes
        assert_eq!(tokenize("\"\\n\"").unwrap(), vec![Token::String("\n".to_string()), Token::Eof]);
        assert_eq!(tokenize("\"\\r\"").unwrap(), vec![Token::String("\r".to_string()), Token::Eof]);
        assert_eq!(tokenize("\"\\t\"").unwrap(), vec![Token::String("\t".to_string()), Token::Eof]);
        assert_eq!(tokenize("\"\\\\\"").unwrap(), vec![Token::String("\\".to_string()), Token::Eof]);
        assert_eq!(tokenize("\"\\\"\"").unwrap(), vec![Token::String("\"".to_string()), Token::Eof]);
        
        // New escapes
        assert_eq!(tokenize("\"\\s\"").unwrap(), vec![Token::String(" ".to_string()), Token::Eof]);
        assert_eq!(tokenize("\"\\b\"").unwrap(), vec![Token::String("\x08".to_string()), Token::Eof]);
        assert_eq!(tokenize("\"\\0\"").unwrap(), vec![Token::String("\0".to_string()), Token::Eof]);
        assert_eq!(tokenize("\"\\a\"").unwrap(), vec![Token::String("\x07".to_string()), Token::Eof]);
        assert_eq!(tokenize("\"\\f\"").unwrap(), vec![Token::String("\x0C".to_string()), Token::Eof]);
        assert_eq!(tokenize("\"\\v\"").unwrap(), vec![Token::String("\x0B".to_string()), Token::Eof]);
    }

    #[test]
    fn test_string_combined_escapes() {
        assert_eq!(tokenize("\"Hello\\sWorld\"").unwrap(), vec![Token::String("Hello World".to_string()), Token::Eof]);
        assert_eq!(tokenize("\"Line1\\nLine2\"").unwrap(), vec![Token::String("Line1\nLine2".to_string()), Token::Eof]);
        assert_eq!(tokenize("\"Tab\\tSeparated\"").unwrap(), vec![Token::String("Tab\tSeparated".to_string()), Token::Eof]);
    }

    #[test]
    fn test_string_dollar_escape() {
        // Escaped dollar sign - internally uses \x00$ marker to prevent interpolation
        assert_eq!(tokenize("\"\\$\"").unwrap(), vec![Token::String("\x00$".to_string()), Token::Eof]);
        assert_eq!(tokenize("\"\\$100\"").unwrap(), vec![Token::String("\x00$100".to_string()), Token::Eof]);
        // Escaped interpolation syntax
        assert_eq!(tokenize("\"\\${}\"").unwrap(), vec![Token::String("\x00${}".to_string()), Token::Eof]);
        assert_eq!(tokenize("\"\\${x}\"").unwrap(), vec![Token::String("\x00${x}".to_string()), Token::Eof]);
    }

    #[test]
    fn test_float_leading_dot() {
        assert_eq!(tokenize(".05").unwrap(), vec![Token::Float(0.05), Token::Eof]);
        assert_eq!(tokenize(".5").unwrap(), vec![Token::Float(0.5), Token::Eof]);
        assert_eq!(tokenize(".0").unwrap(), vec![Token::Float(0.0), Token::Eof]);
        assert_eq!(tokenize(".123").unwrap(), vec![Token::Float(0.123), Token::Eof]);
        assert_eq!(tokenize(".1'000").unwrap(), vec![Token::Float(0.1), Token::Eof]);
    }
}
