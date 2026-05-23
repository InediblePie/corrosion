use std::fmt;

use crate::source::SourceSpan;

#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
    pub span: SourceSpan,
}

impl LexError {
    pub fn new(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "lex error at {}:{}: {}",
            self.span.line, self.span.column, self.message
        )
    }
}

impl std::error::Error for LexError {}
