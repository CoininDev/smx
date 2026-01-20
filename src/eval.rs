use crate::{ast::*, builtin::*, value::*, error::{*, EvalErrorType::*}, io::*};
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

pub fn mount_num(num: f64) -> EvalResult<NotNan<f64>> {
    NotNan::new(num).map_err(|e| eval_error!(NotNanError(e.to_string())))
}

pub fn eval_program_ambient(tree: Program) -> EvalResult<Ambient> {
    let mut rsrcs = HashMap::new();
    for res in &tree.body {
        eval_resource(res, &mut rsrcs)?;
    }

    let mut amb = Ambient{vars: HashMap::new(), rsrcs, natives: vec![]};
    for assign in tree.body {
        eval_assign(assign, &mut amb)?;
    }

    return Ok(amb);
}

pub fn eval_program(tree: Program) -> EvalResult<Value> {
    let vars = eval_program_ambient(tree)?.vars;

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

    
    let mut amb = Ambient{vars: HashMap::new(), rsrcs: resources.clone(), natives:vec![]};
    for res in res.1.clone() {
        match resources.get(&res) {
            _ if is_builtin_res(res.as_str()) => amb.vars.insert(res.clone(), Value::Builtin(res.into())),
            Some(m) => amb.vars.insert(res, m.clone()),
            _ => return Err(eval_error!(VariableDoesNotExists(res))),
        };
    }
    let value = eval_expr(res.2.clone(), &mut amb)?;
    resources.insert(name, value);
    Ok(())
}

pub fn eval_assign(a: Assign, amb: &mut Ambient) -> EvalResult<()> {
    if let Expression::Var(m) = &a.0 {
        if m[0] == "__RESOURCE__" {
            return Ok(());
        }
    }

    let pat = eval_pattern(a.0)?;
    
    let mut amb2 = amb.clone();
    for res in a.1.clone() {
        match amb2.rsrcs.get(&res) {
            _ if is_builtin_res(res.as_str()) => amb2.vars.insert(res.clone(), Value::Builtin(res.into())),
            Some(m) => amb2.vars.insert(res, m.clone()),
            _ => return Err(eval_error!(VariableDoesNotExists(res))),
        };
    }

    let value = eval_expr(a.2, &mut amb2).map_err(|e| EvalError{
        errtype: e.errtype, 
        assign: Some(pat.to_string())
    })?;

    for res in a.1 {
        amb2.vars.remove(&res);
    }
    
    #[cfg(debug_assertions)]
    println!("eval_assign adding: {pat} = {value}");
    amb2.vars.extend(eval_pattern_pair(pat, value)?.into_iter());
    *amb = amb2;
    Ok(())
}
fn is_builtin_res(name: &str) -> bool {
    vector![
        "IO"
    ].contains(&name)
}

