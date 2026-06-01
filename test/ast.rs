use smx::{ast::*, *};


#[test]
fn test_expression_spans() {
    let code = "1 + 2 * 3";
    let tokens = tokenize(code).unwrap();
    let mut parser = Parser::new(tokens);
    let expr = parser.parse_expr_pratt(0.0).unwrap();

    // "1 + 2 * 3" should cover from index 0 to 9
    assert_eq!(expr.span.start, 0);
    assert_eq!(expr.span.end, 9);
}

#[test]
fn test_nested_spans() {
    let code = "(1 + 2)";
    let tokens = tokenize(code).unwrap();
    let mut parser = Parser::new(tokens);
    let expr = parser.parse_expr_pratt(0.0).unwrap();

    assert_eq!(expr.span.start, 0);
    assert_eq!(expr.span.end, 7);
}

#[test]
fn test_let_spans() {
    let code = "let x = 10; in x";
    let tokens = tokenize(code).unwrap();
    let mut parser = Parser::new(tokens);
    let expr = parser.parse_expr_pratt(0.0).unwrap();

    assert_eq!(expr.span.start, 0);
    assert_eq!(expr.span.end, 16);
}
