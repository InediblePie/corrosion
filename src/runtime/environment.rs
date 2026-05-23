use std::collections::HashMap;

use crate::runtime::{NamespaceRef, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Global,
    Namespace,
    Function,
}

#[derive(Debug)]
pub struct Scope {
    pub kind: ScopeKind,
    pub values: HashMap<String, Value>,
    pub namespace: Option<NamespaceRef>,
}

impl Scope {
    pub fn new(kind: ScopeKind) -> Self {
        Self {
            kind,
            values: HashMap::new(),
            namespace: None,
        }
    }

    pub fn namespace(namespace: NamespaceRef) -> Self {
        Self {
            kind: ScopeKind::Namespace,
            values: HashMap::new(),
            namespace: Some(namespace),
        }
    }
}
