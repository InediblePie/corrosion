pub mod array;
pub mod environment;
pub mod error;
pub mod function;
pub mod namespace;
pub mod native;
pub mod object;
pub mod value;

pub use array::{ArrayRef, ArrayValue};
pub use environment::{Scope, ScopeKind};
pub use error::RuntimeError;
pub use function::{FunctionRef, FunctionValue};
pub use namespace::{NamespaceEntry, NamespaceRef, NamespaceValue};
pub use native::NativeFunction;
pub use object::{ObjectRef, ObjectValue};
pub use value::Value;

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::{
    engine::{Engine, Script},
    error::{ScriptError, ScriptResult},
    parser::{BinaryOp, Expr, Program, Stmt, UnaryOp},
    source::SourceSpan,
};

enum ExecFlow {
    Continue(Value),
    Return(Value),
}

pub struct Runtime {
    pub engine: Engine,
    pub scopes: Vec<Scope>,
    pub imports: HashSet<String>,
    next_id: usize,
    current_module: Option<String>,
}

impl Runtime {
    pub fn new(engine: Engine) -> Self {
        let mut runtime = Self {
            engine,
            scopes: vec![Scope::new(ScopeKind::Global)],
            imports: HashSet::new(),
            next_id: 1,
            current_module: None,
        };
        let natives = runtime.engine.natives.clone();
        for (name, function) in natives {
            runtime
                .scopes
                .last_mut()
                .unwrap()
                .values
                .insert(name, Value::NativeFunction(function));
        }
        runtime
    }

    pub fn execute(&mut self, script: &Script) -> ScriptResult<Value> {
        let previous_module = self.current_module.clone();
        self.current_module = script.module_id.clone();
        let result = self
            .execute_program(&script.program)
            .map_err(ScriptError::from);
        self.current_module = previous_module;
        result
    }

    pub fn set_global(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        let global = self.scopes.first_mut().unwrap();
        if global.values.contains_key(name) {
            return Err(RuntimeError::new(
                format!("global '{name}' is already defined"),
                None,
            ));
        }
        global.values.insert(name.to_string(), value);
        Ok(())
    }

    pub fn get_global(&self, name: &str) -> Option<Value> {
        self.scopes
            .first()
            .and_then(|scope| scope.values.get(name).cloned())
    }

    fn execute_program(&mut self, program: &Program) -> Result<Value, RuntimeError> {
        let mut last = Value::Null;
        for stmt in &program.statements {
            match self.execute_stmt(stmt)? {
                ExecFlow::Continue(value) => last = value,
                ExecFlow::Return(value) => return Ok(value),
            }
        }
        Ok(last)
    }

