// temp file to verify syntax of thunk
use crate::{ast::*, eval::*, value::*, error::*};
use std::collections::HashMap;

pub enum Thunk {
    Done(Value),
    Eval { expr: Expression, new_amb: Box<Ambient>, baseline_vars: Environment },
    Apply(Value, Value, Box<Ambient>),
}

pub fn resolve_thunk(mut thunk: Thunk, outer_amb: &mut Ambient) -> EvalResult<Value> {
    let mut accumulated_exports = Environment::new();
    loop {
        match thunk {
            Thunk::Done(val) => {
                for (k, v) in accumulated_exports {
                    outer_amb.vars.insert(k, v);
                }
                return Ok(val);
            }
            Thunk::Eval { expr, mut new_amb, baseline_vars } => {
                thunk = eval_step(expr, &mut new_amb)?;
                for (k, v) in new_amb.vars.clone() {
                    if !baseline_vars.contains_key(&k) {
                        accumulated_exports.insert(k, v);
                    }
                }
            }
            Thunk::Apply(f, x, mut new_amb) => {
                thunk = apply_step(f, x, &mut new_amb)?;
            }
        }
    }
}
