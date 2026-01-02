
use crate::{ast::*, value::*, error::EvalError, runtime::*};
use im::HashMap;
use im::hashmap;
use ordered_float::NotNan;

pub type EvalResult<T> = Result<T, EvalError>;
pub type Environment = HashMap<String, Value>;

fn mount_num(num: f64) -> EvalResult<NotNan<f64>> {
    NotNan::new(num).map_err(|e| EvalError::NotNanError(e.to_string()))
}

pub fn eval_program(tree: Program) -> EvalResult<Value> {
    let mut vars = HashMap::new();
    for assign in tree.body {
        eval_assign(assign, &mut vars)?;
    }

    match vars.get("result".into()) {
        Some(a) => Ok(a.clone()),
        None => match vars.into_iter().last() {
            Some(a) => Ok(a.1),
            None => Ok(Value::Num(mount_num(0.)?)),
        },
    }
}

pub fn eval_assign(a: Assign, vars: &mut Environment) -> EvalResult<()> {
    let pat = eval_pattern(a.0)?;
    let value = eval_expr(a.1, vars)?;
    vars.extend(eval_pattern_pair(pat, value)?.into_iter());
    Ok(())
}

fn is_builtin(name: &str) -> bool {
    vec![
        "try",
        "zip_env",
        "use"
    ].contains(&name)
}

pub fn eval_expr(e: Expression, vars: &Environment) -> EvalResult<Value> {
    match e {
        Expression::Var(v) => {
            if is_builtin(&v) {
                return Ok(Value::Builtin(v))
            }

            match vars.get(&v) {
                Some(i) => Ok(i.clone()),
                None    => Err(EvalError::VariableDoesNotExists(format!("{v}"))),
            }
        }
        Expression::Num(i)    => Ok(Value::Num(i)),
        Expression::Nil       => Ok(Value::Nil),
        Expression::Frozen(m) => Ok(Value::Frozen(*m)),
        Expression::Environment(e) => {
            let mut env: HashMap<String, Value> = hashmap!{};
            for (k, v) in e {
                let k = eval_pattern(k)?;
                let v = eval_expr(v, vars)?;
                env.extend(eval_pattern_pair(k, v)?);
            }
            Ok(Value::Environment(env))
        } 
        Expression::Bool(b)   => Ok(Value::Bool(b)),
        Expression::Operation(op, exprs) => eval_operation(op, exprs, vars),
        Expression::Lambda(param, body) => Ok(Value::Lambda(eval_pattern(*param)?, *body, vars.clone())),
        Expression::Application(f, x) => apply(eval_expr(*f, vars)?, eval_expr(*x, vars)?, vars),
    }
}

pub fn eval_pattern(input: Expression) -> EvalResult<Pattern> {
    let eval = |x: &Expression| -> EvalResult<Pattern> {
        eval_pattern(x.clone())
    };
    
    match input {
        Expression::Operation(op, exprs) if op == "," => match exprs.as_slice() {
                [left, right] => Ok(Pattern::Pair(Box::new(eval(left)?), Box::new(eval(right)?))),
                _ => Err(EvalError::InvalidSizeOfArgsFor(",".to_string())),
        },
        Expression::Var(x) if x == "_" => Ok(Pattern::Wildcard),
        Expression::Var(x) => Ok(Pattern::Name(x)),
        other => Ok(Pattern::Value(Box::new(eval_expr(other, &hashmap!{})?))),
    }
}

pub fn eval_pattern_pair(pat: Pattern, val: Value) -> Result<Environment, EvalError> {
    fn rec(pat: Pattern, val: Value, acc: Environment) -> Result<Environment, String>{
        match (pat, val) {
            (Pattern::Name(x), value) => 
                Ok(acc.update(x, value)),
            (Pattern::Pair(k1, k2), Value::Pair(v1, v2)) => 
                Ok(rec(*k1, *v1, hashmap!{})?.union(rec(*k2, *v2, acc)?)),
            (Pattern::Value(k), v) if *k == v => Ok(acc),
            (Pattern::Wildcard, _) => Ok(acc),
            (a, b) => Err(format!("Mismatching: {}, {}", a.to_string(), b.to_string())),
        }
    }
    rec(pat, val, HashMap::new()).map_err(|x| EvalError::PatternError(x))
}

pub fn apply_builtin(x: &str, arg: Value, vars: &Environment) -> EvalResult<Value> {
    match x {
        "try" => match arg {
            Value::Pair(a, b) => Ok(builtin_try(*a, *b, vars)),
            other => Err(EvalError::WrongTypes("try".to_string(), 
                vec![Value::Pair(
                    Box::new(Value::Lambda(
                        Pattern::Wildcard, 
                        Expression::Nil, 
                        hashmap!{}
                    )),
                    Box::new(Value::Nil)
                )], 
                vec![other]
            )),
        }

        "zip_env" => match arg {
            Value::Pair(box Value::Pattern(a), b) => Ok(builtin_zip_env(a, *b)),
            other => Err(EvalError::WrongTypes("zip_env".to_string(), 
                vec![Value::Pair(
                        Box::new(Value::Pattern(Pattern::Wildcard)),
                        Box::new(Value::Nil)
                )], 
                vec![other]
            )),
        }

        "use" => match arg {
            Value::Pair(box Value::Environment(a), box Value::Frozen(b)) => Ok(builtin_use(a, b, vars)),
            other => Err(EvalError::WrongTypes("use".to_string(), 
                vec![Value::Pair(
                        Box::new(Value::Environment(hashmap!{})),
                        Box::new(Value::Frozen(Expression::Nil))
                )], 
                vec![other]
            )),
        }

        _ => Err(EvalError::VariableDoesNotExists(format!("{x}")))
    }
}

