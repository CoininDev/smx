use rustyline::{DefaultEditor, error::ReadlineError};
use im::hashmap;
use crate::{
    ast::Parser,
    eval::*,
    lexer::{Lexer, Token},
    error::*,
    value::{Ambient, Value},
};

pub struct REPL {
    ambient: Ambient,
    rl: DefaultEditor,
}

impl REPL {
    pub fn new() -> Self {
        let mut ambient = Ambient::default();
        ambient.vars = ambient.vars.union( hashmap!{
            "IO".into() => Value::Builtin("IO".into())
        });
        Self {
            rl: DefaultEditor::new().unwrap(),
            ambient,
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
                if line == "all" {
                    for (k,v) in &self.ambient.vars {
                        println!("{k} = {v};");
                    }

                    println!("resources:");
                    for (k,v) in &self.ambient.rsrcs {
                        println!("{k} = {v};");
                    }

                    println!("natives:");
                    for (k,v) in self.ambient.natives.clone().into_iter().enumerate() {
                        println!("{k} = {v:?};");
                    }
                    return false;
                }

                let tk = match self.tokenize(line.as_str()) {
                    Ok(t) => t,
                    Err(e) => {
                        let err = SmxError::Lexer(e);
                        let report = if err.has_source_code() {
                            miette::Report::from(err)
                        } else {
                            miette::Report::from(err).with_source_code(line.clone())
                        };
                        eprintln!("{:?}", report);
                        return false;
                    }
                };

                let mut parser = Parser::with_ambient(tk.clone(), &self.ambient);

                let assign = parser.parse_assign();
                let assign_pos = parser.pos;
                parser.reset();
                let expr = parser.parse_expr_pratt(0.);
                let expr_pos = parser.pos;

                match (assign, expr) {
                    (Ok(assign), _) => {
                        if let Err(e) = eval_resource(&assign, &mut self.ambient.rsrcs) {
                            let err = SmxError::Eval(e);
                            let report = if err.has_source_code() {
                                miette::Report::from(err)
                            } else {
                                miette::Report::from(err).with_source_code(line.clone())
                            };
                            eprintln!("{:?}", report);
                            return false;
                        }
                        
                        if let Err(e) = eval_assign(assign, &mut self.ambient) {
                            let err = SmxError::Eval(e);
                            let report = if err.has_source_code() {
                                miette::Report::from(err)
                            } else {
                                miette::Report::from(err).with_source_code(line.clone())
                            };
                            eprintln!("{:?}", report);
                            return false;
                        }
                        
                        println!("Ok");
                        return false;
                    }

                    (Err(_a), Ok(expr)) => {
                        let res = match eval_expr(expr, &mut self.ambient) {
                            Ok(r) => r,
                            Err(e) => {
                                let err = SmxError::Eval(e);
                                let report = if err.has_source_code() {
                                    miette::Report::from(err)
                                } else {
                                    miette::Report::from(err).with_source_code(line.clone())
                                };
                                eprintln!("{:?}", report);
                                return false;
                            }
                        };
                        println!("= {res}");
                        return false;
                    }

                    (Err(a), Err(b)) => {
                        if assign_pos >= expr_pos {
                            let err = SmxError::Parsing(a);
                            let report = if err.has_source_code() {
                                miette::Report::from(err)
                            } else {
                                miette::Report::from(err).with_source_code(line.clone())
                            };
                            eprintln!("{:?}", report);
                        } else {
                            let err = SmxError::Parsing(b);
                            let report = if err.has_source_code() {
                                miette::Report::from(err)
                            } else {
                                miette::Report::from(err).with_source_code(line.clone())
                            };
                            eprintln!("{:?}", report);
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

        Ok(vlex)
    }
}
