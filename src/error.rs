use std::{fmt, error::Error};
use crate::value::*;
use crate::ast::*;

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
            Self::InvalidNumber(msg) => write!(f, "Invalid number - {}", msg),
            Self::UnrecognizedChar(ch) => write!(f, "Couldn't recognize character '{}'", ch),
            Self::ParseError(value, msg) => write!(f, "Failed analysing '{}' - {}", value, msg),
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
    NotNanError(String),
}

impl fmt::Display for ParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unexpected(msg) => write!(f, "Unexpected - {}", msg),
            Self::Expected(e, found) => write!(f, "Expected a '{e:?}', but found '{found:?}'"),
            Self::UnexpectedEof => write!(f, "Unexpected end of file"),
            Self::InvalidAssignment => write!(f, "Invalid assignment"),
            Self::InvalidExpression(msg) => write!(f, "Invalid expression - {msg}"),
            Self::NotNanError(s) => write!(f, "Not Nan Error - {s}"),
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
    PatternError(String),
    InvalidPattern(Expression),
    NotNanError(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VariableDoesNotExists(var) => write!(f, "Variable not defined: {var}"),
            Self::InvalidSizeOfArgsFor(op) => write!(f, "Invalid size of args for {op}"),
            Self::InvalidPattern(expr) => write!(f, "Invalid pattern: {expr}"),
            Self::UnexpectedOperator(op) => write!(f, "Unexpected operator {op}"),
            Self::ZeroDivisor => write!(f, "Dividing by zero is not allowed"),
            Self::PatternError(x) => write!(f, "Pattern error - {x}"),
            Self::NotNanError(s) => write!(f, "Not Nan Error - {s}"),
            Self::WrongTypes(op, exp, received) => {
                write!(f, "Wrong types for {op}:")?;
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
