use std::fmt;
use crate::error::LexerError;

pub struct Lexer {
    text: String,
    pos: usize,
    current_line: usize,
}

impl Lexer {
    pub fn new(text: &str) -> Self {
        let mut text = String::from(text);
        Self {
            text,
            pos: 0,
            current_line: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub line: usize,
    pub token_type: TokenType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    Number(f64),
    LParen,
    RParen,
    Op(String),
    Assign,
    Ident(String),
    EndExpr,
    Backslash,
    Dot,
}

impl fmt::Display for TokenType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f,"{n}"),
            Self::LParen => write!(f,"("),
            Self::RParen => write!(f,")"),
            Self::Op(s) => write!(f,"{s}"),
            Self::Assign => write!(f,"="),
            Self::Ident(s) => write!(f,"{s}"),
            Self::EndExpr => write!(f,"$"),
            Self::Backslash => write!(f, "\\"),
            Self::Dot => write!(f, "."),
        }
    }
}

pub fn binding_power(op: &str) -> (f32, f32) {
    match op {
        "+" | "-" => (1., 1.1),
        "*" | "/" => (2., 2.1),
        "<APPLY>" => (5., 5.1),
        _ => (3., 3.1),
    }
}

pub fn is_valid_unary(op: &str) -> bool {
    let valid = vec!["+", "-", "!"];
    valid.contains(&op)
}

pub fn unary_binding_power(op: &str) -> (f32, f32) {
    match op {
        "-" | "+" | "!" => (3.0, 3.1),
        _ => panic!("unknown unary op: {op}"),
    }
}

pub type LexResult<T> = Result<T, LexerError>;
impl Iterator for Lexer {
    type Item = LexResult<Token>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.text.len() {
            return None;
        }

        let slice = &self.text[self.pos..];
        let ch = slice.chars().next().unwrap();
        let ch_len = ch.len_utf8();
        
        let advance = |this: &mut Lexer, nbytes: usize| {
            this.pos += nbytes;
        };

        advance(self, ch_len);
        match ch {
            ' ' | '\t' => {
                self.next()
            }
            '=' => Some(Ok(Token {
                line: self.current_line,
                token_type: TokenType::Assign,
            })),
            '+' | '-' | '*' | '/' => Some(Ok(Token {
                line: self.current_line,
                token_type: TokenType::Op(ch.to_string()),
            })),
            '(' => Some(Ok(Token {
                line: self.current_line,
                token_type: TokenType::LParen,
            })),
            ')' => Some(Ok(Token {
                line: self.current_line,
                token_type: TokenType::RParen,
            })),
            '\n' => {
                self.current_line += 1;
                Some(Ok(Token {
                    line: self.current_line,
                    token_type: TokenType::EndExpr,
                }))
            }
            '\\' => Some(Ok(Token {
                    line: self.current_line,
                    token_type: TokenType::Backslash,
            })),
            '.' => Some(Ok(Token{
                    line: self.current_line,
                    token_type: TokenType::Dot,
            })),

            d if d.is_ascii_digit() || d == '.' => {
                let mut seen_dot = d == '.';
                let mut buf = String::new();
                buf.push(d);

                while self.pos < self.text.len() {
                    let next_slice = &self.text[self.pos..];
                    let next_ch = next_slice.chars().next().unwrap();

                    if next_ch.is_ascii_digit() {
                        buf.push(next_ch);
                        advance(self, next_ch.len_utf8());
                    } else if next_ch == '.' {
                        if seen_dot {
                            return Some(Err(LexerError::InvalidNumber(format!(
                                "Número '{}' contém múltiplos pontos decimais",
                                buf
                            ))));
                        } else {
                            seen_dot = true;
                            buf.push('.');
                            advance(self, next_ch.len_utf8());
                        }
                    } else {
                        break;
                    }
                }

                match buf.parse::<f64>() {
                    Ok(number) => Some(Ok(Token {
                        line: self.current_line,
                        token_type: TokenType::Number(number),
                    })),
                    Err(e) => Some(Err(LexerError::ParseError(
                        buf,
                        format!("Falha ao analisar número: {}", e),
                    ))),
                }
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut buf = String::from(c);
                while self.pos < self.text.len() {
                    let next_slice = &self.text[self.pos..];
                    let next_ch = next_slice.chars().next().unwrap();
                    if next_ch.is_alphanumeric() || next_ch == '_' {
                        buf.push(next_ch);
                        advance(self, next_ch.len_utf8());
                    } else {
                        break;
                    }
                }
                Some(Ok(Token {
                    line: self.current_line,
                    token_type: TokenType::Ident(buf),
                }))
            }
            _ => Some(Err(LexerError::UnrecognizedChar(ch))),
        }
    }
}
