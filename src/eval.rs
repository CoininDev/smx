use crate::{ast::*, value::*, error::{*, EvalErrorType::*}, runtime::*};
use im::HashMap;
use im::hashmap;
use im::vector;
use ordered_float::NotNan;

macro_rules! eval_error {
    ($err: expr) => {
        EvalError::new($err)
    }
}

pub type EvalResult<T> = Result<T, EvalError>;
pub type Environment = HashMap<String, Value>;

fn mount_num(num: f64) -> EvalResult<NotNan<f64>> {
    NotNan::new(num).map_err(|e| eval_error!(NotNanError(e.to_string())))
}

pub fn eval_program(tree: Program) -> EvalResult<Value> {
    let mut resources = HashMap::new();
    for res in &tree.body {
        eval_resource(res, &mut resources)?;
    }

    let mut vars = HashMap::new();
    for assign in tree.body {
        eval_assign(assign, &mut vars, &resources)?;
    }

    match vars.get("result".into()) {
        Some(a) => Ok(a.clone()),
        None => match vars.into_iter().last() {
            Some(a) => Ok(a.1),
            None => Ok(Value::Num(mount_num(0.)?)),
        },
    }
}

pub fn eval_resource(res: &Assign, resources: &mut Environment) -> EvalResult<()> {
    let name = match &res.0 {
        Expression::Var(m) if m[0] == "__RESOURCE__" => m[1].clone(),
        _ => return Ok(()),
    };

    let value = eval_expr(res.2.clone(), &Environment::default(), resources)?;
    resources.insert(name, value);
    Ok(())
}

pub fn eval_assign(a: Assign, vars: &mut Environment, rsrcs: &Environment) -> EvalResult<()> {
    if let Expression::Var(m) = &a.0 {
        if m[0] == "__RESOURCE__" {
            return Ok(());
        }
    }

    let pat = eval_pattern(a.0)?;
    
    let mut vars2 = vars.clone();
    for res in a.1 {
        match rsrcs.get(&res) {
            _ if is_builtin_res(res.as_str()) => vars2.insert(res.clone(), Value::Builtin(res.into())),
            Some(m) => vars2.insert(res, m.clone()),
            _ => return Err(eval_error!(VariableDoesNotExists(res))),
        };
    }

    let value = eval_expr(a.2, &vars2, &rsrcs).map_err(|e| EvalError{
        errtype: e.errtype, 
        assign: Some(pat.to_string())
    })?;
    
    #[cfg(debug_assertions)]
    println!("eval_assign adding: {pat} = {value}");
    vars.extend(eval_pattern_pair(pat, value)?.into_iter());
    Ok(())
}

fn is_builtin(name: &str) -> bool {
    vector![
        "try",
        "zip_env",
        "use",
        "eval"
    ].contains(&name)
}

fn is_builtin_res(name: &str) -> bool {
    vector![
        "IO"
    ].contains(&name)
}

