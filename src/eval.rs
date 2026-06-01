use crate::{
    ast::{NumericType, *},
    builtin::*,
    error::{EvalErrorType::*, *},
    io::*,
    value::*,
};
use im::hashmap;
use im::vector;
use im::HashMap;
use num_bigint::{BigInt, BigUint};
use ordered_float::NotNan;
use std::str::FromStr;

#[macro_export]
macro_rules! eval_error {
    ($err: expr) => {
        EvalError::new($err)
    };
}

pub type EvalResult<T> = Result<T, EvalError>;

pub fn mount_num(num: f64) -> EvalResult<NotNan<f64>> {
    NotNan::new(num).map_err(|e| eval_error!(NotNanError(e.to_string())))
}

pub fn eval_program_ambient(tree: Program) -> EvalResult<Ambient> {
    eval_program_ambient_with_initial(tree, Ambient::default())
}

pub fn eval_program_ambient_with_initial(tree: Program, mut amb: Ambient) -> EvalResult<Ambient> {
    for res in &tree.body {
        eval_resource(res, &mut amb.env.rsrcs)?;
    }
    for assign in tree.body {
        eval_assign(assign, &mut amb)?;
    }
    Ok(amb)
}

pub fn eval_program_with_ambient(tree: Program, amb: Ambient) -> EvalResult<Value> {
    let vars = eval_program_ambient_with_initial(tree, amb)?.env.vars;

    match vars.get("result".into()) {
        Some(a) => Ok(a.clone()),
        None => match vars.into_iter().last() {
            Some(a) => Ok(a.1),
            None => Ok(Value::Num(mount_num(0.)?)),
        },
    }
}

pub fn eval_program(tree: Program) -> EvalResult<Value> {
    let vars = eval_program_ambient(tree)?.env.vars;

    match vars.get("result".into()) {
        Some(a) => Ok(a.clone()),
        None => match vars.into_iter().last() {
            Some(a) => Ok(a.1),
            None => Ok(Value::Num(mount_num(0.)?)),
        },
    }
}

