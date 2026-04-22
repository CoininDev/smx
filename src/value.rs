use crate::{ast::*, eval::*};
use std::{any::Any, rc::Rc, fmt::Debug};
use std::cmp::Ordering;
use ordered_float::NotNan;
use num_bigint::{BigInt, BigUint};
use num_traits::ToPrimitive;
use strum_macros::EnumString;

#[derive(Default, Clone)]
pub struct Ambient {
    pub vars: Environment,
    pub rsrcs: Environment,
    pub natives: Vec<Rc<dyn Any>>,
    pub custom_resources: Vec<Rc<dyn IoObject>>,
}

impl Ambient {
    pub fn extend(&mut self, other: &Ambient) {
        self.vars.extend(other.vars.clone());
        self.rsrcs.extend(other.rsrcs.clone());
        self.natives.extend(other.natives.clone());
        self.custom_resources.extend(other.custom_resources.clone());
    }

    pub fn eject(&mut self, other: &Ambient) {
        for k in other.vars.keys() {
            self.vars.remove(k);
        }
        for k in other.rsrcs.keys() {
            self.rsrcs.remove(k);
        }
        // Note: not ejecting natives or custom_resources, as they might be shared
    }

    pub fn eject_vars(&mut self, vars: &Environment) {
        for k in vars.keys() {
            self.vars.remove(k);
        }
    }

    pub fn add_custom_resource(&mut self, res: std::rc::Rc<dyn IoObject>) {
        let name = res.name().to_string();
        self.custom_resources.push(res.clone());
        self.rsrcs.insert(name.clone(), Value::Builtin(name.clone())); 
    }
}

pub trait IoObject {
    fn redirect(&self, function: Vec<String>, value: Value, amb: &mut Ambient) -> EvalResult<Value>;
    fn name(&self) -> &str;
}


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NumericValue {
    Float(NotNan<f64>),
    Int(BigInt),
    Uint(BigUint),
}

