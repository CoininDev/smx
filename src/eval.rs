use crate::{ast::*, value::*, error::EvalError};
use im::HashMap;
use im::hashmap;

pub type EvalResult<T> = Result<T, EvalError>;
pub type Environment = HashMap<String, Value>;

pub fn eval_program(tree: Program) -> EvalResult<Value> {
    let mut vars = HashMap::new();
    for assign in tree.body {
        eval_assign(assign, &mut vars)?;
    }

    match vars.get("result".into()) {
        Some(a) => Ok(a.clone()),
        None => match vars.into_iter().last() {
            Some(a) => Ok(a.1),
            None => Ok(Value::Num(0.)),
        },
    }
}

pub fn eval_assign(a: Assign, vars: &mut Environment) -> EvalResult<()> {
    let pat = eval_pattern(a.0)?;
    let value = eval_expr(a.1, vars)?;
    vars.extend(eval_pattern_pair(pat, value)?.into_iter());
    Ok(())
}

pub fn eval_expr(e: Expression, vars: &Environment) -> EvalResult<Value> {
    match e {
        Expression::Var(v) => match vars.get(&v) {
            Some(i) => Ok(i.clone()),
            None => Err(EvalError::VariableDoesNotExists(format!("{v}"))),
        },
        Expression::Num(i)  => Ok(Value::Num(i)),
        Expression::Nil     => Ok(Value::Nil),
        Expression::Bool(b) => Ok(Value::Bool(b)),
        Expression::Operation(op, exprs) => eval_operation(op, exprs, vars),
        Expression::Lambda(param, body) => Ok(Value::Lambda(eval_pattern(*param)?, *body, vars.clone())),
        Expression::Application(f, x) => apply(eval_expr(*f, vars)?, eval_expr(*x, vars)?),
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
        _ => Err(EvalError::InvalidPattern(input)),
    }
}

pub fn eval_pattern_pair(pat: Pattern, val: Value) -> Result<Environment, EvalError> {
    fn rec(pat: Pattern, val: Value, acc: Environment) -> Result<Environment, String>{
        match (pat, val) {
            (Pattern::Name(x), value) => 
                Ok(acc.update(x, value)),
            (Pattern::Pair(k1, k2), Value::Pair(v1, v2)) => 
                Ok(rec(*k1, *v1, hashmap!{})?.union(rec(*k2, *v2, acc)?)),
            (Pattern::Wildcard, _) => Ok(acc),
            (a, b) => Err(format!("Mismatching: {}, {}", a.to_string(), b.to_string())),
        }
    }
    rec(pat, val, HashMap::new()).map_err(|x| EvalError::PatternError(x))
}

pub fn apply(func: Value, arg: Value) -> EvalResult<Value> {
    let (param, body, cap_env) = match func {
        Value::Lambda(param, body, cap_env) => (param, body, cap_env),
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
            [left, right] => if eval(right)? != Value::Num(0.0) 
                             || eval(right)? != Value::Nil 
                            { Ok(eval(left)? / eval(right)?) } 
                            else {Err(EvalError::ZeroDivisor)}
            ,
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
        _ => Err(EvalError::UnexpectedOperator(format!("{op}"))),
    }
}
