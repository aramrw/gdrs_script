#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    I32,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i32),
    Variable(String), // New: access variable
}

#[derive(Debug, Clone)]
pub enum Stmt {
    VarDecl {
        name: String,
        is_mutable: bool,
        ty: Type,
        value: Expr,
    },
    Print(Expr),
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub body: Vec<Stmt>,
}
