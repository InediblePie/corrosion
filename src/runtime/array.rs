use std::{cell::RefCell, rc::Rc};

use crate::runtime::Value;

pub type ArrayRef = Rc<RefCell<ArrayValue>>;

#[derive(Debug)]
pub struct ArrayValue {
    pub id: usize,
    pub values: Vec<Value>,
}
