use std::rc::Rc;

use crate::{
    parser::{Param, Stmt},
    runtime::NamespaceRef,
};

pub type FunctionRef = Rc<FunctionValue>;

#[derive(Debug)]
pub struct FunctionValue {
    pub params: Vec<Param>,
    pub body: Vec<Stmt>,
    pub namespace: Option<NamespaceRef>,
}
