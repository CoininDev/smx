use crate::error::{ParsingError, ParsingErrorType};
use crate::lexer::{Keyword, Span, Token, TokenType};
use crate::value::{Ambient, Assoc, OpSig};
use im::{HashMap, hashmap};
use ordered_float::NotNan;
use std::fmt::Display;
use std::str::FromStr;
use strum_macros::{EnumIter, EnumString};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, EnumIter, strum_macros::Display)]
pub enum NumericType {
    #[strum(to_string = "f8")]
    F8,
    #[strum(to_string = "f16")]
    F16,
    #[strum(to_string = "f32")]
    F32,
    #[strum(to_string = "f64")]
    F64,
    #[strum(to_string = "f128")]
    F128,
    #[strum(to_string = "f256")]
    F256,
    #[strum(to_string = "i8")]
    I8,
    #[strum(to_string = "i16")]
    I16,
    #[strum(to_string = "i32")]
    I32,
    #[strum(to_string = "i64")]
    I64,
    #[strum(to_string = "i128")]
    I128,
    #[strum(to_string = "i256")]
    I256,
    #[strum(to_string = "u8")]
    U8,
    #[strum(to_string = "u16")]
    U16,
    #[strum(to_string = "u32")]
    U32,
    #[strum(to_string = "u64")]
    U64,
    #[strum(to_string = "u128")]
    U128,
    #[strum(to_string = "u256")]
    U256,
}
#[derive(Debug, Clone)]
pub struct Program {
    pub body: Vec<Assign>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExprKind {
    Var(Vec<String>),
    OpSigVar(OpSig, Assoc, NotNan<f64>),
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Expression {
    pub kind: ExprKind,
    pub span: crate::lexer::Span,
}

impl Expression {
    pub fn new(kind: ExprKind, span: crate::lexer::Span) -> Self {
        Self { kind, span }
    }

    pub fn dummy(kind: ExprKind) -> Self {
        Self {
            kind,
            span: crate::lexer::Span {
                start: 0,
                end: 0,
                line: 0,
                col: 0,
            },
        }
    }
}

impl Display for ExprKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Var(v) => {
                write!(f, "{}", v[0])?;
                for s in &v[1..] {
                    write!(f, ".{s}")?;
                }
                write!(f, "")
            }
            Self::OpSigVar(OpSig::Infix(x), _, _) => write!(f, "{x}"),
            Self::OpSigVar(OpSig::Prefix(x), _, _) => write!(f, "{x}"),
            Self::Num(i) => write!(f, "{i}"),
            Self::StrictNum(t, v) => write!(f, "{v}{t}"),
            Self::Str(s) => write!(f, "\"{s}\""),
            Self::Lambda(arg, body, res) => {
                if res.is_empty() {
                    write!(f, "\\{arg}. {body}")
                } else {
                    write!(f, "\\{arg} @{{")?;
                    for (i, r) in res.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
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

impl Display for Expression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Assign(pub Expression, pub Vec<String>, pub Expression);

#[derive(Debug, Clone)]
pub struct Operator {
    pub meaning: Option<Expression>,
    pub prec: f32,
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

    pub fn with_ambient(tokens: Vec<Token>, amb: &Ambient) -> Self {
        let mut p = Self::new(tokens);
        for (sig, (assoc, prec)) in &amb.op_table {
            p.op_table
                .insert(sig.clone(), Operator::new(*assoc, prec.into_inner() as f32));
        }
        p
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

    pub fn peek_span(&self, p: usize) -> crate::lexer::Span {
        self.peek(p).map(|t| t.span).unwrap_or(crate::lexer::Span {
            start: 0,
            end: 0,
            line: 0,
            col: 0,
        })
    }

    pub fn last_span(&self) -> crate::lexer::Span {
        if self.pos == 0 || self.pos - 1 >= self.tokens.len() {
            crate::lexer::Span {
                start: 0,
                end: 0,
                line: 0,
                col: 0,
            }
        } else {
            self.tokens[self.pos - 1].span
        }
    }

    pub fn error(&self, errtype: ParsingErrorType) -> ParsingError {
        ParsingError::new(errtype, Some(self.peek_span(0)))
    }

    pub fn last_error(&self, errtype: ParsingErrorType) -> ParsingError {
        ParsingError::new(errtype, Some(self.last_span()))
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
            Some(token) => Err(ParsingError::new(
                ParsingErrorType::Expected(expected.to_string(), token.token_type.to_string()),
                Some(token.span),
            )),
            None => Err(ParsingError::new(
                ParsingErrorType::UnexpectedEof,
                Some(self.last_span()),
            )),
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
        let start_span = self.peek_span(0);
        if let Some(token) = self.peek(0) {
            match &token.token_type {
                TokenType::Ident(name) => {
                    if name == "op" {
                        self.next();
                        return self.parse_op();
                    }
                    if name == "resource" {
                        self.next();
                        return self.parse_resource();
                    }
                }
                TokenType::Keyword(Keyword::Type) => {
                    self.next();
                    let name = match self.peek_type(0) {
                        Some(TokenType::Ident(x)) => x.clone(),
                        _ => return Err(self.error(ParsingErrorType::InvalidAssignment)),
                    };
                    self.next();
                    self.expect(TokenType::Op("=".into()))?;
                    let expr = self.parse_expr_pratt(0.0)?;
                    return Ok(Assign(
                        Expression::new(ExprKind::Var(vec!["__TYPE__".into(), name]), start_span),
                        vec![],
                        expr,
                    ));
                }
                _ => {}
            }
        }

        let mut clone = self.clone();
        if let Ok(_) = clone.parse_pattern() {
            let _ = clone.parse_resource_importation();
            if clone.peek_type(0) == Some(&TokenType::Op("=".into())) {
                let id = self.parse_pattern()?;
                let resources = self.parse_resource_importation()?;
                self.expect(TokenType::Op("=".into()))?;
                let expr = self.parse_expr_pratt(0.)?;
                return Ok(Assign(id, resources, expr));
            }
        }

        Err(self.error(ParsingErrorType::InvalidAssignment))
    }

    pub fn parse_resource_importation(&mut self) -> ParseResult<Vec<String>> {
        match self.peek_type(0) {
            Some(TokenType::Op(x)) if x == "@" => {
                self.next();
                self.expect(TokenType::LBrace)?;

                let first = match self.peek_type(0) {
                    Some(TokenType::Ident(x)) => x.clone(),
                    _ => return Err(self.error(ParsingErrorType::InvalidAssignment)),
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
                        _ => return Err(self.error(ParsingErrorType::InvalidAssignment)),
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
        let start_span = self.peek_span(0);
        let name = match self.peek_type(0) {
            Some(TokenType::Ident(x)) => x.clone(),
            _ => return Err(self.error(ParsingErrorType::InvalidAssignment)),
        };
        self.next();

        let imports = self.parse_resource_importation()?;

        self.expect(TokenType::Op("=".into()))?;

        let val = self.parse_expr_pratt(0.)?;

        Ok(Assign(
            Expression::new(ExprKind::Var(vec!["__RESOURCE__".into(), name]), start_span),
            imports,
            val,
        ))
    }

    pub fn parse_var(&mut self, first: String, start_span: Span) -> ParseResult<Expression> {
        let mut buf = Vec::new();
        buf.push(first);
        if !self.dot_is_separator && self.peek_type(0) == Some(&TokenType::Dot) {
            self.next();
            loop {
                let ident = match self.peek_type(0) {
                    Some(TokenType::Ident(x)) => x.clone(),
                    Some(tok) => {
                        return Err(self.error(ParsingErrorType::Unexpected(tok.to_string())));
                    }
                    None => return Err(self.last_error(ParsingErrorType::UnexpectedEof)),
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
        Ok(Expression::new(
            ExprKind::Var(buf),
            start_span.merge(&self.last_span()),
        ))
    }

    pub fn parse_keyword(&mut self, k: Keyword) -> ParseResult<Expression> {
        let span = self.last_span();
        match k {
            Keyword::True => Ok(Expression::new(ExprKind::Bool(true), span)),
            Keyword::False => Ok(Expression::new(ExprKind::Bool(false), span)),
            Keyword::Nil => Ok(Expression::new(ExprKind::Nil, span)),
            Keyword::Type => Err(self.last_error(ParsingErrorType::InvalidExpression(
                "Keyword 'type' cannot be used as an expression".into(),
            ))),
            Keyword::Let => self.parse_let(),
            Keyword::If => self.parse_if(),
            Keyword::Then => Err(self.last_error(ParsingErrorType::InvalidExpression(
                "Unexpected 'then' keyword (should only appear after 'if')".into(),
            ))),
            Keyword::Else => Err(self.last_error(ParsingErrorType::InvalidExpression(
                "Unexpected 'else' keyword (should only appear after 'then' in an 'if' expression)"
                    .into(),
            ))),
            Keyword::In => Err(self.last_error(ParsingErrorType::InvalidExpression(
                "Unexpected 'in' keyword (should only appear in a 'let' expression)".into(),
            ))),
        }
    }

    /// Parse: let <assigns> in <expr>
    /// Desugar to: {<assigns>}::'(<expr>)
    pub fn parse_let(&mut self) -> ParseResult<Expression> {
        let start_span = self.last_span();
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
                assigns.push(Assign(
                    id,
                    resources,
                    Expression::new(ExprKind::Nil, self.last_span()),
                ));
            }

            self.expect(TokenType::EndExpr)?;
        }

        // Parse the body expression
        let body_expr = self.parse_expr_pratt(0.)?;

        // Desugar to: {assigns}::'(body_expr)
        let env = Expression::new(
            ExprKind::Environment(assigns),
            start_span.merge(&self.last_span()),
        );
        let frozen_body = Expression::new(
            ExprKind::Frozen(Box::new(body_expr)),
            start_span.merge(&self.last_span()),
        );
        Ok(Expression::new(
            ExprKind::Operation("::".into(), vec![env, frozen_body]),
            start_span.merge(&self.last_span()),
        ))
    }

    /// Parse: if <cond> then <expr1> else <expr2>
    /// Desugar to: eval (<cond> ? '(<expr1>), '(<expr2>))
    pub fn parse_if(&mut self) -> ParseResult<Expression> {
        let start_span = self.last_span();
        // Parse condition
        let cond = self.parse_expr_pratt(0.)?;

        // Expect 'then'
        if self.peek_type(0) != Some(&TokenType::Keyword(Keyword::Then)) {
            let other = self.peek(0);
            return Err(self.error(ParsingErrorType::Expected(
                "then".to_string(),
                format!("{:?}", other.map(|t| t.token_type.clone())),
            )));
        }
        self.next();

        // Parse then expression
        let then_expr = self.parse_expr_pratt(0.)?;

        // Expect 'else'
        if self.peek_type(0) != Some(&TokenType::Keyword(Keyword::Else)) {
            let other = self.peek(0);
            return Err(self.error(ParsingErrorType::Expected(
                "else".to_string(),
                format!("{:?}", other.map(|t| t.token_type.clone())),
            )));
        }
        self.next();

        // Parse else expression
        let else_expr = self.parse_expr_pratt(0.)?;

        // Desugar to: eval (cond ? '(then_expr), '(else_expr))
        let frozen_then = Expression::new(
            ExprKind::Frozen(Box::new(then_expr)),
            start_span.merge(&self.last_span()),
        );
        let frozen_else = Expression::new(
            ExprKind::Frozen(Box::new(else_expr)),
            start_span.merge(&self.last_span()),
        );
        let choice_pair = Expression::new(
            ExprKind::Operation(",".into(), vec![frozen_then, frozen_else]),
            start_span.merge(&self.last_span()),
        );
        let cond_choice = Expression::new(
            ExprKind::Operation("?".into(), vec![cond, choice_pair]),
            start_span.merge(&self.last_span()),
        );

        Ok(Expression::new(
            ExprKind::Application(
                Box::new(Expression::new(
                    ExprKind::Var(vec!["eval".into()]),
                    start_span.merge(&self.last_span()),
                )),
                Box::new(cond_choice),
            ),
            start_span.merge(&self.last_span()),
        ))
    }

    pub fn parse_pattern(&mut self) -> ParseResult<Expression> {
        self.parse_expr_pratt(0.0)
    }

    pub fn parse_lambda(&mut self) -> ParseResult<Expression> {
        let start_span = self.last_span();
        let old_flag = self.dot_is_separator;
        self.dot_is_separator = true;
        let param = self.parse_expr_pratt(0.);
        self.dot_is_separator = old_flag;
        let param = param?;
        let resources = self.parse_resource_importation()?;

        self.expect(TokenType::Dot)?;
        let body = self.parse_expr_pratt(0.)?;

        Ok(Expression::new(
            ExprKind::Lambda(Box::new(param), Box::new(body), resources),
            start_span.merge(&self.last_span()),
        ))
    }

    pub fn parse_env(&mut self) -> ParseResult<Expression> {
        let start_span = self.last_span();
        let mut body = vec![];
        while self.peek_type(0) != Some(&TokenType::RBrace) {
            let id = self.parse_pattern()?;
            let resources = self.parse_resource_importation()?;

            if self.peek_type(0) == Some(&TokenType::Op("=".into())) {
                self.next();
                let expr = self.parse_expr_pratt(0.)?;
                body.push(Assign(id, resources, expr));
            } else {
                body.push(Assign(
                    id,
                    resources,
                    Expression::new(ExprKind::Nil, self.last_span()),
                ));
            }

            self.expect(TokenType::EndExpr)?;
        }
        self.next();
        Ok(Expression::new(
            ExprKind::Environment(body),
            start_span.merge(&self.last_span()),
        ))
    }

    pub fn parse_op(&mut self) -> ParseResult<Assign> {
        let start_span = self.peek_span(0);
        let assoc = match self.peek_type(0) {
            Some(TokenType::Ident(a)) if a == "left" => Assoc::Left,
            Some(TokenType::Ident(a)) if a == "right" => Assoc::Right,
            Some(TokenType::Ident(a)) if a == "nonassoc" => Assoc::NonAssoc,
            Some(other) => {
                return Err(self.error(ParsingErrorType::Expected(
                    String::from("assoc (left | right | nonassoc)"),
                    other.to_string(),
                )));
            }
            None => return Err(self.last_error(ParsingErrorType::UnexpectedEof)),
        };
        self.next();

        let prec = match self.peek_type(0) {
            Some(TokenType::Number(n)) => *n,
            Some(other) => {
                return Err(self.error(ParsingErrorType::Expected(
                    String::from("some number"),
                    other.to_string(),
                )));
            }
            None => return Err(self.last_error(ParsingErrorType::UnexpectedEof)),
        };
        self.next();

        self.expect(TokenType::LParen)?;
        let op_sig = match self.peek_type(0) {
            //infix
            Some(TokenType::Ident(_)) => match self.peek_type(1) {
                Some(TokenType::Op(x)) => OpSig::Infix(x.clone()),
                Some(other) => {
                    return Err(self.error(ParsingErrorType::Expected(
                        String::from("some op declaration, example:\"(a +*+ b)\", \"($! a)\""),
                        other.to_string(),
                    )));
                }
                None => return Err(self.last_error(ParsingErrorType::UnexpectedEof)),
            },
            //prefix
            Some(TokenType::Op(x)) => OpSig::Prefix(x.clone()),
            Some(other) => {
                return Err(self.error(ParsingErrorType::Expected(
                    String::from("some op declaration, example:\"(a +*+ b)\", \"($! a)\""),
                    other.to_string(),
                )));
            }
            None => return Err(self.last_error(ParsingErrorType::UnexpectedEof)),
        };

        let res = match op_sig {
            OpSig::Infix(_) => {
                let param1 = self.parse_term()?;
                self.next();
                let param2 = self.parse_term()?;

                let p_span = param1.span.merge(&param2.span);
                let params = Expression::new(
                    ExprKind::Operation(",".into(), vec![param1, param2]),
                    p_span,
                );

                Expression::new(
                    ExprKind::Lambda(
                        Box::new(params),
                        Box::new(Expression::new(ExprKind::Nil, self.last_span())),
                        vec![],
                    ),
                    start_span,
                )
            }
            OpSig::Prefix(_) => {
                self.next();
                let param = self.parse_term()?;

                Expression::new(
                    ExprKind::Lambda(
                        Box::new(param),
                        Box::new(Expression::new(ExprKind::Nil, self.last_span())),
                        vec![],
                    ),
                    start_span,
                )
            }
        };

        self.expect(TokenType::RParen)?;
        self.expect(TokenType::Op("=".into()))?;

        let prec_f32 = prec as f32;
        let prec_notnan = NotNan::new(prec)
            .map_err(|e| self.error(ParsingErrorType::Unexpected(e.to_string())))?;

        let custom_op_table = hashmap! {op_sig.clone() => Operator::new(assoc.clone(), prec_f32)};
        let body = self.parse_expr_pratt_custom_op_table(custom_op_table, 0.)?;
        self.op_table.insert(
            op_sig.clone(),
            Operator {
                assoc,
                prec: prec_f32,
                meaning: Some(body.clone()),
            },
        );

        let res = match res.kind {
            ExprKind::Lambda(param, _, _) => Expression::new(
                ExprKind::Lambda(param, Box::new(body), vec![]),
                start_span.merge(&self.last_span()),
            ),
            _ => unreachable!(),
        };
        Ok(Assign(
            Expression::new(ExprKind::OpSigVar(op_sig, assoc, prec_notnan), start_span),
            vec![],
            res,
        ))
    }

    pub fn parse_term(&mut self) -> ParseResult<Expression> {
        let start_span = self.peek_span(0);
        let expr = match self.next() {
            Some(Token {
                token_type: TokenType::Number(n),
                ..
            }) => ExprKind::Num(
                NotNan::new(n)
                    .map_err(|e| self.error(ParsingErrorType::Unexpected(e.to_string())))?,
            ),

            Some(Token {
                token_type: TokenType::StrictNumber(n, s),
                ..
            }) => {
                let t = NumericType::from_str(&s).map_err(|_| {
                    self.last_error(ParsingErrorType::InvalidExpression(format!(
                        "Invalid numeric suffix: {s}"
                    )))
                })?;
                ExprKind::StrictNum(t, n)
            }

            Some(Token {
                token_type: TokenType::Ident(i),
                ..
            }) => return self.parse_var(i, start_span),

            Some(Token {
                token_type: TokenType::LBrace,
                ..
            }) => return self.parse_env(),

            Some(Token {
                token_type: TokenType::Apostrophe,
                ..
            }) => ExprKind::Frozen(Box::new(self.parse_term()?)),

            Some(Token {
                token_type: TokenType::Backslash,
                ..
            }) => return self.parse_lambda(),

            Some(Token {
                token_type: TokenType::Keyword(k),
                ..
            }) => return self.parse_keyword(k),

            Some(Token {
                token_type: TokenType::Str(s),
                ..
            }) => ExprKind::Str(s),

            Some(Token {
                token_type: TokenType::LParen,
                ..
            }) => {
                let old_flag = self.dot_is_separator;
                self.dot_is_separator = false;
                let mut expr = self.parse_expr_pratt(0.)?;
                self.dot_is_separator = old_flag;
                match self.next() {
                    Some(Token {
                        token_type: TokenType::RParen,
                        ..
                    }) => {
                        expr.span = start_span.merge(&self.last_span());
                        return Ok(expr);
                    }
                    other => {
                        return Err(self.error(ParsingErrorType::Expected(
                            ")".to_string(),
                            format!("{:?}", other.map(|t| t.token_type)),
                        )));
                    }
                }
            }

            Some(Token {
                token_type: TokenType::Op(op),
                ..
            }) if self.op_table.contains_key(&OpSig::Prefix(op.clone())) => {
                let (_, bp_r) = self.binding_power(OpSig::Prefix(op.clone()));
                let rhs = self.parse_expr_pratt(bp_r)?;
                ExprKind::Operation(op.clone(), vec![rhs])
            }

            Some(Token {
                token_type: TokenType::LBrack,
                ..
            }) => {
                if self.peek_type(0) == Some(&TokenType::RBrack) {
                    self.next();
                    return Ok(Expression::new(
                        ExprKind::ListType(None),
                        start_span.merge(&self.last_span()),
                    ));
                }
                let expr = self.parse_expr_pratt(0.)?;
                match self.next() {
                    Some(Token {
                        token_type: TokenType::RBrack,
                        ..
                    }) => ExprKind::ListType(Some(Box::new(expr))),
                    other => {
                        return Err(self.error(ParsingErrorType::Expected(
                            "]".to_string(),
                            format!("{:?}", other.map(|t| t.token_type)),
                        )));
                    }
                }
            }

            Some(token) => {
                return Err(self.last_error(ParsingErrorType::InvalidExpression(format!(
                    "Unexpected token on parse_term: {:?}",
                    token.token_type
                ))));
            }
            None => return Err(self.last_error(ParsingErrorType::UnexpectedEof)),
        };

        Ok(Expression::new(expr, start_span.merge(&self.last_span())))
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
                let start_span = lhs.span;
                lhs = Expression::new(
                    ExprKind::Application(Box::new(lhs), Box::new(rhs)),
                    start_span.merge(&self.last_span()),
                );
            } else {
                self.next();
                let rhs = self.parse_expr_pratt(bp_r)?;
                let start_span = lhs.span;
                lhs = Expression::new(
                    ExprKind::Operation(op.to_owned(), vec![lhs, rhs]),
                    start_span.merge(&self.last_span()),
                );
            }
        }

        Ok(lhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenize;

    #[test]
    fn test_expression_spans() {
        let code = "1 + 2 * 3";
        let tokens = tokenize(code).unwrap();
        let mut parser = Parser::new(tokens);
        let expr = parser.parse_expr_pratt(0.0).unwrap();

        // "1 + 2 * 3" should cover from index 0 to 9
        assert_eq!(expr.span.start, 0);
        assert_eq!(expr.span.end, 9);
    }

    #[test]
    fn test_nested_spans() {
        let code = "(1 + 2)";
        let tokens = tokenize(code).unwrap();
        let mut parser = Parser::new(tokens);
        let expr = parser.parse_expr_pratt(0.0).unwrap();

        assert_eq!(expr.span.start, 0);
        assert_eq!(expr.span.end, 7);
    }

    #[test]
    fn test_let_spans() {
        let code = "let x = 10; in x";
        let tokens = tokenize(code).unwrap();
        let mut parser = Parser::new(tokens);
        let expr = parser.parse_expr_pratt(0.0).unwrap();

        assert_eq!(expr.span.start, 0);
        assert_eq!(expr.span.end, 16);
    }
}
