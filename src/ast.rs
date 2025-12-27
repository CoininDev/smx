use crate::{lexer::*, error::ParsingError};
use std::fmt::Display;

#[derive(Debug, Clone)]
pub struct Program {
    pub body: Vec<Assign>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Var(String),
    Num(f64),
    Lambda(String, Box<Expression>),
    Application(Box<Expression>, Box<Expression>),
    Parenthed(Box<Expression>),
    Operation(String, Vec<Expression>),
}

impl Display for Expression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expression::Var(s) => write!(f, "var '{s}'"),
            Expression::Num(i) => write!(f, "{i}"),
            Expression::Lambda(arg, body) => write!(f, "\\{arg}. {body}"),
            Expression::Parenthed(a) => write!(f, "({a})"),
            Expression::Application(a, b) => write!(f, "{a} {b}"),
            Expression::Operation(op, e) => {
                write!(f, "({op}")?;
                for expr in e {
                    write!(f, " {expr}")?;
                }
                write!(f, ")")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Assign (pub String, pub Expression);

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

pub type ParseResult<T> = Result<T, ParsingError>;

impl Parser {
    pub fn peek(&self, p: usize) -> Option<&Token> {
        self.tokens.get(self.pos + p)
    }

    pub fn peek_type(&self, p: usize) -> Option<&TokenType> {
        self.tokens.get(self.pos + p).map(|t| &t.token_type)
    }

    pub fn next(&mut self) -> Option<Token> {
        if self.pos >= self.tokens.len() {
            return None;
        }

        let t = self.tokens[self.pos].clone();
        self.pos += 1;
        Some(t)
    }

    pub fn expect(&mut self, expected: TokenType) -> ParseResult<()> {
        match self.peek(0) {
            Some(token) if token.token_type == expected => {
                self.next();
                Ok(())
            }
            Some(token) => Err(ParsingError::Expected(
                format!("{:?}", expected),
                format!("{:?}", token.token_type),
            )),
            None => Err(ParsingError::UnexpectedEof),
        }
    }

    pub fn optional(&mut self, expected: TokenType) -> Option<Token> {
        match self.peek(0) {
            Some(token) if token.token_type == expected => self.next(),
            _ => None
        }
    }
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse_program(&mut self) -> ParseResult<Program> {
        let mut buf = vec![];
        while self.peek(0).is_some() {
            if self.peek_type(0) == Some(&TokenType::EndExpr) {
                self.next();
                continue;
            }
            
            match self.parse_assign() {
                Ok(assign) => {
                    buf.push(assign);
                    if let Err(e) = self.expect(TokenType::EndExpr) {
                        if self.peek(0).is_some() {
                            return Err(e);
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }

        Ok(Program { body: buf })
    }

    pub fn parse_assign(&mut self) -> ParseResult<Assign> {
        let id = match self.peek_type(0) {
            Some(TokenType::Ident(a)) => a.clone(),
            Some(TokenType::EndExpr) => {
                self.next();
                return self.parse_assign();
            }
            _ => {
                return Err(ParsingError::Expected(
                    "any identifier".to_string(),
                    format!("{:?}", self.peek(0).map(|t| &t.token_type)),
                ))
            }
        };
        self.next();

        self.expect(TokenType::Assign)
            .map_err(|_| ParsingError::InvalidAssignment)?;

        let expr = self.parse_expr_pratt(0.)?;

        Ok(Assign(id, expr))
    }

    pub fn parse_lambda(&mut self) -> ParseResult<Expression> {
        self.optional(TokenType::Backslash);
        let arg = match self.peek_type(0) {
            Some(TokenType::Ident(x)) => x.clone(),
            _ => return Err(ParsingError::InvalidExpression(format!(
                    "Unexpected token: {:?}",
                    self.peek_type(0)
                )))        
        }; self.next();

        self.expect(TokenType::Dot);
        let body = self.parse_expr_pratt(0.)?;
        
        Ok(Expression::Lambda(arg.to_string(), Box::new(body)))
    }

    pub fn parse_expr_pratt(&mut self, min_bp: f32) -> ParseResult<Expression> {
        let mut lhs = match self.next() {
            Some(Token {
                token_type: TokenType::Number(n),
                ..
            }) => Expression::Num(n),
            Some(Token {
                token_type: TokenType::Ident(i),
                ..
            }) => Expression::Var(i),

            Some(Token {
                token_type: TokenType::Backslash,
                ..
            }) => self.parse_lambda()?,

            Some(Token {
                token_type: TokenType::LParen,
                ..
            }) => {
                let expr = self.parse_expr_pratt(0.)?;
                match self.next() {
                    Some(Token {
                        token_type: TokenType::RParen,
                        ..
                    }) => Expression::Parenthed(Box::new(expr)),
                    other => {
                        return Err(ParsingError::Expected(
                            ")".to_string(),
                            format!("{:?}", other.map(|t| t.token_type)),
                        ))
                    }
                }
            }

            Some(Token {
                token_type: TokenType::Op(op),
                ..
            }) if is_valid_unary(op.as_str()) => {
                let (_, bp_r) = unary_binding_power(&op);
                let rhs = self.parse_expr_pratt(bp_r)?;
                Expression::Operation(op.clone(), vec![rhs])
            }
            Some(token) => {
                return Err(ParsingError::InvalidExpression(format!(
                    "Unexpected token: {:?}",
                    token.token_type
                )))
            }
            None => return Err(ParsingError::UnexpectedEof),
        };

        loop {
            let op = match self.peek_type(0) {
                None | Some(TokenType::EndExpr) | Some(TokenType::RParen) => break,
                Some(TokenType::Op(op)) => op.clone(),
                Some(t) => {
                    "<APPLY>".to_string()
                }
            };
            
            let (bp_l, bp_r) = binding_power(op.as_str());
            if bp_l < min_bp {
                break;
            }
            
            if op == "<APPLY>" {
                let rhs = self.parse_expr_pratt(bp_r)?;
                lhs = Expression::Application(Box::new(lhs), Box::new(rhs));
            } else {
                self.next();
                let rhs = self.parse_expr_pratt(bp_r)?;
                lhs = Expression::Operation(op.to_owned(), vec![lhs, rhs]);
            }
        }

        Ok(lhs)
    }
}
