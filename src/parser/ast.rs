use crate::source::SourceSpan;

#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Import {
        path: String,
        span: SourceSpan,
    },
    Namespace {
        path: Vec<String>,
        body: Vec<Stmt>,
        span: SourceSpan,
    },
    Let {
        name: String,
        value: Expr,
        is_extern: bool,
        span: SourceSpan,
    },
    Expr {
        expr: Expr,
        span: SourceSpan,
    },
    Return {
        value: Option<Expr>,
        span: SourceSpan,
    },
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
        span: SourceSpan,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
        span: SourceSpan,
    },
    For {
        initializer: Option<Box<Stmt>>,
        condition: Option<Expr>,
        increment: Option<Expr>,
        body: Vec<Stmt>,
        span: SourceSpan,
    },
}

impl Stmt {
    pub fn span(&self) -> SourceSpan {
        match self {
            Stmt::Import { span, .. }
            | Stmt::Namespace { span, .. }
            | Stmt::Let { span, .. }
            | Stmt::Expr { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::If { span, .. }
            | Stmt::While { span, .. }
            | Stmt::For { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    Null(SourceSpan),
    Bool(bool, SourceSpan),
    Int(i64, SourceSpan),
    Float(f64, SourceSpan),
    String(String, SourceSpan),
    Variable {
        name: String,
        span: SourceSpan,
    },
    Function {
        params: Vec<Param>,
        body: Vec<Stmt>,
        span: SourceSpan,
    },
    Struct {
        fields: Vec<(String, Expr)>,
        span: SourceSpan,
    },
    Array {
        values: Vec<Expr>,
        span: SourceSpan,
    },
    NewArray {
        size: Box<Expr>,
        span: SourceSpan,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: SourceSpan,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: SourceSpan,
    },
    Assignment {
        target: Box<Expr>,
        value: Box<Expr>,
        span: SourceSpan,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: SourceSpan,
    },
    Member {
        object: Box<Expr>,
        field: String,
        span: SourceSpan,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        span: SourceSpan,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        span: SourceSpan,
    },
    Slice {
        object: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        span: SourceSpan,
    },
}

impl Expr {
    pub fn span(&self) -> SourceSpan {
        match self {
            Expr::Null(span)
            | Expr::Bool(_, span)
            | Expr::Int(_, span)
            | Expr::Float(_, span)
            | Expr::String(_, span)
            | Expr::Variable { span, .. }
            | Expr::Function { span, .. }
            | Expr::Struct { span, .. }
            | Expr::Array { span, .. }
            | Expr::NewArray { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Assignment { span, .. }
            | Expr::Call { span, .. }
            | Expr::Member { span, .. }
            | Expr::MethodCall { span, .. }
            | Expr::Index { span, .. }
            | Expr::Slice { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub is_rest: bool,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}
