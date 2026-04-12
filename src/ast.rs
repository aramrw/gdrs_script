use chumsky::prelude::*;
use std::env; // Added for command line arguments
use std::fs;
use std::process::Command;

// ==========================================
// 1. AST (Unchanged)
// ==========================================
#[derive(Debug, Clone)]
pub enum Expr {
    Int(i32),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Print(Expr),
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub body: Vec<Stmt>,
}
