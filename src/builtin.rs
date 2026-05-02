use im::hashmap;
use std::str::FromStr;
use strum::IntoEnumIterator;
use num_traits::ToPrimitive;
use ordered_float::NotNan;
use num_bigint::{BigInt, BigUint, ToBigInt};
use crate::{
    ast::*,
    error::EvalErrorType::*,
    error::*,
    eval::*,
    value::*,
    io::util_eval_expr_str,
};

macro_rules! eval_error {
    ($err: expr) => {
        EvalError::new($err)
    };
}

// operator #
pub fn builtin_pattern_from_value(v: Value, amb: &Ambient) -> Value {
    fn rec(v: Value, amb: &Ambient) -> Pattern {
        match v {
            Value::Pair(a, b) => Pattern::Pair(Box::new(rec(*a, amb)), Box::new(rec(*b, amb))),
            Value::Frozen(Expression { kind: ExprKind::Var(x), span: _ }) => match x.as_slice() {
                [w] if w == "_" => Pattern::Wildcard,
                [any] => Pattern::Name(any.into()),
                _ => Pattern::Wildcard,
            },
            Value::Frozen(Expression { kind: ExprKind::Operation(op, xs), span: _ }) if op == "~" => match xs.as_slice() {
                [Expression { kind: ExprKind::Var(left), .. }, Expression { kind: ExprKind::Var(_), .. }]
                | [Expression { kind: ExprKind::Var(left), .. }, Expression { kind: ExprKind::ListType(_), .. }] => Pattern::TypedName(
                    left[0].clone(),
                    eval_pattern_type(&xs[1], amb).unwrap_or(PatternType::Nil),
                ),
                _ => Pattern::Wildcard,
            },
            Value::Frozen(Expression { kind: ExprKind::Environment(body), span: _ }) => {
                let mut schema = vec![];
                for Assign(id, _, expr) in body {
                    match id.kind {
            ExprKind::Var(v) if v.len() == 1 => {
                            let name = v[0].clone();
                            let pat = if let ExprKind::Nil = expr.kind {
                                Pattern::Name(name.clone())
                            } else {
                                rec(Value::Frozen(expr), amb)
                            };
                            schema.push((name, pat));
                        }
                        ExprKind::Operation(ref op, ref xs) if op == "~" => {
                            match xs.as_slice() {
                                [Expression { kind: ExprKind::Var(v), .. }, _] if v.len() == 1 => {
                                    let name = v[0].clone();
                                    let pat = rec(Value::Frozen(id), amb);
                                    schema.push((name, pat));
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
                Pattern::Environment(schema)
            }
            other => Pattern::Value(Box::new(other)),
        }
    }
    Value::Pattern(rec(v, amb))
}
pub trait IBuiltin {
    fn matches(&self, name: &str) -> bool;
    fn call(&self, arg: Value, ambient: &Ambient) -> EvalResult<Value>;
}

pub fn builtin_registry() -> Vec<Box<dyn IBuiltin>> {
    fn n(a: impl IBuiltin + 'static) -> Box<dyn IBuiltin> {
        Box::new(a)
    }
    let mut res = vec![
        n(TryBuiltin),
        n(EvalBuiltin),
        n(HasBuiltin),
        n(ZipEnvBuiltin),
        n(HeadBuiltin),
        n(TailBuiltin),
        n(ConvertBuiltin),
    ];

    for t in NumericType::iter() {
        res.push(Box::new(StrictTypeBuiltin(t)));
    }

    res
}

#[derive(Clone)]
pub struct TryBuiltin;
impl IBuiltin for TryBuiltin {
    fn matches(&self, name: &str) -> bool {
        name == "try"
    }
    fn call(&self, arg: Value, amb: &Ambient) -> EvalResult<Value> {
        match arg {
            Value::Pair(func, arg) => match apply(*func, *arg, &mut amb.clone()) {
                Ok(x) => Ok(x),
                _ => Ok(Value::Environment(
                    hashmap! {"err".into() => Value::Bool(true)},
                )),
            },
            other => Err(eval_error!(WrongTypes(
                "try".into(),
                PatternType::List(vec![]),
                other
            ))),
        }
    }
}

#[derive(Clone)]
pub struct EvalBuiltin;
impl IBuiltin for EvalBuiltin {
    fn matches(&self, name: &str) -> bool {
        name == "eval"
    }
    fn call(&self, arg: Value, amb: &Ambient) -> EvalResult<Value> {
        match arg {
            Value::Frozen(frozen) => eval_expr(frozen, &mut amb.clone()),
            Value::Str(text) => util_eval_expr_str(text.as_str(), amb)
                .map_err(|e| eval_error!(GenericError(e.to_string()))),
            other => Err(eval_error!(WrongTypes(
                "eval".into(),
                PatternType::Frozen,
                other
            ))),
        }
    }
}

#[derive(Clone)]
pub struct HasBuiltin;
impl IBuiltin for HasBuiltin {
    fn matches(&self, name: &str) -> bool {
        name == "has"
    }
    fn call(&self, arg: Value, _amb: &Ambient) -> EvalResult<Value> {
        match arg {
            Value::Pair(box Value::Environment(env), box Value::Str(name)) => {
                Ok(Value::Bool(env.contains_key(&name)))
            }
            other => Err(eval_error!(WrongTypes(
                "has".into(),
                PatternType::List(vec![PatternType::Environment, PatternType::String]),
                other
            ))),
        }
    }
}

#[derive(Clone)]
pub struct ZipEnvBuiltin;
impl IBuiltin for ZipEnvBuiltin {
    fn matches(&self, name: &str) -> bool {
        name == "zip_env"
    }
    fn call(&self, arg: Value, _amb: &Ambient) -> EvalResult<Value> {
        match arg {
            Value::Pair(box Value::Pattern(pat), a) => match eval_pattern_pair(pat, *a) {
                Ok(v) => Ok(Value::Environment(v)),
                Err(_) => Ok(Value::Nil),
            },

            other => Err(eval_error!(WrongTypes(
                "zip_env".into(),
                PatternType::List(vec![PatternType::Pattern, PatternType::Nil]),
                other
            ))),
        }
    }
}

#[derive(Clone)]
pub struct HeadBuiltin;
impl IBuiltin for HeadBuiltin {
    fn matches(&self, name: &str) -> bool {
        name == "head"
    }
    fn call(&self, arg: Value, _amb: &Ambient) -> EvalResult<Value> {
        match arg {
            Value::Pair(a, _) => Ok(*a),
            Value::Nil => Ok(Value::Nil),
            Value::Str(s) => match s.chars().next() {
                Some(c) => Ok(Value::Str(c.to_string())),
                None => Ok(Value::Nil),
            },
            other => Err(eval_error!(WrongTypes(
                "head".into(),
                PatternType::List(vec![]),
                other
            ))),
        }
    }
}

#[derive(Clone)]
pub struct TailBuiltin;
impl IBuiltin for TailBuiltin {
    fn matches(&self, name: &str) -> bool {
        name == "tail"
    }
    fn call(&self, arg: Value, _amb: &Ambient) -> EvalResult<Value> {
        match arg {
            Value::Pair(_, b) => Ok(*b),
            Value::Nil => Ok(Value::Nil),
            Value::Str(s) => Ok(Value::Str(s.chars().skip(1).collect())),
            other => Err(eval_error!(WrongTypes(
                "tail".into(),
                PatternType::List(vec![]),
                other
            ))),
        }
    }
}

#[derive(Clone)]
pub struct ConvertBuiltin;
impl IBuiltin for ConvertBuiltin {
    fn matches(&self, name: &str) -> bool {
        name == "convert"
    }

    fn call(&self, arg: Value, _amb: &Ambient) -> EvalResult<Value> {
        match arg {
            Value::Pair(val, box Value::Frozen(Expression { kind: ExprKind::Var(t), span: _ })) => match t.as_slice() {
                [s] if s == "number" => match *val {
                    Value::Str(s) => {
                        if let Ok(num) = s.parse() {
                            Ok(Value::Num(mount_num(num)?))
                        } else {
                            Ok(Value::Environment(
                                hashmap! {"nan".into() => Value::Bool(true)},
                            ))
                        }
                    }
                    Value::Bool(b) => Ok(Value::Num(mount_num(if b { 1. } else { 0. })?)),
                    Value::Num(n) => Ok(Value::Num(n)),
                    other => Err(eval_error!(WrongTypes(
                        "convert".into(),
                        PatternType::Number,
                        other
                    ))),
                },
                [s] if s == "string" => match *val {
                    Value::Str(s) => Ok(Value::Str(s)),
                    other => Ok(Value::Str(other.to_string())),
                },
                [s] if s == "bool" => match *val {
                    Value::Num(n) => Ok(Value::Bool(*n != 0.)),
                    Value::Str(s) if s == "true" => Ok(Value::Bool(true)),
                    Value::Str(s) if s == "false" => Ok(Value::Bool(false)),
                    Value::Bool(b) => Ok(Value::Bool(b)),
                    other => Err(eval_error!(WrongTypes(
                        "convert".into(),
                        PatternType::Bool,
                        other
                    ))),
                },
                [s] if s == "list" => match *val {
                    Value::Environment(env) => {
                        let mut res = Value::Nil;
                        for (k, v) in env {
                            res = Value::Pair(
                                Box::new(Value::Pair(Box::new(Value::Str(k)), Box::new(v))),
                                Box::new(res),
                            );
                        }
                        Ok(res)
                    }
                    other => Err(eval_error!(WrongTypes(
                        "convert".into(),
                        PatternType::List(vec![]),
                        other
                    ))),
                },
                [s] if s == "env" => {
                    let mut env = hashmap! {};
                    let mut current = &*val;
                    while let Value::Pair(car, cdr) = current {
                        if let Value::Pair(k, v) = &**car {
                            if let Value::Str(key) = &**k {
                                env.insert(key.clone(), (**v).clone());
                            }
                        }
                        current = &**cdr;
                    }
                    Ok(Value::Environment(env))
                }
                _ => {
                    if t.len() == 1 {
                        let s = &t[0];
                        if let Ok(nt) = NumericType::from_str(s) {
                            return StrictTypeBuiltin(nt).call(*val, _amb);
                        }
                    }
                    Err(eval_error!(WrongTypes(
                        "convert".into(),
                        PatternType::Frozen,
                        Value::Frozen(Expression::dummy(ExprKind::Var(t)))
                    )))
                }
            },
            other => Err(eval_error!(WrongTypes(
                "convert".into(),
                PatternType::List(vec![]),
                other
            ))),
        }
    }
}

#[derive(Clone)]
pub struct StrictTypeBuiltin(NumericType);
impl IBuiltin for StrictTypeBuiltin {
    fn matches(&self, name: &str) -> bool {
        name == self.0.to_string()
    }
    fn call(&self, arg: Value, _amb: &Ambient) -> EvalResult<Value> {
        let t = self.0;
        let val = match arg {
            Value::Num(n) => {
                match t {
                    NumericType::F8 | NumericType::F16 | NumericType::F32 | NumericType::F64 | NumericType::F128 | NumericType::F256 => {
                        NumericValue::Float(n)
                    }
                    NumericType::I8 | NumericType::I16 | NumericType::I32 | NumericType::I64 | NumericType::I128 | NumericType::I256 => {
                        NumericValue::Int(BigInt::from(*n as i128))
                    }
                    NumericType::U8 | NumericType::U16 | NumericType::U32 | NumericType::U64 | NumericType::U128 | NumericType::U256 => {
                        NumericValue::Uint(BigUint::from(*n as u128))
                    }
                }
            }
            Value::StrictNum(_, v) => {
                 match (t, v) {
                    (NumericType::F8 | NumericType::F16 | NumericType::F32 | NumericType::F64 | NumericType::F128 | NumericType::F256, NumericValue::Float(f)) => NumericValue::Float(f),
                    (NumericType::F8 | NumericType::F16 | NumericType::F32 | NumericType::F64 | NumericType::F128 | NumericType::F256, NumericValue::Int(i)) => NumericValue::Float(NotNan::new(i.to_f64().unwrap_or(0.)).unwrap()),
                    (NumericType::F8 | NumericType::F16 | NumericType::F32 | NumericType::F64 | NumericType::F128 | NumericType::F256, NumericValue::Uint(u)) => NumericValue::Float(NotNan::new(u.to_f64().unwrap_or(0.)).unwrap()),
                    
                    (NumericType::I8 | NumericType::I16 | NumericType::I32 | NumericType::I64 | NumericType::I128 | NumericType::I256, NumericValue::Float(f)) => NumericValue::Int(BigInt::from(*f as i128)),
                    (NumericType::I8 | NumericType::I16 | NumericType::I32 | NumericType::I64 | NumericType::I128 | NumericType::I256, NumericValue::Int(i)) => NumericValue::Int(i),
                    (NumericType::I8 | NumericType::I16 | NumericType::I32 | NumericType::I64 | NumericType::I128 | NumericType::I256, NumericValue::Uint(u)) => NumericValue::Int(u.to_bigint().unwrap()),
                    
                    (NumericType::U8 | NumericType::U16 | NumericType::U32 | NumericType::U64 | NumericType::U128 | NumericType::U256, NumericValue::Float(f)) => NumericValue::Uint(BigUint::from(*f as u128)),
                    (NumericType::U8 | NumericType::U16 | NumericType::U32 | NumericType::U64 | NumericType::U128 | NumericType::U256, NumericValue::Int(i)) => NumericValue::Uint(i.to_biguint().unwrap_or_default()),
                    (NumericType::U8 | NumericType::U16 | NumericType::U32 | NumericType::U64 | NumericType::U128 | NumericType::U256, NumericValue::Uint(u)) => NumericValue::Uint(u),
                 }
            }
            Value::Str(s) => {
                match t {
                    NumericType::F8 | NumericType::F16 | NumericType::F32 | NumericType::F64 | NumericType::F128 | NumericType::F256 => {
                        NumericValue::Float(NotNan::new(s.parse::<f64>().map_err(|e| eval_error!(GenericError(e.to_string())))?).unwrap())
                    }
                    NumericType::I8 | NumericType::I16 | NumericType::I32 | NumericType::I64 | NumericType::I128 | NumericType::I256 => {
                        NumericValue::Int(s.parse::<BigInt>().map_err(|e| eval_error!(GenericError(e.to_string())))?)
                    }
                    NumericType::U8 | NumericType::U16 | NumericType::U32 | NumericType::U64 | NumericType::U128 | NumericType::U256 => {
                        NumericValue::Uint(s.parse::<BigUint>().map_err(|e| eval_error!(GenericError(e.to_string())))?)
                    }
                }
            }
            _ => return Err(eval_error!(WrongTypes(t.to_string(), PatternType::Number, arg))),
        };
        Ok(Value::StrictNum(t, val))
    }
}