    fn execute_stmt(&mut self, stmt: &Stmt) -> Result<ExecFlow, RuntimeError> {
        match stmt {
            Stmt::Import { path, span } => {
                self.execute_import(path, *span)?;
                Ok(ExecFlow::Continue(Value::Null))
            }
            Stmt::Namespace { path, body, .. } => {
                let namespace = self.get_or_create_namespace(path);
                self.scopes.push(self.namespace_scope(namespace.clone()));
                for stmt in body {
                    if let ExecFlow::Return(value) = self.execute_stmt(stmt)? {
                        self.scopes.pop();
                        return Ok(ExecFlow::Return(value));
                    }
                }
                self.scopes.pop();
                Ok(ExecFlow::Continue(Value::Null))
            }
            Stmt::Let {
                name,
                value,
                is_extern,
                span,
            } => {
                let value = self.evaluate(value)?;
                self.declare(name, value, *is_extern, *span)?;
                Ok(ExecFlow::Continue(Value::Null))
            }
            Stmt::Expr { expr, .. } => Ok(ExecFlow::Continue(self.evaluate(expr)?)),
            Stmt::Return { value, .. } => {
                let value = match value {
                    Some(expr) => self.evaluate(expr)?,
                    None => Value::Null,
                };
                Ok(ExecFlow::Return(value))
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                let condition_value = self.evaluate(condition)?;
                if self.condition(condition_value, *span)? {
                    self.execute_statements(then_branch)
                } else if let Some(else_branch) = else_branch {
                    self.execute_statements(else_branch)
                } else {
                    Ok(ExecFlow::Continue(Value::Null))
                }
            }
            Stmt::While {
                condition,
                body,
                span,
            } => {
                loop {
                    let condition_value = self.evaluate(condition)?;
                    if !self.condition(condition_value, *span)? {
                        break;
                    }
                    if let ExecFlow::Return(value) = self.execute_statements(body)? {
                        return Ok(ExecFlow::Return(value));
                    }
                }
                Ok(ExecFlow::Continue(Value::Null))
            }
            Stmt::For {
                initializer,
                condition,
                increment,
                body,
                span,
            } => {
                if let Some(initializer) = initializer {
                    self.execute_stmt(initializer)?;
                }
                loop {
                    if let Some(condition) = condition {
                        let condition_value = self.evaluate(condition)?;
                        if !self.condition(condition_value, *span)? {
                            break;
                        }
                    }
                    if let ExecFlow::Return(value) = self.execute_statements(body)? {
                        return Ok(ExecFlow::Return(value));
                    }
                    if let Some(increment) = increment {
                        self.evaluate(increment)?;
                    }
                }
                Ok(ExecFlow::Continue(Value::Null))
            }
        }
    }

    fn execute_statements(&mut self, statements: &[Stmt]) -> Result<ExecFlow, RuntimeError> {
        let mut last = Value::Null;
        for stmt in statements {
            match self.execute_stmt(stmt)? {
                ExecFlow::Continue(value) => last = value,
                ExecFlow::Return(value) => return Ok(ExecFlow::Return(value)),
            }
        }
        Ok(ExecFlow::Continue(last))
    }

