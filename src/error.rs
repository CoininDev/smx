use crate::{value::*, eval::EvalResult};
use thiserror::Error;
use miette::{Diagnostic, SourceSpan};

impl From<crate::lexer::Span> for SourceSpan {
    fn from(span: crate::lexer::Span) -> Self {
        (span.start, span.end - span.start).into()
    }
}

#[derive(Error, Diagnostic, Debug, Clone, PartialEq)]
pub enum SmxError {
    #[error("Lexer error: {0}")]
    #[diagnostic(transparent)]
    Lexer(#[from] LexerError),

    #[error("Parsing error: {0}")]
    #[diagnostic(transparent)]
    Parsing(#[from] ParsingError),

    #[error("Evaluation error: {0}")]
    #[diagnostic(transparent)]
    Eval(#[from] EvalError),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl SmxError {
    pub fn has_source_code(&self) -> bool {
        match self {
            SmxError::Eval(e) => e.source_code.is_some(),
            _ => false,
        }
    }
}

// =======================================
// =========== Lexer Error ===============
// =======================================

#[derive(Error, Debug, Clone, PartialEq)]
pub enum LexerErrorType {
    #[error("Invalid number - {0}")]
    InvalidNumber(String),
    #[error("Couldn't recognize character '{0}'")]
    UnrecognizedChar(char),
    #[error("Failed analysing '{0}' - {1}")]
    ParseError(String, String), // (value, error_message)
}

#[derive(Error, Diagnostic, Debug, Clone, PartialEq)]
#[error("{errtype}")]
pub struct LexerError {
    pub errtype: LexerErrorType,
    #[label("here")]
    pub span: SourceSpan,
}

// =======================================
// =========== Parsing Error =============
// =======================================

#[derive(Error, Diagnostic, Debug, Clone, PartialEq)]
#[error("{errtype}")]
pub struct ParsingError {
    pub errtype: ParsingErrorType,
    #[label("here")]
    pub span: Option<SourceSpan>,
}

impl ParsingError {
    pub fn new(errtype: ParsingErrorType, span: Option<crate::lexer::Span>) -> Self {
        Self { errtype, span: span.map(|s| s.into()) }
    }
}

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ParsingErrorType {
    #[error("Unexpected - {0}")]
    Unexpected(String),
    #[error("Expected a '{0}', but found '{1}'")]
    Expected(String, String), // expected, found
    #[error("Unexpected end of file")]
    UnexpectedEof,
    #[error("Invalid assignment")]
    InvalidAssignment,
    #[error("Invalid expression - {0}")]
    InvalidExpression(String),
    #[error("Not Nan Error - {0}")]
    NotNanError(String),
}

// =======================================
// =========== Eval Error ================
// =======================================

#[derive(Error, Diagnostic, Debug, Clone, PartialEq)]
#[error("{errtype}")]
#[diagnostic(help("{}", self.render_call_stack()))]
pub struct EvalError {
    pub errtype: EvalErrorType,
    pub assign: Option<String>,
    #[label("here")]
    pub span: Option<SourceSpan>,
    pub call_stack: Vec<String>,
    #[source_code]
    pub source_code: Option<String>,
}

impl EvalError {
    fn render_call_stack(&self) -> String {
        if self.call_stack.is_empty() {
            String::new()
        } else {
            let mut res = String::from("Call stack:");
            for f in self.call_stack.iter().rev() {
                res.push_str(&format!("\n  at {f}"));
            }
            res
        }
    }

    pub fn new(errtype: EvalErrorType) -> Self {
        Self { 
            errtype, 
            assign: None, 
            span: None,
            call_stack: Vec::new(),
            source_code: None,
        }
    }

    pub fn with_span(mut self, span: crate::lexer::Span) -> Self {
        if self.span.is_none() {
            self.span = Some(span.into());
        }
        self
    }
}

// Removing manual fmt::Display as it's now handled by thiserror and miette.

#[derive(Error, Debug, Clone, PartialEq)]
pub enum EvalErrorType {
    #[error("Variable not defined: {0}")]
    VariableDoesNotExists(String),
    #[error("Invalid size of args for {0}")]
    InvalidSizeOfArgsFor(String),
    #[error("Unexpected operator {0}")]
    UnexpectedOperator(String),
    #[error("Wrong types for {0}:\nexpected: ({1})\nreceived: ({2})")]
    WrongTypes(String, PatternType, Value), // operator, expected, received
    #[error("Dividing by zero is not allowed")]
    ZeroDivisor,
    #[error("Trying to apply something that is not a function: {0}")]
    NonFunctionApplication(Value),
    #[error("Pattern error - {0}")]
    PatternError(String),
    #[error("Not Nan Error - {0}")]
    NotNanError(String),
    #[error("Resource not provided by caller: {0}")]
    ResourceNotProvided(String),
    #[error("Error - {0}")]
    GenericError(String),
}

// Smooth error conversion
pub trait MapEvalError<T> {
    fn map_eval_error(self) -> EvalResult<T>;
}

impl<T, E: std::error::Error> MapEvalError<T> for Result<T, E> {
    fn map_eval_error(self) -> EvalResult<T> {
        self.map_err(|e| EvalError::new(EvalErrorType::GenericError(e.to_string())))
    }
}