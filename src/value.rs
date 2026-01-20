use crate::{ast::*, eval::*};
use std::{any::Any, rc::Rc, fmt::Debug};
use std::cmp::Ordering;
use ordered_float::NotNan;


#[derive(Debug, Default, Clone)]
pub struct Ambient {
    pub vars: Environment,
    pub rsrcs: Environment,
    pub natives: Vec<Rc<dyn Any>>,
}

impl Ambient {
    pub fn extend(&mut self, other: &Ambient) {
        self.vars.extend(other.vars.clone());
        self.rsrcs.extend(other.rsrcs.clone());
        self.natives.extend(other.natives.clone());
    }

    pub fn eject(&mut self, other: &Ambient) {
        for k in other.vars.keys() {
            self.vars.remove(k);
        }
        for k in other.rsrcs.keys() {
            self.rsrcs.remove(k);
        }
    }

    pub fn eject_vars(&mut self, vars: &Environment) {
        for k in vars.keys() {
            self.vars.remove(k);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    Num(NotNan<f64>),
    Str(String),
    Lambda(Pattern, Expression, Environment),
    Environment(Environment),
    Frozen(Expression),
    Pattern(Pattern),
    Builtin(String),
    Bool(bool),
    Pair(Box<Value>, Box<Value>),
    Native(usize),
    Nil,
}

impl Value {
    pub fn pair_to_vec(&self) -> Vec<Value> {
        let mut result = Vec::new();
        let mut current = self;

        loop {
            match current {
                Value::Pair(car, cdr) => {
                    result.push((**car).clone());
                    current = cdr;
                }
                other => {
                    result.push(other.clone());
                    break;
                }
            }
        }

        result
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum Pattern {
    Name(String),
    TypedName(String, PatternType),
    Value(Box<Value>),
    Pair(Box<Pattern>, Box<Pattern>),
    Wildcard,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum PatternType {
    Nil,
    Pattern,
    Number,
    String,
    Lambda,
    Bool,
    Environment,
    Frozen,
    List(Vec<PatternType>),
}

impl std::fmt::Display for PatternType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatternType::Nil => write!(f, "nil"),
            PatternType::Number => write!(f, "number"),
            PatternType::String => write!(f, "string"),
            PatternType::Bool => write!(f, "bool"),
            PatternType::Lambda => write!(f, "fn"),
            PatternType::Pattern => write!(f, "pattern"),
            PatternType::Environment => write!(f, "env"),
            PatternType::Frozen => write!(f, "frozen"),
            PatternType::List(items) => {
                let joined = items
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(" | ");
                write!(f, "[{}]", joined)
            }
        }
    }
}
impl std::fmt::Display for Pattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(x)         => write!(f, "{x}"),
            Self::TypedName(x, t) => write!(f, "{x} ~ {t}"),
            Self::Value(x)        => write!(f, "{}", *x),
            Self::Pair(a, b)      => write!(f, "({}, {})", *a, *b),
            Self::Wildcard        => write!(f, "_"),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Num(x) => write!(f, "{x}"),
            Self::Str(s) => write!(f, "\"{s}\""),
            Self::Lambda(arg, body, _) => write!(f, "(\\{arg}. {body})"),
            Self::Builtin(b) => write!(f, "{b}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Pattern(p) => write!(f, "#{p}"),
            Self::Pair(a, b) => write!(f, "({}, {})", *a, *b),
            Self::Frozen(e) => write!(f, "'{e}"),
            Self::Native(a) => write!(f, "<#{a:02}>"),
            Self::Environment(e) => {
                write!(f, "{{")?;
                for (k, v) in e {
                    write!(f, " {k} =")?;
                    write!(f, " {v}; ")?;
                }
                write!(f, "}}")
            }
            Self::Nil => write!(f, "nil")
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Value::Num(a), Value::Num(b)) => a.partial_cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.partial_cmp(b),
            (Value::Nil, Value::Nil) => Some(Ordering::Equal),
            _ => None,
        }
    }
}

impl std::ops::Add for Value {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Num(a), Value::Num(b)) => Value::Num(a + b),
            (Value::Str(a), Value::Str(b)) => Value::Str(format!("{a}{b}")),
            (Value::Str(s), other) => Value::Str(format!("{s}{other}")),
            (other, Value::Str(s)) => Value::Str(format!("{other}{s}")),
            _ => Value::Nil,
        }
    }
}
impl std::ops::Neg for Value {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Value::Num(a) => Value::Num(-a),
            _ => Value::Nil,
        }
    }
}

impl std::ops::Not for Value {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Value::Bool(b) => Value::Bool(!b),
            _ => Value::Nil,
        }
    }
}

impl std::ops::Sub for Value {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Num(a), Value::Num(b)) => Value::Num(a - b),
            _ => Value::Nil,
        }
    }
}

impl std::ops::Mul for Value {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Num(a), Value::Num(b)) => Value::Num(a * b),
            _ => Value::Nil,
        }
    }
}


impl std::ops::Div for Value {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Num(a), Value::Num(b)) => Value::Num(a / b),
            _ => Value::Nil,
        }
    }
}
