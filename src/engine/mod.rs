pub mod config;
pub mod import;

pub use config::EngineConfig;
pub use import::{ImportResolver, NoopImportResolver, ResolvedModule};

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    error::ScriptError,
    lexer,
    parser::{self, Program},
    runtime::{NativeFunction, RuntimeError, Value},
};

#[derive(Clone)]
pub struct Script {
    pub program: Program,
    pub module_id: Option<String>,
}

#[derive(Clone)]
pub struct Engine {
    pub(crate) natives: HashMap<String, NativeFunction>,
    pub(crate) resolver: Rc<dyn ImportResolver>,
    pub(crate) output: Rc<RefCell<Vec<String>>>,
}

impl Engine {
    pub fn new() -> Self {
        let mut engine = Self {
            natives: HashMap::new(),
            resolver: Rc::new(NoopImportResolver),
            output: Rc::new(RefCell::new(Vec::new())),
        };
        engine.register_default_builtins();
        engine
    }

    pub fn compile(&self, source: &str) -> Result<Script, ScriptError> {
        self.compile_module(source, None)
    }

    pub fn compile_module(
        &self,
        source: &str,
        module_id: Option<String>,
    ) -> Result<Script, ScriptError> {
        let tokens = lexer::lex(source)?;
        let program = parser::parse(tokens)?;
        Ok(Script { program, module_id })
    }

    pub fn register_native<F>(&mut self, name: &str, function: F)
    where
        F: Fn(&mut crate::runtime::Runtime, Vec<Value>) -> Result<Value, RuntimeError> + 'static,
    {
        self.natives.insert(name.to_string(), Rc::new(function));
    }

    pub fn set_import_resolver<R>(&mut self, resolver: R)
    where
        R: ImportResolver + 'static,
    {
        self.resolver = Rc::new(resolver);
    }

    pub fn take_output(&self) -> Vec<String> {
        self.output.borrow_mut().drain(..).collect()
    }

    pub fn output(&self) -> Vec<String> {
        self.output.borrow().clone()
    }

    fn register_default_builtins(&mut self) {
        self.register_native("print", |runtime, args| {
            let value = args.get(0).cloned().unwrap_or(Value::Null);
            let text = runtime.value_to_string(value)?;
            runtime.engine.output.borrow_mut().push(text);
            Ok(Value::Null)
        });

        self.register_native("String", |runtime, args| {
            let value = args.get(0).cloned().unwrap_or(Value::Null);
            Ok(Value::String(runtime.value_to_string(value)?))
        });

        self.register_native("Int", |runtime, args| {
            let value = args.get(0).cloned().unwrap_or(Value::Null);
            runtime.convert_to_int(value)
        });

        self.register_native("Float", |runtime, args| {
            let value = args.get(0).cloned().unwrap_or(Value::Null);
            runtime.convert_to_float(value)
        });
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
