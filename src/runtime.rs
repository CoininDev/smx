use crate::{value::*, lexer::*, eval::*, ast::*, error::EvalErrorType::*, error::*};
use std::fs::File;
use std::io::prelude::*;
use im::hashmap;

macro_rules! eval_error {
    ($err: expr) => {
        EvalError::new($err)
    }
}

pub struct IoResource;
impl IoResource {
    pub fn redirect(&self, function: String, value: Value) -> EvalResult<Value> {
        match function.as_str() {
            "print" => self.print(value),
            "read"  => self.read(value),
            "mkdir" => self.mkdir(value),
            "rmdir" => self.rmdir(value),
            "read_file"  => self.read_file(value),
            "write_file" => self.write_file(value),
            _ => Err(eval_error!(VariableDoesNotExists(function)))
        }
    }

    //stdout
    pub fn print(&self, arg: Value) -> EvalResult<Value> {
        match arg {
            Value::Str(message) => {
                println!("{message}");
                Ok(Value::Nil)
            }
            other => Err(eval_error!(WrongTypes("IO.print".into(), PatternType::String, other))),
        }
    }

    pub fn read(&self, arg: Value) -> EvalResult<Value> {
        match arg {
            Value::Str(prefix) => {
                print!("{prefix}");
                let mut buf = String::new();
                match std::io::stdin().read_line(&mut buf) {
                    // remove the \n at the end
                    Ok(_) => {buf.pop(); Ok(Value::Str(buf))},
                    Err(e) => Ok(Value::Environment(hashmap!{"read_failed".into() =>Value::Str(e.to_string())})),
                }
            }
            other => Err(eval_error!(WrongTypes("IO.read".into(), PatternType::String, other))),
        }
    }
    
    // FS
    pub fn read_file(&self, arg: Value) -> EvalResult<Value> {
        match arg {
            Value::Str(file) => match std::fs::read_to_string(file) {
                Ok(t) => Ok(Value::Str(t)),
                Err(e) => Ok(Value::Environment(hashmap!{"read_file_failed".into() => Value::Str(e.to_string())})),
            }
            other => Err(eval_error!(WrongTypes("IO.read_file".into(), PatternType::String, other))),
        }
    }

    pub fn write_file(&self, arg: Value) -> EvalResult<Value> {
        match arg {
            Value::Pair(box Value::Str(file), box Value::Str(content)) => {
                let mut file = match File::create(file.as_str()) {
                    Ok(a) => a,
                    Err(e) => return Ok(Value::Environment(
                        hashmap!{"write_file_error".into() => Value::Str(e.to_string())}
                    )),
                };

                match file.write_all(&content.clone().into_bytes()) {
                    Ok(_) => Ok(Value::Environment(
                        hashmap!{
                            "write_file_success".into() => Value::Bool(true),
                            "content".into() => Value::Str(content)
                        }
                    )),
                    Err(e) => Ok(Value::Environment(
                        hashmap!{"write_file_error".into() => Value::Str(e.to_string())}
                    )),
                }
            }
            other => Err(eval_error!(WrongTypes("IO.write_file".into(), PatternType::String, other))),
        }
    }

    pub fn mkdir(&self, arg: Value) -> EvalResult<Value> {
        match arg {
            Value::Str(name) => match std::fs::create_dir(name.clone()) {
                Ok(_) => Ok(Value::Environment(
                    hashmap!{
                        "mkdir_success".into() => Value::Bool(true),
                        "dir_name".into() => Value::Str(name),
                    }
                )),
                Err(e) => Ok(Value::Environment(
                    hashmap!{"mkdir_error".into() => Value::Str(e.to_string())}
                )),        
            }
            other => Err(eval_error!(WrongTypes("IO.mkdir".into(), PatternType::String, other))),
        }
    }

    pub fn rmdir(&self, arg: Value) -> EvalResult<Value> {
        match arg {
            Value::Str(name) => 
            match std::fs::remove_dir(name.clone()) {
                Ok(_) => Ok(Value::Environment(
                    hashmap!{
                        "rmdir_success".into() => Value::Bool(true),
                        "dir_name".into() => Value::Str(name),
                    }
                )),
                Err(e) => Ok(Value::Environment(
                    hashmap!{"rmdir_error".into() => Value::Str(e.to_string())}
                )),
            }
            other => Err(eval_error!(WrongTypes("IO.mkdir".into(), PatternType::String, other))),
        }
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

pub fn builtin_use(env: Environment, frozen: Expression, vars: &Environment, rsrcs: &Environment) -> Value {
    eval_expr(frozen, &env.union(vars.clone()), rsrcs).unwrap_or(Value::Nil)
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
