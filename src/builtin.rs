use im::hashmap;

use crate::{ast::*, error::EvalErrorType::*, error::*, eval::*, value::*};

macro_rules! eval_error {
    ($err: expr) => {
        EvalError::new($err)
    };
}

// operator #
pub fn builtin_pattern_from_value(v: Value) -> Value {
    fn rec(v: Value) -> Pattern {
        match v {
            Value::Pair(a, b) => Pattern::Pair(Box::new(rec(*a)), Box::new(rec(*b))),
            Value::Frozen(Expression::Var(x)) => match x.as_slice() {
                [w] if w == "_" => Pattern::Wildcard,
                [any] => Pattern::Name(any.into()),
                _ => Pattern::Wildcard,
            },
            Value::Frozen(Expression::Operation(op, xs)) if op == "~" => match xs.as_slice() {
                [Expression::Var(left), Expression::Var(_)]
                | [Expression::Var(left), Expression::ListType(_)] => Pattern::TypedName(
                    left[0].clone(),
                    eval_pattern_type(&xs[1]).unwrap_or(PatternType::Nil),
                ),
                _ => Pattern::Wildcard,
            },
            other => Pattern::Value(Box::new(other)),
        }
    }
    Value::Pattern(rec(v))
}
pub trait IBuiltin {
    fn matches(&self, name: &str) -> bool;
    fn call(&self, arg: Value, ambient: &Ambient) -> EvalResult<Value>;
}

pub fn builtin_registry() -> Vec<Box<dyn IBuiltin>> {
    fn n(a: impl IBuiltin + 'static) -> Box<dyn IBuiltin> {
        Box::new(a)
    }
    vec![
        n(TryBuiltin),
        n(EvalBuiltin),
        n(HasBuiltin),
        n(ZipEnvBuiltin),
        n(HeadBuiltin),
        n(TailBuiltin),
        n(ConvertBuiltin),
    ]
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
            Value::Pair(val, box Value::Frozen(Expression::Var(t))) => {
                match t.as_slice() {
                    [s] if s == "number" => match *val {
                        Value::Str(s) => Ok(Value::Num(mount_num(
                            s.parse().map_err(|_| {
                                eval_error!(WrongTypes("convert".into(), PatternType::Number, Value::Str(s)))
                            })?,
                        )?)),
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
                    _ => Err(eval_error!(WrongTypes(
                        "convert".into(),
                        PatternType::Frozen,
                        Value::Frozen(Expression::Var(t))
                    ))),
                }
            }
            other => Err(eval_error!(WrongTypes(
                "convert".into(),
                PatternType::List(vec![]),
                other
            ))),
        }
    }
}