pub fn eval_resource(res: &Assign, resources: &mut HashMap<String, Value>) -> EvalResult<()> {
    let name = match &res.0.kind {
        ExprKind::Var(m) if m[0] == "__RESOURCE__" => m[1].clone(),
        _ => return Ok(()),
    };

    let mut amb = Ambient {
        env: Environment {
            rsrcs: resources.clone(),
            ..Default::default()
        },
        natives: vec![],
        custom_resources: vec![],
    };
    for res in res.1.clone() {
        match resources.get(&res) {
            _ if is_builtin_res(res.as_str()) => {
                amb.env.vars.insert(res.clone(), Value::Builtin(res.into()))
            }
            Some(m) => amb.env.vars.insert(res, m.clone()),
            _ => return Err(eval_error!(VariableDoesNotExists(res))),
        };
    }
    let value = eval_expr(res.2.clone(), &mut amb)?;
    resources.insert(name, value);
    Ok(())
}
pub fn eval_assign_imut(a: Assign, amb: &Ambient) -> EvalResult<Ambient> {
    if let ExprKind::Var(m) = &a.0.kind {
        if m[0] == "__RESOURCE__" {
            return Ok(Ambient::default());
        }
        if m.len() == 2 && m[0] == "__TYPE__" {
            let ty = eval_pattern_type(&a.2, amb)?;
            let mut r = Ambient::default();
            r.env.vars.insert(m[1].clone(), Value::Type(ty));
            return Ok(r);
        }
    }

    let pat = eval_pattern(a.0, amb)?;

    let mut amb2 = amb.clone();
    let mut added_resources = vec![];
    for res in a.1.clone() {
        if !amb2.env.vars.contains_key(&res) {
            added_resources.push(res.clone());
            match amb2.env.rsrcs.get(&res) {
                _ if is_builtin_res(res.as_str()) => amb2
                    .env
                    .vars
                    .insert(res.clone(), Value::Builtin(res.into())),
                Some(m) => amb2.env.vars.insert(res, m.clone()),
                _ => return Err(eval_error!(VariableDoesNotExists(res))),
            };
        }
    }

    let value = eval_expr(a.2, &mut amb2).map_err(|mut e| {
        e.assign = Some(pat.to_string());
        e
    })?;

    for res in added_resources {
        amb2.env.vars.remove(&res);
    }

    let mut r = Ambient::default();
    r.env
        .vars
        .extend(eval_pattern_pair(pat, value)?.into_iter());
    Ok(r)
}
pub fn eval_assign(a: Assign, amb: &mut Ambient) -> EvalResult<()> {
    if let ExprKind::OpSigVar(sig, assoc, prec) = &a.0.kind {
        eprintln!("[DEBUG] eval_assign: Adding operator {:?} with assoc={:?}, prec={}", sig, assoc, prec);
        amb.env.op_table.insert(sig.clone(), (*assoc, *prec));
        eprintln!("[DEBUG] eval_assign: op_table now has {} operators", amb.env.op_table.len());
    }

    if let ExprKind::Var(m) = &a.0.kind {
        if m[0] == "__RESOURCE__" {
            return Ok(());
        }
        if m.len() == 2 && m[0] == "__TYPE__" {
            let ty = eval_pattern_type(&a.2, amb)?;
            amb.env.vars.insert(m[1].clone(), Value::Type(ty));
            return Ok(());
        }
    }

    let pat = eval_pattern(a.0, amb)?;

    let mut amb2 = amb.clone();
    let mut added_resources = vec![];
    for res in a.1.clone() {
        if !amb2.env.vars.contains_key(&res) {
            added_resources.push(res.clone());
            match amb2.env.rsrcs.get(&res) {
                _ if is_builtin_res(res.as_str()) => amb2
                    .env
                    .vars
                    .insert(res.clone(), Value::Builtin(res.into())),
                Some(m) => amb2.env.vars.insert(res, m.clone()),
                _ => return Err(eval_error!(VariableDoesNotExists(res))),
            };
        }
    }

    let mut value = eval_expr(a.2, &mut amb2).map_err(|mut e| {
        e.assign = Some(pat.to_string());
        e
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
        amb2.env.vars.remove(&res);
    }

    amb2.env
        .vars
        .extend(eval_pattern_pair(pat, value)?.into_iter());
    *amb = amb2;
    Ok(())
}
fn is_builtin_res(name: &str) -> bool {
    vector!["IO"].contains(&name)
}

pub enum Thunk {
    Done(Value),
    Eval {
        expr: Expression,
        new_amb: Box<Ambient>,
        baseline_vars: HashMap<String, Value>,
    },
    Apply(Value, Value, Box<Ambient>),
    EvalRightOfComma(Value, Expression, Box<Ambient>),
}

pub fn resolve_thunk(mut thunk: Thunk, outer_amb: &mut Ambient) -> EvalResult<Value> {
    let mut accumulated_exports = HashMap::new();
    let mut comma_stack = Vec::new();
    loop {
        match thunk {
            Thunk::Done(val) => {
                if let Some(left_val) = comma_stack.pop() {
                    thunk = Thunk::Done(Value::Pair(Box::new(left_val), Box::new(val)));
                    continue;
                }
                for (k, v) in accumulated_exports {
                    outer_amb.env.vars.insert(k, v);
                }
                return Ok(val);
            }
            Thunk::Eval {
                expr,
                mut new_amb,
                baseline_vars,
            } => {
                thunk = eval_step(expr, &mut new_amb)?;
                for (k, v) in new_amb.env.vars.clone() {
                    if !baseline_vars.contains_key(&k) {
                        accumulated_exports.insert(k, v);
                    }
                }
            }
            Thunk::Apply(f, x, mut new_amb) => {
                thunk = apply_step(f, x, &mut new_amb)?;
            }
            Thunk::EvalRightOfComma(left_val, right_expr, mut right_amb) => {
                comma_stack.push(left_val);
                thunk = eval_step(right_expr, &mut right_amb)?;
            }
        }
    }
}

pub fn eval_expr(e: Expression, amb: &mut Ambient) -> EvalResult<Value> {
    let thunk = eval_step(e, amb)?;
    resolve_thunk(thunk, amb)
}

pub fn eval_step(e: Expression, amb: &mut Ambient) -> EvalResult<Thunk> {
    let span = e.span.clone();
    let res = match &e.kind {
        ExprKind::Application(f, x) => {
            return apply_step(
                eval_expr((**f).clone(), amb)?,
                eval_expr((**x).clone(), amb)?,
                amb,
            )
            .map_err(|err| err.with_span(span));
        }
        ExprKind::Operation(op, exprs) => {
            return eval_operation_step(op.clone(), exprs.clone(), amb)
                .map_err(|err| err.with_span(span));
        }
        _ => eval_expr_base(e, amb).map(Thunk::Done),
    };
    res.map_err(|err| err.with_span(span))
}

pub fn eval_operation_step(
    op: String,
    exprs: Vec<Expression>,
    amb: &mut Ambient,
) -> EvalResult<Thunk> {
    let op_clone = op.clone();
    let res = match op.as_str() {
        "," => match exprs.as_slice() {
            [left, right] => {
                let left_val = eval_expr(left.clone(), amb)?;
                Ok(Thunk::EvalRightOfComma(
                    left_val,
                    right.clone(),
                    Box::new(amb.clone()),
                ))
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor(",".to_string()))),
        },
        ":" => match exprs.as_slice() {
            [left, right] => match (
                eval_expr(left.clone(), amb)?,
                eval_expr(right.clone(), amb)?,
            ) {
                (a, Value::Lambda(_, _, _, _)) => {
                    apply_step(eval_expr(right.clone(), amb)?, a, amb)
                }
                (a, Value::Builtin(_)) => apply_step(eval_expr(right.clone(), amb)?, a, amb),
                _ => Err(eval_error!(WrongTypes(
                    ":".to_string(),
                    PatternType::List(vec![PatternType::Lambda]),
                    Value::Pair(
                        Box::new(eval_expr(left.clone(), amb)?),
                        Box::new(eval_expr(right.clone(), amb)?)
                    )
                ))),
            },
            _ => Err(eval_error!(InvalidSizeOfArgsFor(":".to_string()))),
        },
        _ => eval_operation(op.clone(), exprs, amb).map(Thunk::Done),
    };
    res.map_err(|mut e| {
        e.call_stack.push(op_clone);
        e
    })
}

