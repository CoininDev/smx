use crate::{ast::{*, NumericType}, builtin::*, value::*, error::{*, EvalErrorType::*}, io::*};
use im::HashMap;
use im::hashmap;
use im::vector;
use ordered_float::NotNan;
use std::str::FromStr;
use num_bigint::{BigInt, BigUint};

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

    let mut amb = Ambient{vars: HashMap::new(), rsrcs, natives: vec![], custom_resources: vec![]};
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

    
    let mut amb = Ambient {vars: HashMap::new(), rsrcs: resources.clone(), natives:vec![], custom_resources: vec![]};
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
pub fn eval_assign_imut(a: Assign, amb: &Ambient) -> EvalResult<Ambient> {
    if let Expression::Var(m) = &a.0 {
        if m[0] == "__RESOURCE__" {
            return Ok(Ambient::default());
        }
        if m.len() == 2 && m[0] == "__TYPE__" {
            let ty = eval_pattern_type(&a.2, amb)?;
            let mut r = Ambient::default();
            r.vars.insert(m[1].clone(), Value::Type(ty));
            return Ok(r);
        }
    }

    let pat = eval_pattern(a.0, amb)?;
    
    let mut amb2 = amb.clone();
    let mut added_resources = vec![];
    for res in a.1.clone() {
        if !amb2.vars.contains_key(&res) {
            added_resources.push(res.clone());
            match amb2.rsrcs.get(&res) {
                _ if is_builtin_res(res.as_str()) => amb2.vars.insert(res.clone(), Value::Builtin(res.into())),
                Some(m) => amb2.vars.insert(res, m.clone()),
                _ => return Err(eval_error!(VariableDoesNotExists(res))),
            };
        }
    }

    let value = eval_expr(a.2, &mut amb2).map_err(|e| EvalError{
        errtype: e.errtype, 
        assign: Some(pat.to_string())
    })?;

    for res in added_resources {
        amb2.vars.remove(&res);
    }
    
    #[cfg(debug_assertions)]
    println!("eval_assign adding: {pat} = {value}");
    let mut r = Ambient::default();
    r.vars.extend(eval_pattern_pair(pat, value)?.into_iter());
    Ok(r)
}
pub fn eval_assign(a: Assign, amb: &mut Ambient) -> EvalResult<()> {
    if let Expression::Var(m) = &a.0 {
        if m[0] == "__RESOURCE__" {
            return Ok(());
        }
        if m.len() == 2 && m[0] == "__TYPE__" {
            let ty = eval_pattern_type(&a.2, amb)?;
            amb.vars.insert(m[1].clone(), Value::Type(ty));
            return Ok(());
        }
    }

    let pat = eval_pattern(a.0, amb)?;
    
    let mut amb2 = amb.clone();
    let mut added_resources = vec![];
    for res in a.1.clone() {
        if !amb2.vars.contains_key(&res) {
            added_resources.push(res.clone());
            match amb2.rsrcs.get(&res) {
                _ if is_builtin_res(res.as_str()) => amb2.vars.insert(res.clone(), Value::Builtin(res.into())),
                Some(m) => amb2.vars.insert(res, m.clone()),
                _ => return Err(eval_error!(VariableDoesNotExists(res))),
            };
        }
    }

    let mut value = eval_expr(a.2, &mut amb2).map_err(|e| EvalError{
        errtype: e.errtype, 
        assign: Some(pat.to_string())
    })?;

    // Propagation of resources to the lambda
    if let Value::Lambda(p, b, e, mut r) = value {
        for res in &a.1 {
            if !r.contains(res) {
                r.push(res.clone());
            }
        }
        value = Value::Lambda(p, b, e, r);
    }

    for res in added_resources {
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
                        natives: amb.natives.clone(),
                        custom_resources: amb.custom_resources.clone()
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
        Expression::StrictNum(t, s) => {
            let val = match t {
                NumericType::F8 | NumericType::F16 | NumericType::F32 | NumericType::F64 | NumericType::F128 | NumericType::F256 => {
                    NumericValue::Float(NotNan::new(s.parse::<f64>().map_err(|e| eval_error!(GenericError(e.to_string())))?).unwrap())
                }
                NumericType::I8 | NumericType::I16 | NumericType::I32 | NumericType::I64 | NumericType::I128 | NumericType::I256 => {
                    NumericValue::Int(s.parse::<BigInt>().map_err(|e| eval_error!(GenericError(e.to_string())))?)
                }
                NumericType::U8 | NumericType::U16 | NumericType::U32 | NumericType::U64 | NumericType::U128 | NumericType::U256 => {
                    NumericValue::Uint(s.parse::<BigUint>().map_err(|e| eval_error!(GenericError(e.to_string())))?)
                }
            };
            Ok(Value::StrictNum(t, val))
        }
        Expression::Str(s)    => Ok(Value::Str(s)),
        Expression::Nil       => Ok(Value::Nil),
        Expression::Frozen(m) => Ok(Value::Frozen(*m)),
        Expression::Environment(e) => {
            let mut amb2: Ambient = amb.clone();
            let mut new_amb: Ambient = Ambient::default();
            for ass in e {
                let aaa = eval_assign_imut(ass, &amb2)?;
                new_amb.extend(&aaa);
                amb2.extend(&aaa);
            }
            Ok(Value::Environment(new_amb.vars))
        } 
        Expression::Bool(b)   => Ok(Value::Bool(b)),
        Expression::Operation(op, exprs) => eval_operation(op, exprs, amb),
        Expression::Lambda(param, body, resources) => {
            let mut cap_env = amb.vars.clone();
            for r in &resources {
                cap_env.remove(r);
            }
            Ok(Value::Lambda(
                eval_pattern(*param, amb)?, *body, cap_env, resources)
            )
        },

        Expression::Application(f, x) => apply(
            eval_expr(*f, amb)?, 
            eval_expr(*x, amb)?, 
            amb
        ),
        Expression::ListType(_) => Err(eval_error!(PatternError(
                    "The [1, 2, 3] syntax is just valid in types, please, use (1, 2, 3) instead".into()
        ))),
        Expression::TypeAlias(_name, expr) => {
            // This is handled in eval_assign, but if called directly:
            let ty = eval_pattern_type(&expr, amb)?;
            Ok(Value::Type(ty))
        }
    }
}