    fn evaluate(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Null(_) => Ok(Value::Null),
            Expr::Bool(value, _) => Ok(Value::Bool(*value)),
            Expr::Int(value, _) => Ok(Value::Int(*value)),
            Expr::Float(value, _) => Ok(Value::Float(*value)),
            Expr::String(value, _) => Ok(Value::String(value.clone())),
            Expr::Variable { name, span } => self.lookup(name, *span),
            Expr::Function { params, body, .. } => Ok(Value::Function(Rc::new(FunctionValue {
                params: params.clone(),
                body: body.clone(),
                namespace: self.current_namespace(),
            }))),
            Expr::Struct { fields, span } => self.evaluate_struct(fields, *span),
            Expr::Array { values, .. } => {
                let mut result = Vec::new();
                for value in values {
                    result.push(self.evaluate(value)?);
                }
                Ok(Value::Array(self.new_array(result)))
            }
            Expr::NewArray { size, span } => {
                let size = self.evaluate(size)?;
                let Value::Int(size) = size else {
                    return Err(RuntimeError::new(
                        "array size must be an integer",
                        Some(*span),
                    ));
                };
                if size < 0 {
                    return Err(RuntimeError::new(
                        "array size cannot be negative",
                        Some(*span),
                    ));
                }
                Ok(Value::Array(
                    self.new_array(vec![Value::Null; size as usize]),
                ))
            }
            Expr::Unary { op, expr, span } => {
                let value = self.evaluate(expr)?;
                self.evaluate_unary(*op, value, *span)
            }
            Expr::Binary {
                left,
                op,
                right,
                span,
            } => self.evaluate_binary(left, *op, right, *span),
            Expr::Assignment {
                target,
                value,
                span,
            } => {
                let value = self.evaluate(value)?;
                self.assign(target, value, *span)
            }
            Expr::Call { callee, args, span } => {
                let callee = self.evaluate(callee)?;
                let args = self.evaluate_args(args)?;
                self.call_value(callee, args, *span)
            }
            Expr::Member {
                object,
                field,
                span,
            } => {
                let object = self.evaluate(object)?;
                self.get_member(object, field, *span)
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                span,
            } => {
                let receiver = self.evaluate(receiver)?;
                let callee = self.get_member(receiver.clone(), method, *span)?;
                let mut values = vec![receiver];
                values.extend(self.evaluate_args(args)?);
                self.call_value(callee, values, *span)
            }
            Expr::Index {
                object,
                index,
                span,
            } => {
                let object = self.evaluate(object)?;
                let index = self.evaluate(index)?;
                self.index_value(object, index, *span)
            }
            Expr::Slice {
                object,
                start,
                end,
                span,
            } => {
                let object = self.evaluate(object)?;
                let start = match start {
                    Some(expr) => Some(self.evaluate(expr)?),
                    None => None,
                };
                let end = match end {
                    Some(expr) => Some(self.evaluate(expr)?),
                    None => None,
                };
                self.slice_value(object, start, end, *span)
            }
        }
    }

    fn evaluate_args(&mut self, args: &[Expr]) -> Result<Vec<Value>, RuntimeError> {
        let mut values = Vec::new();
        for arg in args {
            values.push(self.evaluate(arg)?);
        }
        Ok(values)
    }

    fn evaluate_struct(
        &mut self,
        fields: &[(String, Expr)],
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        let mut map = HashMap::new();
        for (name, expr) in fields {
            if map.contains_key(name) {
                return Err(RuntimeError::new(
                    format!("duplicate struct field '{name}'"),
                    Some(span),
                ));
            }
            map.insert(name.clone(), self.evaluate(expr)?);
        }
        let id = self.next_id();
        Ok(Value::Object(Rc::new(RefCell::new(ObjectValue {
            id,
            fields: map,
        }))))
    }

    fn evaluate_unary(
        &self,
        op: UnaryOp,
        value: Value,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        match op {
            UnaryOp::Negate => match value {
                Value::Int(value) => Ok(Value::Int(-value)),
                Value::Float(value) => Ok(Value::Float(-value)),
                _ => Err(RuntimeError::new("unary '-' requires a number", Some(span))),
            },
            UnaryOp::Not => match value.is_truthy_condition() {
                Some(value) => Ok(Value::Bool(!value)),
                None => Err(RuntimeError::new(
                    "'!' requires a boolean or null",
                    Some(span),
                )),
            },
        }
    }

    fn evaluate_binary(
        &mut self,
        left: &Expr,
        op: BinaryOp,
        right: &Expr,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        if op == BinaryOp::And {
            let left = self.evaluate(left)?;
            if !self.condition(left, span)? {
                return Ok(Value::Bool(false));
            }
            let right = self.evaluate(right)?;
            return Ok(Value::Bool(self.condition(right, span)?));
        }
        if op == BinaryOp::Or {
            let left = self.evaluate(left)?;
            if self.condition(left, span)? {
                return Ok(Value::Bool(true));
            }
            let right = self.evaluate(right)?;
            return Ok(Value::Bool(self.condition(right, span)?));
        }

        let left = self.evaluate(left)?;
        let right = self.evaluate(right)?;
        match op {
            BinaryOp::Add => self.add_values(left, right, span),
            BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Divide | BinaryOp::Modulo => {
                self.numeric_values(left, right, op, span)
            }
            BinaryOp::Equal => Ok(Value::Bool(self.values_equal(&left, &right))),
            BinaryOp::NotEqual => Ok(Value::Bool(!self.values_equal(&left, &right))),
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                self.compare_values(left, right, op, span)
            }
            BinaryOp::And | BinaryOp::Or => unreachable!(),
        }
    }

    fn add_values(
        &mut self,
        left: Value,
        right: Value,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        match (&left, &right) {
            (Value::String(_), _) | (_, Value::String(_)) => Ok(Value::String(format!(
                "{}{}",
                self.value_to_string(left)?,
                self.value_to_string(right)?
            ))),
            (Value::Array(left), Value::Array(right)) => {
                let mut values = left.borrow().values.clone();
                values.extend(right.borrow().values.clone());
                Ok(Value::Array(self.new_array(values)))
            }
            _ => self.numeric_values(left, right, BinaryOp::Add, span),
        }
    }

    fn numeric_values(
        &self,
        left: Value,
        right: Value,
        op: BinaryOp,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Int(left), Value::Int(right)) => match op {
                BinaryOp::Add => Ok(Value::Int(left + right)),
                BinaryOp::Subtract => Ok(Value::Int(left - right)),
                BinaryOp::Multiply => Ok(Value::Int(left * right)),
                BinaryOp::Divide => {
                    if right == 0 {
                        Err(RuntimeError::new("division by zero", Some(span)))
                    } else {
                        Ok(Value::Int(left / right))
                    }
                }
                BinaryOp::Modulo => {
                    if right == 0 {
                        Err(RuntimeError::new("modulus by zero", Some(span)))
                    } else {
                        Ok(Value::Int(left % right))
                    }
                }
                _ => unreachable!(),
            },
            (left, right) => {
                if op == BinaryOp::Modulo {
                    return Err(RuntimeError::new(
                        "modulus requires integer operands",
                        Some(span),
                    ));
                }
                let Some(left) = value_as_float(&left) else {
                    return Err(RuntimeError::new(
                        "numeric operation requires numbers",
                        Some(span),
                    ));
                };
                let Some(right) = value_as_float(&right) else {
                    return Err(RuntimeError::new(
                        "numeric operation requires numbers",
                        Some(span),
                    ));
                };
                let result = match op {
                    BinaryOp::Add => left + right,
                    BinaryOp::Subtract => left - right,
                    BinaryOp::Multiply => left * right,
                    BinaryOp::Divide => {
                        if right == 0.0 {
                            return Err(RuntimeError::new("division by zero", Some(span)));
                        }
                        left / right
                    }
                    _ => unreachable!(),
                };
                Ok(Value::Float(result))
            }
        }
    }

    fn compare_values(
        &self,
        left: Value,
        right: Value,
        op: BinaryOp,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        let result = match (&left, &right) {
            (Value::Int(_), _)
            | (Value::Float(_), _)
            | (_, Value::Int(_))
            | (_, Value::Float(_)) => {
                let Some(left) = value_as_float(&left) else {
                    return Err(RuntimeError::new("comparison requires numbers", Some(span)));
                };
                let Some(right) = value_as_float(&right) else {
                    return Err(RuntimeError::new("comparison requires numbers", Some(span)));
                };
                match op {
                    BinaryOp::Less => left < right,
                    BinaryOp::LessEqual => left <= right,
                    BinaryOp::Greater => left > right,
                    BinaryOp::GreaterEqual => left >= right,
                    _ => unreachable!(),
                }
            }
            (Value::String(left), Value::String(right)) => match op {
                BinaryOp::Less => left < right,
                BinaryOp::LessEqual => left <= right,
                BinaryOp::Greater => left > right,
                BinaryOp::GreaterEqual => left >= right,
                _ => unreachable!(),
            },
            _ => {
                return Err(RuntimeError::new(
                    "comparison requires matching strings or numbers",
                    Some(span),
                ));
            }
        };
        Ok(Value::Bool(result))
    }

    fn assign(
        &mut self,
        target: &Expr,
        value: Value,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        match target {
            Expr::Variable { name, .. } => {
                self.assign_name(name, value.clone(), span)?;
                Ok(value)
            }
            Expr::Member { object, field, .. } => {
                let object = self.evaluate(object)?;
                self.set_member(object, field, value.clone(), span)?;
                Ok(value)
            }
            Expr::Index { object, index, .. } => {
                let object = self.evaluate(object)?;
                let index = self.evaluate(index)?;
                self.set_index(object, index, value.clone(), span)?;
                Ok(value)
            }
            _ => Err(RuntimeError::new("invalid assignment target", Some(span))),
        }
    }

    fn call_value(
        &mut self,
        callee: Value,
        args: Vec<Value>,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        match callee {
            Value::Function(function) => {
                let pushed_namespace = if let Some(namespace) = &function.namespace {
                    self.scopes.push(self.namespace_scope(namespace.clone()));
                    true
                } else {
                    false
                };
                let mut scope = Scope::new(ScopeKind::Function);
                let mut arg_index = 0;
                for param in &function.params {
                    let value = if param.is_rest {
                        let rest = args[arg_index.min(args.len())..].to_vec();
                        arg_index = args.len();
                        Value::Array(self.new_array(rest))
                    } else {
                        let value = args.get(arg_index).cloned().unwrap_or(Value::Null);
                        arg_index += 1;
                        value
                    };
                    scope.values.insert(param.name.clone(), value);
                }
                self.scopes.push(scope);
                let result = match self.execute_statements(&function.body)? {
                    ExecFlow::Continue(_) => Value::Null,
                    ExecFlow::Return(value) => value,
                };
                self.scopes.pop();
                if pushed_namespace {
                    self.scopes.pop();
                }
                Ok(result)
            }
            Value::NativeFunction(function) => function(self, args),
            _ => Err(RuntimeError::new(
                "cannot call non-function value",
                Some(span),
            )),
        }
    }

    fn get_member(
        &mut self,
        object: Value,
        field: &str,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        match object {
            Value::Object(object) => object.borrow().fields.get(field).cloned().ok_or_else(|| {
                RuntimeError::new(format!("unknown object field '{field}'"), Some(span))
            }),
            Value::Array(array) => match field {
                "length" => Ok(Value::Int(array.borrow().values.len() as i64)),
                "push" | "push_front" | "pop" | "pop_front" => {
                    Ok(Value::NativeFunction(array_method(field)))
                }
                _ => Err(RuntimeError::new(
                    format!("unknown array field '{field}'"),
                    Some(span),
                )),
            },
            Value::Namespace(namespace) => {
                let namespace = namespace.borrow();
                if let Some(child) = namespace.children.get(field) {
                    return Ok(Value::Namespace(child.clone()));
                }
                if let Some(entry) = namespace.values.get(field) {
                    if entry.is_extern || self.is_current_namespace_name(&namespace.name) {
                        return Ok(entry.value.clone());
                    }
                    return Err(RuntimeError::new(
                        format!("namespace field '{field}' is private"),
                        Some(span),
                    ));
                }
                Err(RuntimeError::new(
                    format!("unknown namespace field '{field}'"),
                    Some(span),
                ))
            }
            _ => Err(RuntimeError::new(
                "member access requires an object, array, or namespace",
                Some(span),
            )),
        }
    }

    fn set_member(
        &mut self,
        object: Value,
        field: &str,
        value: Value,
        span: SourceSpan,
    ) -> Result<(), RuntimeError> {
        match object {
            Value::Object(object) => {
                object.borrow_mut().fields.insert(field.to_string(), value);
                Ok(())
            }
            Value::Namespace(namespace) => {
                let namespace_name = namespace.borrow().name.clone();
                let is_internal = self.is_current_namespace_name(&namespace_name);
                let mut namespace = namespace.borrow_mut();
                if let Some(entry) = namespace.values.get_mut(field) {
                    if entry.is_extern || is_internal {
                        entry.value = value;
                        return Ok(());
                    }
                    return Err(RuntimeError::new(
                        format!("namespace field '{field}' is private"),
                        Some(span),
                    ));
                }
                Err(RuntimeError::new(
                    format!("cannot add new namespace field '{field}' from assignment"),
                    Some(span),
                ))
            }
            _ => Err(RuntimeError::new(
                "field assignment requires an object or namespace",
                Some(span),
            )),
        }
    }

    fn index_value(
        &self,
        object: Value,
        index: Value,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        let Value::Array(array) = object else {
            return Err(RuntimeError::new("indexing requires an array", Some(span)));
        };
        let index = index_to_usize(index, span)?;
        array.borrow().values.get(index).cloned().ok_or_else(|| {
            RuntimeError::new(format!("array index {index} out of bounds"), Some(span))
        })
    }

    fn set_index(
        &self,
        object: Value,
        index: Value,
        value: Value,
        span: SourceSpan,
    ) -> Result<(), RuntimeError> {
        let Value::Array(array) = object else {
            return Err(RuntimeError::new(
                "index assignment requires an array",
                Some(span),
            ));
        };
        let index = index_to_usize(index, span)?;
        let mut array = array.borrow_mut();
        if index >= array.values.len() {
            return Err(RuntimeError::new(
                format!("array index {index} out of bounds"),
                Some(span),
            ));
        }
        array.values[index] = value;
        Ok(())
    }

    fn slice_value(
        &mut self,
        object: Value,
        start: Option<Value>,
        end: Option<Value>,
        span: SourceSpan,
    ) -> Result<Value, RuntimeError> {
        let Value::Array(array) = object else {
            return Err(RuntimeError::new("slicing requires an array", Some(span)));
        };
        let array = array.borrow();
        let start = match start {
            Some(value) => index_to_usize(value, span)?,
            None => 0,
        };
        let end = match end {
            Some(value) => index_to_usize(value, span)?,
            None => array.values.len(),
        };
        if start > end || end > array.values.len() {
            return Err(RuntimeError::new("invalid slice range", Some(span)));
        }
        Ok(Value::Array(
            self.new_array(array.values[start..end].to_vec()),
        ))
    }

    fn declare(
        &mut self,
        name: &str,
        value: Value,
        is_extern: bool,
        span: SourceSpan,
    ) -> Result<(), RuntimeError> {
        let scope = self.scopes.last_mut().unwrap();
        if let Some(namespace) = &scope.namespace {
            let mut namespace = namespace.borrow_mut();
            if namespace.values.contains_key(name) || namespace.children.contains_key(name) {
                return Err(RuntimeError::new(
                    format!("'{name}' is already defined in this namespace"),
                    Some(span),
                ));
            }
            namespace.values.insert(
                name.to_string(),
                NamespaceEntry {
                    value: value.clone(),
                    is_extern,
                },
            );
        } else if scope.values.contains_key(name) {
            return Err(RuntimeError::new(
                format!("'{name}' is already defined in this scope"),
                Some(span),
            ));
        }
        scope.values.insert(name.to_string(), value);
        Ok(())
    }

    fn lookup(&self, name: &str, span: SourceSpan) -> Result<Value, RuntimeError> {
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.values.get(name) {
                return Ok(value.clone());
            }
            if let Some(namespace) = &scope.namespace {
                let namespace = namespace.borrow();
                if let Some(child) = namespace.children.get(name) {
                    return Ok(Value::Namespace(child.clone()));
                }
                if let Some(entry) = namespace.values.get(name) {
                    return Ok(entry.value.clone());
                }
            }
        }
        Err(RuntimeError::new(
            format!("undefined variable '{name}'"),
            Some(span),
        ))
    }

    fn assign_name(
        &mut self,
        name: &str,
        value: Value,
        span: SourceSpan,
    ) -> Result<(), RuntimeError> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.values.contains_key(name) {
                scope.values.insert(name.to_string(), value.clone());
                if let Some(namespace) = &scope.namespace {
                    if let Some(entry) = namespace.borrow_mut().values.get_mut(name) {
                        entry.value = value;
                    }
                }
                return Ok(());
            }
            if let Some(namespace) = &scope.namespace {
                if let Some(entry) = namespace.borrow_mut().values.get_mut(name) {
                    entry.value = value;
                    return Ok(());
                }
            }
        }
        Err(RuntimeError::new(
            format!("cannot assign undefined variable '{name}'"),
            Some(span),
        ))
    }

    fn execute_import(&mut self, path: &str, span: SourceSpan) -> Result<(), RuntimeError> {
        let resolver = self.engine.resolver.clone();
        let resolved = resolver
            .resolve(self.current_module.as_deref(), path)
            .map_err(|mut error| {
                if error.span.is_none() {
                    error.span = Some(span);
                }
                error
            })?;
        if self.imports.contains(&resolved.id) {
            return Ok(());
        }
        self.imports.insert(resolved.id.clone());
        let script = self
            .engine
            .compile_module(&resolved.source, Some(resolved.id.clone()))
            .map_err(|error| match error {
                ScriptError::Runtime(error) => error,
                other => RuntimeError::new(other.to_string(), Some(span)),
            })?;
        self.execute(&script).map_err(|error| match error {
            ScriptError::Runtime(error) => error,
            other => RuntimeError::new(other.to_string(), Some(span)),
        })?;
        Ok(())
    }

    fn get_or_create_namespace(&mut self, path: &[String]) -> NamespaceRef {
        let mut current = {
            let name = &path[0];
            if let Some(Value::Namespace(namespace)) = self.scopes[0].values.get(name) {
                namespace.clone()
            } else {
                let namespace = Rc::new(RefCell::new(NamespaceValue {
                    name: name.clone(),
                    values: HashMap::new(),
                    children: HashMap::new(),
                }));
                self.scopes[0]
                    .values
                    .insert(name.clone(), Value::Namespace(namespace.clone()));
                namespace
            }
        };

        for part in path.iter().skip(1) {
            let next = {
                let mut current_mut = current.borrow_mut();
                if let Some(child) = current_mut.children.get(part) {
                    child.clone()
                } else {
                    let full_name = format!("{}.{}", current_mut.name, part);
                    let child = Rc::new(RefCell::new(NamespaceValue {
                        name: full_name,
                        values: HashMap::new(),
                        children: HashMap::new(),
                    }));
                    current_mut.children.insert(part.clone(), child.clone());
                    child
                }
            };
            current = next;
        }
        current
    }

    fn namespace_scope(&self, namespace: NamespaceRef) -> Scope {
        let namespace_borrow = namespace.borrow();
        let mut scope = Scope::namespace(namespace.clone());
        for (name, entry) in &namespace_borrow.values {
            scope.values.insert(name.clone(), entry.value.clone());
        }
        for (name, child) in &namespace_borrow.children {
            scope
                .values
                .insert(name.clone(), Value::Namespace(child.clone()));
        }
        drop(namespace_borrow);
        scope
    }

    fn condition(&self, value: Value, span: SourceSpan) -> Result<bool, RuntimeError> {
        value
            .is_truthy_condition()
            .ok_or_else(|| RuntimeError::new("condition must be a boolean or null", Some(span)))
    }

    fn current_namespace(&self) -> Option<NamespaceRef> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.namespace.clone())
    }

    fn is_current_namespace_name(&self, name: &str) -> bool {
        self.current_namespace()
            .map(|namespace| namespace.borrow().name == name)
            .unwrap_or(false)
    }

    fn new_array(&mut self, values: Vec<Value>) -> ArrayRef {
        let id = self.next_id();
        Rc::new(RefCell::new(ArrayValue { id, values }))
    }

    fn next_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn value_to_string(&mut self, value: Value) -> Result<String, RuntimeError> {
        match value {
            Value::Null => Ok("null".to_string()),
            Value::Bool(value) => Ok(value.to_string()),
            Value::Int(value) => Ok(value.to_string()),
            Value::Float(value) => Ok(value.to_string()),
            Value::String(value) => Ok(value),
            Value::Array(array) => Ok(format!("<array {}>", array.borrow().id)),
            Value::Object(object) => {
                let to_string = object.borrow().fields.get("to_string").cloned();
                if let Some(callee) = to_string {
                    let text = self.call_value(
                        callee,
                        vec![Value::Object(object.clone())],
                        SourceSpan::default(),
                    )?;
                    if let Value::String(text) = text {
                        return Ok(text);
                    }
                }
                Ok(format!("<object {}>", object.borrow().id))
            }
            Value::Function(function) => Ok(format!("<function {:p}>", Rc::as_ptr(&function))),
            Value::NativeFunction(_) => Ok("<native function>".to_string()),
            Value::Namespace(namespace) => Ok(format!("<namespace {}>", namespace.borrow().name)),
        }
    }

    pub fn convert_to_int(&mut self, value: Value) -> Result<Value, RuntimeError> {
        match value {
            Value::Int(value) => Ok(Value::Int(value)),
            Value::Float(value) => Ok(Value::Int(value as i64)),
            Value::String(value) => {
                if let Ok(value) = value.parse::<i64>() {
                    return Ok(Value::Int(value));
                }
                if let Ok(value) = value.parse::<f64>() {
                    return Ok(Value::Int(value as i64));
                }
                Ok(self.conversion_error("Cannot convert value to Int"))
            }
            _ => Ok(self.conversion_error("Cannot convert value to Int")),
        }
    }

    pub fn convert_to_float(&mut self, value: Value) -> Result<Value, RuntimeError> {
        match value {
            Value::Int(value) => Ok(Value::Float(value as f64)),
            Value::Float(value) => Ok(Value::Float(value)),
            Value::String(value) => match value.parse::<f64>() {
                Ok(value) => Ok(Value::Float(value)),
                Err(_) => Ok(self.conversion_error("Cannot convert value to Float")),
            },
            _ => Ok(self.conversion_error("Cannot convert value to Float")),
        }
    }

    fn conversion_error(&mut self, message: &str) -> Value {
        Value::Array(self.new_array(vec![Value::String(message.to_string()), Value::Bool(false)]))
    }

    fn values_equal(&self, left: &Value, right: &Value) -> bool {
        match (left, right) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(left), Value::Bool(right)) => left == right,
            (Value::Int(left), Value::Int(right)) => left == right,
            (Value::Float(left), Value::Float(right)) => left == right,
            (Value::Int(left), Value::Float(right)) => (*left as f64) == *right,
            (Value::Float(left), Value::Int(right)) => *left == (*right as f64),
            (Value::String(left), Value::String(right)) => left == right,
            (Value::Array(left), Value::Array(right)) => Rc::ptr_eq(left, right),
            (Value::Object(left), Value::Object(right)) => Rc::ptr_eq(left, right),
            (Value::Function(left), Value::Function(right)) => Rc::ptr_eq(left, right),
            (Value::Namespace(left), Value::Namespace(right)) => Rc::ptr_eq(left, right),
            _ => false,
        }
    }
}

