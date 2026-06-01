use crate::{
    ast::*,
    error::{EvalErrorType::*, *},
    eval::*,
    lexer::*,
    value::*,
};

pub use im::hashmap;

use std::io::Write;
use rand::RngExt;
use std::thread::sleep;
use ordered_float::NotNan;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::time::Duration;

mod file;
mod net;

macro_rules! eval_error {
    ($err: expr) => {
        EvalError::new($err)
    };
}

pub struct IoResource {
    pub custom_resources: Vec<Arc<Mutex<dyn IoObject + Send>>>,
}

impl Default for IoResource {
    fn default() -> Self {
        Self { custom_resources: vec![] }
    }
}
impl IoResource {
    pub fn redirect(&mut self, function: String, value: Value, amb: &mut Ambient) -> EvalResult<Value> {
        match function.as_str() {
            "print" => self.print(value),
            "read" => self.read(value),
            "import_as_env" => self.import_as_env(value),
            "import" => self.import(value, amb),
            "import_smxlib" => self.import_smxlib(value, amb),
            "run" => self.run(value),
            "random" => self.random(value),
            "time" => self.time(value, amb),
            "wait" => self.wait(value),
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
        fn n(a: impl IoObject + Send + 'static) -> Arc<Mutex<dyn IoObject + Send>> {
            Arc::new(Mutex::new(a))
        }

        let mut all_objects = self.custom_resources.clone();
        all_objects.extend(vec![n(file::FileIoObj), n(net::NetIoObj)]);

        all_objects
            .into_iter()
            .filter(|x| obj == x.lock().unwrap().name())
            .next()
            .ok_or(eval_error!(VariableDoesNotExists(obj)))?
            .lock()
            .unwrap()
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
                    Err(e) => Ok(Value::Environment(Environment {
                        vars:hashmap! {"read_failed".into() =>Value::Str(e.to_string())},
                        ..Default::default()
                    })),
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
                let content = match std::fs::read_to_string(&name) {
                    Ok(t) => t,
                    Err(e) => {
                        return Ok(Value::Environment(Environment {
                        vars:hashmap! {
                            "import_as_env_failed".into() => Value::Str(e.to_string())
                        }, ..Default::default()}));
                    }
                };
                let content = format!("{{{content}}}");
                util_eval_expr_str(&content, &Ambient::default())
                    .map_err(|err| {
                        let (mut e, span) = match err {
                            SmxError::Eval(e) => (e, None),
                            SmxError::Parsing(p) => (EvalError::new(GenericError(p.errtype.to_string())), p.span),
                            SmxError::Lexer(l) => (EvalError::new(GenericError(l.errtype.to_string())), Some(l.span)),
                            _ => (EvalError::new(GenericError(err.to_string())), None),
                        };
                        e.call_stack.push(format!("file: {name}"));
                        if e.source_code.is_none() {
                            e.source_code = Some(content.clone());
                            if let Some(s) = span {
                                e.span = Some(s);
                            }
                        }
                        e
                    })
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
                let file = match env.vars.get("file") {
                    Some(Value::Str(x)) => x,
                    _ => return env_error(),
                };

                let skip_underscored = match env.vars.get("skip_underscored") {
                    Some(Value::Bool(x)) => x.clone(),
                    _ => false,
                };

                let content = match std::fs::read_to_string(&file) {
                    Ok(t) => t,
                    Err(e) => {
                        return Ok(Value::Environment(
                            Environment {
                        vars:hashmap! {
                            "import_failed".into() => Value::Str(e.to_string())
                        }, ..Default::default()}));
                    }
                };

                let mut new_amb: Ambient = util_eval_program_ambient_with_op_table(&content, &amb.env.op_table)
                    .map_err(|err| {
                        let (mut e, span) = match err {
                            SmxError::Eval(e) => (e, None),
                            SmxError::Parsing(p) => (EvalError::new(GenericError(p.errtype.to_string())), p.span),
                            SmxError::Lexer(l) => (EvalError::new(GenericError(l.errtype.to_string())), Some(l.span)),
                            _ => (EvalError::new(GenericError(err.to_string())), None),
                        };
                        e.call_stack.push(format!("file: {file}"));
                        if e.source_code.is_none() {
                            e.source_code = Some(content.clone());
                            if let Some(s) = span {
                                e.span = Some(s);
                            }
                        }
                        e
                    })?;

                if skip_underscored {
                    let va = new_amb
                        .env
                        .vars
                        .into_iter()
                        .filter(|(k, _)| !k.starts_with("_"))
                        .collect::<im::HashMap<String, Value>>();

                    new_amb.env.vars = va;
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
            Ok(var) => self.import(Value::Environment(Environment {
                        vars: hashmap! {"file".into() => Value::Str(format!("{var}/smx.smx")), "skip_underscored".into() => Value::Bool(true)}
                        ,..Default::default()}), amb),
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
                Ok(Value::Environment(Environment {
                        vars:hashmap! {
                    "stdout".into() => Value::Str(crop_newline(String::from_utf8_lossy(&output.stdout).to_string())),
                    "stderr".into() => Value::Str(crop_newline(String::from_utf8_lossy(&output.stderr).to_string())),
                    "status".into() => Value::Num(mount_num(output.status.code().unwrap_or(0).into()).unwrap()),
                }, ..Default::default()}))
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
                let min = match env.vars.get("min") {
                    Some(&Value::Num(a)) => a,
                    _ => mount_num(0f64).unwrap(),
                };

                let max = match env.vars.get("max") {
                    Some(&Value::Num(a)) => a,
                    _ => mount_num(1f64).unwrap(),
                };

                let integer = match env.vars.get("integer") {
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
                Ok(Value::Environment(Environment {
                        vars:hashmap!{
                    "secs".into() => Value::Num(mount_num(time.as_secs_f64()).unwrap()),
                    "result".into() => res,
                }, ..Default::default()}))
            }
            other => Err(eval_error!(WrongTypes(
                "IO.time".into(),
                PatternType::Frozen,
                other
            )))
        }
    }

    pub fn wait(&self, arg: Value) -> EvalResult<Value> {
        match arg {
            Value::Num(n) => {
                let time = n.into_inner();
                sleep(Duration::from_secs_f64(time));
                return Ok(Value::Environment(Environment {
                        vars:
                    hashmap!{"ok".into() => Value::Bool(true)
                    },..Default::default()}));
            }
            other => Err(eval_error!(WrongTypes(
                "IO.wait".into(),
                PatternType::Number,
                other
            )))
        }
    }
}

pub fn util_eval_program_ambient_str(input: &str) -> Result<Ambient, SmxError> {
    util_eval_program_ambient_with_op_table(input, &im::HashMap::new())
}

pub fn util_eval_program_ambient_with_op_table(input: &str, op_table: &im::HashMap<OpSig, (Assoc, NotNan<f64>)>) -> Result<Ambient, SmxError> {
    eprintln!("[DEBUG] util_eval_program_ambient_with_op_table: Called with op_table containing {} operators", op_table.len());
    let tks = Lexer::new(input)
        .collect::<Result<Vec<Token>, LexerError>>()?;
    let mut amb = Ambient::default();
    amb.env.op_table = op_table.clone();
    eprintln!("[DEBUG] util_eval_program_ambient_with_op_table: Before parse, op_table has {} operators", amb.env.op_table.len());
    let program = Parser::with_ambient(tks, &amb)
        .parse_program()?;

    let result_amb = eval_program_ambient_with_initial(program, amb)?;
    eprintln!("[DEBUG] util_eval_program_ambient_with_op_table: After eval, op_table has {} operators", result_amb.env.op_table.len());
    Ok(result_amb)
}

pub fn util_eval_expr_str(input: &str, amb: &Ambient) -> Result<Value, SmxError> {
    let tks = Lexer::new(input)
        .collect::<Result<Vec<Token>, LexerError>>()?;
    let expr = Parser::with_ambient(tks, amb)
        .parse_expr_pratt(0.)?;

    eval_expr(
        expr,
        &mut Ambient {
            env: Environment{
                vars: amb.env.vars.clone(),
                rsrcs: amb.env.rsrcs.clone(),
                op_table: amb.env.op_table.clone(),
            },
            natives: amb.natives.clone(),
            custom_resources: amb.custom_resources.clone(),
        },
    )
    .map_err(|e| e.into())
}
