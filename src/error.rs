use std::fmt;

use crate::{lexer::LexError, parser::ParseError, runtime::RuntimeError};

pub type ScriptResult<T> = Result<T, ScriptError>;

#[derive(Debug, Clone)]
pub enum ScriptError {
    Lex(LexError),
    Parse(ParseError),
    Runtime(RuntimeError),
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptError::Lex(error) => write!(f, "{error}"),
            ScriptError::Parse(error) => write!(f, "{error}"),
            ScriptError::Runtime(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ScriptError {}

impl From<LexError> for ScriptError {
    fn from(value: LexError) -> Self {
        ScriptError::Lex(value)
    }
}

impl From<ParseError> for ScriptError {
    fn from(value: ParseError) -> Self {
        ScriptError::Parse(value)
    }
}

impl From<RuntimeError> for ScriptError {
    fn from(value: RuntimeError) -> Self {
        ScriptError::Runtime(value)
    }
}
