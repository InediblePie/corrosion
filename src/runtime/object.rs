use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::runtime::Value;

pub type ObjectRef = Rc<RefCell<ObjectValue>>;

#[derive(Debug)]
pub struct ObjectValue {
    pub id: usize,
    pub fields: HashMap<String, Value>,
}