fn value_as_float(value: &Value) -> Option<f64> {
    match value {
        Value::Int(value) => Some(*value as f64),
        Value::Float(value) => Some(*value),
        _ => None,
    }
}

fn index_to_usize(value: Value, span: SourceSpan) -> Result<usize, RuntimeError> {
    match value {
        Value::Int(value) if value >= 0 => Ok(value as usize),
        Value::Int(_) => Err(RuntimeError::new("index cannot be negative", Some(span))),
        _ => Err(RuntimeError::new("index must be an integer", Some(span))),
    }
}

fn array_method(name: &str) -> NativeFunction {
    let name = name.to_string();
    Rc::new(move |_runtime, args| {
        let Some(Value::Array(array)) = args.get(0).cloned() else {
            return Err(RuntimeError::new(
                format!("array method '{}' requires an array receiver", name),
                None,
            ));
        };
        match name.as_str() {
            "push" => {
                let value = args.get(1).cloned().unwrap_or(Value::Null);
                array.borrow_mut().values.push(value);
                Ok(Value::Array(array))
            }
            "push_front" => {
                let value = args.get(1).cloned().unwrap_or(Value::Null);
                array.borrow_mut().values.insert(0, value);
                Ok(Value::Array(array))
            }
            "pop" => array
                .borrow_mut()
                .values
                .pop()
                .ok_or_else(|| RuntimeError::new("cannot pop from empty array", None)),
            "pop_front" => {
                let mut array = array.borrow_mut();
                if array.values.is_empty() {
                    Err(RuntimeError::new("cannot pop from empty array", None))
                } else {
                    Ok(array.values.remove(0))
                }
            }
            _ => unreachable!(),
        }
    })
}
