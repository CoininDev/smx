use rustyline::{DefaultEditor, error::ReadlineError};
use std::collections::HashMap;

use crate::{
    ast::Parser,
    eval::*,
    lexer::{Lexer, Token},
    error::*,
};

pub struct REPL {
    vars: Environment,
    rl: DefaultEditor,
}

impl REPL {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
            rl: DefaultEditor::new().unwrap(),
        }
    }

    pub fn step(&mut self) -> bool {
        match self.rl.readline("> ") {
            Ok(line) => {
                if line == "exit" {
                    println!("Bye bye!");
                    return true;
                }

                let tk = self.tokenize(line.as_str());
                let tk = match tk {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("Lexer Error: {e}");
                        self.rl.add_history_entry(&line).unwrap();
                        return false;
                    }
                };

                let mut parser = Parser::new(tk);
                if line.contains("=") {
                    let assign = match parser.parse_assign() {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("Parsing Error: {e}");
                            self.rl.add_history_entry(&line).unwrap();
                            return false;
                        }
                    };

                    println!("{assign:?}");
                    let (n, v) = match eval_assign(assign, &self.vars) {
                        Ok(tuple) => tuple,
                        Err(e) => {
                            eprintln!("Eval Error: {e}");
                            self.rl.add_history_entry(&line).unwrap();
                            return false;
                        }
                    };
                    println!("< {n} = {v}");
                    self.vars.insert(n, v);
                } else {
                    if line.chars().all(|c| c.is_alphabetic() || c == '_') {
                        let v = self.vars.get(&line);
                        match v {
                            Some(a) => println!("= {a}"),
                            None => eprintln!("This variable does not exist"),
                        }
                    } else {
                        let expr = match parser.parse_expr_pratt(0.){
                            Ok(p) => p,
                            Err(e) => {
                                eprintln!("Parsing Error: {e}");
                                self.rl.add_history_entry(&line).unwrap();
                                return false;
                            }
                        };
                        println!("{expr:?}");
                        let res = match eval_expr(expr, &self.vars){
                            Ok(r) => r,
                            Err(e) => {
                                eprintln!("Eval Error: {e}");
                                self.rl.add_history_entry(&line).unwrap();
                                return false;
                            }
                        };

                        println!("= {res}");
                    }
                }

                self.rl.add_history_entry(&line).unwrap();

                println!("<-------------------------->");
                false
            }
            Err(ReadlineError::Interrupted) => true,
            Err(ReadlineError::Eof) => true,
            Err(e) => {
                eprintln!("Erro: {e}");
                true
            }
        }
    }

    pub fn tokenize(&self, s: &str) -> Result<Vec<Token>, LexerError> {
        let lex = Lexer::new(s);
        let mut vlex = vec![];
        for t in lex {
            let t = t?;
            vlex.push(t);
        }
        Ok(vlex)
    }
}
