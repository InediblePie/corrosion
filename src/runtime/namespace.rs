use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::runtime::Value;

pub type NamespaceRef = Rc<RefCell<NamespaceValue>>;

#[derive(Debug)]
pub struct NamespaceValue {
    pub name: String,
    pub values: HashMap<String, NamespaceEntry>,
    pub children: HashMap<String, NamespaceRef>,
}

#[derive(Debug, Clone)]
pub struct NamespaceEntry {
    pub value: Value,
    pub is_extern: bool,
}
