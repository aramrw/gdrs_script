#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Unit,
    I32, I64, F32, F64, Bool, Str, String, File,
    Array(Box<Type>, usize),
    BoxPtr(Box<Type>),
    RawPtr(Box<Type>, bool),
    Ref(Box<Type>, bool),
    Generic(String),
    Custom(String, Vec<Type>),
    SelfType,
    Result(Box<Type>, Box<Type>),
    Any,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add, Subtract, Multiply, Divide, GreaterThan, LessThan, Equal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocKind {
    Box, RawMut, RawConst,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Unit,
    Int(i32), Int64(i64), Float(f32), Float64(f64), Bool(bool), String(String),
    Variable(String),
    Binary(Box<Expr>, BinaryOp, Box<Expr>),
    Call(String, Vec<Expr>, Option<String>),
    MacroCall(String, Vec<Expr>),
    MethodCall(Box<Expr>, String, Vec<Expr>, Option<String>),
    StructLiteral { name: String, fields: Vec<(String, Expr)>, resolved_name: Option<String> },
    MemberAccess(Box<Expr>, String),
    IndexAccess(Box<Expr>, Box<Expr>),
    Alloc(Box<Expr>, AllocKind),
    Borrow(Box<Expr>, bool),
    Deref(Box<Expr>),
    Unwrap(Box<Expr>),
    Await(Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    VarDecl { name: String, is_mutable: bool, ty: Option<Type>, value: Expr },
    Assign { target: Expr, value: Expr },
    If { condition: Expr, then_branch: Box<Stmt>, else_branch: Option<Box<Stmt>> },
    While { condition: Expr, body: Box<Stmt> },
    Block(Vec<Stmt>),
    ExprStmt(Expr),
    Return(Option<Expr>),
    Match { expr: Expr, arms: Vec<Arm> },
}

#[derive(Debug, Clone)]
pub struct Arm { pub pattern: Pattern, pub body: Stmt }

#[derive(Debug, Clone)]
pub enum Pattern {
    Variant(String, String, Vec<String>),
    Variable(String),
    Literal(Expr),
}

#[derive(Debug, Clone)]
pub struct Field { pub name: String, pub ty: Type }

#[derive(Debug, Clone)]
pub struct Param { 
    pub name: String, 
    pub ty: Type,
    pub is_mutable: bool,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub generics: Vec<String>,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Stmt,
    pub is_async: bool,
    pub rust_path: Option<String>,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ObjectDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub fields: Vec<Field>,
    pub rust_path: Option<String>,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Variant { pub name: String, pub types: Vec<Type> }

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub variants: Vec<Variant>,
    pub rust_path: Option<String>,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ImplDecl {
    pub target: String,
    pub generics: Vec<String>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct UseDecl {
    pub path: Vec<String>,
    pub items: Vec<String>, // Empty means import the whole path (last element), or use a wildcard? 
    pub is_crate: bool,     // If it starts with 'crate::'
}

#[derive(Debug, Clone)]
pub enum Decl {
    Function(Function),
    Object(ObjectDecl),
    Enum(EnumDecl),
    Impl(ImplDecl),
    ExternFunction(Function),
    ExternObject(ObjectDecl),
    ExternEnum(EnumDecl),
    ExternImpl(ImplDecl),
    Module(String, Vec<Decl>),
    Use(UseDecl),
    RustDependency(String, String),
    RustBlock(String),
}

#[derive(Debug, Clone)]
pub struct Program { pub declarations: Vec<Decl> }
