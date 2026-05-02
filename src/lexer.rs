use std::fmt;
use std::str::FromStr;
use crate::error::{LexerError, LexerErrorType};
use strum_macros::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub col: usize,
}

impl Span {
    pub fn merge(&self, other: &Span) -> Span {
        Span {
            start: std::cmp::min(self.start, other.start),
            end: std::cmp::max(self.end, other.end),
            line: std::cmp::min(self.line, other.line),
            col: if self.start <= other.start { self.col } else { other.col },
        }
    }
}

pub struct Lexer {
    text: String,
    pos: usize,
    current_line: usize,
    current_col: usize,
}

impl Lexer {
    pub fn new(text: &str) -> Self {
        let text = String::from(text);
        Self {
            text,
            pos: 0,
            current_line: 0,
            current_col: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub span: Span,
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
    StrictNumber(String, String),
    Str(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBrack,
    RBrack,
    Op(String),
    Ident(String),
    Keyword(Keyword),
    EndExpr,
    Backslash,
    Apostrophe,
    Dot,
    DebugDot,
}

impl fmt::Display for TokenType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f,"{n}"),
            Self::StrictNumber(n, s) => write!(f, "{n}{s}"),
            Self::Str(s) => write!(f, "\"{s}\""),
            Self::LParen => write!(f,"("),
            Self::RParen => write!(f,")"),
            Self::LBrace => write!(f,"{{"),
            Self::RBrace => write!(f,"}}"),
            Self::LBrack => write!(f,"["),
            Self::RBrack => write!(f,"]"),
            Self::Op(s) => write!(f,"{s}"),
            Self::Ident(s) => write!(f,"{s}"),
            Self::Keyword(s) => write!(f,"{s:?}"),
            Self::EndExpr => write!(f,";"),
            Self::Backslash => write!(f, "\\"),
            Self::Apostrophe => write!(f, "'"),
            Self::Dot => write!(f, "."),
            Self::DebugDot => write!(f, "ç"),
        }
    }
}

fn op_alphabet() -> &'static str {
    "+-*/<>=!?&|,#:!@$%¨`~^·•»«ø→↓←´ªº°§¹²³£¢¬"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum Keyword {
    #[strum(to_string = "true")]
    True,
    #[strum(to_string = "false")]
    False,
    #[strum(to_string = "nil")]
    Nil,
    #[strum(to_string = "type")]
    Type,
    #[strum(to_string = "let")]
    Let,
    #[strum(to_string = "in")]
    In,
    #[strum(to_string = "if")]
    If,
    #[strum(to_string = "then")]
    Then,
    #[strum(to_string = "else")]
    Else,
}

impl FromStr for Keyword {
   type Err = ();

   fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "true"  => Ok(Keyword::True),
            "false" => Ok(Keyword::False),
            "nil"   => Ok(Keyword::Nil),
            "type"  => Ok(Keyword::Type),
            "let"   => Ok(Keyword::Let),
            "in"    => Ok(Keyword::In),
            "if"    => Ok(Keyword::If),
            "then"  => Ok(Keyword::Then),
            "else"  => Ok(Keyword::Else),
            _ => Err(()),
        }
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
        let chars: Vec<char>= slice.chars().collect();
        let first_ch_len = chars[0].len_utf8();
        
        let start_pos = self.pos;
        let line = self.current_line;
        let start_col = self.current_col;

        self.pos += first_ch_len;
        self.current_col += 1;

        let mount_token = |this: &Lexer, mam: TokenType| {
            Some(Ok(Token {
                span: Span {
                    start: start_pos,
                    end: this.pos,
                    line,
                    col: start_col,
                },
                token_type: mam,
            }))
        };

        let err = |this: &Lexer, errtype: LexerErrorType| {
            Some(Err(LexerError {
                errtype,
                span: Span {
                    start: start_pos,
                    end: this.pos,
                    line,
                    col: start_col,
                }.into(),
            }))
        };

        match chars.as_slice() {
            [' ', ..] | ['\t', ..] => self.next(),
            ['\n', ..] => {
                self.current_line += 1;
                self.current_col = 0;
                self.next()
            }
            ['/', '/', ..] => {
                while self.pos < self.text.len() {
                    let next_slice = &self.text[self.pos..];
                    let next_ch = next_slice.chars().next().unwrap();
                    if next_ch != '\n' {
                        self.pos += next_ch.len_utf8();
                        self.current_col += 1;
                    } else {
                        break;
                    }
                }
                self.next()
            }

            ['ç', ..] => mount_token(self, TokenType::DebugDot),
            ['(', ..] => mount_token(self, TokenType::LParen),
            [')', ..] => mount_token(self, TokenType::RParen),
            ['{', ..] => mount_token(self, TokenType::LBrace),
            ['}', ..] => mount_token(self, TokenType::RBrace),
            ['[', ..] => mount_token(self, TokenType::LBrack),
            [']', ..] => mount_token(self, TokenType::RBrack),
            [';', ..] => mount_token(self, TokenType::EndExpr),
            ['.', ..] => mount_token(self, TokenType::Dot),

            ['\'', ..] => mount_token(self, TokenType::Apostrophe),
            ['\\', ..] => mount_token(self, TokenType::Backslash),
 
            [u, ..] if op_alphabet().contains(*u) => {
                let mut buf = String::from(*u);
                while self.pos < self.text.len() {
                    let next_slice = &self.text[self.pos..];
                    let next_ch = next_slice.chars().next().unwrap();
                    if op_alphabet().contains(next_ch) {
                        buf.push(next_ch);
                        self.pos += next_ch.len_utf8();
                        self.current_col += 1;
                    } else {
                        break;
                    }
                }
                mount_token(self, TokenType::Op(buf))
            }           

            [d, ..] if d.is_ascii_digit() || *d == '.' => {
                let mut seen_dot = *d == '.';
                let mut buf = String::new();
                buf.push(*d);

                while self.pos < self.text.len() {
                    let next_slice = &self.text[self.pos..];
                    let next_ch = next_slice.chars().next().unwrap();

                    if next_ch.is_ascii_digit() {
                        buf.push(next_ch);
                        self.pos += next_ch.len_utf8();
                        self.current_col += 1;
                    } else if next_ch == '.' {
                        if seen_dot {
                            return err(self, LexerErrorType::InvalidNumber(format!(
                                "Número '{}' contém múltiplos pontos decimais",
                                buf
                            )));
                        } else {
                            seen_dot = true;
                            buf.push('.');
                            self.pos += next_ch.len_utf8();
                            self.current_col += 1;
                        }
                    } else {
                        break;
                    }
                }

                let mut suffix = String::new();
                while self.pos < self.text.len() {
                    let next_slice = &self.text[self.pos..];
                    let next_ch = next_slice.chars().next().unwrap();
                    if next_ch.is_alphanumeric() {
                        suffix.push(next_ch);
                        self.pos += next_ch.len_utf8();
                        self.current_col += 1;
                    } else {
                        break;
                    }
                }

                if !suffix.is_empty() {
                    return mount_token(self, TokenType::StrictNumber(buf, suffix));
                }

                match buf.parse::<f64>() {
                    Ok(number) => mount_token(self, TokenType::Number(number)),
                    Err(e) => err(self, LexerErrorType::ParseError(
                        buf,
                        format!("Falha ao analisar número: {}", e),
                    )),
                }
            }

            [c, ..] if c.is_alphabetic() || *c == '_' => {
                let mut buf = String::from(*c);
                while self.pos < self.text.len() {
                    let next_slice = &self.text[self.pos..];
                    let next_ch = next_slice.chars().next().unwrap();
                    if next_ch.is_alphanumeric() || next_ch == '_' {
                        buf.push(next_ch);
                        self.pos += next_ch.len_utf8();
                        self.current_col += 1;
                    } else {
                        break;
                    }
                }

                if let Ok(x) = Keyword::from_str(buf.as_str()) {
                    mount_token(self, TokenType::Keyword(x))
                } else {
                    mount_token(self, TokenType::Ident(buf))
                }
            }

            ['"', ..] => {
                let mut buf = String::new();
                while self.pos < self.text.len() {
                    let next_slice = &self.text[self.pos..];
                    let next_ch = next_slice.chars().next().unwrap();
                    if next_ch != '"' {
                        buf.push(next_ch);
                        self.pos += next_ch.len_utf8();
                        self.current_col += 1;
                    } else {
                        self.pos += 1;
                        self.current_col += 1;
                        break;
                    }
                }
                mount_token(self, TokenType::Str(buf))
            }

            [ch, ..] => err(self, LexerErrorType::UnrecognizedChar(*ch)),
            &[] => None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spans() {
        let mut lexer = Lexer::new("123 + \"abc\"");
        
        let t1 = lexer.next().unwrap().unwrap();
        assert_eq!(t1.token_type, TokenType::Number(123.0));
        assert_eq!(t1.span.start, 0);
        assert_eq!(t1.span.end, 3);
        assert_eq!(t1.span.line, 0);
        assert_eq!(t1.span.col, 0);

        let t2 = lexer.next().unwrap().unwrap();
        assert_eq!(t2.token_type, TokenType::Op("+".into()));
        assert_eq!(t2.span.start, 4);
        assert_eq!(t2.span.end, 5);
        assert_eq!(t2.span.line, 0);
        assert_eq!(t2.span.col, 4);

        let t3 = lexer.next().unwrap().unwrap();
        assert_eq!(t3.token_type, TokenType::Str("abc".into()));
        assert_eq!(t3.span.start, 6);
        assert_eq!(t3.span.end, 11);
        assert_eq!(t3.span.line, 0);
        assert_eq!(t3.span.col, 6);
    }

    #[test]
    fn test_multiline_spans() {
        let mut lexer = Lexer::new("a\nb");
        
        let t1 = lexer.next().unwrap().unwrap();
        assert_eq!(t1.token_type, TokenType::Ident("a".into()));
        assert_eq!(t1.span.line, 0);
        assert_eq!(t1.span.col, 0);

        let t2 = lexer.next().unwrap().unwrap();
        assert_eq!(t2.token_type, TokenType::Ident("b".into()));
        assert_eq!(t2.span.line, 1);
        assert_eq!(t2.span.col, 0);
    }
}
