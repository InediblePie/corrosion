pub mod error;
pub mod token;

pub use error::LexError;
pub use token::{Token, TokenKind};

use crate::source::SourceSpan;

pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    Lexer::new(source).lex()
}

struct Lexer<'a> {
    source: &'a str,
    current: usize,
    start: usize,
    token_line: usize,
    token_column: usize,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            current: 0,
            start: 0,
            token_line: 1,
            token_column: 1,
            line: 1,
            column: 1,
            tokens: Vec::new(),
        }
    }

    fn lex(mut self) -> Result<Vec<Token>, LexError> {
        while !self.is_at_end() {
            self.start = self.current;
            self.token_line = self.line;
            self.token_column = self.column;
            self.scan_token()?;
        }

        let span = SourceSpan::new(self.current, self.current, self.line, self.column);
        self.tokens.push(Token::new(TokenKind::Eof, span));
        Ok(self.tokens)
    }

    fn scan_token(&mut self) -> Result<(), LexError> {
        let c = self.advance();
        match c {
            '(' => self.add(TokenKind::LeftParen),
            ')' => self.add(TokenKind::RightParen),
            '{' => self.add(TokenKind::LeftBrace),
            '}' => self.add(TokenKind::RightBrace),
            '[' => self.add(TokenKind::LeftBracket),
            ']' => self.add(TokenKind::RightBracket),
            ',' => self.add(TokenKind::Comma),
            ':' => self.add(TokenKind::Colon),
            ';' => self.add(TokenKind::Semicolon),
            '+' => self.add(TokenKind::Plus),
            '*' => self.add(TokenKind::Star),
            '%' => self.add(TokenKind::Percent),
            '!' => {
                if self.match_char('=') {
                    self.add(TokenKind::BangEqual);
                } else {
                    self.add(TokenKind::Bang);
                }
            }
            '=' => {
                if self.match_char('=') {
                    self.add(TokenKind::EqualEqual);
                } else {
                    self.add(TokenKind::Equal);
                }
            }
            '<' => {
                if self.match_char('=') {
                    self.add(TokenKind::LessEqual);
                } else {
                    self.add(TokenKind::Less);
                }
            }
            '>' => {
                if self.match_char('=') {
                    self.add(TokenKind::GreaterEqual);
                } else {
                    self.add(TokenKind::Greater);
                }
            }
            '&' => {
                if self.match_char('&') {
                    self.add(TokenKind::AmpAmp);
                } else {
                    return Err(self.error("unexpected '&'; did you mean '&&'?"));
                }
            }
            '|' => {
                if self.match_char('|') {
                    self.add(TokenKind::PipePipe);
                } else {
                    return Err(self.error("unexpected '|'; did you mean '||'?"));
                }
            }
            '-' => {
                if self.match_char('>') {
                    self.add(TokenKind::Arrow);
                } else {
                    self.add(TokenKind::Minus);
                }
            }
            '.' => {
                if self.match_char('.') && self.match_char('.') {
                    self.add(TokenKind::DotDotDot);
                } else {
                    self.add(TokenKind::Dot);
                }
            }
            '/' => {
                if self.match_char('/') {
                    while self.peek() != '\n' && !self.is_at_end() {
                        self.advance();
                    }
                } else if self.match_char('*') {
                    self.block_comment()?;
                } else {
                    self.add(TokenKind::Slash);
                }
            }
            '"' => self.string()?,
            ' ' | '\r' | '\t' | '\n' => {}
            c if c.is_ascii_digit() => self.number()?,
            c if is_identifier_start(c) => self.identifier(),
            _ => return Err(self.error(format!("invalid character '{c}'"))),
        }
        Ok(())
    }

    fn block_comment(&mut self) -> Result<(), LexError> {
        while !self.is_at_end() {
            if self.peek() == '*' && self.peek_next() == '/' {
                self.advance();
                self.advance();
                return Ok(());
            }
            self.advance();
        }

        Err(LexError::new(
            "unterminated block comment",
            SourceSpan::new(self.start, self.current, self.token_line, self.token_column),
        ))
    }

    fn string(&mut self) -> Result<(), LexError> {
        let mut value = String::new();

        while !self.is_at_end() {
            let c = self.advance();
            match c {
                '"' => {
                    self.add(TokenKind::String(value));
                    return Ok(());
                }
                '\\' => {
                    if self.is_at_end() {
                        return Err(self.error("unterminated string"));
                    }
                    let escaped = self.advance();
                    match escaped {
                        '"' => value.push('"'),
                        '\\' => value.push('\\'),
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        other => {
                            return Err(self.error(format!("invalid string escape '\\{other}'")));
                        }
                    }
                }
                '\n' => value.push('\n'),
                other => value.push(other),
            }
        }

        Err(LexError::new(
            "unterminated string",
            SourceSpan::new(self.start, self.current, self.token_line, self.token_column),
        ))
    }

    fn number(&mut self) -> Result<(), LexError> {
        while self.peek().is_ascii_digit() {
            self.advance();
        }

        let mut is_float = false;
        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            is_float = true;
            self.advance();
            while self.peek().is_ascii_digit() {
                self.advance();
            }
        }

        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            return Err(self.error("invalid number format"));
        }

        let text = &self.source[self.start..self.current];
        if is_float {
            let value = text
                .parse::<f64>()
                .map_err(|_| self.error("invalid float literal"))?;
            self.add(TokenKind::Float(value));
        } else {
            let value = text
                .parse::<i64>()
                .map_err(|_| self.error("invalid integer literal"))?;
            self.add(TokenKind::Int(value));
        }
        Ok(())
    }

    fn identifier(&mut self) {
        while is_identifier_part(self.peek()) {
            self.advance();
        }

        let text = &self.source[self.start..self.current];
        let kind = match text {
            "let" => TokenKind::Let,
            "fn" => TokenKind::Fn,
            "return" => TokenKind::Return,
            "struct" => TokenKind::Struct,
            "namespace" => TokenKind::Namespace,
            "extern" => TokenKind::Extern,
            "import" => TokenKind::Import,
            "new" => TokenKind::New,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            _ => TokenKind::Identifier(text.to_string()),
        };
        self.add(kind);
    }

    fn add(&mut self, kind: TokenKind) {
        self.tokens.push(Token::new(
            kind,
            SourceSpan::new(self.start, self.current, self.token_line, self.token_column),
        ));
    }

    fn advance(&mut self) -> char {
        let c = self.source[self.current..].chars().next().unwrap();
        self.current += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        c
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() || self.peek() != expected {
            return false;
        }
        self.advance();
        true
    }

    fn peek(&self) -> char {
        self.source[self.current..].chars().next().unwrap_or('\0')
    }

    fn peek_next(&self) -> char {
        self.source[self.current..].chars().nth(1).unwrap_or('\0')
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn error(&self, message: impl Into<String>) -> LexError {
        LexError::new(
            message,
            SourceSpan::new(self.start, self.current, self.token_line, self.token_column),
        )
    }
}

fn is_identifier_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_identifier_part(c: char) -> bool {
    is_identifier_start(c) || c.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_required_surface() {
        let tokens = lex(r#"
            let x = 1.5;
            let f = fn(args...) {};
            let y = 10 % 3;
            object->method();
            arr[1:3];
            // comment
            /* comment */
            "#)
        .unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Let));
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::Float(1.5))
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::DotDotDot)
        );
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Arrow));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Percent));
    }

    #[test]
    fn rejects_unterminated_string() {
        assert!(lex("\"oops").is_err());
    }
}