pub fn apply(func: Value, arg: Value, vars: &Environment) -> EvalResult<Value> {
    let (param, body, cap_env) = match func {
        Value::Lambda(param, body, cap_env) => (param, body, cap_env),
        Value::Builtin(x) => return apply_builtin(&x, arg, vars),
        _ => return Err(EvalError::NonFunctionApplication(func)),
    };
    let mut vars2 = cap_env.clone();
    vars2.extend(eval_pattern_pair(param, arg)?.into_iter());
    eval_expr(body, &vars2)
}

pub fn eval_operation(op: String, exprs: Vec<Expression>, vars: &Environment) -> EvalResult<Value> {  
    let eval = |x: &Expression| -> EvalResult<Value> {
        eval_expr(x.clone(), vars)
    };

    match op.as_str() {
        "#" => match exprs.as_slice() {
            [expr] => Ok(builtin_pattern_from_value(eval(expr)?)),
            _ => Err(EvalError::InvalidSizeOfArgsFor("#".to_string())),
        },
        "+" => match exprs.as_slice() {
            [expr] => eval(expr),
            [left, right] => Ok(eval(left)? + eval(right)?),
            _ => Err(EvalError::InvalidSizeOfArgsFor("+".to_string())),
        },
        "-" => match exprs.as_slice() {
            [expr] => Ok(-(eval(expr)?)),
            [left, right] => Ok(eval(left)? - eval(right)?),
            _ => Err(EvalError::InvalidSizeOfArgsFor("-".to_string())),
        },
        "*" => match exprs.as_slice() {
            [left, right] => Ok(eval(left)? * eval(right)?),
            _ => Err(EvalError::InvalidSizeOfArgsFor("*".to_string())),
        },
        "/" => match exprs.as_slice() {
            [left, right] => match eval(right)? {
                Value::Nil => Err(EvalError::ZeroDivisor),
                Value::Num(x) if x == 0. => Err(EvalError::ZeroDivisor),
                otherwise => Ok(eval(left)? / otherwise),
            },
            _ => Err(EvalError::InvalidSizeOfArgsFor("/".to_string())),
        },
        "," => match exprs.as_slice() {
            [left, right] => Ok(Value::Pair(Box::new(eval(left)?), Box::new(eval(right)?))),
            _ => Err(EvalError::InvalidSizeOfArgsFor(",".to_string())),
        },
        "?" => match exprs.as_slice() {
            [left, right] => {
                match (eval(left)?, eval(right)?) {
                    (Value::Bool(cond), Value::Pair(l, r)) => {
                        if cond { Ok(*l) } else { Ok(*r) }
                    }
                    (a, b) => Err(EvalError::WrongTypes("?".to_string(), 
                            vec![Value::Bool(false), Value::Pair(
                                Box::new(Value::Nil), 
                                Box::new(Value::Nil))],
                            vec![a, b]))
                }
            }
            _ => Err(EvalError::InvalidSizeOfArgsFor("?".to_string())),
        }
        "<" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? < eval(right)?)),
            _ => Err(EvalError::InvalidSizeOfArgsFor("<".to_string())),
        },
        ">" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? > eval(right)?)),
            _ => Err(EvalError::InvalidSizeOfArgsFor(">".to_string())),
        },
        "<=" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? <= eval(right)?)),
            _ => Err(EvalError::InvalidSizeOfArgsFor("<=".to_string())),
        },
        ">=" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? >= eval(right)?)),
            _ => Err(EvalError::InvalidSizeOfArgsFor(">=".to_string())),
        },
        "==" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? == eval(right)?)),
            _ => Err(EvalError::InvalidSizeOfArgsFor("==".to_string())),
        },
        "!=" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? != eval(right)?)),
            _ => Err(EvalError::InvalidSizeOfArgsFor("!=".to_string())),
        },
        "||" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a || b)),
                _ => Err(EvalError::WrongTypes("||".to_string(), 
                            vec![Value::Bool(true), Value::Bool(true)],
                            vec![eval(left)?, eval(right)?])),
            }
            _ => Err(EvalError::InvalidSizeOfArgsFor("||".to_string())),
        }
        "&&" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
                _ => Err(EvalError::WrongTypes("&&".to_string(), 
                            vec![Value::Bool(true), Value::Bool(true)],
                            vec![eval(left)?, eval(right)?])),
            }
            _ => Err(EvalError::InvalidSizeOfArgsFor("&&".to_string())),
        }
        ":" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (a, Value::Lambda(_, _, _)) => apply(eval(right)?, a, vars),
                _ => Err(EvalError::WrongTypes(":".to_string(), 
                            vec![
                                Value::Nil, 
                                Value::Lambda(Pattern::Wildcard, Expression::Nil, hashmap!{})
                            ],
                            vec![eval(left)?, eval(right)?])),
            }
            _ => Err(EvalError::InvalidSizeOfArgsFor(":".to_string())),
        }
        "::" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (Value::Environment(a), Value::Frozen(e)) => eval_expr(e, &a.union(vars.clone())),
                _ => Err(EvalError::WrongTypes("&&".to_string(), 
                            vec![
                                Value::Environment(hashmap!{}), 
                                Value::Frozen(Expression::Nil),
                            ],
                            vec![eval(left)?, eval(right)?])),
            }
            _ => Err(EvalError::InvalidSizeOfArgsFor("&&".to_string())),
        }
        _ => Err(EvalError::UnexpectedOperator(format!("{op}"))),
    }
}
