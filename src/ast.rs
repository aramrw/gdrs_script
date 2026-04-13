#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    I32, F32, Bool, Str, File,
    Array(Box<Type>, usize),
    BoxPtr(Box<Type>),
    RawPtr(Box<Type>, bool),
    Generic(String),
    Custom(String, Vec<Type>),
    SelfType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add, Subtract, Multiply, Divide, GreaterThan, LessThan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocKind {
    Box, RawMut, RawConst,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i32), Float(f32), Bool(bool), String(String),
    Variable(String),
    Binary(Box<Expr>, BinaryOp, Box<Expr>),
    Call(String, Vec<Expr>),
    MethodCall(Box<Expr>, String, Vec<Expr>), // <--- New!
    StructLiteral { name: String, fields: Vec<(String, Expr)> },
    MemberAccess(Box<Expr>, String),
    IndexAccess(Box<Expr>, Box<Expr>),
    Alloc(Box<Expr>, AllocKind),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    VarDecl { name: String, is_mutable: bool, ty: Option<Type>, value: Expr },
    Assign { target: Expr, value: Expr },
    Print(Expr),
    If { condition: Expr, then_branch: Box<Stmt>, else_branch: Option<Box<Stmt>> },
    While { condition: Expr, body: Box<Stmt> },
    Block(Vec<Stmt>),
    ExprStmt(Expr),
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
}

#[derive(Debug, Clone)]
pub struct ObjectDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Variant { pub name: String, pub types: Vec<Type> }

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone)]
pub struct ImplDecl {
    pub target: String,
    pub generics: Vec<String>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub enum Decl {
    Function(Function),
    Object(ObjectDecl),
    Enum(EnumDecl),
    Impl(ImplDecl),
}

#[derive(Debug, Clone)]
pub struct Program { pub declarations: Vec<Decl> }
