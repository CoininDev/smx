use crate::{ast::*, value::*, error::EvalError};
use std::{collections::HashMap, fmt};

pub type EvalResult<T> = Result<T, EvalError>;
pub type Environment = HashMap<String, Value>;

pub fn eval_program(tree: Program) -> EvalResult<Value> {
    let mut vars = HashMap::new();
    for assign in tree.body {
        let (name, val) = eval_assign(assign, &vars)?;
        vars.insert(name, val);
    }
    match vars.get(&"result".to_string()) {
        Some(a) => Ok(a.clone()),
        None => match vars.into_iter().last() {
            Some(a) => Ok(a.1),
            None => Ok(Value::Num(0.)),
        },
    }
}

pub fn eval_assign(a: Assign, vars: &Environment) -> EvalResult<(String, Value)> {
    let name = a.0;
    let value = eval_expr(a.1, vars)?;
    Ok((name, value))
}

pub fn eval_expr(e: Expression, vars: &Environment) -> EvalResult<Value> {
    match e {
        Expression::Var(v) => match vars.get(&v) {
            Some(i) => Ok(i.clone()),
            None => Err(EvalError::VariableDoesNotExists(format!("{v}"))),
        },
        Expression::Num(i) => Ok(Value::Num(i)),
        Expression::Parenthed(f) => eval_expr(*f, vars),
        Expression::Operation(op, exprs) => eval_operation(op, exprs, vars),
        Expression::Bool(b) => Ok(Value::Bool(b)),
        Expression::Lambda(arg, body) => Ok(Value::Lambda(arg, *body, vars.clone())),
        Expression::Application(f, x) => apply(eval_expr(*f, vars)?, eval_expr(*x, vars)?),
    }
}

pub fn apply(func: Value, arg: Value) -> EvalResult<Value> {
    let (param, body, cap_env) = match func {
        Value::Lambda(param, body, cap_env) => (param, body, cap_env),
        _ => return Err(EvalError::NonFunctionApplication(func)),
    };
    let mut vars2 = cap_env.clone();
    vars2.insert(param, arg);

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
        "<" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? < eval(right)?)),
            _ => Err(EvalError::InvalidSizeOfArgsFor("/".to_string())),
        },
        ">" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? > eval(right)?)),
            _ => Err(EvalError::InvalidSizeOfArgsFor("/".to_string())),
        },
        "<=" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? <= eval(right)?)),
            _ => Err(EvalError::InvalidSizeOfArgsFor("/".to_string())),
        },
        ">=" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? >= eval(right)?)),
            _ => Err(EvalError::InvalidSizeOfArgsFor("/".to_string())),
        },
        "==" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? == eval(right)?)),
            _ => Err(EvalError::InvalidSizeOfArgsFor("/".to_string())),
        },
        "!=" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? != eval(right)?)),
            _ => Err(EvalError::InvalidSizeOfArgsFor("/".to_string())),
        },
        "||" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a || b)),
                _ => Err(EvalError::WrongTypes("||".to_string(), 
                            vec![Value::Bool(true), Value::Bool(true)],
                            vec![eval(left)?, eval(right)?])),
            }
            _ => Err(EvalError::InvalidSizeOfArgsFor("/".to_string())),
        }
        "&&" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
                _ => Err(EvalError::WrongTypes("&&".to_string(), 
                            vec![Value::Bool(true), Value::Bool(true)],
                            vec![eval(left)?, eval(right)?])),
            }
            _ => Err(EvalError::InvalidSizeOfArgsFor("/".to_string())),
        }
        _ => Err(EvalError::UnexpectedOperator(format!("{op}"))),
    }
}