pub fn eval_expr(e: Expression, vars: &Environment, rsrcs: &Environment) -> EvalResult<Value> {
    match e {
        Expression::Var(v) => {
            match v.as_slice() {
                [one] => {
                    if is_builtin(&one) {
                        return Ok(Value::Builtin(one.into()))
                    }

                    match vars.get(one) {
                        Some(i) => Ok(i.clone()),
                        None    => Err(eval_error!(VariableDoesNotExists(format!("{one}")))),
                    }
                },
                _ => {
                    if is_builtin_res(&v[0]) {
                        return Ok(Value::Builtin(v.join(".")));
                    }

                    let a = match vars.get(&v[0]) {
                        Some(Value::Environment(env)) => Ok(env.clone()),
                        Some(_) => Err(eval_error!(
                                VariableDoesNotExists(format!("{} of type env", v[0])))
                        ),
                        None=> Err(eval_error!(VariableDoesNotExists(format!("{}", v[0])))),
                    }?;

                    eval_expr(Expression::Var(v[1..].to_vec()), &a, rsrcs)
                }
            }
            
        }
        Expression::OpSigVar(OpSig::Prefix(v))|
        Expression::OpSigVar(OpSig::Infix(v))=> {
            match vars.get(&format!("<operator>{v}")) {
                Some(i) => Ok(i.clone()),
                None    => Err(eval_error!(VariableDoesNotExists(format!("{v}")))),
            }
        },
        Expression::Num(i)    => Ok(Value::Num(i)),
        Expression::Str(s)    => Ok(Value::Str(s)),
        Expression::Nil       => Ok(Value::Nil),
        Expression::Frozen(m) => Ok(Value::Frozen(*m)),
        Expression::Environment(e) => {
            let mut env: HashMap<String, Value> = hashmap!{};
            for ass in e {
                eval_assign(ass, &mut env, rsrcs)?;
            }
            Ok(Value::Environment(env))
        } 
        Expression::Bool(b)   => Ok(Value::Bool(b)),
        Expression::Operation(op, exprs) => eval_operation(op, exprs, vars, rsrcs),
        Expression::Lambda(param, body) => Ok(Value::Lambda(eval_pattern(*param)?, *body, vars.clone())),
        Expression::Application(f, x) => apply(eval_expr(*f, vars, rsrcs)?, eval_expr(*x, vars, rsrcs)?, vars, rsrcs),
    }
}