pub fn apply_builtin_step(x: &str, arg: Value, amb: &mut Ambient) -> EvalResult<Thunk> {
    if x == "eval" {
        return match arg {
            Value::Frozen(frozen) => Ok(Thunk::Eval {
                expr: frozen,
                new_amb: Box::new(amb.clone()),
                baseline_vars: amb.env.vars.clone(),
            }),
            Value::Str(text) => crate::io::util_eval_expr_str(text.as_str(), amb)
                .map(Thunk::Done)
                .map_err(|e| eval_error!(GenericError(e.to_string()))),
            other => Err(eval_error!(WrongTypes(
                "eval".into(),
                PatternType::Frozen,
                other
            ))),
        };
    }
    apply_builtin(x, arg, amb).map(Thunk::Done)
}

pub fn apply_step(func: Value, arg: Value, amb: &mut Ambient) -> EvalResult<Thunk> {
    let func_clone = func.clone();
    let (param, body, cap_env, resources) = match func {
        Value::Lambda(param, body, cap_env, resources) => (param, body, cap_env, resources),
        Value::Builtin(x) => return apply_builtin_step(&x, arg, amb),
        _ => return Err(eval_error!(NonFunctionApplication(func))),
    };
    let env_pat = eval_pattern_pair(param, arg)?;
    let env_pat = env_pat.union(hashmap! {"__self".into() => func_clone});

    let Environment {
        vars: mut vars2,
        rsrcs,
        op_table,
    } = cap_env;

    for r in resources {
        if let Some(val) = amb.env.vars.get(&r) {
            vars2.insert(r.clone(), val.clone());
        } else {
            return Err(eval_error!(ResourceNotProvided(r)));
        }
    }

    vars2.extend(env_pat);

    let new_amb = Ambient {
        env: Environment {
            vars: vars2.clone(),
            rsrcs,
            op_table,
        },
        natives: amb.natives.clone(),
        custom_resources: amb.custom_resources.clone(),
    };

    Ok(Thunk::Eval {
        expr: body,
        new_amb: Box::new(new_amb),
        baseline_vars: vars2,
    })
}

