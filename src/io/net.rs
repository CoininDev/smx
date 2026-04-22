   /////////////////////////////
  //    WORK IN PROGRESS     //
 // COMPLETELY EXPERIMENTAL //
/////////////////////////////

use crate::{io::*};
use std::{rc::Rc, cell::RefCell};
use std::net::{TcpStream};
use std::io::Write;

macro_rules! eval_error {
    ($err: expr) => {
        EvalError::new($err)
    }
}

#[allow(dead_code)]
enum NetNativeObj {
    Tcp {val: TcpStream, receive: Value},
    Test,
}

pub struct NetIoObj;
impl IoObject for NetIoObj {
    fn redirect(&mut self, function:Vec<String>, value: Value, amb: &mut Ambient)
        -> EvalResult<Value>
    {
        assert_eq!(1, function.len());
        match function[0].as_str() {
            "open"    => self.open(value, amb),
            "send"    => self.send(value, amb),
            "run"     => self.run(value, amb),
            _         => Err(eval_error!(VariableDoesNotExists(function[0].clone())))
        }
    }

    fn name(&self) -> &str {"net"}
}

impl NetIoObj {
    fn open (&self, arg: Value, amb: &mut Ambient) -> EvalResult<Value> {
        fn error<T, E: ToString>(a: Result<T, E>) -> EvalResult<T> {
            a.map_err(|e| eval_error!(GenericError(e.to_string())))
        }
        fn wrong_types(additional_text: &str) -> EvalResult<Value> {
            Err(eval_error!(GenericError(format!("
                expecting an env for IO.net.open with:
                address ~ string *
                receive    ~ fn     *
                => {additional_text}
            "))))
        }
        match arg {
            Value::Environment(env) => {
                let address = match env.get("address") {
                    Some(Value::Str(x)) => x, 
                    _ => return wrong_types("address"),
                };

                let receive = match env.get("receive") {
                    Some(Value::Lambda(_,_,_,_)) => env.get("receive").unwrap(),
                    _ => return wrong_types("receive")
                };

                let stream = error(TcpStream::connect(address))?;
                let obj = NetNativeObj::Tcp {val: stream, receive: receive.clone()};
                amb.natives.push(Rc::new(RefCell::new(obj)));
                Ok(Value::Native(amb.natives.len() - 1))
            }
            other => Err(eval_error!(WrongTypes(String::from("IO.net.open"), PatternType::Environment, other)))
        }
    }

    fn send (&self, arg: Value, amb: &mut Ambient) -> EvalResult<Value> {
        match arg {
            Value::Pair(box Value::Native(native_index), box Value::Str(message)) => {
                let a0 = amb.natives.get(native_index)
                    .ok_or(eval_error!(GenericError("Couldn't get the NATIVE by its id.".into())))?;
                let a1: Rc<RefCell<NetNativeObj>> = a0.clone().downcast::<RefCell<NetNativeObj>>()
                    .map_err(|_| eval_error!(GenericError("This native doesn't have NetNativeObj type.".into())))?;
                let mut a2 = a1.borrow_mut();
                match &mut *a2 {
                    NetNativeObj::Tcp {val: v, receive: _} => {
                        let _ = v.write_all(message.as_bytes());
                    },
                    _ => {}
                }
    
                Ok(Value::Native(native_index))
            }
            _ => Err(eval_error!(GenericError(String::from("ijaefjjij"))))
        }
    }

    fn run(&self, arg: Value, amb: &mut Ambient) -> EvalResult<Value> {
        match arg {
            Value::Native(native_index) => {
                let a0 = amb.natives.get(native_index)
                    .ok_or(eval_error!(GenericError("Couldn't get the NATIVE by its id.".into())))?;
                let a1: Rc<RefCell<NetNativeObj>> = a0.clone().downcast::<RefCell<NetNativeObj>>()
                    .map_err(|_| eval_error!(GenericError("This native doesn't have NetNativeObj type.".into())))?;
                let mut a2 = a1.borrow_mut();
                loop {
                    match &mut *a2 {
                        NetNativeObj::Tcp {val: _, receive: _} => {
                            println!("WORK IN PROGRESS;");
                        }
                        NetNativeObj::Test => print!("WORK IN PROGRESS;"),
                    }
                }
            }
            _ => Err(eval_error!(GenericError(String::from("pinto"))))
        }
    }
}
