#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub module_id: Option<String>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self { module_id: None }
    }
}
