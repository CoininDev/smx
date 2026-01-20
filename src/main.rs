#![feature(box_patterns)]
use crate::eval::eval_program;
use crate::repl::REPL;
use crate::{
    ast::Parser,
    lexer::{Lexer, Token},
};
use std::{env, fs};
mod ast;
mod io;
mod builtin;
mod eval;
mod value;
mod error;
mod lexer;
mod repl;

fn main() {
    let args = env::args().skip(1).collect::<Vec<String>>();

    match args.first() {
        Some(a) if a == "-i" => {
            println!("<== Welcome to Simple Interactive Mode ==>");
            let mut repl = REPL::new();
            loop {
                if repl.step() {
                    break;
                }
            }
        }

        Some(a) if a == "-f" => {
            let f_flag_pos = args.iter().position(|x| x == "-f").unwrap();
            let path = args
                .get(f_flag_pos + 1)
                .expect("Please, provide a path after `-f`");
            let content = fs::read_to_string(path)
                .expect(format!("Error reading file {path}")
                .as_str());

            let tk = tokenize(content.as_str());
            let mut parser = Parser::new(tk);
            let program = match parser.parse_program() {
                Ok(p) => p,
                Err(e) => panic!("Parser error: {e}"),
            };

            let result = match eval_program(program) {
                Ok(r) => r,
                Err(e) => panic!("Evaluation Error: {e:#?}"),
            };

            println!("result = {result}");
        }

        Some(a) if a == "-p" => {
            let content = match args.get(1) {
                Some(x)   => x,
                None  => panic!("Erro: Não tem nada na posição 1"),
            };

            let tk = tokenize(content.as_str());
            let mut parser = Parser::new(tk);
            let program = match parser.parse_program() {
                Ok(p)   => p,
                Err(e)  => panic!("Parser error: {e}"),
            };
            for assign in program.body {
                println!("{} = {};", assign.0, assign.2);
            }
        }

        Some(a) if a == "-pf" || a == "-fp" => {
            let path = match args.get(1) {
                Some(x)   => x,
                None  => panic!("Erro: Não tem nada na posição 1"),
            };
            
            let content = fs::read_to_string(path)
                .expect(format!("Error reading file {path}")
                .as_str());

            let tk = tokenize(content.as_str());
            let mut parser = Parser::new(tk);
            let program = match parser.parse_program() {
                Ok(p) => p,
                Err(e) => panic!("Parser error: {e}"),
            };

            for assign in program.body {
                println!("{} = {:#?}", assign.0, assign.1);
            }
        }

        None | Some(_) => usage(),
    }
}

fn usage() {
    let message = "Usage: simple_math [OPTIONS]

Options:
  -i             Enter interactive mode
  -f <filename>  Evaluate a file and print the result
  -p             Parse an expression
    ";
    println!("{message}");
}

// used by file interpreting
fn tokenize(s: &str) -> Vec<Token> {
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
