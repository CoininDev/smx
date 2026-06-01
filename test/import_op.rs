use smx::{*, ast::*};


#[test]
fn test_import_op(){
    let code = "op right 67.67 (a +*+ b) = (a + b) * (a + b);";
    let tks = tokenize(code).unwrap();
    let mut parser = Parser::new(tks);
    let res = parser.parse_program().unwrap();

    println!("{res:?}");
}