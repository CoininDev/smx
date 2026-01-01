use crate::{ast::*, eval::*};
use std::cmp::Ordering;



#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(f64),
    Lambda(Pattern, Expression, Environment),
    Bool(bool),
    Pair(Box<Value>, Box<Value>),
    Nil,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum Pattern {
    Name(String),
    Pair(Box<Pattern>, Box<Pattern>),
    Wildcard,
}

impl std::fmt::Display for Pattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(x)    => write!(f, "{x}"),
            Self::Pair(a, b) => write!(f, "({}, {})", *a, *b),
            Self::Wildcard   => write!(f, "_"),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Num(x) => write!(f, "{x}"),
            Self::Lambda(arg, body, _) => write!(f, "\\{arg}. {body}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Pair(a, b) => write!(f, "({}, {})", *a, *b),
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


