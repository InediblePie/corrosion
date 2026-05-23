use std::fmt;

use crate::source::SourceSpan;

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeError {
    pub message: String,
    pub span: Option<SourceSpan>,
}

impl RuntimeError {
    pub fn new(message: impl Into<String>, span: Option<SourceSpan>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(span) = self.span {
            write!(
                f,
                "runtime error at {}:{}: {}",
                span.line, span.column, self.message
            )
        } else {
            write!(f, "runtime error: {}", self.message)
        }
    }
}

impl std::error::Error for RuntimeError {}
