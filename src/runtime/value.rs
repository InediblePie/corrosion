use std::fmt;

use crate::runtime::{ArrayRef, FunctionRef, NamespaceRef, NativeFunction, ObjectRef};

#[derive(Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(ArrayRef),
    Object(ObjectRef),
    Function(FunctionRef),
    NativeFunction(NativeFunction),
    Namespace(NamespaceRef),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "Null"),
            Value::Bool(value) => write!(f, "Bool({value})"),
            Value::Int(value) => write!(f, "Int({value})"),
            Value::Float(value) => write!(f, "Float({value})"),
            Value::String(value) => write!(f, "String({value:?})"),
            Value::Array(array) => write!(f, "Array({})", array.borrow().id),
            Value::Object(object) => write!(f, "Object({})", object.borrow().id),
            Value::Function(_) => write!(f, "Function"),
            Value::NativeFunction(_) => write!(f, "NativeFunction"),
            Value::Namespace(namespace) => write!(f, "Namespace({})", namespace.borrow().name),
        }
    }
}

impl Value {
    pub fn is_truthy_condition(&self) -> Option<bool> {
        match self {
            Value::Bool(value) => Some(*value),
            Value::Null => Some(false),
            _ => None,
        }
    }
}