pub fn eval_pattern(input: Expression) -> EvalResult<Pattern> {
    let eval = |x: &Expression| -> EvalResult<Pattern> {
        eval_pattern(x.clone())
    };
    
    match input {
        Expression::Operation(op, exprs) if op == "," => match exprs.as_slice() {
                [left, right] => Ok(Pattern::Pair(Box::new(eval(left)?), Box::new(eval(right)?))),
                _ => Err(eval_error!(InvalidSizeOfArgsFor(",".to_string()))),
        },
        Expression::Var(v) if matches!(v.as_slice(), [x] if x == "_") => Ok(Pattern::Wildcard),
        Expression::Var(v) if v.len() == 1 => Ok(Pattern::Name(v[0].clone())),

        //used by custom operators only
        Expression::OpSigVar(OpSig::Prefix(x)) |
        Expression::OpSigVar(OpSig::Infix(x))  => Ok(Pattern::Name(format!("<custom_operator>{x}"))),
        other => Ok(Pattern::Value(Box::new(eval_expr(other, &hashmap!{}, &hashmap!{})?))),
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
    rec(pat, val, HashMap::new()).map_err(|x| eval_error!(PatternError(x)))
}

pub fn apply_builtin(x: &str, arg: Value, vars: &Environment, rsrcs: &Environment) -> EvalResult<Value> {
    let io_resource_imported = matches!(vars.get("IO"), Some(Value::Builtin(x)) if x == "IO");

    match x {
        "try" => match arg {
            Value::Pair(a, b) => Ok(builtin_try(*a, *b, vars, rsrcs)),
            other => Err(eval_error!(WrongTypes("try".to_string(), 
                vec![Value::Pair(
                    Box::new(Value::Lambda(
                        Pattern::Wildcard, 
                        Expression::Nil, 
                        hashmap!{}
                    )),
                    Box::new(Value::Nil)
                )], 
                vec![other]
            ))),
        }

        "zip_env" => match arg {
            Value::Pair(box Value::Pattern(a), b) => Ok(builtin_zip_env(a, *b)),
            other => Err(eval_error!(WrongTypes("zip_env".to_string(), 
                vec![Value::Pair(
                        Box::new(Value::Pattern(Pattern::Wildcard)),
                        Box::new(Value::Nil)
                )], 
                vec![other]
            ))),
        }

        "use" => match arg {
            Value::Pair(box Value::Environment(a), box Value::Frozen(b)) => Ok(builtin_use(a, b, vars, rsrcs)),
            other => Err(eval_error!(WrongTypes("use".to_string(), 
                vec![Value::Pair(
                        Box::new(Value::Environment(hashmap!{})),
                        Box::new(Value::Frozen(Expression::Nil))
                )], 
                vec![other]
            ))),
        }

        "eval" => match arg {
            Value::Frozen(x) => Ok(builtin_eval(x, vars, rsrcs)),
            other => Err(eval_error!(WrongTypes("eval".to_string(),
                vec![Value::Frozen(Expression::Nil)], vec![other]))),
        }

        "IO.println" if io_resource_imported => match arg {
            Value::Str(x) => Ok(IoResource::println(x)),
            other => Err(eval_error!(WrongTypes(
                        "IO.println".to_string(), 
                        vec![Value::Str("".into())], 
                        vec![other]
                     ))),
        }

        _ => Err(eval_error!(VariableDoesNotExists(format!("{x}"))))
    }
}

pub fn apply(func: Value, arg: Value, vars: &Environment, rsrcs: &Environment) -> EvalResult<Value> {
    let func_clone = func.clone();
    let (param, body, cap_env) = match func {
        Value::Lambda(param, body, cap_env) => (param, body, cap_env),
        Value::Builtin(x) => return apply_builtin(&x, arg, vars, rsrcs),
        _ => return Err(eval_error!(NonFunctionApplication(func))),
    };
    let new_env = eval_pattern_pair(param, arg)?;
    let vars2 = cap_env.clone().union(new_env);
    let vars2 = vars2.union(hashmap!{"__self".into() => func_clone});
    #[cfg(debug_assertions)]
    println!("apply env: {vars2:#?}");
    eval_expr(body, &vars2, rsrcs)
}

pub fn eval_operation(op: String, exprs: Vec<Expression>, vars: &Environment, rsrcs: &Environment) 
    -> EvalResult<Value> {  
    let eval = |x: &Expression| -> EvalResult<Value> {
        eval_expr(x.clone(), vars, rsrcs)
    };

    match op.as_str() {
        "#" => match exprs.as_slice() {
            [expr] => Ok(builtin_pattern_from_value(eval(expr)?)),
            _ => Err(eval_error!(InvalidSizeOfArgsFor("#".to_string()))),
        },
        "!" => match exprs.as_slice() {
            [expr] => Ok(!eval(expr)?),
            _ => Err(eval_error!(InvalidSizeOfArgsFor("!".to_string()))),
        },
        "+" => match exprs.as_slice() {
            [expr] => eval(expr),
            [left, right] => Ok(eval(left)? + eval(right)?),
            _ => Err(eval_error!(InvalidSizeOfArgsFor("+".to_string()))),
        },
        "-" => match exprs.as_slice() {
            [expr] => Ok(-(eval(expr)?)),
            [left, right] => Ok(eval(left)? - eval(right)?),
            _ => Err(eval_error!(InvalidSizeOfArgsFor("-".to_string()))),
        },
        "*" => match exprs.as_slice() {
            [left, right] => Ok(eval(left)? * eval(right)?),
            _ => Err(eval_error!(InvalidSizeOfArgsFor("*".to_string()))),
        },
        "/" => match exprs.as_slice() {
            [left, right] => match eval(right)? {
                Value::Nil => Err(eval_error!(ZeroDivisor)),
                Value::Num(x) if x == 0. => Err(eval_error!(ZeroDivisor)),
                otherwise => Ok(eval(left)? / otherwise),
            },
            _ => Err(eval_error!(InvalidSizeOfArgsFor("/".to_string()))),
        },
        "," => match exprs.as_slice() {
            [left, right] => Ok(Value::Pair(Box::new(eval(left)?), Box::new(eval(right)?))),
            _ => Err(eval_error!(InvalidSizeOfArgsFor(",".to_string()))),
        },
        "?" => match exprs.as_slice() {
            [left, right] => {
                match (eval(left)?, eval(right)?) {
                    (Value::Bool(cond), Value::Pair(l, r)) => {
                        if cond { Ok(*l) } else { Ok(*r) }
                    }
                    (a, b) => Err(eval_error!(WrongTypes("?".to_string(), 
                            vec![Value::Bool(false), Value::Pair(
                                Box::new(Value::Nil), 
                                Box::new(Value::Nil))],
                            vec![a, b])))
                }
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor("?".to_string()))),
        }
        "<" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? < eval(right)?)),
            _ => Err(eval_error!(InvalidSizeOfArgsFor("<".to_string()))),
        },
        ">" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? > eval(right)?)),
            _ => Err(eval_error!(InvalidSizeOfArgsFor(">".to_string()))),
        },
        "<=" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? <= eval(right)?)),
            _ => Err(eval_error!(InvalidSizeOfArgsFor("<=".to_string()))),
        },
        ">=" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? >= eval(right)?)),
            _ => Err(eval_error!(InvalidSizeOfArgsFor(">=".to_string()))),
        },
        "==" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? == eval(right)?)),
            _ => Err(eval_error!(InvalidSizeOfArgsFor("==".to_string()))),
        },
        "!=" => match exprs.as_slice() {
            [left, right] => Ok(Value::Bool(eval(left)? != eval(right)?)),
            _ => Err(eval_error!(InvalidSizeOfArgsFor("!=".to_string()))),
        },
        "||" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a || b)),
                _ => Err(eval_error!(WrongTypes("||".to_string(), 
                            vec![Value::Bool(true), Value::Bool(true)],
                            vec![eval(left)?, eval(right)?]))),
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor("||".to_string()))),
        }
        "&&" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
                _ => Err(eval_error!(WrongTypes("&&".to_string(), 
                            vec![Value::Bool(true), Value::Bool(true)],
                            vec![eval(left)?, eval(right)?]))),
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor("&&".to_string()))),
        }
        ":" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (a, Value::Lambda(_, _, _)) => apply(eval(right)?, a, vars, rsrcs),
                (a, Value::Builtin(_)) => apply(eval(right)?, a, vars, rsrcs),
                _ => Err(eval_error!(WrongTypes(":".to_string(), 
                            vec![
                                Value::Nil, 
                                Value::Lambda(Pattern::Wildcard, Expression::Nil, hashmap!{})
                            ],
                            vec![eval(left)?, eval(right)?]))),
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor(":".to_string()))),
        }
        "::" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (Value::Environment(a), Value::Frozen(e)) => eval_expr(e, &a.union(vars.clone()), rsrcs),
                _ => Err(eval_error!(WrongTypes("::".to_string(), 
                            vec![
                                Value::Environment(hashmap!{}), 
                                Value::Frozen(Expression::Nil),
                            ],
                            vec![eval(left)?, eval(right)?]))),
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor("&&".to_string()))),
        }
        otherwise if vars.contains_key(&format!("<custom_operator>{otherwise}")) => {
            let op_key = format!("<custom_operator>{otherwise}");
            let op_def = match vars.get(&op_key) {
                Some(v) => v.clone(), 
                None => return Err(eval_error!(UnexpectedOperator(otherwise.to_string()))),
            };
            match exprs.as_slice() {
                [expr] => apply(
                    op_def.clone(),
                    eval(expr)?, 
                    vars,
                    rsrcs
                ),
                [left, right] => apply(
                    op_def.clone(),
                    Value::Pair(Box::new(eval(left)?), Box::new(eval(right)?)),
                    vars,
                    rsrcs
                ),
                _ => Err(eval_error!(InvalidSizeOfArgsFor(otherwise.to_string()))),
            }
        }
        _ => Err(eval_error!(UnexpectedOperator(format!("{op}")))),
    }
}