pub fn eval_pattern_type(ty: &Expression, amb: &Ambient) -> EvalResult<PatternType> {
    fn listing(a: Expression, amb: &Ambient) -> Result<im::Vector<PatternType>, String> {
        match a {
            Expression::Operation(op, xs) if op == "|" => match xs.as_slice() {
                [left, right] => Ok(
                    vector![eval_pattern_type(left, amb).map_err(|e| format!("left error: {e}"))?] + listing(right.clone(), amb)?
                ),
                _ => Err("dhuah".into())
            }

            _ => Ok(vector![eval_pattern_type(&a, amb).map_err(|e| format!("{e}"))?]),
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
            "fn"      => Ok(PatternType::Lambda),
            other     => {
                if let Ok(t) = NumericType::from_str(other) {
                    Ok(PatternType::StrictNumber(t))
                } else {
                    if let Some(Value::Type(t)) = amb.vars.get(other) {
                        return Ok(t.clone());
                    }
                    Err(eval_error!(PatternError(format!("unknown type name: {other}"))))
                }
            }
        }
        Expression::Environment(body) => {
            let mut schema = vec![];
            for Assign(id, _, expr) in body {
                match id {
                    Expression::Var(v) if v.len() == 1 => {
                        let name = v[0].clone();
                        let ty = if let Expression::Nil = expr {
                            PatternType::Any
                        } else {
                            eval_pattern_type(expr, amb)?
                        };
                        schema.push((name, ty));
                    }
                    Expression::Operation(op, xs) if op == "~" => {
                        match xs.as_slice() {
                            [Expression::Var(v), ty_expr] if v.len() == 1 => {
                                schema.push((v[0].clone(), eval_pattern_type(ty_expr, amb)?));
                            }
                            _ => return Err(eval_error!(PatternError(format!("Invalid schema entry: {}", id))))
                        }
                    }
                    _ => return Err(eval_error!(PatternError(format!("Invalid schema entry: {}", id))))
                }
            }
            Ok(PatternType::EnvironmentWithSchema(schema))
        }
        Expression::ListType(Some(box x)) => {
            let list: Vec<_> = listing(x.clone(), amb).map_err(|e| eval_error!(PatternError(e)))?.into_iter().collect();
            Ok(PatternType::List(list))
        },

        Expression::ListType(None) => Ok(PatternType::List(vec![])),
        Expression::Nil => Ok(PatternType::Nil),
        _ => Err(eval_error!(PatternError(format!("invalid pattern expression: {ty:?}"))))
    }
}

