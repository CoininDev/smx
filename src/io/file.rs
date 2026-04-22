use std::fs::File;
use std::io::prelude::*;
use crate::io::*;
use std::env;
use shellexpand::full;

macro_rules! eval_error {
    ($err: expr) => {
        EvalError::new($err)
    }
}

pub struct FileIoObj;
impl IoObject for FileIoObj {
    fn redirect(&mut self, function:Vec<String>, value: Value, _: &mut Ambient)
        -> EvalResult<Value>
    {
        assert_eq!(1, function.len());
        match function[0].as_str() {
            "write"  => self.write(value),
            "read"   => self.read(value),
            "delete" => self.delete(value),
            "mkdir"  => self.mkdir(value),
            "rmdir"  => self.rmdir(value),
            _        => Err(eval_error!(VariableDoesNotExists(function[0].clone())))
        }
    }

    fn name(&self) -> &str {"file"}
}
impl FileIoObj {
    fn write(&self, value: Value) -> EvalResult<Value> {
        fn error () -> EvalResult<Value> {
            Err(eval_error!(GenericError(String::from(
                r"
                expected for IO.file.read env with:
                    name ~ string *
                    content ~ string *
                "
            ))))
        }
        match value {
            Value::Environment(env) => {
                let name = match env.get("name") {
                    Some(Value::Str(x)) => full(&x).expect("Could not expand file terms.").into_owned(),
                    _ => return error(),
                };

                let mut file = match File::create(&name) {
                    Ok(a) => a,
                    Err(e) => return Ok(Value::Environment(
                        hashmap!{"file_write_error".into() => Value::Str(e.to_string())}
                    )),
                };

                let content = match env.get("content") {
                    Some(Value::Str(x)) => x.clone(),
                    _ => return error()
                };
                let path = env::current_dir().unwrap_or_default().display().to_string();
                match file.write_all(&content.clone().into_bytes()) {
                    Ok(_) => Ok(Value::Environment(
                        hashmap!{
                            "write_file_success".into() => Value::Bool(true),
                            "path".into() => Value::Str(format!("{path}/{name}"))
                        }
                    )),
                    Err(e) => Ok(Value::Environment(
                        hashmap!{"write_file_error".into() => Value::Str(e.to_string())}
                    )),
                }

            }
            _ => error(),
        }
    }

    fn read(&self, value: Value) -> EvalResult<Value> {
        fn error () -> EvalResult<Value> {
            Err(eval_error!(GenericError(String::from(
                r"
                expected for IO.file.read env with:
                    name ~ string *
                "
            ))))
        }
        match value {
            Value::Str(name) => {
                let exp = full(&name).expect("Could not expand file terms.").into_owned();
                let mut file = match File::open(exp) {
                    Ok(a) => a,
                    Err(e) => return Ok(Value::Environment(
                        hashmap!{"file_read_error".into() => Value::Str(e.to_string())}
                    )),
                };

                let mut content = String::new();
                match file.read_to_string(&mut content) {
                    Ok(_) => Ok(Value::Str(content)),
                    Err(e) => Ok(Value::Environment(hashmap!{
                        "file_read_error".into() => Value::Str(e.to_string())
                    }))
                }
            }
            _ => error(),
        }
    }


    fn mkdir(&self, arg: Value) -> EvalResult<Value> {
        match arg {
            Value::Str(name) => {
                let expanded = full(&name).expect("Could not expand file terms.").into_owned();
                match std::fs::create_dir(expanded.clone()) {
                    Ok(_) => Ok(Value::Environment(
                        hashmap!{
                            "mkdir_success".into() => Value::Bool(true),
                            "dir_name".into() => Value::Str(expanded),
                        }
                    )),
                    Err(e) => Ok(Value::Environment(
                        hashmap!{"mkdir_error".into() => Value::Str(e.to_string())}
                    )),        
                }
            }
            other => Err(eval_error!(WrongTypes("IO.file.mkdir".into(), PatternType::String, other))),
        }
    }

    fn rmdir(&self, arg: Value) -> EvalResult<Value> {
        match arg {
            Value::Str(name) => {
                let expanded = full(&name).expect("Could not expand file terms.").into_owned();
                match std::fs::remove_dir(expanded.clone()) {
                    Ok(_) => Ok(Value::Environment(
                        hashmap!{
                            "rmdir_success".into() => Value::Bool(true),
                            "dir_name".into() => Value::Str(expanded),
                        }
                    )),
                    Err(e) => Ok(Value::Environment(
                        hashmap!{"rmdir_error".into() => Value::Str(e.to_string())}
                    )),
                }
            }
            other => Err(eval_error!(WrongTypes("IO.file.rmdir".into(), PatternType::String, other))),
        }
    }

    fn delete(&self, arg: Value) -> EvalResult<Value> {
        match arg {
            Value::Str(name) => {
                let expanded = full(&name).expect("Could not expand file terms.").into_owned();
                match std::fs::remove_file(expanded.clone()) {
                    Ok(_) => Ok(Value::Environment(
                        hashmap!{
                            "delete_success".into() => Value::Bool(true),
                            "file_name".into() => Value::Str(expanded),
                        }
                    )),
                    Err(e) => Ok(Value::Environment(
                        hashmap!{"delete_error".into() => Value::Str(e.to_string())}
                    )),
                }
            }
            other => Err(eval_error!(WrongTypes("IO.file.delete".into(), PatternType::String, other))),
        }
    }
}


