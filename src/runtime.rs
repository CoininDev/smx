use crate::{value::*, eval::*, ast::*};

// BUILTIN FUNCTIONS
pub fn builtin_try(func: Value, arg: Value, vars: &Environment) -> Value {
    match apply(func, arg, vars) {
        Ok(x) => x,
        _     => Value::Nil,
    }
}

// operator #
pub fn builtin_pattern_from_value(v: Value) -> Value {
    fn rec(v: Value) -> Pattern {
         match v {
            Value::Pair(a, b) => Pattern::Pair(
                    Box::new(rec(*a)), 
                    Box::new(rec(*b))
            ),
            Value::Frozen(Expression::Var(x)) => match x {
                w if w == "_" => Pattern::Wildcard,
                any => Pattern::Name(any),
            },
            other => Pattern::Value(Box::new(other)),
         }
    }
    Value::Pattern(rec(v))
}

pub fn builtin_zip_env(pat: Pattern, arg: Value) -> Value {
    match eval_pattern_pair(pat, arg) {
        Ok(v) => Value::Environment(v),
        Err(_) => Value::Nil,
    }
}

pub fn builtin_use(env: Environment, frozen: Expression, vars: &Environment) -> Value {
    eval_expr(frozen, &env.union(vars.clone())).unwrap_or(Value::Nil)
}

pub fn builtin_eval(frozen: Expression, vars: &Environment) -> Value {
    eval_expr(frozen, vars).unwrap_or(Value::Nil)
}