pub fn eval_expr_base(e: Expression, amb: &mut Ambient) -> EvalResult<Value> {
    let span = e.span;
    let res = match e.kind {
        ExprKind::Var(v) => {
            match v.as_slice() {
                [one] => {
                    if let Some(_) = builtin_registry()
                        .into_iter()
                        .filter(|a| (*a).matches(&one))
                        .next()
                    {
                        return Ok(Value::Builtin(one.into()));
                    }

                    match amb.env.vars.get(one) {
                        Some(i) => Ok(i.clone()),
                        None => Err(eval_error!(VariableDoesNotExists(format!("{one}")))),
                    }
                }
                _ => {
                    if is_builtin_res(&v[0]) {
                        return Ok(Value::Builtin(v.join(".")));
                    }

                    // Check if v[0] is a custom resource or builtin that should be qualified
                    if let Some(Value::Builtin(_)) = amb.env.vars.get(&v[0]) {
                        return Ok(Value::Builtin(v.join(".")));
                    }

                    let a = match amb.env.vars.get(&v[0]) {
                        Some(Value::Environment(c)) => Ok(c.clone()),
                        Some(_) => Err(eval_error!(VariableDoesNotExists(format!(
                            "{} of type env",
                            v[0]
                        )))),
                        None => Err(eval_error!(VariableDoesNotExists(format!("{}", v[0])))),
                    }?;

                    eval_expr(
                        Expression::dummy(ExprKind::Var(v[1..].to_vec())),
                        &mut Ambient {
                            env: a,
                            natives: amb.natives.clone(),
                            custom_resources: amb.custom_resources.clone(),
                        },
                    )
                }
            }
        }
        ExprKind::OpSigVar(OpSig::Prefix(v), _, _) | ExprKind::OpSigVar(OpSig::Infix(v), _, _) => {
            match amb.env.vars.get(&format!("<operator>{v}")) {
                Some(i) => Ok(i.clone()),
                None => Err(eval_error!(VariableDoesNotExists(format!("{v}")))),
            }
        }
        ExprKind::Num(i) => Ok(Value::Num(i)),
        ExprKind::StrictNum(t, s) => {
            let val = match t {
                NumericType::F8
                | NumericType::F16
                | NumericType::F32
                | NumericType::F64
                | NumericType::F128
                | NumericType::F256 => NumericValue::Float(
                    NotNan::new(
                        s.parse::<f64>()
                            .map_err(|e| eval_error!(GenericError(e.to_string())))?,
                    )
                    .unwrap(),
                ),
                NumericType::I8
                | NumericType::I16
                | NumericType::I32
                | NumericType::I64
                | NumericType::I128
                | NumericType::I256 => NumericValue::Int(
                    s.parse::<BigInt>()
                        .map_err(|e| eval_error!(GenericError(e.to_string())))?,
                ),
                NumericType::U8
                | NumericType::U16
                | NumericType::U32
                | NumericType::U64
                | NumericType::U128
                | NumericType::U256 => NumericValue::Uint(
                    s.parse::<BigUint>()
                        .map_err(|e| eval_error!(GenericError(e.to_string())))?,
                ),
            };
            Ok(Value::StrictNum(t, val))
        }
        ExprKind::Str(s) => Ok(Value::Str(s)),
        ExprKind::Nil => Ok(Value::Nil),
        ExprKind::Frozen(m) => Ok(Value::Frozen(*m)),
        ExprKind::Environment(e) => {
            let mut amb2: Ambient = amb.clone();
            let mut new_amb: Ambient = Ambient::default();
            for ass in e {
                let aaa = eval_assign_imut(ass, &amb2)?;
                new_amb.extend(&aaa);
                amb2.extend(&aaa);
            }
            Ok(Value::Environment(new_amb.env))
        }
        ExprKind::Bool(b) => Ok(Value::Bool(b)),
        ExprKind::Operation(op, exprs) => eval_operation(op, exprs, amb),
        ExprKind::Lambda(param, body, resources) => {
            let mut cap_env = amb.env.clone();
            for r in &resources {
                cap_env.vars.remove(r);
            }
            Ok(Value::Lambda(
                eval_pattern(*param, amb)?,
                *body,
                cap_env,
                resources,
            ))
        }

        ExprKind::Application(f, x) => apply(eval_expr(*f, amb)?, eval_expr(*x, amb)?, amb),
        ExprKind::ListType(_) => Err(eval_error!(PatternError(
            "The [1, 2, 3] syntax is just valid in types, please, use (1, 2, 3) instead".into()
        ))),
        ExprKind::TypeAlias(_name, expr) => {
            // This is handled in eval_assign, but if called directly:
            let ty = eval_pattern_type(&expr, amb)?;
            Ok(Value::Type(ty))
        }
    };
    res.map_err(|e| e.with_span(span))
}

