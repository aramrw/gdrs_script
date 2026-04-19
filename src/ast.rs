use crate::lexer::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Unit,
    I32,
    I64,
    F32,
    F64,
    Bool,
    Str,
    String,
    File,
    Array(Box<Type>, usize),
    BoxPtr(Box<Type>),
    RawPtr(Box<Type>, bool),
    Ref(Box<Type>, bool),
    Managed(Box<Type>),
    ThreadSafe(Box<Type>),
    WeakManaged(Box<Type>),
    WeakThreadSafe(Box<Type>),
    Generic(String),
    Custom(String, Vec<Type>),
    SelfType,
    Result(Box<Type>, Box<Type>),
    Tuple(Vec<Type>),
    Any,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Equal,
    Modulo,
    AddAssign,
    SubAssign
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocKind {
    Box,
    RawMut,
    RawConst,
    Rc,
    Arc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgKind {
    Value,
    Ref,
    MutRef,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
    pub ty: Option<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PathPart {
    pub name: String,
    pub generics: Vec<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Unit,
    Int(i32),
    Int64(i64),
    Float(f32),
    Float64(f64),
    Bool(bool),
    String(String),
    Variable(Vec<PathPart>),
    Binary(Box<Expr>, BinaryOp, Box<Expr>),
    Tuple(Vec<Expr>),
    Call(
        Vec<PathPart>,
        Vec<Expr>,
        Option<String>,
        Option<Vec<ArgKind>>,
        Option<Vec<Type>>,
    ),
    MacroCall(String, Vec<Expr>),
    Array(Vec<Expr>),
    Block(Vec<Stmt>),
    MethodCall(
        Box<Expr>,
        String,
        Vec<Expr>,
        Option<String>,
        Option<Vec<ArgKind>>,
        Option<Vec<Type>>,
    ),
    StructLiteral {
        path: Vec<PathPart>,
        fields: Vec<(String, Expr)>,
        resolved_name: Option<String>,
    },
    MemberAccess(Box<Expr>, String),
    IndexAccess(Box<Expr>, Box<Expr>),
    Alloc(Box<Expr>, AllocKind),
    Borrow(Box<Expr>, bool),
    Deref(Box<Expr>),
    Downgrade(Box<Expr>),
    Negate(Box<Expr>),
    Unwrap(Box<Expr>),
    Await(Box<Expr>),
    Try(Box<Expr>),
    Cast(Box<Expr>, Type),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    VarDecl {
        name: String,
        is_mutable: bool,
        ty: Option<Type>,
        value: Expr,
    },
    Assign {
        target: Expr,
        value: Expr,
    },
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
    },
    For {
        var_name: String,
        iterator: Expr,
        body: Box<Stmt>,
    },
    Loop {
        body: Box<Stmt>,
    },
    Break(Option<Expr>),
    Block(Vec<Stmt>),
    UnsafeBlock(Vec<Stmt>),
    ExprStmt(Expr),
    Return(Option<Expr>),
    Match {
        expr: Expr,
        arms: Vec<Arm>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arm {
    pub pattern: Pattern,
    pub body: Stmt,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Variant(String, String, Vec<String>),
    Variable(String),
    Literal(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub is_mutable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub generics: Vec<(String, Vec<String>)>,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Stmt,
    pub is_async: bool,
    pub rust_path: Option<String>,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectDecl {
    pub name: String,
    pub generics: Vec<(String, Vec<String>)>,
    pub fields: Vec<Field>,
    pub rust_path: Option<String>,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub name: String,
    pub types: Vec<Type>,
    pub attributes: Vec<String>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name: String,
    pub generics: Vec<(String, Vec<String>)>,
    pub variants: Vec<Variant>,
    pub rust_path: Option<String>,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraitDecl {
    pub name: String,
    pub generics: Vec<(String, Vec<String>)>,
    pub bounds: Vec<String>, // Supertraits
    pub functions: Vec<Function>,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImplDecl {
    pub trait_name: Option<String>,
    pub target: String,
    pub generics: Vec<(String, Vec<String>)>,
    pub associated_types: Vec<(String, Type)>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UseDecl {
    pub path: Vec<String>,
    pub items: Vec<String>,
    pub is_wildcard: bool, // Support ::*
    pub is_crate: bool,    // If it starts with 'crate::'
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    Function(Function),
    Object(ObjectDecl),
    Enum(EnumDecl),
    Trait(TraitDecl),
    Impl(ImplDecl),
    ExternFunction(Function),
    ExternObject(ObjectDecl),
    ExternEnum(EnumDecl),
    ExternTrait(TraitDecl),
    ExternImpl(ImplDecl),
    Module(String, Vec<Decl>),
    Use(UseDecl),
    RustDependency(String, String),
    RustBlock(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub declarations: Vec<Decl>,
}