pub fn eval_expr(e: Expression, amb: &mut Ambient) -> EvalResult<Value> {
    match e {
        Expression::Var(v) => {
            match v.as_slice() {
                [one] => {
                    if let Some(_) = builtin_registry().into_iter().filter(|a| (*a).matches(&one)).next() {
                        return Ok(Value::Builtin(one.into()))
                    }

                    match amb.vars.get(one) {
                        Some(i) => Ok(i.clone()),
                        None    => Err(eval_error!(VariableDoesNotExists(format!("{one}")))),
                    }
                },
                _ => {
                    if is_builtin_res(&v[0]) {
                        return Ok(Value::Builtin(v.join(".")));
                    }

                    let a = match amb.vars.get(&v[0]) {
                        Some(Value::Environment(env)) => Ok(env.clone()),
                        Some(_) => Err(eval_error!(
                                VariableDoesNotExists(format!("{} of type env", v[0])))
                        ),
                        None=> Err(eval_error!(VariableDoesNotExists(format!("{}", v[0])))),
                    }?;

                    eval_expr(Expression::Var(v[1..].to_vec()), &mut Ambient{
                        vars: a, 
                        rsrcs: amb.rsrcs.clone(), 
                        natives: amb.natives.clone()
                    })
                }
            }
            
        }
        Expression::OpSigVar(OpSig::Prefix(v))|
        Expression::OpSigVar(OpSig::Infix(v))=> {
            match amb.vars.get(&format!("<operator>{v}")) {
                Some(i) => Ok(i.clone()),
                None    => Err(eval_error!(VariableDoesNotExists(format!("{v}")))),
            }
        },
        Expression::Num(i)    => Ok(Value::Num(i)),
        Expression::Str(s)    => Ok(Value::Str(s)),
        Expression::Nil       => Ok(Value::Nil),
        Expression::Frozen(m) => Ok(Value::Frozen(*m)),
        Expression::Environment(e) => {
            let mut amb2: Ambient = Ambient::default();
            for ass in e {
                eval_assign(ass, &mut amb2)?;
            }
            Ok(Value::Environment(amb2.vars))
        } 
        Expression::Bool(b)   => Ok(Value::Bool(b)),
        Expression::Operation(op, exprs) => eval_operation(op, exprs, amb),
        Expression::Lambda(param, body) => Ok(Value::Lambda(
                eval_pattern(*param)?, *body, amb.vars.clone())
        ),

        Expression::Application(f, x) => apply(
            eval_expr(*f, amb)?, 
            eval_expr(*x, amb)?, 
            amb
        ),
        Expression::ListType(_) => Err(eval_error!(PatternError(
                    "The [1, 2, 3] syntax is just valid in types, please, use (1, 2, 3) instead".into()
        )))
    }
}

pub fn eval_pattern_type(ty: &Expression) -> EvalResult<PatternType> {
    fn listing (a: Expression) -> Result<im::Vector<PatternType>, String> {
        match a {
            Expression::Operation(op, xs) if op == "|" => match xs.as_slice() {
                [left, right] => Ok(
                    vector![eval_pattern_type(left).map_err(|e| format!("left error: {e}"))?] + listing(right.clone())?
                ),
                _ => Err("dhuah".into())
            }

            _ => Ok(vector![eval_pattern_type(&a).map_err(|e| format!("{e}"))?]),
        }
    }

    match ty {
        Expression::Var(v) if v.len() == 1 => match v[0].as_str() {
            "nil"     => Ok(PatternType::Nil),
            "pattern" => Ok(PatternType::Pattern),
            "bool"    => Ok(PatternType::Bool),
            "number"  => Ok(PatternType::Number),
            "string"  => Ok(PatternType::String),
            "env"     => Ok(PatternType::Environment),
            "frozen"  => Ok(PatternType::Frozen),
            other     => Err(eval_error!(PatternError(format!("unknown type name: {other}")))),
        }
        Expression::ListType(Some(box x)) => {
            let list: Vec<_> = listing(x.clone()).map_err(|e| eval_error!(PatternError(e)))?.into_iter().collect();
            Ok(PatternType::List(list))
        },

        Expression::ListType(None) => Ok(PatternType::List(vec![])),
        _ => Err(eval_error!(PatternError(format!("invalid pattern expression: {ty:?}"))))
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

        Expression::Operation(op, exprs) if op == "~" => match exprs.as_slice() {
                [left, right] => Ok(Pattern::TypedName(left.to_string(), eval_pattern_type(right)?)),
                _ => Err(eval_error!(InvalidSizeOfArgsFor("~".to_string()))),
        },

        Expression::Var(v) if matches!(v.as_slice(), [x] if x == "_") => Ok(Pattern::Wildcard),
        Expression::Var(v) if v.len() == 1 => Ok(Pattern::Name(v[0].clone())),
        

        //used in custom operators definition only
        Expression::OpSigVar(OpSig::Prefix(x)) |
        Expression::OpSigVar(OpSig::Infix(x))  => Ok(Pattern::Name(format!("<custom_operator>{x}"))),
        other => Ok(Pattern::Value(Box::new(eval_expr(other, &mut Ambient::default())?))),
    }
}


