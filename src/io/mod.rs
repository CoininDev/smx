use crate::{
    ast::*,
    error::{EvalErrorType::*, *},
    eval::*,
    lexer::*,
    value::*,
};
use im::hashmap;
use rand::RngExt;
use std::io::Write;
use std::time::Instant;
mod file;
mod net;

macro_rules! eval_error {
    ($err: expr) => {
        EvalError::new($err)
    };
}

pub trait IoObject {
    fn redirect(&self, function: Vec<String>, value: Value, amb: &mut Ambient)
    -> EvalResult<Value>;
    fn name(&self) -> &str;
}

pub struct IoResource;
impl IoResource {
    pub fn redirect(&self, function: String, value: Value, amb: &mut Ambient) -> EvalResult<Value> {
        match function.as_str() {
            "print" => self.print(value),
            "read" => self.read(value),
            "import_as_env" => self.import_as_env(value),
            "import" => self.import(value, amb),
            "import_smxlib" => self.import_smxlib(value, amb),
            "run" => self.run(value),
            "random" => self.random(value),
            "time" => self.time(value, amb),
            _ => Err(eval_error!(VariableDoesNotExists(function))),
        }
    }

    pub fn objects(
        &self,
        obj: String,
        redirect: Vec<String>,
        value: Value,
        amb: &mut Ambient,
    ) -> EvalResult<Value> {
        fn n(a: impl IoObject + 'static) -> Box<dyn IoObject> {
            Box::new(a)
        }

        vec![n(file::FileIoObj), n(net::NetIoObj)]
            .into_iter()
            .filter(|x| obj == x.name())
            .next()
            .ok_or(eval_error!(VariableDoesNotExists(obj)))?
            .redirect(redirect, value, amb)
    }

    pub fn print(&self, arg: Value) -> EvalResult<Value> {
        match arg {
            Value::Str(message) => {
                println!("{message}");
                Ok(Value::Nil)
            }
            other => Err(eval_error!(WrongTypes(
                "IO.print".into(),
                PatternType::String,
                other
            ))),
        }
    }

    pub fn read(&self, arg: Value) -> EvalResult<Value> {
        match arg {
            Value::Str(prefix) => {
                print!("{prefix}");
                std::io::stdout().flush().unwrap();
                let mut buf = String::new();
                match std::io::stdin().read_line(&mut buf) {
                    // remove the \n at the end
                    Ok(_) => {
                        buf.pop();
                        Ok(Value::Str(buf))
                    }
                    Err(e) => Ok(Value::Environment(
                        hashmap! {"read_failed".into() =>Value::Str(e.to_string())},
                    )),
                }
            }
            other => Err(eval_error!(WrongTypes(
                "IO.read".into(),
                PatternType::String,
                other
            ))),
        }
    }

    pub fn import_as_env(&self, arg: Value) -> EvalResult<Value> {
        match arg {
            Value::Str(name) => {
                let content = match std::fs::read_to_string(name) {
                    Ok(t) => t,
                    Err(e) => {
                        return Ok(Value::Environment(hashmap! {
                            "import_as_env_failed".into() => Value::Str(e.to_string())
                        }));
                    }
                };
                let content = format!("{{{content}}}");
                util_eval_expr_str(&content, &Ambient::default())
                    .map_err(|s| eval_error!(GenericError(s)))
            }
            other => Err(eval_error!(WrongTypes(
                "IO.import_as_env".into(),
                PatternType::String,
                other
            ))),
        }
    }

    pub fn import(&self, arg: Value, amb: &mut Ambient) -> EvalResult<Value> {
        fn env_error() -> EvalResult<Value> {
            Err(eval_error!(GenericError(format!(
                "
                expected env for IO.import with:
                file ~ string *
                skip_underscored ~ bool
            "
            ))))
        }

        match arg {
            Value::Environment(env) => {
                let file = match env.get("file") {
                    Some(Value::Str(x)) => x,
                    _ => return env_error(),
                };

                let skip_underscored = match env.get("skip_underscored") {
                    Some(Value::Bool(x)) => x.clone(),
                    _ => false,
                };

                let content = match std::fs::read_to_string(file) {
                    Ok(t) => t,
                    Err(e) => {
                        return Ok(Value::Environment(hashmap! {
                            "import_failed".into() => Value::Str(e.to_string())
                        }));
                    }
                };

                let mut new_amb: Ambient = util_eval_program_ambient_str(&content)
                    .map_err(|u| eval_error!(GenericError(u)))?;

                if skip_underscored {
                    let va = new_amb
                        .vars
                        .into_iter()
                        .filter(|(k, _)| !k.starts_with("_"))
                        .collect::<im::HashMap<String, Value>>();

                    new_amb.vars = va;
                }

                amb.extend(&new_amb);

                Ok(Value::Nil)
            }
            other => Err(eval_error!(WrongTypes(
                "IO.import".into(),
                PatternType::String,
                other
            ))),
        }
    }