pub fn eval_pattern_type(ty: &Expression, amb: &Ambient) -> EvalResult<PatternType> {
    fn listing(a: Expression, amb: &Ambient) -> Result<im::Vector<PatternType>, String> {
        match a.kind {
            ExprKind::Operation(op, xs) if op == "|" => match xs.as_slice() {
                [left, right] => Ok(vector![
                    eval_pattern_type(left, amb).map_err(|e| format!("left error: {e}"))?
                ] + listing(right.clone(), amb)?),
                _ => Err("dhuah".into()),
            },

            _ => Ok(vector![
                eval_pattern_type(&a, amb).map_err(|e| format!("{e}"))?
            ]),
        }
    }

    match &ty.kind {
        ExprKind::Var(v) if v.len() == 1 => match v[0].as_str() {
            "nil" => Ok(PatternType::Nil),
            "pattern" => Ok(PatternType::Pattern),
            "bool" => Ok(PatternType::Bool),
            "number" => Ok(PatternType::Number),
            "string" => Ok(PatternType::String),
            "env" => Ok(PatternType::Environment),
            "frozen" => Ok(PatternType::Frozen),
            "fn" => Ok(PatternType::Lambda),
            other => {
                if let Ok(t) = NumericType::from_str(other) {
                    Ok(PatternType::StrictNumber(t))
                } else {
                    if let Some(Value::Type(t)) = amb.env.vars.get(other) {
                        return Ok(t.clone());
                    }
                    Err(eval_error!(PatternError(format!(
                        "unknown type name: {other}"
                    ))))
                }
            }
        },
        ExprKind::Environment(body) => {
            let mut schema = vec![];
            for Assign(id, _, expr) in body {
                match &id.kind {
                    ExprKind::Var(v) if v.len() == 1 => {
                        let name = v[0].clone();
                        let ty = if let ExprKind::Nil = expr.kind {
                            PatternType::Any
                        } else {
                            eval_pattern_type(&expr, amb)?
                        };
                        schema.push((name, ty));
                    }
                    ExprKind::Operation(op, xs) if op == "~" => match xs.as_slice() {
                        [Expression {
                            kind: ExprKind::Var(v),
                            ..
                        }, ty_expr]
                            if v.len() == 1 =>
                        {
                            schema.push((v[0].clone(), eval_pattern_type(ty_expr, amb)?));
                        }
                        _ => {
                            return Err(eval_error!(PatternError(format!(
                                "Invalid schema entry: {}",
                                id
                            ))))
                        }
                    },
                    _ => {
                        return Err(eval_error!(PatternError(format!(
                            "Invalid schema entry: {}",
                            id
                        ))))
                    }
                }
            }
            Ok(PatternType::EnvironmentWithSchema(schema))
        }
        ExprKind::ListType(Some(box x)) => {
            let list: Vec<_> = listing(x.clone(), amb)
                .map_err(|e| eval_error!(PatternError(e)))?
                .into_iter()
                .collect();
            Ok(PatternType::List(list))
        }

        ExprKind::ListType(None) => Ok(PatternType::List(vec![])),
        ExprKind::Nil => Ok(PatternType::Nil),
        _ => Err(eval_error!(PatternError(format!(
            "invalid pattern expression: {ty:?}"
        )))),
    }
}