// Ok, i give up, i'm using AI code on this one i dont care.
pub fn eval_pattern_pair(pat: Pattern, val: Value) -> Result<Environment, EvalError> {
    /// ONLY checks if a value matches a PatternType. 
    /// Does NOT modify the environment.
    // fn check_multiple_types(tys: &Vec<PatternType>, value: &Value) -> Result<(), String> {
    //     for ty in tys {
    //
    //     }
    // }

    fn check_type(ty: &PatternType, value: &Value) -> Result<(), String> {
        match (ty, value) {
            (PatternType::Nil, Value::Nil) => Ok(()),
            (PatternType::String, Value::Str(_)) => Ok(()),
            (PatternType::Bool, Value::Bool(_)) => Ok(()),
            (PatternType::Environment, Value::Environment(_)) => Ok(()),
            (PatternType::Number, Value::Num(_)) => Ok(()),
            (PatternType::Frozen, Value::Frozen(_)) => Ok(()),
            (PatternType::List(types), _) => {
                let elements = value.pair_to_vec();
                // If the type is [string], we check if EVERY element is a string.
                // Note: If your language uses [type1, type2] as a schema, 
                // you would check lengths and zip them here.
                if types.len() == 1 {
                    let expected_ty = &types[0];
                    for el in elements {
                        check_type(expected_ty, &el)?;
                    }
                } else if types.len() == 0 {
                    return match value {
                        Value::Pair(_, _) => Ok(()),
                        _ => Err(String::from("this is not a list"))
                    };
                } else {
                    for (e, t) in elements.iter().map(|x| (x, types)) {
                        // check_multiple_types(t, e)?;
                        let res = t.iter().fold(false, |acc, ty| acc || check_type(ty, e).is_ok());
                        let types_names = t.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(" | ");
                        match res {
                            true  => Ok(()),
                            false => Err(format!("Type mismatch: Expected one of [{types_names}], but found {e}"))
                        }?
                    }
                }

                Ok(())
            }
            (a, b) => Err(format!("Type Mismatch: expected {}, got {}", a, b)),
        }
    }

    /// Handles binding a name to a value after validating its type.
    fn typing(x: String, ty: &PatternType, value: Value, acc: Environment) -> Result<Environment, String> {
        check_type(ty, &value)?;
        // After validation passes, bind the ENTIRE value to the name x
        Ok(acc.update(x, value))
    }

    /// The main recursive pattern matcher.
    fn rec(pat: Pattern, val: Value, acc: Environment) -> Result<Environment, String> {
        match (pat, val) {
            // Case: n = value
            (Pattern::Name(x), value) => 
                Ok(acc.update(x, value)),
            
            // Case: n ~ type = value
            (Pattern::TypedName(x, ty), value) => 
                typing(x, &ty, value, acc),
            
            // Case: (a, b) = (1, 2)
            (Pattern::Pair(k1, k2), Value::Pair(v1, v2)) => {
                let env1 = rec(*k1, *v1, HashMap::new())?;
                let env2 = rec(*k2, *v2, acc)?;
                Ok(env1.union(env2))
            },
            
            // Case: 5 = 5 (Literal match)
            (Pattern::Value(k), v) if *k == v => Ok(acc),
            
            // Case: _ = value
            (Pattern::Wildcard, _) => Ok(acc),
            
            (a, b) => Err(format!("Pattern match failed: {} does not match {}", a, b)),
        }
    }

    rec(pat, val, HashMap::new()).map_err(|x| eval_error!(PatternError(x)))
}

pub fn apply_builtin(x: &str, arg: Value, amb: &mut Ambient) -> EvalResult<Value> {
    let io_resource_imported = matches!(amb.vars.get("IO"), Some(Value::Builtin(x)) if x == "IO");
    
    let bi = builtin_registry().into_iter().filter(|b| b.matches(x)).next();
    if let Some(ci) = bi {
        return ci.call(arg, amb);
    }

    if x.split('.').next() == Some("IO") && io_resource_imported {
        let fnames: Vec<String> = x.split('.').map(|f| f.to_string()).collect();
        let io = IoResource;
        if fnames.len() == 2 {
            return io.redirect(fnames.last().unwrap().to_string(), arg, amb);
        }
        return io.objects(fnames[1].clone(), fnames[2..].to_vec(), arg, amb)
    }

    Err(eval_error!(VariableDoesNotExists(x.to_string())))
}

