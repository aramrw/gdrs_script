use crate::ast::*;
use std::collections::HashMap;

pub struct SemanticAnalyzer {
    functions: HashMap<String, (Vec<Type>, Option<Type>)>,
    objects: HashMap<String, HashMap<String, Type>>,
    enums: HashMap<String, HashMap<String, Vec<Type>>>,
    symbols: HashMap<String, (Type, bool)>,
    current_obj: Option<String>,
    current_prefix: String,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            objects: HashMap::new(),
            enums: HashMap::new(),
            symbols: HashMap::new(),
            current_obj: None,
            current_prefix: String::new(),
        }
    }

    fn types_equal(&self, a: &Type, b: &Type) -> bool {
        match (a, b) {
            (Type::Str, Type::Str) => true,
            (Type::String, Type::String) => true,
            (Type::Str, Type::String) => true,
            (Type::String, Type::Str) => true,
            (Type::File, Type::Custom(n, _)) if n == "std::fs::File" => true,
            (Type::Custom(n, _), Type::File) if n == "std::fs::File" => true,
            (Type::Array(t1, _), Type::Array(t2, _)) => self.types_equal(t1, t2),
            (Type::BoxPtr(t1), Type::BoxPtr(t2)) => self.types_equal(t1, t2),
            (Type::RawPtr(t1, m1), Type::RawPtr(t2, m2)) => m1 == m2 && self.types_equal(t1, t2),
            (Type::Custom(n1, g1), Type::Custom(n2, g2)) => {
                if n1 != n2 { return false; }
                if g1.is_empty() || g2.is_empty() { return true; }
                if g1.len() != g2.len() { return false; }
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
        self.functions.insert("fs::create".to_string(), (vec![Type::Str], Some(Type::File)));
        self.functions.insert("fs::open".to_string(), (vec![Type::Str], Some(Type::File)));
        self.functions.insert("fs::append".to_string(), (vec![Type::Str], Some(Type::File)));
        self.functions.insert("fs::read".to_string(), (vec![Type::File], Some(Type::String)));
        self.functions.insert("fs::read_to_string".to_string(), (vec![Type::Str], Some(Type::String)));
        self.functions.insert("fs::write".to_string(), (vec![Type::File, Type::Str], None));
        self.functions.insert("fs::write_string".to_string(), (vec![Type::File, Type::String], None));

        self.functions.insert("io::readline".to_string(), (vec![], Some(Type::String)));
        self.functions.insert("io::write".to_string(), (vec![Type::Str], None));
        self.functions.insert("io::println".to_string(), (vec![Type::Str], None));
        self.functions.insert("io::exit".to_string(), (vec![Type::I32], None));
        self.functions.insert("io::args".to_string(), (vec![], Some(Type::Array(Box::new(Type::String), 0))));

        // Built-in string methods
        self.functions.insert("str::to_owned_string".to_string(), (vec![Type::Str], Some(Type::String)));
        self.functions.insert("str::to_string".to_string(), (vec![Type::Str], Some(Type::String)));
        self.functions.insert("str::len".to_string(), (vec![Type::Str], Some(Type::I32)));
        self.functions.insert("str::contains".to_string(), (vec![Type::Str, Type::Str], Some(Type::Bool)));
        self.functions.insert("str::split".to_string(), (vec![Type::Str, Type::Str], Some(Type::Array(Box::new(Type::String), 0))));
        
        self.functions.insert("string::append".to_string(), (vec![Type::String, Type::Str], None));
        self.functions.insert("string::len".to_string(), (vec![Type::String], Some(Type::I32)));
        self.functions.insert("string::contains".to_string(), (vec![Type::String, Type::Str], Some(Type::Bool)));
        self.functions.insert("string::as_str".to_string(), (vec![Type::String], Some(Type::Str)));
        self.functions.insert("string::lines".to_string(), (vec![Type::String], Some(Type::Array(Box::new(Type::String), 0))));
        self.functions.insert("string::split".to_string(), (vec![Type::String, Type::Str], Some(Type::Array(Box::new(Type::String), 0))));

        // Vector methods
        self.functions.insert("vector::len".to_string(), (vec![Type::Array(Box::new(Type::Generic("T".into())), 0)], Some(Type::I32)));
        self.functions.insert("vector::get".to_string(), (vec![Type::Array(Box::new(Type::Generic("T".into())), 0), Type::I32], Some(Type::Generic("T".into()))));

        // Primitive methods
        self.functions.insert("i32::to_string".to_string(), (vec![Type::I32], Some(Type::String)));
        self.functions.insert("i64::to_string".to_string(), (vec![Type::I64], Some(Type::String)));
        self.functions.insert("f32::to_string".to_string(), (vec![Type::F32], Some(Type::String)));
        self.functions.insert("f64::to_string".to_string(), (vec![Type::F64], Some(Type::String)));
        
        self.functions.insert("f32::sin".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("f32::cos".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("f32::tan".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("f32::sqrt".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("f32::abs".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("f32::floor".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("f32::ceil".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("f32::pow".to_string(), (vec![Type::F32, Type::F32], Some(Type::F32)));
        self.functions.insert("f32::exp".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("f32::ln".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("f32::log10".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("f32::asin".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("f32::acos".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("f32::atan".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("f32::to_degrees".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("f32::to_radians".to_string(), (vec![Type::F32], Some(Type::F32)));

        self.functions.insert("f64::sin".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("f64::cos".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("f64::tan".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("f64::sqrt".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("f64::abs".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("f64::floor".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("f64::ceil".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("f64::pow".to_string(), (vec![Type::F64, Type::F64], Some(Type::F64)));
        self.functions.insert("f64::exp".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("f64::ln".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("f64::log10".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("f64::asin".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("f64::acos".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("f64::atan".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("f64::to_degrees".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("f64::to_radians".to_string(), (vec![Type::F64], Some(Type::F64)));

        self.functions.insert("math::pi".to_string(), (vec![], Some(Type::F32)));
        self.functions.insert("math::e".to_string(), (vec![], Some(Type::F32)));
        self.functions.insert("math::tau".to_string(), (vec![], Some(Type::F32)));
        self.functions.insert("math::pi64".to_string(), (vec![], Some(Type::F64)));
        self.functions.insert("math::e64".to_string(), (vec![], Some(Type::F64)));
        self.functions.insert("math::tau64".to_string(), (vec![], Some(Type::F64)));

        self.functions.insert("math::sin".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("math::cos".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("math::tan".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("math::sqrt".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("math::abs".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("math::floor".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("math::ceil".to_string(), (vec![Type::F32], Some(Type::F32)));
        self.functions.insert("math::pow".to_string(), (vec![Type::F32, Type::F32], Some(Type::F32)));

        self.functions.insert("math::sin64".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("math::cos64".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("math::tan64".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("math::sqrt64".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("math::abs64".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("math::floor64".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("math::ceil64".to_string(), (vec![Type::F64], Some(Type::F64)));
        self.functions.insert("math::pow64".to_string(), (vec![Type::F64, Type::F64], Some(Type::F64)));

        // Memory management
        self.functions.insert("mem::free".to_string(), (vec![Type::RawPtr(Box::new(Type::Generic("T".into())), true), Type::I32], None));

        self.collect_decls(&program.declarations, "")?;

        for (enum_name, variants) in &self.enums {
            for (variant_name, param_types) in variants {
                let full_name = format!("{}::{}", enum_name, variant_name);
                self.functions.insert(full_name, (param_types.clone(), Some(Type::Custom(enum_name.clone(), Vec::new()))));
            }
        }

        self.analyze_decls(&program.declarations, "")?;
        Ok(())
    }

    fn collect_decls(&mut self, decls: &[Decl], prefix: &str) -> Result<(), String> {
        for decl in decls {
            match decl {
                Decl::Object(obj) | Decl::ExternObject(obj) => {
                    let full_name = if prefix.is_empty() { obj.name.clone() } else { format!("{}::{}", prefix, obj.name) };
                    let mut fields = HashMap::new();
                    for f in &obj.fields { fields.insert(f.name.clone(), self.resolve_type(&f.ty)); }
                    self.objects.insert(full_name, fields);
                }
                Decl::Enum(enm) | Decl::ExternEnum(enm) => {
                    let full_name = if prefix.is_empty() { enm.name.clone() } else { format!("{}::{}", prefix, enm.name) };
                    let mut variants = HashMap::new();
                    for v in &enm.variants { variants.insert(v.name.clone(), v.types.iter().map(|t| self.resolve_type(t)).collect()); }
                    self.enums.insert(full_name, variants);
                }
                Decl::Function(func) | Decl::ExternFunction(func) => {
                    let full_name = if prefix.is_empty() { func.name.clone() } else { format!("{}::{}", prefix, func.name) };
                    let ret = func.return_type.as_ref().map(|t| self.resolve_type(t));
                    self.functions.insert(full_name, (func.params.iter().map(|p| self.resolve_type(&p.ty)).collect(), ret));
                }
                Decl::Impl(imp) | Decl::ExternImpl(imp) => {
                    let full_target = if prefix.is_empty() { imp.target.clone() } else { 
                        if imp.target.contains("::") { imp.target.clone() } else { format!("{}::{}", prefix, imp.target) }
                    };
                    self.current_obj = Some(full_target.clone());
                    for func in &imp.functions {
                        let name = format!("{}::{}", full_target, func.name);
                        let ret = func.return_type.as_ref().map(|t| self.resolve_type(t));
                        self.functions.insert(name, (func.params.iter().map(|p| self.resolve_type(&p.ty)).collect(), ret));
                    }
                    self.current_obj = None;
                }
                Decl::Module(name, inner) => {
                    let new_prefix = if prefix.is_empty() { name.clone() } else { format!("{}::{}", prefix, name) };
                    self.collect_decls(inner, &new_prefix)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn analyze_decls(&mut self, decls: &[Decl], prefix: &str) -> Result<(), String> {
        let old_prefix = std::mem::replace(&mut self.current_prefix, prefix.to_string());
        for decl in decls {
            match decl {
                Decl::Function(func) => {
                    self.analyze_function(func, None)?;
                }
                Decl::ExternFunction(_func) => {
                    // No body to analyze
                }
                Decl::Impl(imp) => {
                    let full_target = if prefix.is_empty() { imp.target.clone() } else { 
                        if imp.target.contains("::") { imp.target.clone() } else { format!("{}::{}", prefix, imp.target) }
                    };
                    for func in &imp.functions { self.analyze_function(func, Some(&full_target))?; }
                }
                Decl::Module(name, inner) => {
                    let new_prefix = if prefix.is_empty() { name.clone() } else { format!("{}::{}", prefix, name) };
                    self.analyze_decls(inner, &new_prefix)?;
                }
                _ => {}
            }
        }
        self.current_prefix = old_prefix;
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
            Stmt::Return(expr) => {
                if let Some(e) = expr { self.analyze_expr(e)?; }
                Ok(())
            }
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
            Expr::Int64(_) => Ok(Type::I64),
            Expr::Float(_) => Ok(Type::F32),
            Expr::Float64(_) => Ok(Type::F64),
            Expr::Bool(_) => Ok(Type::Bool),
            Expr::String(_) => Ok(Type::Str),
            Expr::Variable(name) => {
                if let Some((ty, _)) = self.symbols.get(name) { Ok(ty.clone()) }
                else { Err(format!("Undeclared variable '{}'", name)) }
            }
            Expr::Binary(lhs, op, rhs) => {
                let l = self.analyze_expr(lhs)?;
                let r = self.analyze_expr(rhs)?;
                
                if *op == BinaryOp::Add {
                    let is_l_string = l == Type::String || l == Type::Str;
                    let is_r_string = r == Type::String || r == Type::Str;
                    
                    if is_l_string || is_r_string {
                        return Ok(Type::String);
                    }
                }

                if !self.types_equal(&l, &r) {
                    return Err(format!("Type mismatch in binary operation: {:?} and {:?}", l, r));
                }
                match op {
                    BinaryOp::GreaterThan | BinaryOp::LessThan | BinaryOp::Equal => Ok(Type::Bool),
                    BinaryOp::Add => Ok(l),
                    _ => Ok(l),
                }
            }
            Expr::Call(name, args) => {
                if name == "array_init" {
                    let val_ty = self.analyze_expr(&args[0])?;
                    return Ok(Type::Array(Box::new(val_ty), 0));
                }
                
                // Try looking up with prefix if it's a simple name
                let name_to_lookup = if !name.contains("::") && !self.current_prefix.is_empty() {
                    format!("{}::{}", self.current_prefix, name)
                } else {
                    name.clone()
                };

                if let Some((_, ret_type)) = self.functions.get(&name_to_lookup) {
                    Ok(ret_type.clone().unwrap_or(Type::I32))
                } else if let Some((_, ret_type)) = self.functions.get(name) {
                    Ok(ret_type.clone().unwrap_or(Type::I32))
                } else { Err(format!("Undeclared function or variant '{}'", name)) }
            }
            Expr::MethodCall(lhs, name, _args) => {
                let lhs_ty = self.analyze_expr(lhs)?;
                if name == "clone" {
                    return Ok(lhs_ty);
                }
                let (obj_name, _is_array) = match lhs_ty {
                    Type::Str => ("str".to_string(), false),
                    Type::String => ("string".to_string(), false),
                    Type::I32 => ("i32".to_string(), false),
                    Type::I64 => ("i64".to_string(), false),
                    Type::F32 => ("f32".to_string(), false),
                    Type::F64 => ("f64".to_string(), false),
                    Type::Array(_, _) => ("vector".to_string(), true),
                    Type::Custom(ref n, _) => (n.clone(), false),
                    Type::BoxPtr(ref inner) | Type::RawPtr(ref inner, _) => match **inner {
                        Type::Custom(ref n, _) => (n.clone(), false),
                        _ => return Err(format!("Method call '{}' on non-object type {:?}", name, lhs_ty)),
                    },
                    _ => return Err(format!("Method call '{}' on non-object type {:?}", name, lhs_ty)),
                };
                
                let full_name = format!("{}::{}", obj_name, name);
                
                if let Some((_, ret_type)) = self.functions.get(&full_name) {
                    let rt = ret_type.clone().unwrap_or(Type::I32);
                    if let Type::Generic(_) = rt {
                        if let Type::Array(inner, _) = lhs_ty { Ok(*inner) }
                        else { Ok(rt) }
                    } else { Ok(rt) }
                } else {
                    Err(format!("No method '{}' on {}", name, obj_name)) 
                }
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
                    Type::Array(inner, _) => *inner,
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
            Expr::Unwrap(inner) => {
                let ty = self.analyze_expr(inner)?;
                match ty {
                    Type::Result(ok, _) => Ok(*ok),
                    _ => Ok(ty), 
                }
            }
            Expr::Await(inner) => {
                self.analyze_expr(inner)
            }
        }
    }
}