pub fn eval_pattern(input: Expression, amb: &Ambient) -> EvalResult<Pattern> {
    let eval = |x: &Expression| -> EvalResult<Pattern> { eval_pattern(x.clone(), amb) };

    match input.kind {
        ExprKind::Operation(op, exprs) if op == "," => match exprs.as_slice() {
            [left, right] => Ok(Pattern::Pair(Box::new(eval(left)?), Box::new(eval(right)?))),
            _ => Err(eval_error!(InvalidSizeOfArgsFor(",".to_string()))),
        },

        ExprKind::Operation(ref op, ref exprs) if op == "~" => match exprs.as_slice() {
            [Expression {
                kind: ExprKind::Var(v),
                ..
            }, ty]
                if v.len() == 1 =>
            {
                Ok(Pattern::TypedName(
                    v[0].clone(),
                    eval_pattern_type(ty, amb)?,
                ))
            }

            _ => Err(eval_error!(PatternError(format!(
                "Invalid typed name: {:?}",
                input
            )))),
        },

        ExprKind::Operation(ref op, ref exprs) if op == "#" => {
            match eval_operation(op.clone(), exprs.clone(), &mut amb.clone())? {
                Value::Pattern(p) => Ok(p),
                other => Ok(Pattern::Value(Box::new(other))),
            }
        }

        ExprKind::Var(v) if matches!(v.as_slice(), [x] if x == "_") => Ok(Pattern::Wildcard),
        ExprKind::Var(v) if v.len() == 1 => Ok(Pattern::Name(v[0].clone())),

        //used in custom operators definition only
        ExprKind::OpSigVar(OpSig::Prefix(x), _, _) | ExprKind::OpSigVar(OpSig::Infix(x), _, _) => {
            Ok(Pattern::Name(format!("<custom_operator>{x}")))
        }

        ExprKind::Environment(body) => {
            let mut schema = vec![];
            for Assign(id, _, expr) in body {
                match id.kind {
                    ExprKind::Var(v) if v.len() == 1 => {
                        let name = v[0].clone();
                        let pat = if let ExprKind::Nil = expr.kind {
                            Pattern::Name(name.clone())
                        } else {
                            eval_pattern(expr, amb)?
                        };
                        schema.push((name, pat));
                    }
                    ExprKind::Operation(ref op, ref xs) if op == "~" => match xs.as_slice() {
                        [Expression {
                            kind: ExprKind::Var(v),
                            ..
                        }, _]
                            if v.len() == 1 =>
                        {
                            let name = v[0].clone();
                            let pat = eval_pattern(id, amb)?;
                            schema.push((name, pat));
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            Ok(Pattern::Environment(schema))
        }

        other => Ok(Pattern::Value(Box::new(eval_expr(
            Expression::dummy(other),
            &mut Ambient::default(),
        )?))),
    }
}

pub fn eval_pattern_pair(pat: Pattern, val: Value) -> Result<HashMap<String, Value>, EvalError> {
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
                    if let Some(val) = env.vars.get(name) {
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
            (PatternType::Lambda, Value::Lambda(_, _, _, _))
            | (PatternType::Lambda, Value::Builtin(_)) => Ok(()),
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
                        _ => Err(String::from("this is not a list")),
                    };
                } else {
                    for (e, t) in elements.iter().map(|x| (x, types)) {
                        // check_multiple_types(t, e)?;
                        let res = t
                            .iter()
                            .fold(false, |acc, ty| acc || check_type(ty, e).is_ok());
                        let types_names = t
                            .iter()
                            .map(|m| m.to_string())
                            .collect::<Vec<_>>()
                            .join(" | ");
                        match res {
                            true => Ok(()),
                            false => Err(format!(
                                "Type mismatch: Expected one of [{types_names}], but found {e}"
                            )),
                        }?
                    }
                }

                Ok(())
            }
            (a, b) => Err(format!("Type Mismatch: expected {}, got {}", a, b)),
        }
    }

    /// Handles binding a name to a value after validating its type.
    fn typing(
        x: String,
        ty: &PatternType,
        value: Value,
        acc: HashMap<String, Value>,
    ) -> Result<HashMap<String, Value>, String> {
        check_type(ty, &value)?;
        // After validation passes, bind the ENTIRE value to the name x
        Ok(acc.update(x, value))
    }

    /// The main recursive pattern matcher.
    fn rec(
        pat: Pattern,
        val: Value,
        acc: HashMap<String, Value>,
    ) -> Result<HashMap<String, Value>, String> {
        match (pat, val) {
            // Case: n = value
            (Pattern::Name(x), value) => Ok(acc.update(x, value)),

            // Case: n ~ type = value
            (Pattern::TypedName(x, ty), value) => typing(x, &ty, value, acc),

            // Case: (a, b) = (1, 2)
            (Pattern::Pair(k1, k2), Value::Pair(v1, v2)) => {
                let env1 = rec(*k1, *v1, HashMap::new())?;
                let env2 = rec(*k2, *v2, acc)?;
                Ok(env1.union(env2))
            }

            // Case: 5 = 5 (Literal match)
            (Pattern::Value(k), v) if *k == v => Ok(acc),

            // Case: _ = value
            (Pattern::Wildcard, _) => Ok(acc),

            (Pattern::Environment(schema), Value::Environment(env)) => {
                let mut current_acc = acc;
                for (name, pat) in schema {
                    if let Some(val) = env.vars.get(&name) {
                        current_acc = rec(pat, val.clone(), current_acc)?;
                    } else {
                        return Err(format!("Environment missing required field: {}", name));
                    }
                }
                Ok(current_acc)
            }

            (a, b) => Err(format!("Pattern match failed: {} does not match {}", a, b)),
        }
    }

    rec(pat, val, HashMap::new()).map_err(|x| eval_error!(PatternError(x)))
}

pub fn apply_builtin(x: &str, arg: Value, amb: &mut Ambient) -> EvalResult<Value> {
    let io_resource_imported =
        matches!(amb.env.vars.get("IO"), Some(Value::Builtin(x)) if x == "IO");

    let bi = builtin_registry()
        .into_iter()
        .filter(|b| b.matches(x))
        .next();
    let res = if let Some(ci) = bi {
        ci.call(arg, amb)
    } else if let Some(prefix) = x.split('.').next() {
        if prefix == "IO" && io_resource_imported {
            let fnames: Vec<String> = x.split('.').map(|f| f.to_string()).collect();
            let mut io = IoResource {
                custom_resources: amb.custom_resources.clone(),
            };
            if fnames.len() == 2 {
                io.redirect(fnames.last().unwrap().to_string(), arg, amb)
            } else {
                io.objects(fnames[1].clone(), fnames[2..].to_vec(), arg, amb)
            }
        } else {
            // Check custom resources
            let mut result = Err(eval_error!(VariableDoesNotExists(x.to_string())));
            let custom = amb.custom_resources.clone();
            for res in custom {
                if res.lock().unwrap().name() == prefix {
                    let fnames: Vec<String> = x.split('.').skip(1).map(|s| s.to_string()).collect();
                    result = res.lock().unwrap().redirect(fnames, arg, amb);
                    break;
                }
            }
            result
        }
    } else {
        Err(eval_error!(VariableDoesNotExists(x.to_string())))
    };

    res.map_err(|mut e| {
        e.call_stack.push(x.to_string());
        e
    })
}