pub fn apply(func: Value, arg: Value, amb: &mut Ambient) -> EvalResult<Value> {
    let func_clone = func.clone();
    let (param, body, cap_env) = match func {
        Value::Lambda(param, body, cap_env) => (param, body, cap_env),
        Value::Builtin(x) => return apply_builtin(&x, arg, amb),
        _ => return Err(eval_error!(NonFunctionApplication(func))),
    };
    let env_pat = eval_pattern_pair(param, arg)?;
    let env_pat = env_pat.union(hashmap!{"__self".into() => func_clone});
    let vars2 = env_pat.clone().union(cap_env);
    #[cfg(debug_assertions)]
    println!("apply env: {vars2:#?}");
    amb.vars.extend(vars2);
    let res = eval_expr(body, amb);
    amb.eject_vars(&env_pat);
    res
}

pub fn eval_operation(op: String, exprs: Vec<Expression>, amb: &mut Ambient) 
    -> EvalResult<Value> {  
    let amb_clone = amb.clone();
    let mut eval = |x: &Expression| -> EvalResult<Value> {
        eval_expr(x.clone(), amb)
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
                            PatternType::List(vec![PatternType::Bool, PatternType::List(vec![])]),
                            Value::Pair(Box::new(a), Box::new(b))))),
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
                            PatternType::List(vec![PatternType::Bool]),
                            Value::Pair(Box::new(eval(left)?), Box::new(eval(right)?))))),
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor("||".to_string()))),
        }
        "&&" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
                _ => Err(eval_error!(WrongTypes("&&".to_string(), 
                            PatternType::List(vec![PatternType::Bool]),
                            Value::Pair(Box::new(eval(left)?), Box::new(eval(right)?))))),
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor("&&".to_string()))),
        }
        ":" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (a, Value::Lambda(_, _, _)) => apply(eval(right)?, a, amb),
                (a, Value::Builtin(_)) => apply(eval(right)?, a, amb),
                _ => Err(eval_error!(WrongTypes(":".to_string(), 
                            PatternType::List(vec![PatternType::Lambda]),
                            Value::Pair(Box::new(eval(left)?), Box::new(eval(right)?))))),
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor(":".to_string()))),
        }
        "::" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (Value::Environment(a), Value::Frozen(e)) => eval_expr(e, 
                    &mut Ambient {
                        vars: a.union(amb.vars.clone()), 
                        rsrcs: amb.rsrcs.clone(), 
                        natives: amb.natives.clone()
                    }
                ),
                _ => Err(eval_error!(WrongTypes("::".to_string(), 
                            PatternType::List(vec![PatternType::Environment, PatternType::Frozen]),
                            Value::Pair(Box::new(eval(left)?), Box::new(eval(right)?))))),
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor("&&".to_string()))),
        }
        otherwise if amb_clone.vars.contains_key(&format!("<custom_operator>{otherwise}")) => {
            let op_key = format!("<custom_operator>{otherwise}");
            let op_def = match amb_clone.vars.get(&op_key) {
                Some(v) => v.clone(), 
                None => return Err(eval_error!(UnexpectedOperator(otherwise.to_string()))),
            };
            match exprs.as_slice() {
                [expr] => apply(
                    op_def.clone(),
                    eval(expr)?, 
                    amb
                ),
                [left, right] => apply(
                    op_def.clone(),
                    Value::Pair(Box::new(eval(left)?), Box::new(eval(right)?)),
                    amb
                ),
                _ => Err(eval_error!(InvalidSizeOfArgsFor(otherwise.to_string()))),
            }
        }
        _ => Err(eval_error!(UnexpectedOperator(format!("{op}")))),
    }
}
