use crate::{error::ParsingError, lexer::*};
use im::{HashMap, hashmap};
use ordered_float::NotNan;
use std::fmt::Display;
use std::str::FromStr;
use strum_macros::{EnumString, EnumIter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, EnumIter, strum_macros::Display)]
pub enum NumericType {
    #[strum(to_string = "f8")] F8,
    #[strum(to_string = "f16")] F16,
    #[strum(to_string = "f32")] F32,
    #[strum(to_string = "f64")] F64,
    #[strum(to_string = "f128")] F128,
    #[strum(to_string = "f256")] F256,
    #[strum(to_string = "i8")] I8,
    #[strum(to_string = "i16")] I16,
    #[strum(to_string = "i32")] I32,
    #[strum(to_string = "i64")] I64,
    #[strum(to_string = "i128")] I128,
    #[strum(to_string = "i256")] I256,
    #[strum(to_string = "u8")] U8,
    #[strum(to_string = "u16")] U16,
    #[strum(to_string = "u32")] U32,
    #[strum(to_string = "u64")] U64,
    #[strum(to_string = "u128")] U128,
    #[strum(to_string = "u256")] U256,
}
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
    StrictNum(NumericType, String),
    Str(String),
    Bool(bool),
    Nil,
    Frozen(Box<Expression>),
    Environment(Vec<Assign>),
    Lambda(
        Box<Expression>, /* param */
        Box<Expression>, /* body */
        Vec<String>,     /* resources */
    ),
    Application(Box<Expression>, Box<Expression>),
    Operation(String, Vec<Expression>),
    ListType(Option<Box<Expression>>), // [string | number]
    TypeAlias(String, Box<Expression>),
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
            Self::StrictNum(t, v) => write!(f, "{v}{t}"),
            Self::Str(s) => write!(f, "\"{s}\""),
            Self::Lambda(arg, body, res) => {
                if res.is_empty() {
                    write!(f, "\\{arg}. {body}")
                } else {
                    write!(f, "\\{arg} @{{")?;
                    for (i, r) in res.iter().enumerate() {
                        if i > 0 { write!(f, ", ")?; }
                        write!(f, "{}", r)?;
                    }
                    write!(f, "}}. {body}")
                }
            }
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
            Self::TypeAlias(name, expr) => write!(f, "type {name} = {expr}"),
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

            OpSig::Infix("~".into())  => Operator::new(Assoc::NonAssoc, 9.),

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

        if self.peek_type(0) == Some(&TokenType::Keyword(Keyword::Type)) {
            self.next();
            let name = match self.next() {
                Some(Token { token_type: TokenType::Ident(n), .. }) => n,
                _ => return Err(ParsingError::InvalidAssignment),
            };
            self.expect(TokenType::Op("=".into()))?;
            let expr = self.parse_expr_pratt(0.0)?;
            return Ok(Assign(
                Expression::Var(vec!["__TYPE__".into(), name]),
                vec![],
                expr
            ));
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
            Keyword::Type => Err(ParsingError::InvalidExpression("Keyword 'type' cannot be used as an expression".into())),
            Keyword::Let => self.parse_let(),
            Keyword::If => self.parse_if(),
            Keyword::Then => Err(ParsingError::InvalidExpression("Unexpected 'then' keyword (should only appear after 'if')".into())),
            Keyword::Else => Err(ParsingError::InvalidExpression("Unexpected 'else' keyword (should only appear after 'then' in an 'if' expression)".into())),
            Keyword::In => Err(ParsingError::InvalidExpression("Unexpected 'in' keyword (should only appear in a 'let' expression)".into())),
        }
    }

    /// Parse: let <assigns> in <expr>
    /// Desugar to: {<assigns>}::'(<expr>)
    pub fn parse_let(&mut self) -> ParseResult<Expression> {
        // Parse assignments until we hit 'in'
        let mut assigns = vec![];
        
        loop {
            // Check if we hit 'in'
            if self.peek_type(0) == Some(&TokenType::Keyword(Keyword::In)) {
                self.next();
                break;
            }

            // Parse assignment
            let id = self.parse_pattern()?;
            let resources = self.parse_resource_importation()?;

            if self.peek_type(0) == Some(&TokenType::Op("=".into())) {
                self.next();
                let expr = self.parse_expr_pratt(0.)?;
                assigns.push(Assign(id, resources, expr));
            } else {
                assigns.push(Assign(id, resources, Expression::Nil));
            }

            self.expect(TokenType::EndExpr)?;
        }

        // Parse the body expression
        let body_expr = self.parse_expr_pratt(0.)?;

        // Desugar to: {assigns}::'(body_expr)
        let env = Expression::Environment(assigns);
        let frozen_body = Expression::Frozen(Box::new(body_expr));
        Ok(Expression::Operation("::".into(), vec![env, frozen_body]))
    }

    /// Parse: if <cond> then <expr1> else <expr2>
    /// Desugar to: eval (<cond> ? '(<expr1>), '(<expr2>))
    pub fn parse_if(&mut self) -> ParseResult<Expression> {
        // Parse condition
        let cond = self.parse_expr_pratt(0.)?;

        // Expect 'then'
        if self.peek_type(0) != Some(&TokenType::Keyword(Keyword::Then)) {
            return Err(ParsingError::Expected(
                "'then' keyword".into(),
                format!("{:?}", self.peek_type(0)),
            ));
        }
        self.next();

        // Parse then expression
        let then_expr = self.parse_expr_pratt(0.)?;

        // Expect 'else'
        if self.peek_type(0) != Some(&TokenType::Keyword(Keyword::Else)) {
            return Err(ParsingError::Expected(
                "'else' keyword".into(),
                format!("{:?}", self.peek_type(0)),
            ));
        }
        self.next();

        // Parse else expression
        let else_expr = self.parse_expr_pratt(0.)?;

        // Desugar to: eval (cond ? '(then_expr), '(else_expr))
        let frozen_then = Expression::Frozen(Box::new(then_expr));
        let frozen_else = Expression::Frozen(Box::new(else_expr));
        let choice_pair = Expression::Operation(",".into(), vec![frozen_then, frozen_else]);
        let cond_choice = Expression::Operation("?".into(), vec![cond, choice_pair]);
        
        Ok(Expression::Application(
            Box::new(Expression::Var(vec!["eval".into()])),
            Box::new(cond_choice),
        ))
    }

    pub fn parse_pattern(&mut self) -> ParseResult<Expression> {
        self.parse_expr_pratt(0.0)
    }

    pub fn parse_lambda(&mut self) -> ParseResult<Expression> {
        let old_flag = self.dot_is_separator;
        self.dot_is_separator = true;
        let param = self.parse_expr_pratt(0.);
        self.dot_is_separator = old_flag;
        let param = param?;
        let resources = self.parse_resource_importation()?;

        self.expect(TokenType::Dot)?;
        let body = self.parse_expr_pratt(0.)?;

        Ok(Expression::Lambda(Box::new(param), Box::new(body), resources))
    }

    pub fn parse_env(&mut self) -> ParseResult<Expression> {
        let mut body = vec![];
        while self.peek_type(0) != Some(&TokenType::RBrace) {
            let id = self.parse_pattern()?;
            let resources = self.parse_resource_importation()?;

            if self.peek_type(0) == Some(&TokenType::Op("=".into())) {
                self.next();
                let expr = self.parse_expr_pratt(0.)?;
                body.push(Assign(id, resources, expr));
            } else {
                body.push(Assign(id, resources, Expression::Nil));
            }

            self.expect(TokenType::EndExpr)?;
        }
        self.next();
        Ok(Expression::Environment(body))
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

                Expression::Lambda(Box::new(params), Box::new(Expression::Nil), vec![])
            }
            OpSig::Prefix(_) => {
                self.next();
                let param = self.parse_pattern()?;

                Expression::Lambda(Box::new(param), Box::new(Expression::Nil), vec![])
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
            Expression::Lambda(param, _, _) => Expression::Lambda(param, Box::new(body), vec![]),
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
                token_type: TokenType::StrictNumber(n, s),
                ..
            }) => {
                let t = NumericType::from_str(&s)
                    .map_err(|_| ParsingError::InvalidExpression(format!("Invalid numeric suffix: {s}")))?;
                Ok(Expression::StrictNum(t, n))
            }

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
                | Some(TokenType::Backslash)
                | Some(TokenType::Keyword(Keyword::Then))
                | Some(TokenType::Keyword(Keyword::Else))
                | Some(TokenType::Keyword(Keyword::In)) => break,

                Some(TokenType::Op(op)) if op == "=" || op == "@" => break,

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
