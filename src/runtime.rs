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
    pub fn redirect(&self, function: String, value: Value, _vars: &Environment, _rsrcs: &Environment) -> EvalResult<Value> {
        match function.as_str() {
            "print" => self.print(value),
            "read"  => self.read(value),
            "mkdir" => self.mkdir(value),
            "rmdir" => self.rmdir(value),
            "read_file"  => self.read_file(value),
            "write_file" => self.write_file(value),
            "import_as_env" => self.import_as_env(value),
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
                    Err(e) => Ok(
                        Value::Environment(
                            hashmap!{"read_failed".into() =>Value::Str(e.to_string())}
                        )
                    ),
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

    pub fn import_as_env(&self, arg: Value) -> EvalResult<Value> {
        match arg {
            Value::Str(name) => {
                let content = match std::fs::read_to_string(name) {
                    Ok(t) => t,
                    Err(e) => return Ok(Value::Environment(
                            hashmap!{
                                "import_as_env_failed".into() => Value::Str(e.to_string())
                            }
                    )),
                };
                let content = format!("{{{content}}}");
                util_eval_expr_str(&content, &hashmap!{}, &hashmap!{})
                    .map_err(|s| eval_error!(GenericError(s)))
            }
            other => Err(eval_error!(WrongTypes("IO.mkdir".into(), PatternType::String, other))),
        }
    }

    pub fn import(&self, arg: Value, vars: &mut Environment, rsrcs: &mut Environment) -> EvalResult<Value> {
        fn error<T>(x: Option<T>) -> EvalResult<T> {
            x.ok_or(eval_error!(GenericError(String::from(
                "expected an env for IO.import, with variables:\n
                dont_import_underlined ~bool (opt)\n
                file_name ~string\n
                "
            ))))
        }

        match arg {
            Value::Environment(env) => {
                let file_name = match env.get("file_name") {
                    Some(Value::Str(s)) => Ok(s),
                    Some(_)  => error(None),
                    None => error(None)
                }?;

                let dont_import_underlined = env.get("dont_import_underlined")
                    .clone().unwrap_or(&Value::Bool(false));

                let dont_import_underlined = match dont_import_underlined {
                    Value::Bool(x) => x,
                    _ => return error(None)
                };

                let content = match std::fs::read_to_string(file_name) {
                    Ok(t) => t,
                    Err(e) => return Ok(Value::Environment(
                            hashmap!{
                                "import_failed".into() => Value::Str(e.to_string())
                            }
                    )),
                };

                let (mut vars2, mut rsrcs2) = util_eval_program_ambient_str(&content)
                    .map_err(|e| eval_error!(GenericError(e)))?;

                if *dont_import_underlined {
                    vars2 = vars2
                        .into_iter()
                        .filter(|(x, _)| x.starts_with("_"))
                        .collect();
                    rsrcs2 = rsrcs2
                        .into_iter()
                        .filter(|(x, _)| x.starts_with("_"))
                        .collect();
                }

                *vars = vars.clone().union(vars2);
                *rsrcs = rsrcs.clone().union(rsrcs2);
                
                Ok(Value::Nil)
            }

            _ => error(None)
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
                [Expression::Var(left), Expression::Var(_)]
                | [Expression::Var(left), Expression::ListType(_)]=> 
                    Pattern::TypedName(
                        left[0].clone(), 
                        eval_pattern_type(&xs[1]).unwrap_or(PatternType::Nil)
                    ),
                _ => Pattern::Wildcard,
            },
            other => Pattern::Value(Box::new(other)),
         }
    }
    Value::Pattern(rec(v))
}

pub fn util_eval_program_ambient_str(input:&str) -> Result<(Environment, Environment), String> {
    let tks = Lexer::new(input)
        .map(|res| res.map_err(|e| e.to_string()))
        .collect::<Result<Vec<Token>, String>>()?;
    let program = Parser::new(tks)
        .parse_program()
        .map_err(|e| e.to_string())?;

    eval_program_ambient(program).map_err(|e| e.to_string())
}

pub fn util_eval_expr_str(input: &str, vars: &Environment, rsrcs: &Environment) -> Result<Value, String> {
    let tks = Lexer::new(input)
        .map(|res| res.map_err(|e| e.to_string()))
        .collect::<Result<Vec<Token>, String>>()?;
    let expr = Parser::new(tks)
        .parse_expr_pratt(0.)
        .map_err(|e| e.to_string())?;

    eval_expr(expr, &mut vars.clone(), &mut rsrcs.clone()).map_err(|e| e.to_string())
}