pub fn apply(func: Value, arg: Value, amb: &mut Ambient) -> EvalResult<Value> {
    let func_clone = func.clone();
    let (param, body, cap_env, resources) = match func {
        Value::Lambda(param, body, cap_env, resources) => (param, body, cap_env, resources),
        Value::Builtin(x) => return apply_builtin(&x, arg, amb),
        _ => return Err(eval_error!(NonFunctionApplication(func))),
    };
    let env_pat = eval_pattern_pair(param, arg)?;
    let env_pat: HashMap<String, Value> = env_pat.union(hashmap! {"__self".into() => func_clone});

    let Environment {
        vars: mut vars2,
        rsrcs,
        op_table,
    } = cap_env;

    for r in resources {
        if let Some(val) = amb.env.vars.get(&r) {
            vars2.insert(r.clone(), val.clone());
        } else {
            return Err(eval_error!(ResourceNotProvided(r)));
        }
    }

    vars2.extend(env_pat);

    let mut new_amb = Ambient {
        env: Environment {
            vars: vars2.clone(),
            rsrcs,
            op_table,
        },
        natives: amb.natives.clone(),
        custom_resources: amb.custom_resources.clone(),
    };

    let res = eval_expr(body, &mut new_amb).map_err(|mut e| {
        e.call_stack.push("<lambda>".into());
        e
    });

    // Sync back variables that were NOT in the original local_bindings (parameters/captures)
    // and were added to new_amb.vars during eval_expr (e.g. by IO.import)
    for (k, v) in new_amb.env.vars {
        if !vars2.contains_key(&k) {
            amb.env.vars.insert(k, v);
        }
    }

    res
}

