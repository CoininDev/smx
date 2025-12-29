use std::{fmt, error::Error};
use crate::value::*;

// =======================================
// =========== Lexer Error ===============
// =======================================

#[derive(Debug, Clone, PartialEq)]
pub enum LexerError {
    InvalidNumber(String),
    UnrecognizedChar(char),
    ParseError(String, String), // (value, error_message)
}

impl fmt::Display for LexerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexerError::InvalidNumber(msg) => {
                write!(f, "Invalid number - {}", msg)
            }
            LexerError::UnrecognizedChar(ch) => {
                write!(f, "Couldn't recognize character '{}'", ch)
            }
            LexerError::ParseError(value, msg) => {
                write!(f, "Failed analysing '{}' - {}", value, msg)
            }
        }
    }
}

impl Error for LexerError {}

// =======================================
// =========== Parsing Error =============
// =======================================

#[derive(Debug, Clone, PartialEq)]
pub enum ParsingError {
    Unexpected(String),
    Expected(String, String), // expected, found
    UnexpectedEof,
    InvalidAssignment,
    InvalidExpression(String),
}

impl fmt::Display for ParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParsingError::Unexpected(msg) => write!(f, "Unexpected - {}", msg),
            ParsingError::Expected(e, found) => write!(f, "Expected a '{e:?}', but found '{found:?}'"),
            ParsingError::UnexpectedEof => write!(f, "Unexpected end of file"),
            ParsingError::InvalidAssignment => write!(f, "Invalid assignment"),
            ParsingError::InvalidExpression(msg) => write!(f, "Invalid expression - {}", msg),
        }
    }
}

impl Error for ParsingError {}

// =======================================
// =========== Eval Error ================
// =======================================

#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    VariableDoesNotExists(String),
    InvalidSizeOfArgsFor(String),
    UnexpectedOperator(String),
    WrongTypes(String, Vec<Value>, Vec<Value>), // operator, expected, received
    ZeroDivisor,
    NonFunctionApplication(Value),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VariableDoesNotExists(var) => write!(f, "Variable not defined: {var}"),
            Self::InvalidSizeOfArgsFor(op) => write!(f, "Invalid size of args for {op}"),
            Self::UnexpectedOperator(op) => write!(f, "Unexpected operator {op}"),
            Self::ZeroDivisor => write!(f, "Dividing by zero is not allowed"),
            Self::WrongTypes(op, exp, received) => {
                write!(f, "Wrong types:")?;
                write!(f, "\nexpected: (")?;
                for x in exp.iter() {
                    write!(f, "{x} ")?;
                }
                write!(f, ")")?;
                write!(f, "\nreceived: (")?;
                for x in received.iter() {
                    write!(f, "{x} ")?;
                }
                write!(f, ")")
            }
            Self::NonFunctionApplication(v) => write!(f, "Tryng to apply something that is not a function: {v}"),
        }
    }
}

impl Error for EvalError {}
