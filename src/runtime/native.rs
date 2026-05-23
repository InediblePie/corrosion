use std::rc::Rc;

use crate::runtime::{Runtime, RuntimeError, Value};

pub type NativeFunction = Rc<dyn Fn(&mut Runtime, Vec<Value>) -> Result<Value, RuntimeError>>;
