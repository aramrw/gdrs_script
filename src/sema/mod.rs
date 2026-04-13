use crate::ast::*;
use std::collections::HashMap;

pub struct SemanticAnalyzer {
    functions: HashMap<String, (Vec<Type>, Option<Type>)>,
    objects: HashMap<String, HashMap<String, Type>>,
    enums: HashMap<String, HashMap<String, Vec<Type>>>,
    symbols: HashMap<String, (Type, bool)>,
    current_obj: Option<String>,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            objects: HashMap::new(),
            enums: HashMap::new(),
            symbols: HashMap::new(),
            current_obj: None,
        }
    }

    fn types_equal(&self, a: &Type, b: &Type) -> bool {
        match (a, b) {
            (Type::Array(t1, _), Type::Array(t2, _)) => self.types_equal(t1, t2),
            (Type::BoxPtr(t1), Type::BoxPtr(t2)) => self.types_equal(t1, t2),
            (Type::RawPtr(t1, m1), Type::RawPtr(t2, m2)) => m1 == m2 && self.types_equal(t1, t2),
            (Type::Custom(n1, g1), Type::Custom(n2, g2)) => {
                if n1 != n2 || g1.len() != g2.len() { return false; }
                g1.iter().zip(g2).all(|(a, b)| self.types_equal(a, b))
            }
            _ => a == b,
        }
    }

    fn resolve_type(&self, ty: &Type) -> Type {
        match ty {
            Type::SelfType => {
                if let Some(name) = &self.current_obj { Type::Custom(name.clone(), Vec::new()) }
                else { Type::SelfType }
            }
            Type::Array(inner, size) => Type::Array(Box::new(self.resolve_type(inner)), *size),
            Type::BoxPtr(inner) => Type::BoxPtr(Box::new(self.resolve_type(inner))),
            Type::RawPtr(inner, mutable) => Type::RawPtr(Box::new(self.resolve_type(inner)), *mutable),
            _ => ty.clone(),
        }
    }

    pub fn analyze(&mut self, program: &Program) -> Result<(), String> {
        self.functions.insert("file_create".to_string(), (vec![Type::Str], Some(Type::File)));
        self.functions.insert("file_open".to_string(), (vec![Type::Str], Some(Type::File)));
        self.functions.insert("file_read".to_string(), (vec![Type::File], Some(Type::Str)));
        self.functions.insert("file_write".to_string(), (vec![Type::File, Type::Str], None));

        for decl in &program.declarations {
            if let Decl::Object(obj) = decl {
                let mut fields = HashMap::new();
                for f in &obj.fields { fields.insert(f.name.clone(), self.resolve_type(&f.ty)); }
                self.objects.insert(obj.name.clone(), fields);
            }
        }

        for decl in &program.declarations {
            if let Decl::Enum(enm) = decl {
                let mut variants = HashMap::new();
                for v in &enm.variants { variants.insert(v.name.clone(), v.types.iter().map(|t| self.resolve_type(t)).collect()); }
                self.enums.insert(enm.name.clone(), variants);
            }
        }

        for decl in &program.declarations {
            match decl {
                Decl::Function(func) => {
                    let ret = func.return_type.as_ref().map(|t| self.resolve_type(t));
                    self.functions.insert(func.name.clone(), (func.params.iter().map(|p| self.resolve_type(&p.ty)).collect(), ret));
                }
                Decl::Impl(imp) => {
                    self.current_obj = Some(imp.target.clone());
                    for func in &imp.functions {
                        let name = format!("{}::{}", imp.target, func.name);
                        let ret = func.return_type.as_ref().map(|t| self.resolve_type(t));
                        self.functions.insert(name, (func.params.iter().map(|p| self.resolve_type(&p.ty)).collect(), ret));
                    }
                    self.current_obj = None;
                }
                _ => {}
            }
        }

        for (enum_name, variants) in &self.enums {
            for (variant_name, param_types) in variants {
                let full_name = format!("{}::{}", enum_name, variant_name);
                self.functions.insert(full_name, (param_types.clone(), Some(Type::Custom(enum_name.clone(), Vec::new()))));
            }
        }

        for decl in &program.declarations {
            match decl {
                Decl::Function(func) => self.analyze_function(func, None)?,
                Decl::Impl(imp) => {
                    for func in &imp.functions { self.analyze_function(func, Some(&imp.target))?; }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn analyze_function(&mut self, func: &Function, target: Option<&String>) -> Result<(), String> {
        self.symbols.clear();
        self.current_obj = target.cloned();
        if let Some(t) = target { self.symbols.insert("self".to_string(), (Type::Custom(t.clone(), Vec::new()), false)); }
        for p in &func.params { self.symbols.insert(p.name.clone(), (self.resolve_type(&p.ty), p.is_mutable)); }
        self.analyze_stmt(&func.body)?;
        self.current_obj = None;
        Ok(())
    }

    fn analyze_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
        match stmt {
            Stmt::VarDecl { name, is_mutable, ty, value, .. } => {
                let val_ty = self.analyze_expr(value)?;
                let expected_ty = ty.as_ref().map(|t| self.resolve_type(t));
                let final_ty = if let Some(t) = expected_ty {
                    if !self.types_equal(&val_ty, &t) { return Err(format!("Type mismatch for {}", name)); }
                    t
                } else { val_ty };
                self.symbols.insert(name.clone(), (final_ty, *is_mutable));
                Ok(())
            }
            Stmt::Assign { target, value } => {
                let _target_ty = self.analyze_expr(target)?;
                let _val_ty = self.analyze_expr(value)?;
                if let Expr::Variable(name) = target {
                    if let Some((_, mutable)) = self.symbols.get(name) {
                        if !mutable { return Err(format!("Cannot assign to immutable variable '{}'", name)); }
                    }
                }
                Ok(())
            }
            Stmt::Print(expr) => { self.analyze_expr(expr)?; Ok(()) }
            Stmt::If { condition, then_branch, else_branch } => {
                if self.analyze_expr(condition)? != Type::Bool { return Err("If condition must be bool".to_string()); }
                self.analyze_stmt(then_branch)?;
                if let Some(eb) = else_branch { self.analyze_stmt(eb)?; }
                Ok(())
            }
            Stmt::While { condition, body } => {
                if self.analyze_expr(condition)? != Type::Bool { return Err("While condition must be bool".to_string()); }
                self.analyze_stmt(body)?;
                Ok(())
            }
            Stmt::Block(stmts) => { for s in stmts { self.analyze_stmt(s)?; } Ok(()) }
            Stmt::ExprStmt(expr) => { self.analyze_expr(expr)?; Ok(()) }
            Stmt::Match { expr, arms } => {
                let expr_ty = self.analyze_expr(expr)?;
                for arm in arms {
                    let old_symbols = self.symbols.clone();
                    self.analyze_pattern(&arm.pattern, &expr_ty)?;
                    self.analyze_stmt(&arm.body)?;
                    self.symbols = old_symbols;
                }
                Ok(())
            }
        }
    }

    fn analyze_pattern(&mut self, pattern: &Pattern, expr_ty: &Type) -> Result<(), String> {
        match pattern {
            Pattern::Variant(enum_name, variant_name, params) => {
                if let Some(variants) = self.enums.get(enum_name) {
                    if let Some(param_types) = variants.get(variant_name) {
                        for (name, ty) in params.iter().zip(param_types) { self.symbols.insert(name.clone(), (ty.clone(), false)); }
                        return Ok(());
                    }
                }
                Err(format!("Undeclared variant {}::{}", enum_name, variant_name))
            }
            Pattern::Variable(name) => { self.symbols.insert(name.clone(), (expr_ty.clone(), false)); Ok(()) }
            Pattern::Literal(lit) => { self.analyze_expr(lit)?; Ok(()) }
        }
    }

    fn analyze_expr(&self, expr: &Expr) -> Result<Type, String> {
        match expr {
            Expr::Int(_) => Ok(Type::I32),
            Expr::Float(_) => Ok(Type::F32),
            Expr::Bool(_) => Ok(Type::Bool),
            Expr::String(_) => Ok(Type::Str),
            Expr::Variable(name) => {
                if let Some((ty, _)) = self.symbols.get(name) { Ok(ty.clone()) }
                else { Err(format!("Undeclared variable '{}'", name)) }
            }
            Expr::Binary(lhs, op, rhs) => {
                let l = self.analyze_expr(lhs)?;
                let _r = self.analyze_expr(rhs)?;
                match op {
                    BinaryOp::GreaterThan | BinaryOp::LessThan => Ok(Type::Bool),
                    _ => Ok(l),
                }
            }
            Expr::Call(name, args) => {
                if name == "array_init" {
                    let val_ty = self.analyze_expr(&args[0])?;
                    return Ok(Type::Array(Box::new(val_ty), 0));
                }
                if let Some((_, ret_type)) = self.functions.get(name) {
                    Ok(ret_type.clone().unwrap_or(Type::I32))
                } else { Err(format!("Undeclared function or variant '{}'", name)) }
            }
            Expr::MethodCall(lhs, name, _args) => {
                let lhs_ty = self.analyze_expr(lhs)?;
                let obj_name = match lhs_ty {
                    Type::Custom(n, _) => n,
                    Type::BoxPtr(inner) | Type::RawPtr(inner, _) => match *inner {
                        Type::Custom(n, _) => n,
                        _ => return Err("Method call on non-object".into()),
                    },
                    _ => return Err("Method call on non-object".into()),
                };
                let full_name = format!("{}::{}", obj_name, name);
                if let Some((_, ret_type)) = self.functions.get(&full_name) {
                    Ok(ret_type.clone().unwrap_or(Type::I32))
                } else { Err(format!("No method '{}' on {}", name, obj_name)) }
            }
            Expr::StructLiteral { name, fields: _ } => {
                let target_name = if name == "self" {
                    if let Some((Type::Custom(n, _), _)) = self.symbols.get("self") { n.clone() }
                    else { return Err("Cannot use self outside impl".into()); }
                } else { name.clone() };
                Ok(Type::Custom(target_name, Vec::new()))
            }
            Expr::MemberAccess(lhs, name) => {
                let lhs_ty = self.analyze_expr(lhs)?;
                let actual_ty = match lhs_ty {
                    Type::BoxPtr(inner) => *inner,
                    Type::RawPtr(inner, _) => *inner,
                    _ => lhs_ty,
                };
                if let Type::Custom(obj_name, _) = actual_ty {
                    if let Some(fields) = self.objects.get(&obj_name) {
                        if let Some(ty) = fields.get(name) { return Ok(ty.clone()); }
                    }
                }
                Err(format!("No member '{}'", name))
            }
            Expr::IndexAccess(lhs, _index) => {
                let lhs_ty = self.analyze_expr(lhs)?;
                let actual_ty = match lhs_ty {
                    Type::BoxPtr(inner) => *inner,
                    Type::RawPtr(inner, _) => *inner,
                    Type::Array(inner, _) => *inner, // Already works for arrays
                    _ => return Err("Indexing only works on arrays or pointers".into()),
                };
                Ok(actual_ty)
            }
            Expr::Alloc(inner, kind) => {
                let ty = self.analyze_expr(inner)?;
                match kind {
                    AllocKind::Box => Ok(Type::BoxPtr(Box::new(ty))),
                    AllocKind::RawMut => Ok(Type::RawPtr(Box::new(ty), true)),
                    AllocKind::RawConst => Ok(Type::RawPtr(Box::new(ty), false)),
                }
            }
        }
    }
}