    pub fn import_smxlib(&self, _: Value, amb: &mut Ambient) -> EvalResult<Value> {
        match std::env::var("SMXLIB_PATH") {
            Ok(var) => self.import(Value::Environment(hashmap! {"file".into() => Value::Str(format!("{var}/smx.smx")), "skip_underscored".into() => Value::Bool(true)}), amb),
            Err(cu) => Err(eval_error!(GenericError(format!("SMXLIB_PATH: {}", cu)))),
        }
    }

    pub fn run(&self, arg: Value) -> EvalResult<Value> {
        fn crop_newline(mut s: String) -> String {
            if s.ends_with("\n") {
                s.pop();
            }
            s
        }

        match arg {
            Value::Str(command) => {
                let mut cmd = std::process::Command::new("bash");
                cmd.arg("-c").arg(command);
                let output = cmd
                    .output()
                    .map_err(|e| eval_error!(GenericError(e.to_string())))?;
                Ok(Value::Environment(hashmap! {
                    "stdout".into() => Value::Str(crop_newline(String::from_utf8_lossy(&output.stdout).to_string())),
                    "stderr".into() => Value::Str(crop_newline(String::from_utf8_lossy(&output.stderr).to_string())),
                    "status".into() => Value::Num(mount_num(output.status.code().unwrap_or(0).into()).unwrap()),
                }))
            }
            other => Err(eval_error!(WrongTypes(
                "IO.run".into(),
                PatternType::String,
                other
            ))),
        }
    }

    pub fn random(&self, arg: Value) -> EvalResult<Value> {
        fn error() -> EvalResult<Value> {
            Err(eval_error!(GenericError(String::from(
                r"
            expected for IO.random env with:
                min ~ number
                max ~ number 
                integer ~ bool
            "
            ))))
        }

        match arg {
            Value::Environment(env) => {
                let min = match env.get("min") {
                    Some(&Value::Num(a)) => a,
                    _ => mount_num(0f64).unwrap(),
                };

                let max = match env.get("max") {
                    Some(&Value::Num(a)) => a,
                    _ => mount_num(1f64).unwrap(),
                };

                let integer = match env.get("integer") {
                    Some(&Value::Bool(a)) => a,
                    _ => false,
                };
                let mut rng = rand::rng();

                if integer {
                    let min_i = min.into_inner().floor() as i64;
                    let max_i = max.into_inner().floor() as i64;

                    if min_i >= max_i {
                        return Err(eval_error!(GenericError(
                            "IO.random: min must be less than max".into()
                        )));
                    }

                    let i = rng.random_range(min_i..max_i);
                    match mount_num(i as f64) {
                        Ok(num) => return Ok(Value::Num(num)),
                        Err(a) => return Err(eval_error!(GenericError(a.to_string()))),
                    }
                }

                if min >= max {
                    return Err(eval_error!(GenericError(
                        "IO.random: min must be less than max".into()
                    )));
                }

                let i = rng.random_range(min.into_inner()..max.into_inner());
                match mount_num(i) {
                    Ok(num) => return Ok(Value::Num(num)),
                    Err(a) => return Err(eval_error!(GenericError(a.to_string()))),
                }
            }
            _ => error(),
        }
    }

    pub fn time(&self, arg: Value, amb: &mut Ambient) -> EvalResult<Value> {
        match arg {
            Value::Frozen(exp) => {
                let start = Instant::now();
                let res = eval_expr(exp, amb)?;
                let time = start.elapsed();
                Ok(Value::Environment(hashmap!{
                    "secs".into() => Value::Num(mount_num(time.as_secs_f64()).unwrap()),
                    "result".into() => res,
                }))
            }
            other => Err(eval_error!(WrongTypes(
                "IO.time".into(),
                PatternType::Frozen,
                other
            )))
        }
    }
}

pub fn util_eval_program_ambient_str(input: &str) -> Result<Ambient, String> {
    let tks = Lexer::new(input)
        .map(|res| res.map_err(|e| e.to_string()))
        .collect::<Result<Vec<Token>, String>>()?;
    let program = Parser::new(tks)
        .parse_program()
        .map_err(|e| e.to_string())?;

    eval_program_ambient(program).map_err(|e| e.to_string())
}

pub fn util_eval_expr_str(input: &str, amb: &Ambient) -> Result<Value, String> {
    let tks = Lexer::new(input)
        .map(|res| res.map_err(|e| e.to_string()))
        .collect::<Result<Vec<Token>, String>>()?;
    let expr = Parser::new(tks)
        .parse_expr_pratt(0.)
        .map_err(|e| e.to_string())?;

    eval_expr(
        expr,
        &mut Ambient {
            vars: amb.vars.clone(),
            rsrcs: amb.rsrcs.clone(),
            natives: vec![],
        },
    )
    .map_err(|e| e.to_string())
}
