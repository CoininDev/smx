use std::rc::Rc;

use smx::value::IoObject;
use smx::eval_error;
use smx::error::EvalErrorType::*;
use smx::error::EvalError;

#[test]
fn test_lib_run_file() {
    // We can't easily run a file that depends on stdlib without setting up the environment,
    // but we can try a simple one.
    let temp_file = "scratch/temp_test.smx";
    std::fs::write(temp_file, "result = 1 + 2;").unwrap();
    
    let res = smx::run_file(temp_file).unwrap();
    assert_eq!(res.to_string(), "3");
    
    std::fs::remove_file(temp_file).unwrap();
}

#[test]
fn test_lib_eval() {
    let mut amb = smx::val!(ambient);
    let res = smx::eval("1 + 2", &mut amb).unwrap();
    assert_eq!(res.to_string(), "3");
    
    // Teste com variáveis (se o parser suportasse assign no eval seria ótimo, 
    // mas aqui testamos apenas a avaliação de expressão em um ambiente)
    amb.vars.insert("x".into(), 10.into());
    amb.vars.insert("y".into(), smx::val!(5));
    amb.vars.insert("IO".into(), smx::val!(IO));
    let res2 = smx::eval("IO.wait 2 :\\_. x + y", &mut amb).unwrap();
    assert_eq!(res2, smx::val!(15));
}

#[test]
fn assignment_test() {
    let mut amb = smx::val!(ambient);
    let res = smx::eval("a = 5 + 3", &mut amb).unwrap();
    assert!(amb.vars.contains_key("a") && amb.vars["a"] == smx::val!(8));
    
    // Teste de reatribuição
    let res2 = smx::eval("a = 5 * 2", &mut amb).unwrap();
    assert!(amb.vars.contains_key("a") && amb.vars["a"] == smx::val!(10));
}


struct ExampleIo;
impl IoObject for ExampleIo {
    fn name(&self) -> &str {
        "E"
    }

    fn redirect(&self, function: Vec<String>, value: smx::value::Value, amb: &mut smx::value::Ambient) -> smx::eval::EvalResult<smx::value::Value> {
        if function == ["wait"] {
            if let smx::value::Value::Num(n) = value {
                std::thread::sleep(std::time::Duration::from_secs_f64(*n));
                Ok(smx::val!())
            } else {
                Err(eval_error!(WrongTypes(function.join("."), smx::value::PatternType::Number, value)))
            }
        } else {
            Err(eval_error!(VariableDoesNotExists(format!("Unknown function for E: {:?}", function))))
        }
    }
}

#[test]
fn test_io_object() {
    let mut amb = smx::val!(ambient);
    amb.add_custom_resource(Rc::new(ExampleIo));
    
    let start = std::time::Instant::now();
    let res = smx::eval("_ @{E} = E.wait 1", &mut amb).unwrap();
    let duration = start.elapsed();
    
    assert!(duration.as_secs_f64() >= 1.0, "Expected to wait at least 1 second");
    assert_eq!(res, smx::val!());

    let res = smx::eval("_ @{IO} = IO.wait 0.5", &mut amb).unwrap();
    let duration = start.elapsed();
    assert!(duration.as_secs_f64() >= 1.5, "Expected to wait at least 1.5 seconds");
    assert_eq!(res, smx::val!());
}