#![feature(box_patterns)]

pub mod ast;
pub mod builtin;
pub mod error;
pub mod eval;
pub mod io;
pub mod lexer;
pub mod repl;
pub mod value;

pub use crate::error::SmxError;

use crate::{
    ast::Parser,
    error::LexerError,
    eval::{eval_assign, eval_expr, eval_program, eval_resource},
    lexer::{Lexer, Token},
    value::{Ambient, Value},
};

pub fn eval(code: &str, mut amb: &mut Ambient) -> Result<Value, SmxError> {
    let tk = tokenize(code)?;
    let mut parser = Parser::new(tk);

    let assign = parser.parse_assign();
    let assign_pos = parser.pos;
    parser.reset();
    let expr = parser.parse_expr_pratt(0.);
    let expr_pos = parser.pos;

    match (assign, expr) {
        (Ok(assign), _) => {

            // eval_resource actively detects and ignores other assigns
            eval_resource(&assign, &mut amb.env.rsrcs)?;

            // eval_assign actively detects and ignores resources
            eval_assign(assign, &mut amb)?;
            return Ok(val!());
        }

        (Err(_a), Ok(expr)) => {
            // println!("Assign Err: {}", _a);
            let res = eval_expr(expr, &mut amb)?;
            println!("= {res}");
            return Ok(res);
        }

        (Err(a), Err(b)) => {
            if assign_pos >= expr_pos {
                return Err(a.into());
            } else {
                return Err(b.into());
            }
        }
    }
}

pub fn eval_file(path: &str, amb: &mut Ambient) -> Result<Value, SmxError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| SmxError::Io(format!("Error reading file {path}: {e}")))?;

    eval(&content, amb)
}

pub fn run_file(path: &str) -> Result<Value, SmxError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| SmxError::Io(format!("Error reading file {path}: {e}")))?;

    let tk = tokenize(&content)?;
    let mut parser = Parser::new(tk);
    let program = parser.parse_program()?;

    Ok(eval_program(program)?)
}

pub fn run(content: &str) -> Result<Value, SmxError> {
    let tk = tokenize(&content)?;
    let mut parser = Parser::new(tk);
    let program = parser.parse_program()?;

    Ok(eval_program(program)?)
}

pub fn tokenize(s: &str) -> Result<Vec<Token>, LexerError> {
    let lex = Lexer::new(s);
    let mut vlex = vec![];
    for t in lex {
        vlex.push(t?);
    }
    Ok(vlex)
}

#[macro_export]
macro_rules! val {
    (nil) => {
        $crate::value::Value::Nil
    };
    () => {
        $crate::value::Value::Nil
    };
    (true) => {
        $crate::value::Value::Bool(true)
    };
    (false) => {
        $crate::value::Value::Bool(false)
    };
    (IO) => {
        $crate::value::Value::Builtin("IO".into())
    };
    (ambient) => {
        $crate::value::Ambient::default()
    };
    ($n:expr) => {
        $crate::value::Value::from($n)
    };
}
