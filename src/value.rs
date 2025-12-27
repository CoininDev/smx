use crate::{ast::*, eval::*, error::EvalError};
use std::{collections::HashMap, fmt};



#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    Num(f64),
    Lambda(String, Expression, Environment),
    Nil,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Num(x) => write!(f, "{x}"),
            Self::Lambda(arg, body, _) => write!(f, "\\{arg}. {body}"),
            Self::Nil => write!(f, "nil")
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


