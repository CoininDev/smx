#![feature(box_patterns)]

pub mod ast;
pub mod io;
pub mod builtin;
pub mod eval;
pub mod value;
pub mod error;
pub mod lexer;
pub mod repl;

use crate::{
    ast::Parser, eval::{eval_assign, eval_expr, eval_program, eval_resource}, lexer::{Lexer, Token}, value::{Ambient, Value}
};

pub fn eval(code: &str, mut amb: &mut Ambient) -> Result<Value, String> {
    let tk = tokenize(code);
    let mut parser = Parser::new(tk);
    
    let assign = parser.parse_assign();
    let assign_pos = parser.pos;
    parser.reset();
    let expr = parser.parse_expr_pratt(0.);
    let expr_pos = parser.pos;

    match (assign, expr) {
        (Ok(assign), _) => {
            #[cfg(debug_assertions)]
            println!("(debug)\tAST: {assign:#?}");

            // eval_resource actively detects and ignores other assigns
            if let Err(e) = eval_resource(&assign, &mut amb.rsrcs) {
                return Err(e.to_string());
            }
            
            // eval_assign actively detects and ignores resources
            if let Err(e) = eval_assign(assign, &mut amb) {
                eprintln!("Eval Error: {e}");
                return Err(e.to_string());
            }
            return Ok(val!());
        }

        (Err(_a), Ok(expr)) => {
            // println!("Assign Err: {}", _a);
            #[cfg(debug_assertions)]
            println!("(debug)\tAST: {expr:#?}");
            let res = match eval_expr(expr, &mut amb) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Eval Error: {e}");
                    return Err(e.to_string());
                }
            };
            println!("= {res}");
            return Ok(res);
        }

        (Err(a), Err(b)) => {
            // println!("Assign Err: {}", a);
            if assign_pos >= expr_pos {
                println!("Assign Err: {}", a);
            } else {
                println!("Expr Err: {}", b);
            }
        }
    }
    Err("Parsing failed".into())
}


pub fn eval_file(path: &str, amb: &mut Ambient) -> Result<Value, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Error reading file {path}: {e}"))?;

    eval(&content, amb)
}

pub fn run_file(path: &str) -> Result<Value, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Error reading file {path}: {e}"))?;

    let tk = tokenize(&content);
    let mut parser = Parser::new(tk);
    let program = parser.parse_program()
        .map_err(|e| format!("Parser error: {e}"))?;

    eval_program(program)
        .map_err(|e| format!("Evaluation Error: {e:#?}"))
}


pub fn tokenize(s: &str) -> Vec<Token> {
    let lex = Lexer::new(s);
    let mut vlex = vec![];
    for t in lex {
        match t {
            Err(e) => eprintln!("Tokenizer error: {e}"),
            Ok(token) => vlex.push(token),
        }
    }
    vlex
}

#[macro_export]
macro_rules! val {
    (nil) => { $crate::value::Value::Nil };
    () => { $crate::value::Value::Nil };
    (true) => { $crate::value::Value::Bool(true) };
    (false) => { $crate::value::Value::Bool(false) };
    (IO) => { $crate::value::Value::Builtin("IO".into()) };
    (ambient) => { $crate::value::Ambient::default() };
    ($n:expr) => { $crate::value::Value::from($n) };
}