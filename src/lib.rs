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
    ast::Parser,
    lexer::{Lexer, Token},
    eval::{eval_program, eval_expr},
    value::{Value, Ambient},
};

pub fn eval(code: &str, amb: &mut Ambient) -> Result<Value, String> {
    let tk = tokenize(code);
    let mut parser = Parser::new(tk);
    let expr = parser.parse_expr_pratt(0.0)
        .map_err(|e| format!("Parser error: {e}"))?;

    eval_expr(expr, amb)
        .map_err(|e| format!("Evaluation Error: {e:#?}"))
}

pub fn eval_file(path: &str, amb: &mut Ambient) -> Result<Value, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Error reading file {path}: {e}"))?;

    let tk = tokenize(&content);
    let mut parser = Parser::new(tk);
    let expr = parser.parse_expr_pratt(0.0)
        .map_err(|e| format!("Parser error: {e}"))?;

    eval_expr(expr, amb)
        .map_err(|e| format!("Evaluation Error: {e:#?}"))
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