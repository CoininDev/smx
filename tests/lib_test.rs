use smx::smx_val;

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
    let mut amb = smx::value::Ambient::default();
    let res = smx::eval("1 + 2", &mut amb).unwrap();
    assert_eq!(res.to_string(), "3");
    
    // Teste com variáveis (se o parser suportasse assign no eval seria ótimo, 
    // mas aqui testamos apenas a avaliação de expressão em um ambiente)
    amb.vars.insert("x".into(), 10.into());
    amb.vars.insert("y".into(), smx_val!(5));
    let res2 = smx::eval("x + y", &mut amb).unwrap();
    assert_eq!(res2, smx_val!(15));
}