pub fn eval_pattern(input: Expression, amb: &Ambient) -> EvalResult<Pattern> {
    let eval = |x: &Expression| -> EvalResult<Pattern> {
        eval_pattern(x.clone(), amb)
    };
    
    match input {
        Expression::Operation(op, exprs) if op == "," => match exprs.as_slice() {
                [left, right] => Ok(Pattern::Pair(Box::new(eval(left)?), Box::new(eval(right)?))),
                _ => Err(eval_error!(InvalidSizeOfArgsFor(",".to_string()))),
        },

        Expression::Operation(ref op, ref exprs) if op == "~" => match exprs.as_slice() {
            [Expression::Var(v), ty] if v.len() == 1 => {
                Ok(Pattern::TypedName(v[0].clone(), eval_pattern_type(ty, amb)?))
            }
            _ => Err(eval_error!(PatternError(format!("Invalid typed name: {:?}", input))))
        },
        Expression::Operation(ref op, ref exprs) if op == "#" => {
            match eval_operation(op.clone(), exprs.clone(), &mut amb.clone())? {
                Value::Pattern(p) => Ok(p),
                other => Ok(Pattern::Value(Box::new(other))),
            }
        }

        Expression::Var(v) if matches!(v.as_slice(), [x] if x == "_") => Ok(Pattern::Wildcard),
        Expression::Var(v) if v.len() == 1 => Ok(Pattern::Name(v[0].clone())),
        

        //used in custom operators definition only
        Expression::OpSigVar(OpSig::Prefix(x)) |
        Expression::OpSigVar(OpSig::Infix(x))  => Ok(Pattern::Name(format!("<custom_operator>{x}"))),

        Expression::Environment(body) => {
            let mut schema = vec![];
            for Assign(id, _, expr) in body {
                match id {
                    Expression::Var(v) if v.len() == 1 => {
                        let name = v[0].clone();
                        let pat = if let Expression::Nil = expr {
                            Pattern::Name(name.clone())
                        } else {
                            eval_pattern(expr, amb)?
                        };
                        schema.push((name, pat));
                    }
                    Expression::Operation(ref op, ref xs) if op == "~" => {
                        match xs.as_slice() {
                            [Expression::Var(v), _] if v.len() == 1 => {
                                let name = v[0].clone();
                                let pat = eval_pattern(id, amb)?;
                                schema.push((name, pat));
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            Ok(Pattern::Environment(schema))
        }

        other => Ok(Pattern::Value(Box::new(eval_expr(other, &mut Ambient::default())?))),
    }
}


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
            (PatternType::Any, _) => Ok(()),
            (PatternType::Nil, Value::Nil) => Ok(()),
            (PatternType::String, Value::Str(_)) => Ok(()),
            (PatternType::Bool, Value::Bool(_)) => Ok(()),
            (PatternType::Environment, Value::Environment(_)) => Ok(()),
            (PatternType::EnvironmentWithSchema(schema), Value::Environment(env)) => {
                for (name, expected_ty) in schema {
                    if let Some(val) = env.get(name) {
                        check_type(expected_ty, val)?;
                    } else {
                        return Err(format!("Environment missing required field: {}", name));
                    }
                }
                Ok(())
            }
            (PatternType::Number, Value::Num(_)) => Ok(()),
            (PatternType::StrictNumber(t), Value::StrictNum(vt, _)) if t == vt => Ok(()),
            (PatternType::Frozen, Value::Frozen(_)) => Ok(()),
            (PatternType::Lambda, Value::Lambda(_, _, _, _)) | (PatternType::Lambda, Value::Builtin(_)) => Ok(()),
            (PatternType::List(types), _) => {
                if let Value::Nil = value {
                    return Ok(());
                }
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
            
            (Pattern::Environment(schema), Value::Environment(env)) => {
                let mut current_acc = acc;
                for (name, pat) in schema {
                    if let Some(val) = env.get(&name) {
                        current_acc = rec(pat, val.clone(), current_acc)?;
                    } else {
                        return Err(format!("Environment missing required field: {}", name));
                    }
                }
                Ok(current_acc)
            },
            
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

    if let Some(prefix) = x.split('.').next() {
        if prefix == "IO" && io_resource_imported {
            let fnames: Vec<String> = x.split('.').map(|f| f.to_string()).collect();
            let io = IoResource { custom_resources: amb.custom_resources.clone() };
            if fnames.len() == 2 {
                return io.redirect(fnames.last().unwrap().to_string(), arg, amb);
            }
            return io.objects(fnames[1].clone(), fnames[2..].to_vec(), arg, amb);
        } else {
            // Check custom resources
            let custom = amb.custom_resources.clone();
            for res in &custom {
                if res.name() == prefix {
                    let fnames: Vec<String> = x.split('.').skip(1).map(|s| s.to_string()).collect();
                    return res.redirect(fnames, arg, amb);
                }
            }
        }
    }

    Err(eval_error!(VariableDoesNotExists(x.to_string())))
}
pub fn apply(func: Value, arg: Value, amb: &mut Ambient) -> EvalResult<Value> {
    let func_clone = func.clone();
    let (param, body, cap_env, resources) = match func {
        Value::Lambda(param, body, cap_env, resources) => (param, body, cap_env, resources),
        Value::Builtin(x) => return apply_builtin(&x, arg, amb),
        _ => return Err(eval_error!(NonFunctionApplication(func))),
    };
    let env_pat = eval_pattern_pair(param, arg)?;
    let env_pat = env_pat.union(hashmap!{"__self".into() => func_clone});
    
    let mut vars2 = cap_env;
    for r in resources {
        if let Some(val) = amb.vars.get(&r) {
            vars2.insert(r.clone(), val.clone());
        } else {
            return Err(eval_error!(ResourceNotProvided(r)));
        }
    }
    vars2.extend(env_pat);
    
    let mut new_amb = Ambient {
        vars: vars2.clone(),
        rsrcs: amb.rsrcs.clone(),
        natives: amb.natives.clone(),
        custom_resources: amb.custom_resources.clone(),
    };
    
    let res = eval_expr(body, &mut new_amb);

    // Sync back variables that were NOT in the original local_bindings (parameters/captures)
    // and were added to new_amb.vars during eval_expr (e.g. by IO.import)
    for (k, v) in new_amb.vars {
        if !vars2.contains_key(&k) {
            amb.vars.insert(k, v);
        }
    }
    
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
            [expr] => Ok(builtin_pattern_from_value(eval(expr)?, amb)),
            _ => Err(eval_error!(InvalidSizeOfArgsFor("#".to_string()))),
        },
        "!" => match exprs.as_slice() {
            [expr] => Ok(!eval(expr)?),
            _ => Err(eval_error!(InvalidSizeOfArgsFor("!".to_string()))),
        },
        "+" => match exprs.as_slice() {
            [expr] => eval(expr),
            [left, right] => {
                let l = eval(left)?;
                let r = eval(right)?;
                match (&l, &r) {
                    (Value::StrictNum(t1, _), Value::StrictNum(t2, _)) if t1 != t2 => {
                        Err(eval_error!(GenericError(format!("Strict type mismatch: {:?} and {:?}", t1, t2))))
                    }
                    _ => Ok(l + r)
                }
            },
            _ => Err(eval_error!(InvalidSizeOfArgsFor("+".to_string()))),
        },
        "-" => match exprs.as_slice() {
            [expr] => Ok(-(eval(expr)?)),
            [left, right] => {
                let l = eval(left)?;
                let r = eval(right)?;
                match (&l, &r) {
                    (Value::StrictNum(t1, _), Value::StrictNum(t2, _)) if t1 != t2 => {
                        Err(eval_error!(GenericError(format!("Strict type mismatch: {:?} and {:?}", t1, t2))))
                    }
                    _ => Ok(l - r)
                }
            },
            _ => Err(eval_error!(InvalidSizeOfArgsFor("-".to_string()))),
        },
        "*" => match exprs.as_slice() {
            [left, right] => {
                let l = eval(left)?;
                let r = eval(right)?;
                match (&l, &r) {
                    (Value::StrictNum(t1, _), Value::StrictNum(t2, _)) if t1 != t2 => {
                        Err(eval_error!(GenericError(format!("Strict type mismatch: {:?} and {:?}", t1, t2))))
                    }
                    _ => Ok(l * r)
                }
            },
            _ => Err(eval_error!(InvalidSizeOfArgsFor("*".to_string()))),
        },
        "/" => match exprs.as_slice() {
            [left, right] => {
                let l = eval(left)?;
                let r = eval(right)?;
                match (&l, &r) {
                    (_, Value::Nil) => Err(eval_error!(ZeroDivisor)),
                    (_, Value::Num(x)) if *x == 0. => Err(eval_error!(ZeroDivisor)),
                    (Value::StrictNum(t1, _), Value::StrictNum(t2, _)) if t1 != t2 => {
                        Err(eval_error!(GenericError(format!("Strict type mismatch: {:?} and {:?}", t1, t2))))
                    }
                    _ => Ok(l / r)
                }
            },
            _ => Err(eval_error!(InvalidSizeOfArgsFor("/".to_string()))),
        },
        "**" => match exprs.as_slice() {
            [left, right] => {
                let l = eval(left)?;
                let r = eval(right)?;
                match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => Ok(Value::Num(mount_num(a.powf(**b))?)),
                    _ => Err(eval_error!(WrongTypes("**".into(), 
                            PatternType::Number,
                            Value::Pair(Box::new(l), Box::new(r))))),
                }
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor("**".to_string()))),
        },
        "%" => match exprs.as_slice() {
            [left, right] => {
                let l = eval(left)?;
                let r = eval(right)?;
                match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => Ok(Value::Num(mount_num(**a % **b)?)),
                    _ => Err(eval_error!(WrongTypes("%".into(), 
                            PatternType::Number,
                            Value::Pair(Box::new(l), Box::new(r))))),
                }
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor("%".to_string()))),
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
                (a, Value::Lambda(_, _, _, _)) => apply(eval(right)?, a, amb),
                (a, Value::Builtin(_)) => apply(eval(right)?, a, amb),
                _ => Err(eval_error!(WrongTypes(":".to_string(), 
                            PatternType::List(vec![PatternType::Lambda]),
                            Value::Pair(Box::new(eval(left)?), Box::new(eval(right)?))))),
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor(":".to_string()))),
        }
        "::" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (Value::Environment(a), Value::Frozen(e)) => {
                    let mut new_vars = amb.vars.clone();
                    new_vars.extend(a);
                    eval_expr(e, 
                    &mut Ambient {
                        vars: new_vars, 
                        rsrcs: amb.rsrcs.clone(), 
                        natives: amb.natives.clone(),
                        custom_resources: amb.custom_resources.clone()
                    }
                )},
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
