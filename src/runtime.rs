use crate::{value::*, lexer::*, eval::*, ast::*};
use im::hashmap;


pub struct IoResource;
impl IoResource {
    pub fn print(message: String) -> Value {
        print!("{message}");
        Value::Nil
    }

    pub fn read(prefix: String) -> Value {
        println!("{prefix}");
        let mut buf = String::new();
        match std::io::stdin().read_line(&mut buf) {
            Ok(_) => Value::Str(buf),
            Err(e) => Value::Environment(hashmap!{"read_failed".into() =>Value::Str(e.to_string())}),
        }
    }

    pub fn read_file(file: String) -> Value {
        match std::fs::read_to_string(file) {
            Ok(t) => Value::Str(t),
            Err(e) => Value::Environment(hashmap!{"read_file_failed".into() => Value::Str(e.to_string())}),
        }
    }
}


// BUILTIN FUNCTIONS
pub fn builtin_try(func: Value, arg: Value, vars: &Environment, rsrcs: &Environment) -> Value {
    match apply(func, arg, vars, rsrcs) {
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
            Value::Frozen(Expression::Var(x)) => match x.as_slice() {
                [w] if w == "_" => Pattern::Wildcard,
                [any] => Pattern::Name(any.into()),
                _ => Pattern::Wildcard,
            },
            Value::Frozen(Expression::Operation(op, xs)) if op == "~" => match xs.as_slice() {
                [Expression::Var(left), Expression::Var(_)] => 
                    Pattern::TypedName(
                        left[0].clone(), 
                        eval_pattern_type(&xs[1])
                            .map_err(|_| PatternType::Nil).unwrap()
                    ),
                _ => Pattern::Wildcard,
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

pub fn builtin_use(env: Environment, frozen: Expression, vars: &Environment, rsrcs: &Environment) -> Value {
    eval_expr(frozen, &env.union(vars.clone()), rsrcs).unwrap_or(Value::Nil)
}

pub fn builtin_eval(frozen: Expression, vars: &Environment, rsrcs: &Environment) -> Value {
    eval_expr(frozen, vars, rsrcs).unwrap_or(Value::Nil)
}



pub fn util_eval_expr_str(input: &str, vars: &Environment, rsrcs: &Environment) -> Result<Value, String> {
    let tks = Lexer::new(input)
        .map(|res| res.map_err(|e| e.to_string()))
        .collect::<Result<Vec<Token>, String>>()?;
    let expr = Parser::new(tks)
        .parse_expr_pratt(0.)
        .map_err(|e| e.to_string())?;

    eval_expr(expr, vars, rsrcs).map_err(|e| e.to_string())
}