pub fn eval_operation(op: String, exprs: Vec<Expression>, amb: &mut Ambient) -> EvalResult<Value> {
    let op_clone = op.clone();
    let amb_clone = amb.clone();
    let mut eval = |x: &Expression| -> EvalResult<Value> { eval_expr(x.clone(), amb) };

    let res = match op.as_str() {
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
                        Err(eval_error!(GenericError(format!(
                            "Strict type mismatch: {:?} and {:?}",
                            t1, t2
                        ))))
                    }
                    _ => Ok(l + r),
                }
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor("+".to_string()))),
        },
        "-" => match exprs.as_slice() {
            [expr] => Ok(-(eval(expr)?)),
            [left, right] => {
                let l = eval(left)?;
                let r = eval(right)?;
                match (&l, &r) {
                    (Value::StrictNum(t1, _), Value::StrictNum(t2, _)) if t1 != t2 => {
                        Err(eval_error!(GenericError(format!(
                            "Strict type mismatch: {:?} and {:?}",
                            t1, t2
                        ))))
                    }
                    _ => Ok(l - r),
                }
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor("-".to_string()))),
        },
        "*" => match exprs.as_slice() {
            [left, right] => {
                let l = eval(left)?;
                let r = eval(right)?;
                match (&l, &r) {
                    (Value::StrictNum(t1, _), Value::StrictNum(t2, _)) if t1 != t2 => {
                        Err(eval_error!(GenericError(format!(
                            "Strict type mismatch: {:?} and {:?}",
                            t1, t2
                        ))))
                    }
                    _ => Ok(l * r),
                }
            }
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
                        Err(eval_error!(GenericError(format!(
                            "Strict type mismatch: {:?} and {:?}",
                            t1, t2
                        ))))
                    }
                    _ => Ok(l / r),
                }
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor("/".to_string()))),
        },
        "**" => match exprs.as_slice() {
            [left, right] => {
                let l = eval(left)?;
                let r = eval(right)?;
                match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => Ok(Value::Num(mount_num(a.powf(**b))?)),
                    _ => Err(eval_error!(WrongTypes(
                        "**".into(),
                        PatternType::Number,
                        Value::Pair(Box::new(l), Box::new(r))
                    ))),
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
                    _ => Err(eval_error!(WrongTypes(
                        "%".into(),
                        PatternType::Number,
                        Value::Pair(Box::new(l), Box::new(r))
                    ))),
                }
            }
            _ => Err(eval_error!(InvalidSizeOfArgsFor("%".to_string()))),
        },
        "," => match exprs.as_slice() {
            [left, right] => Ok(Value::Pair(Box::new(eval(left)?), Box::new(eval(right)?))),
            _ => Err(eval_error!(InvalidSizeOfArgsFor(",".to_string()))),
        },
        "?" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (Value::Bool(cond), Value::Pair(l, r)) => {
                    if cond {
                        Ok(*l)
                    } else {
                        Ok(*r)
                    }
                }
                (a, b) => Err(eval_error!(WrongTypes(
                    "?".to_string(),
                    PatternType::List(vec![PatternType::Bool, PatternType::List(vec![])]),
                    Value::Pair(Box::new(a), Box::new(b))
                ))),
            },
            _ => Err(eval_error!(InvalidSizeOfArgsFor("?".to_string()))),
        },
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
                _ => Err(eval_error!(WrongTypes(
                    "||".to_string(),
                    PatternType::List(vec![PatternType::Bool]),
                    Value::Pair(Box::new(eval(left)?), Box::new(eval(right)?))
                ))),
            },
            _ => Err(eval_error!(InvalidSizeOfArgsFor("||".to_string()))),
        },
        "&&" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
                _ => Err(eval_error!(WrongTypes(
                    "&&".to_string(),
                    PatternType::List(vec![PatternType::Bool]),
                    Value::Pair(Box::new(eval(left)?), Box::new(eval(right)?))
                ))),
            },
            _ => Err(eval_error!(InvalidSizeOfArgsFor("&&".to_string()))),
        },
        ":" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (a, Value::Lambda(_, _, _, _)) => apply(eval(right)?, a, amb),
                (a, Value::Builtin(_)) => apply(eval(right)?, a, amb),
                _ => Err(eval_error!(WrongTypes(
                    ":".to_string(),
                    PatternType::List(vec![PatternType::Lambda]),
                    Value::Pair(Box::new(eval(left)?), Box::new(eval(right)?))
                ))),
            },
            _ => Err(eval_error!(InvalidSizeOfArgsFor(":".to_string()))),
        },
        "::" => match exprs.as_slice() {
            [left, right] => match (eval(left)?, eval(right)?) {
                (Value::Environment(env), Value::Frozen(e)) => eval_expr(
                    e,
                    &mut Ambient {
                        env: Environment {
                            vars: env.vars.union(amb_clone.env.vars.clone()),
                            rsrcs: amb_clone.env.rsrcs.clone(),
                            op_table: amb_clone.env.op_table.clone(),
                        },
                        natives: amb_clone.natives.clone(),
                        custom_resources: amb_clone.custom_resources.clone(),
                    },
                ),
                _ => Err(eval_error!(WrongTypes(
                    "::".to_string(),
                    PatternType::List(vec![PatternType::Environment, PatternType::Frozen]),
                    Value::Pair(Box::new(eval(left)?), Box::new(eval(right)?))
                ))),
            },
            _ => Err(eval_error!(InvalidSizeOfArgsFor("&&".to_string()))),
        },
        otherwise
            if amb_clone
                .env
                .vars
                .contains_key(&format!("<custom_operator>{otherwise}")) =>
        {
            let op_key = format!("<custom_operator>{otherwise}");
            let op_def = match amb_clone.env.vars.get(&op_key) {
                Some(v) => v.clone(),
                None => return Err(eval_error!(UnexpectedOperator(otherwise.to_string()))),
            };
            match exprs.as_slice() {
                [expr] => apply(op_def.clone(), eval(expr)?, amb),
                [left, right] => apply(
                    op_def.clone(),
                    Value::Pair(Box::new(eval(left)?), Box::new(eval(right)?)),
                    amb,
                ),
                _ => Err(eval_error!(InvalidSizeOfArgsFor(otherwise.to_string()))),
            }
        }
        _ => Err(eval_error!(UnexpectedOperator(format!("{op}")))),
    };
    res.map_err(|mut e| {
        e.call_stack.push(op_clone);
        e
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Parser;
    use crate::tokenize;

    #[test]
    fn test_call_stack_on_error() {
        let code = "
            f = \\x. x + z;
            g = \\y. f y;
            result = g 10;
        ";
        let tokens = tokenize(code).unwrap();
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().unwrap();

        let err = eval_program(program).unwrap_err();

        // Call stack should contain two <lambda> entries
        assert_eq!(err.call_stack.len(), 2);
        assert_eq!(err.call_stack[0], "<lambda>");
        assert_eq!(err.call_stack[1], "<lambda>");
    }

    #[test]
    fn test_builtin_call_stack() {
        // Test an error inside a builtin or something that calls a builtin
        let code = "result = 1 / 0;";
        let tokens = tokenize(code).unwrap();
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().unwrap();

        let err = eval_program(program).unwrap_err();
        assert_eq!(err.call_stack.len(), 1);
        assert_eq!(err.call_stack[0], "/");
    }
}