impl std::fmt::Display for NumericValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Float(x) => write!(f, "{x}"),
            Self::Int(x)   => write!(f, "{x}"),
            Self::Uint(x)  => write!(f, "{x}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    Num(NotNan<f64>),
    StrictNum(NumericType, NumericValue),
    Str(String),
    Lambda(Pattern, Expression, Environment, Vec<String>),
    Environment(Environment),
    Type(PatternType),
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

        if let Value::Nil = current {
            return result;
        }

        loop {
            match current {
                Value::Pair(car, cdr) => {
                    result.push((**car).clone());
                    current = cdr;
                }
                Value::Nil => break,
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
    Environment(Vec<(String, Pattern)>),
    Wildcard,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum PatternType {
    Any,
    Nil,
    Pattern,
    Number,
    StrictNumber(NumericType),
    String,
    Lambda,
    Bool,
    Environment,
    EnvironmentWithSchema(Vec<(String, PatternType)>),
    Frozen,
    List(Vec<PatternType>),
}

impl std::fmt::Display for PatternType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatternType::Any => write!(f, "any"),
            PatternType::Nil => write!(f, "nil"),
            PatternType::Number => write!(f, "number"),
            PatternType::StrictNumber(t) => write!(f, "{}", t),
            PatternType::String => write!(f, "string"),
            PatternType::Bool => write!(f, "bool"),
            PatternType::Lambda => write!(f, "fn"),
            PatternType::Pattern => write!(f, "pattern"),
            PatternType::Environment => write!(f, "env"),
            PatternType::EnvironmentWithSchema(schema) => {
                write!(f, "{{")?;
                for (i, (k, t)) in schema.iter().enumerate() {
                    if i > 0 { write!(f, "; ")?; }
                    write!(f, "{} ~ {}", k, t)?;
                }
                write!(f, "}}")
            }
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
            Self::Environment(e)  => {
                write!(f, "{{")?;
                for (i, (k, p)) in e.iter().enumerate() {
                    if i > 0 { write!(f, "; ")?; }
                    write!(f, "{} = {}", k, p)?;
                }
                write!(f, "}}")
            }
            Self::Wildcard        => write!(f, "_"),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Num(x) => write!(f, "{x}"),
            Self::StrictNum(t, v) => write!(f, "{}{}", v, t),
            Self::Str(s) => write!(f, "\"{s}\""),
            Self::Lambda(arg, body, _, res) => {
                if res.is_empty() {
                    write!(f, "(\\{arg}. {body})")
                } else {
                    write!(f, "(\\{arg} @{{")?;
                    for (i, r) in res.iter().enumerate() {
                        if i > 0 { write!(f, ", ")?; }
                        write!(f, "{}", r)?;
                    }
                    write!(f, "}}. {body})")
                }
            },
            Self::Builtin(b) => write!(f, "{b}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Pattern(p) => write!(f, "#{p}"),
            Self::Pair(a, b) => write!(f, "({}, {})", *a, *b),
            Self::Type(p) => write!(f, "type {p}"),
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
            (Value::StrictNum(t1, v1), Value::StrictNum(t2, v2)) if t1 == t2 => {
                match (v1, v2) {
                    (NumericValue::Float(a), NumericValue::Float(b)) => a.partial_cmp(b),
                    (NumericValue::Int(a), NumericValue::Int(b)) => a.partial_cmp(b),
                    (NumericValue::Uint(a), NumericValue::Uint(b)) => a.partial_cmp(b),
                    _ => None,
                }
            }
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
            (Value::StrictNum(t1, v1), Value::StrictNum(t2, v2)) if t1 == t2 => {
                match (v1, v2) {
                    (NumericValue::Float(a), NumericValue::Float(b)) => Value::StrictNum(t1, NumericValue::Float(a + b)),
                    (NumericValue::Int(a), NumericValue::Int(b)) => Value::StrictNum(t1, NumericValue::Int(a + b)),
                    (NumericValue::Uint(a), NumericValue::Uint(b)) => Value::StrictNum(t1, NumericValue::Uint(a + b)),
                    _ => Value::Nil,
                }
            }
            (Value::Str(a), Value::Str(b)) => Value::Str(format!("{a}{b}")),
            (Value::Str(s), other) => Value::Str(format!("{s}{other}")),
            (other, Value::Str(s)) => Value::Str(format!("{other}{s}")),
            (Value::Environment(mut a), Value::Environment(b)) => {
                a.extend(b);
                Value::Environment(a)
            }
            _ => Value::Nil,
        }
    }
}
impl std::ops::Neg for Value {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Value::Num(a) => Value::Num(-a),
            Value::StrictNum(t, NumericValue::Float(a)) => Value::StrictNum(t, NumericValue::Float(-a)),
            Value::StrictNum(t, NumericValue::Int(a)) => Value::StrictNum(t, NumericValue::Int(-a)),
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
            (Value::StrictNum(t1, v1), Value::StrictNum(t2, v2)) if t1 == t2 => {
                match (v1, v2) {
                    (NumericValue::Float(a), NumericValue::Float(b)) => Value::StrictNum(t1, NumericValue::Float(a - b)),
                    (NumericValue::Int(a), NumericValue::Int(b)) => Value::StrictNum(t1, NumericValue::Int(a - b)),
                    (NumericValue::Uint(a), NumericValue::Uint(b)) => Value::StrictNum(t1, NumericValue::Uint(a - b)),
                    _ => Value::Nil,
                }
            }
            (Value::Environment(mut a), rhs) => {
                let keys = rhs.pair_to_vec();
                for k in keys {
                    match k {
                        Value::Str(s) => { a.remove(&s); }
                        Value::Frozen(Expression::Var(v)) if v.len() == 1 => { a.remove(&v[0]); }
                        _ => {}
                    }
                }
                Value::Environment(a)
            }
            _ => Value::Nil,
        }
    }
}

impl std::ops::Mul for Value {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Num(a), Value::Num(b)) => Value::Num(a * b),
            (Value::StrictNum(t1, v1), Value::StrictNum(t2, v2)) if t1 == t2 => {
                match (v1, v2) {
                    (NumericValue::Float(a), NumericValue::Float(b)) => Value::StrictNum(t1, NumericValue::Float(a * b)),
                    (NumericValue::Int(a), NumericValue::Int(b)) => Value::StrictNum(t1, NumericValue::Int(a * b)),
                    (NumericValue::Uint(a), NumericValue::Uint(b)) => Value::StrictNum(t1, NumericValue::Uint(a * b)),
                    _ => Value::Nil,
                }
            }
            _ => Value::Nil,
        }
    }
}


impl std::ops::Div for Value {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Num(a), Value::Num(b)) => Value::Num(a / b),
            (Value::StrictNum(t1, v1), Value::StrictNum(t2, v2)) if t1 == t2 => {
                match (v1, v2) {
                    (NumericValue::Float(a), NumericValue::Float(b)) => Value::StrictNum(t1, NumericValue::Float(a / b)),
                    (NumericValue::Int(a), NumericValue::Int(b)) => Value::StrictNum(t1, NumericValue::Int(a / b)),
                    (NumericValue::Uint(a), NumericValue::Uint(b)) => Value::StrictNum(t1, NumericValue::Uint(a / b)),
                    _ => Value::Nil,
                }
            }
            _ => Value::Nil,
        }
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Num(NotNan::new(f).expect("NaN is not allowed in SMX"))
    }
}

impl From<i32> for Value {
    fn from(i: i32) -> Self {
        Value::Num(NotNan::new(i as f64).unwrap())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.to_string())
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}
