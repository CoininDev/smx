use std::fmt;
use std::str::FromStr;
use crate::error::LexerError;
use strum_macros::Display;


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

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.token_type)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    Number(f64),
    LParen,
    RParen,
    Op(String),
    Assign,
    Ident(String),
    Keyword(Keyword),
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
            Self::Keyword(s) => write!(f,"{s:?}"),
            Self::EndExpr => write!(f,"$"),
            Self::Backslash => write!(f, "\\"),
            Self::Dot => write!(f, "."),
        }
    }
}

fn op_alphabet() -> &'static str {
    "+-*/<>=?&|"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum Keyword {
    #[strum(to_string = "true")]
    True,
    #[strum(to_string = "false")]
    False
}

impl FromStr for Keyword {
   type Err = ();

   fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "true"  => Ok(Keyword::True),
            "false" => Ok(Keyword::False),
            _ => Err(()),
        }
   }
}

pub fn is_valid_unary(op: &str) -> bool {
    let valid = vec!["+", "-", "!"];
    valid.contains(&op)
}

pub type LexResult<T> = Result<T, LexerError>;
impl Iterator for Lexer {
    type Item = LexResult<Token>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.text.len() {
            return None;
        }
        let slice = &self.text[self.pos..];
        let chars: Vec<char>= slice.chars().collect();
        let first_ch_len = chars[0].len_utf8();
        
        let advance = |this: &mut Lexer, nbytes: usize| {
            this.pos += nbytes;
        };

        let line = self.current_line;
        let mount_token = |mam: TokenType| {
            Some(Ok(Token {
                line,
                token_type: mam,
            }))
        };

        advance(self, first_ch_len);
        match chars.as_slice() {
            [' ', ..] | ['\t', ..] => self.next(),

            ['=', x, ..] if *x != '=' => mount_token(TokenType::Assign),

            [u, ..] if op_alphabet().contains(*u) => {
                let mut buf = String::from(*u);
                while self.pos < self.text.len() {
                    let next_slice = &self.text[self.pos..];
                    let next_ch = next_slice.chars().next().unwrap();
                    if op_alphabet().contains(next_ch) {
                        buf.push(next_ch);
                        advance(self, next_ch.len_utf8());
                    } else {
                        break;
                    }
                }
                mount_token(TokenType::Op(buf))
            }

            ['(', ..] => mount_token(TokenType::LParen),

            [')', ..] => mount_token(TokenType::RParen),

            ['\n', ..] => {
                self.current_line += 1;
                self.next()
            }

            ['\\', ..] => mount_token(TokenType::Backslash),

            ['.', ..] => mount_token(TokenType::Dot),

            [d, ..] if d.is_ascii_digit() || *d == '.' => {
                let mut seen_dot = *d == '.';
                let mut buf = String::new();
                buf.push(*d);

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
                    Ok(number) => mount_token(TokenType::Number(number)),
                    Err(e) => Some(Err(LexerError::ParseError(
                        buf,
                        format!("Falha ao analisar número: {}", e),
                    ))),
                }
            }

            [c, ..] if c.is_alphabetic() || *c == '_' => {
                let mut buf = String::from(*c);
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

                if let Ok(x) = Keyword::from_str(buf.as_str()) {
                    mount_token(TokenType::Keyword(x))
                } else {
                    mount_token(TokenType::Ident(buf))
                }
            }

            [ch, ..] => Some(Err(LexerError::UnrecognizedChar(*ch))),
            &[] => None
        }
    }
}

