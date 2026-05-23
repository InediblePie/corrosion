use crate::runtime::RuntimeError;

pub trait ImportResolver {
    fn resolve(&self, from: Option<&str>, path: &str) -> Result<ResolvedModule, RuntimeError>;
}

#[derive(Debug, Clone)]
pub struct ResolvedModule {
    pub id: String,
    pub source: String,
}

#[derive(Debug, Default)]
pub struct NoopImportResolver;

impl ImportResolver for NoopImportResolver {
    fn resolve(&self, _from: Option<&str>, path: &str) -> Result<ResolvedModule, RuntimeError> {
        Err(RuntimeError::new(
            format!("no import resolver configured for '{path}'"),
            None,
        ))
    }
}
