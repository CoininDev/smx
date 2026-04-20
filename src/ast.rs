use crate::{error::ParsingError, lexer::*};
use im::{HashMap, hashmap};
use ordered_float::NotNan;
use std::fmt::Display;

#[derive(Debug, Clone)]
pub struct Program {
    pub body: Vec<Assign>,
}

fn mount_num(num: f64) -> ParseResult<NotNan<f64>> {
    NotNan::new(num).map_err(|e| ParsingError::NotNanError(e.to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expression {
    Var(Vec<String>),
    OpSigVar(OpSig),
    Num(NotNan<f64>),
    Str(String),
    Bool(bool),
    Nil,
    Frozen(Box<Expression>),
    Environment(Vec<Assign>),
    Lambda(
        Box<Expression>, /* param */
        Box<Expression>, /* body */
    ),
    Application(Box<Expression>, Box<Expression>),
    Operation(String, Vec<Expression>),
    ListType(Option<Box<Expression>>), // [string | number]
}

impl Display for Expression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Var(v) => {
                write!(f, "{}", v[0])?;
                for s in &v[1..] {
                    write!(f, ".{s}")?;
                }
                write!(f, "")
            }
            Self::OpSigVar(OpSig::Infix(x)) => write!(f, "{x}"),
            Self::OpSigVar(OpSig::Prefix(x)) => write!(f, "{x}"),
            Self::Num(i) => write!(f, "{i}"),
            Self::Str(s) => write!(f, "\"{s}\""),
            Self::Lambda(arg, body) => write!(f, "\\{arg}. {body}"),
            Self::Nil => write!(f, "nil"),
            Self::Frozen(x) => write!(f, "'({})", *x),
            Self::Application(a, b) => write!(f, "({a} {b})"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Operation(op, e) => {
                write!(f, "({op}")?;
                for expr in e {
                    write!(f, " {expr}")?;
                }
                write!(f, ")")
            }
            Self::Environment(e) => {
                write!(f, "{{")?;
                for a in e {
                    write!(f, " {} =", a.0)?;
                    write!(f, " {}; ", a.2)?;
                }
                write!(f, "}}")
            }
            Self::ListType(Some(x)) => write!(f, "[{}]", *x),
            Self::ListType(None) => write!(f, "[]"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Assign(pub Expression, pub Vec<String>, pub Expression);

#[derive(Debug, Clone)]
pub struct Operator {
    pub meaning: Option<Expression>,
    prec: f32,
    pub assoc: Assoc,
}

impl Operator {
    pub fn new(assoc: Assoc, prec: f32) -> Self {
        Self {
            prec,
            assoc,
            meaning: None,
        }
    }

    pub fn prec_pair(&self) -> (f32, f32) {
        match self.assoc {
            Assoc::Left => (self.prec, self.prec + 0.1),
            Assoc::Right => (self.prec + 0.1, self.prec),
            Assoc::NonAssoc => (self.prec, self.prec),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Assoc {
    Left,
    Right,
    NonAssoc,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum OpSig {
    Infix(String),
    Prefix(String),
}

#[derive(Debug, Clone)]
pub struct Parser {
    tokens: Vec<Token>,
    pub op_table: HashMap<OpSig, Operator>,
    pub pos: usize,
    pub dot_is_separator: bool,
}

pub type ParseResult<T> = Result<T, ParsingError>;

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        let op_table = hashmap! {
            OpSig::Infix("<APPLY>".into()) => Operator::new(Assoc::Left, 100.0),

            OpSig::Prefix("#".into()) => Operator::new(Assoc::NonAssoc, 10.),
            OpSig::Prefix("+".into()) => Operator::new(Assoc::NonAssoc, 10.),
            OpSig::Prefix("-".into()) => Operator::new(Assoc::NonAssoc, 10.),
            OpSig::Prefix("!".into()) => Operator::new(Assoc::NonAssoc, 10.),



            OpSig::Infix("**".into()) => Operator::new(Assoc::Right, 8.),
            OpSig::Infix("*".into())  => Operator::new(Assoc::Left, 7.),
            OpSig::Infix("/".into())  => Operator::new(Assoc::Left, 7.),
            OpSig::Infix("%".into())  => Operator::new(Assoc::Left, 7.),
            OpSig::Infix("+".into())  => Operator::new(Assoc::Left, 6.),
            OpSig::Infix("-".into())  => Operator::new(Assoc::Left, 6.),

            OpSig::Infix("<".into())  => Operator::new(Assoc::Left, 5.),
            OpSig::Infix(">".into())  => Operator::new(Assoc::Left, 5.),
            OpSig::Infix("==".into()) => Operator::new(Assoc::Left, 5.),
            OpSig::Infix("!=".into()) => Operator::new(Assoc::Left, 5.),
            OpSig::Infix("<=".into()) => Operator::new(Assoc::Left, 5.),
            OpSig::Infix(">=".into()) => Operator::new(Assoc::Left, 5.),

            OpSig::Infix("||".into()) => Operator::new(Assoc::Left, 4.),
            OpSig::Infix("&&".into()) => Operator::new(Assoc::Left, 4.),

            OpSig::Infix(",".into())  => Operator::new(Assoc::Right, 2.5),
            OpSig::Infix("?".into())  => Operator::new(Assoc::Right, 2.5),
            OpSig::Infix(":".into())  => Operator::new(Assoc::Left, 2.),
            OpSig::Infix("::".into()) => Operator::new(Assoc::Left, 2.),

            //only used in types
            OpSig::Infix("|".into()) => Operator::new(Assoc::Right, 3.5),

        };
        Self {
            tokens,
            pos: 0,
            op_table,
            dot_is_separator: false,
        }
    }

    pub fn binding_power(&self, op: OpSig) -> (f32, f32) {
        match self.op_table.get(&op) {
            Some(a) => a.prec_pair(),
            _ => Operator::new(Assoc::Left, 1.).prec_pair(),
        }
    }

    pub fn peek(&self, p: usize) -> Option<&Token> {
        self.tokens.get(self.pos + p)
    }

    pub fn reset(&mut self) {
        self.pos = 0;
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
}

impl Parser {
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
        if self.peek_type(0) == Some(&TokenType::Ident("op".into())) {
            self.next();
            return self.parse_op();
        }

        if self.peek_type(0) == Some(&TokenType::Ident("resource".into())) {
            self.next();
            return self.parse_resource();
        }

        let id = self.parse_pattern()?;

        let resources = self.parse_resource_importation()?;

        self.expect(TokenType::Op("=".into()))
            .map_err(|_| ParsingError::InvalidAssignment)?;

        let expr = self.parse_expr_pratt(0.)?;

        Ok(Assign(id, resources, expr))
    }

    pub fn parse_resource_importation(&mut self) -> ParseResult<Vec<String>> {
        match self.peek_type(0) {
            Some(TokenType::Op(x)) if x == "@" => {
                self.next();
                self.expect(TokenType::LBrace)?;

                let first = match self.peek_type(0) {
                    Some(TokenType::Ident(x)) => x.clone(),
                    _ => return Err(ParsingError::InvalidAssignment),
                };
                self.next();

                let mut buf = vec![first];
                loop {
                    match self.peek_type(0) {
                        Some(TokenType::Op(x)) if x == "," => {}
                        _ => break,
                    };
                    self.next();

                    let more = match self.peek_type(0) {
                        Some(TokenType::Ident(x)) => x.clone(),
                        _ => return Err(ParsingError::InvalidAssignment),
                    };
                    self.next();

                    buf.push(more);
                }

                self.expect(TokenType::RBrace)?;
                Ok(buf)
            }
            _ => Ok(vec![]),
        }
    }

    pub fn parse_resource(&mut self) -> ParseResult<Assign> {
        let name = match self.peek_type(0) {
            Some(TokenType::Ident(x)) => x.clone(),
            _ => return Err(ParsingError::InvalidAssignment),
        };
        self.next();

        let imports = self.parse_resource_importation()?;

        self.expect(TokenType::Op("=".into()))?;

        let val = self.parse_expr_pratt(0.)?;

        Ok(Assign(
            Expression::Var(vec!["__RESOURCE__".into(), name]),
            imports,
            val,
        ))
    }

    pub fn parse_var(&mut self, first: String) -> ParseResult<Expression> {
        let mut buf = Vec::new();
        buf.push(first);
        if !self.dot_is_separator && self.peek_type(0) == Some(&TokenType::Dot) {
            self.next();
            loop {
                let ident = match self.peek_type(0) {
                    Some(TokenType::Ident(x)) => x.clone(),
                    Some(tok) => return Err(ParsingError::Unexpected(tok.to_string())),
                    None => return Err(ParsingError::UnexpectedEof),
                };

                self.next();
                buf.push(ident);

                match self.peek_type(0) {
                    Some(TokenType::Dot) => {
                        self.next();
                        continue;
                    }
                    _ => break,
                }
            }
        }
        Ok(Expression::Var(buf))
    }

    pub fn parse_keyword(&mut self, k: Keyword) -> ParseResult<Expression> {
        match k {
            Keyword::True => Ok(Expression::Bool(true)),
            Keyword::False => Ok(Expression::Bool(false)),
            Keyword::Nil => Ok(Expression::Nil),
        }
    }

    pub fn parse_pattern(&mut self) -> ParseResult<Expression> {
        match self.peek_type(0) {
            Some(TokenType::Ident(x)) => {
                let cu = Ok(Expression::Var(vec![x.to_string()]));
                self.next();
                cu
            }
            Some(TokenType::LParen) => {
                self.next();
                let a = self.parse_expr_pratt(0.)?;
                self.expect(TokenType::RParen)?;
                Ok(a)
            }
            Some(x) => Err(ParsingError::Expected("pattern".to_string(), x.to_string())),
            None => Err(ParsingError::UnexpectedEof),
        }
    }

    pub fn parse_lambda(&mut self) -> ParseResult<Expression> {
        let old_flag = self.dot_is_separator;
        self.dot_is_separator = true;
        let param = self.parse_expr_pratt(0.);
        self.dot_is_separator = old_flag;
        let param = param?;

        self.expect(TokenType::Dot)?;
        let body = self.parse_expr_pratt(0.)?;

        Ok(Expression::Lambda(Box::new(param), Box::new(body)))
    }

    pub fn parse_env(&mut self) -> ParseResult<Expression> {
        let mut env = vec![];

        while self.peek_type(0) != Some(&TokenType::RBrace) {
            let ass = self.parse_assign()?;
            env.push(ass);
            self.expect(TokenType::EndExpr)?;
        }

        self.expect(TokenType::RBrace)?;

        Ok(Expression::Environment(env))
    }

    pub fn parse_op(&mut self) -> ParseResult<Assign> {
        let assoc = match self.peek_type(0) {
            Some(TokenType::Ident(a)) if a == "left" => Assoc::Left,
            Some(TokenType::Ident(a)) if a == "right" => Assoc::Right,
            Some(TokenType::Ident(a)) if a == "nonassoc" => Assoc::NonAssoc,
            Some(other) => {
                return Err(ParsingError::Expected(
                    String::from("op assoc (left, right or nonassoc)"),
                    other.to_string(),
                ));
            }
            None => return Err(ParsingError::UnexpectedEof),
        };
        self.next();

        let prec = match self.peek_type(0) {
            Some(TokenType::Number(n)) => *n as f32,
            Some(other) => {
                return Err(ParsingError::Expected(
                    String::from("some number"),
                    other.to_string(),
                ));
            }
            None => return Err(ParsingError::UnexpectedEof),
        };
        self.next();

        self.expect(TokenType::LParen)?;
        let op_sig = match self.peek_type(0) {
            //infix
            Some(TokenType::Ident(_)) => match self.peek_type(1) {
                Some(TokenType::Op(x)) => OpSig::Infix(x.clone()),
                Some(other) => {
                    return Err(ParsingError::Expected(
                        String::from("some op declaration, example:\"(a +*+ b)\", \"($! a)\""),
                        other.to_string(),
                    ));
                }
                None => return Err(ParsingError::UnexpectedEof),
            },
            //prefix
            Some(TokenType::Op(x)) => OpSig::Prefix(x.clone()),
            Some(other) => {
                return Err(ParsingError::Expected(
                    String::from("some op declaration, example:\"(a +*+ b)\", \"($! a)\""),
                    other.to_string(),
                ));
            }
            None => return Err(ParsingError::UnexpectedEof),
        };

        let res = match op_sig {
            OpSig::Infix(_) => {
                let param1 = self.parse_pattern()?;
                self.next();
                let param2 = self.parse_pattern()?;

                let params = Expression::Operation(",".into(), vec![param1, param2]);

                Expression::Lambda(Box::new(params), Box::new(Expression::Nil))
            }
            OpSig::Prefix(_) => {
                self.next();
                let param = self.parse_pattern()?;

                Expression::Lambda(Box::new(param), Box::new(Expression::Nil))
            }
        };

        self.expect(TokenType::RParen)?;
        self.expect(TokenType::Op("=".into()))?;

        let custom_op_table = hashmap! {op_sig.clone() => Operator::new(assoc.clone(), prec)};
        let body = self.parse_expr_pratt_custom_op_table(custom_op_table, 0.)?;
        self.op_table.insert(
            op_sig.clone(),
            Operator {
                assoc,
                prec,
                meaning: Some(body.clone()),
            },
        );

        let res = match res {
            Expression::Lambda(param, _) => Expression::Lambda(param, Box::new(body)),
            _ => unreachable!(),
        };
        Ok(Assign(Expression::OpSigVar(op_sig), vec![], res))
    }

    pub fn parse_term(&mut self) -> ParseResult<Expression> {
        match self.next() {
            Some(Token {
                token_type: TokenType::Number(n),
                ..
            }) => Ok(Expression::Num(mount_num(n)?)),

            Some(Token {
                token_type: TokenType::Ident(i),
                ..
            }) => self.parse_var(i),

            Some(Token {
                token_type: TokenType::LBrace,
                ..
            }) => self.parse_env(),

            Some(Token {
                token_type: TokenType::Apostrophe,
                ..
            }) => Ok(Expression::Frozen(Box::new(self.parse_term()?))),

            Some(Token {
                token_type: TokenType::Backslash,
                ..
            }) => self.parse_lambda(),

            Some(Token {
                token_type: TokenType::Keyword(k),
                ..
            }) => self.parse_keyword(k),

            Some(Token {
                token_type: TokenType::Str(s),
                ..
            }) => Ok(Expression::Str(s)),

            Some(Token {
                token_type: TokenType::LParen,
                ..
            }) => {
                let old_flag = self.dot_is_separator;
                self.dot_is_separator = false;
                let expr = self.parse_expr_pratt(0.);
                self.dot_is_separator = old_flag;
                let expr = expr?;
                match self.next() {
                    Some(Token {
                        token_type: TokenType::RParen,
                        ..
                    }) => Ok(expr),
                    other => Err(ParsingError::Expected(
                        ")".to_string(),
                        format!("{:?}", other.map(|t| t.token_type)),
                    )),
                }
            }

            Some(Token {
                token_type: TokenType::Op(op),
                ..
            }) if self.op_table.contains_key(&OpSig::Prefix(op.clone())) => {
                let (_, bp_r) = self.binding_power(OpSig::Prefix(op.clone()));
                let rhs = self.parse_expr_pratt(bp_r)?;
                Ok(Expression::Operation(op.clone(), vec![rhs]))
            }

            Some(Token {
                token_type: TokenType::LBrack,
                ..
            }) => {
                if self.peek_type(0) == Some(&TokenType::RBrack) {
                    self.next();
                    return Ok(Expression::ListType(None));
                }
                let expr = self.parse_expr_pratt(0.)?;
                match self.next() {
                    Some(Token {
                        token_type: TokenType::RBrack,
                        ..
                    }) => Ok(Expression::ListType(Some(Box::new(expr)))),
                    other => Err(ParsingError::Expected(
                        "]".to_string(),
                        format!("{:?}", other.map(|t| t.token_type)),
                    )),
                }
            }

            Some(token) => Err(ParsingError::InvalidExpression(format!(
                "Unexpected token on parse_term: {:?}",
                token.token_type
            ))),
            None => return Err(ParsingError::UnexpectedEof),
        }
    }

    pub fn parse_expr_pratt_custom_op_table(
        &mut self,
        op_table: HashMap<OpSig, Operator>,
        min_bp: f32,
    ) -> ParseResult<Expression> {
        let mut my_parser = self.clone();
        my_parser.op_table.extend(op_table);
        let result = my_parser.parse_expr_pratt(min_bp)?;
        self.pos = my_parser.pos;
        Ok(result)
    }

    pub fn parse_expr_pratt(&mut self, min_bp: f32) -> ParseResult<Expression> {
        let mut lhs = self.parse_term()?;
        loop {
            let op = match self.peek_type(0) {
                None
                | Some(TokenType::EndExpr)
                | Some(TokenType::RParen)
                | Some(TokenType::RBrack)
                | Some(TokenType::RBrace)
                | Some(TokenType::Dot)
                | Some(TokenType::DebugDot)
                | Some(TokenType::Backslash) => break,

                Some(TokenType::Op(op)) if op == "=" => break,

                Some(TokenType::Op(op)) => op.clone(),
                Some(_) => "<APPLY>".to_string(),
            };

            let (bp_l, bp_r) = self.binding_power(OpSig::Infix(op.clone()));
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
