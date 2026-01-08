use crate::{eval::*, value::*, error::*, error::EvalErrorType::*};

macro_rules! eval_error {
    ($err: expr) => {
        EvalError::new($err)
    }
}

pub trait IBuiltin {
    fn matches(&self, name: &str) -> bool;
    fn call(&self, arg: Value, env: &Environment, rsrcs: &Environment) -> EvalResult<Value>;
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
    ]
}

#[derive(Clone)]
pub struct TryBuiltin;
impl IBuiltin for TryBuiltin {
    fn matches(&self, name: &str) -> bool {name == "try"}
    fn call(&self, arg: Value, vars: &Environment, rsrcs: &Environment) -> EvalResult<Value> {
        match arg {
            Value::Pair(func, arg) => match apply(*func, *arg, vars, rsrcs) {
                    Ok(x) => Ok(x),
                    _     => Ok(Value::Nil),
            }
            other => Err(eval_error!(WrongTypes("try".into(), PatternType::List(vec![]), other))),
        }
    }
}

#[derive(Clone)]
pub struct EvalBuiltin;
impl IBuiltin for EvalBuiltin {
    fn matches(&self, name: &str) -> bool {name == "eval"}
    fn call(&self, arg: Value, vars: &Environment, rsrcs: &Environment) -> EvalResult<Value> {
        match arg {
            Value::Frozen(frozen) => eval_expr(frozen, vars, rsrcs),
            other => Err(eval_error!(WrongTypes("eval".into(), PatternType::Frozen, other))),
        }
    }
}

#[derive(Clone)]
pub struct HasBuiltin;
impl IBuiltin for HasBuiltin {
    fn matches(&self, name: &str) -> bool {name == "has"}
    fn call(&self, arg: Value, vars: &Environment, rsrcs: &Environment) -> EvalResult<Value> {
        match arg {
            Value::Pair(box Value::Environment(env), box Value::Str(name)) => 
                Ok(Value::Bool(env.contains_key(&name))),
            other => Err(eval_error!(WrongTypes("has".into(), PatternType::List(vec![PatternType::Environment, PatternType::String]), other))),
        }
    }
}

#[derive(Clone)]
pub struct ZipEnvBuiltin;
impl IBuiltin for ZipEnvBuiltin {
    fn matches(&self, name: &str) -> bool {name == "zip_env"}
    fn call(&self, arg: Value, vars: &Environment, rsrcs: &Environment) -> EvalResult<Value> {
        match arg {
            Value::Pair(box Value::Pattern(pat), a) => 
                match eval_pattern_pair(pat, *a) {
                    Ok(v) => Ok(Value::Environment(v)),
                    Err(_) => Ok(Value::Nil),
                }

            other => Err(eval_error!(WrongTypes("zip_env".into(), PatternType::List(vec![PatternType::Pattern, PatternType::Nil]), other))),
        }
    }
}
