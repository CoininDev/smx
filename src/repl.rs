use rustyline::{DefaultEditor, error::ReadlineError};
use im::HashMap;

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
                self.rl.add_history_entry(&line).unwrap();
                
                if line == "exit" {
                    println!("Bye bye!");
                    return true;
                }

                let tk = self.tokenize(line.as_str());
                let tk = match tk {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("Lexer Error: {e}");
                        return false;
                    }
                };

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

                        if let Err(e) = eval_assign(assign, &mut self.vars) {
                            eprintln!("Eval Error: {e}");
                            return false;
                        }
                        
                        println!("Ok");
                        return false;
                    }

                    (Err(_), Ok(expr)) => {
                        #[cfg(debug_assertions)]
                        println!("(debug)\tAST: {expr:#?}");
                        let res = match eval_expr(expr, &self.vars){
                            Ok(r) => r,
                            Err(e) => {
                                eprintln!("Eval Error: {e}");
                                return false;
                            }
                        };
                        println!("= {res}");
                        return false;
                    }

                    (Err(a), Err(b)) => {
                        if assign_pos >= expr_pos {
                            println!("Assign Err: {}", a);
                        } else {
                            println!("Expr Err: {}", b);
                        }
                    }
                }
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

        if cfg!(debug_assertions) {
            print!("(debug)\tTokens: ");
            println!("{}", vlex.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", "));
        }
        Ok(vlex)
    }
}
