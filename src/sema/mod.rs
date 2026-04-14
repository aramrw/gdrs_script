use crate::ast::*;
use crate::error::CompilerError;
use crate::lexer::Span;
use miette::SourceSpan;
use std::collections::{HashMap, HashSet};

pub struct SemanticAnalyzer {
    functions: HashMap<String, (Vec<Type>, Option<Type>)>,
    objects: HashMap<String, HashMap<String, Type>>,
    enums: HashMap<String, HashMap<String, Vec<Type>>>,
    symbols: HashMap<String, (Type, bool)>,
    phantom_types: HashSet<String>,
    has_wildcard_phantom: bool,
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
            phantom_types: HashSet::new(),
            has_wildcard_phantom: false,
            current_obj: None,
            current_prefix: String::new(),
        }
    }

    fn semantic_error<T>(&self, message: String, span: Span) -> Result<T, CompilerError> {
        let source_span = SourceSpan::new(span.start.into(), (span.end - span.start).into());
        Err(CompilerError::Semantic { message, span: source_span })
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
            (Type::Ref(t1, m1), Type::Ref(t2, m2)) => m1 == m2 && self.types_equal(t1, t2),
            (Type::Custom(n1, g1), Type::Custom(n2, g2)) => {
                if n1 != n2 { return false; }
                if g1.is_empty() || g2.is_empty() { return true; }
                if g1.len() != g2.len() { return false; }
                g1.iter().zip(g2).all(|(a, b)| self.types_equal(a, b))
            }
            (Type::Result(ok1, err1), Type::Result(ok2, err2)) => {
                self.types_equal(ok1, ok2) && self.types_equal(err1, err2)
            }
            (Type::Any, _) | (_, Type::Any) => true,
            (Type::Generic(_), _) | (_, Type::Generic(_)) => true, // Simple unification for now
            _ => a == b,
        }
    }

    fn resolve_type(&self, ty: &Type) -> Type {
        match ty {
            Type::Any => Type::Any,
            Type::SelfType => {
                if let Some(name) = &self.current_obj { Type::Custom(name.clone(), Vec::new()) }
                else { Type::SelfType }
            }
            Type::Custom(name, generics) => {
                let resolved_generics = generics.iter().map(|g| self.resolve_type(g)).collect();
                if name == "Self" || name == "self" {
                    if let Some(obj_name) = &self.current_obj {
                        Type::Custom(obj_name.clone(), resolved_generics)
                    } else {
                        Type::Custom(name.clone(), resolved_generics)
                    }
                } else {
                    Type::Custom(name.clone(), resolved_generics)
                }
            }
            Type::Result(ok, err) => {
                Type::Result(Box::new(self.resolve_type(ok)), Box::new(self.resolve_type(err)))
            }
            Type::Array(inner, size) => Type::Array(Box::new(self.resolve_type(inner)), *size),
            Type::BoxPtr(inner) => Type::BoxPtr(Box::new(self.resolve_type(inner))),
            Type::RawPtr(inner, mutable) => Type::RawPtr(Box::new(self.resolve_type(inner)), *mutable),
            Type::Ref(inner, mutable) => Type::Ref(Box::new(self.resolve_type(inner)), *mutable),
            _ => ty.clone(),
        }
    }

    pub fn analyze(&mut self, program: &mut Program) -> Result<(), CompilerError> {
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

        self.functions.insert("serde_json::to_string".to_string(), (vec![Type::Generic("T".into())], Some(Type::String)));

        self.collect_decls(&program.declarations, "")?;

        for (enum_name, variants) in &self.enums {
            for (variant_name, param_types) in variants {
                let full_name = format!("{}::{}", enum_name, variant_name);
                self.functions.insert(full_name, (param_types.clone(), Some(Type::Custom(enum_name.clone(), Vec::new()))));
            }
        }

        self.analyze_decls(&mut program.declarations, "")?;
        Ok(())
    }

    fn collect_decls(&mut self, decls: &[Decl], prefix: &str) -> Result<(), CompilerError> {
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
                Decl::Use(u) if u.is_crate => {
                    // This is an external crate import, we can treat these as 'Phantom' types
                    let base_path = u.path.join("::");
                    if u.is_wildcard {
                        self.has_wildcard_phantom = true;
                    }

                    if u.items.is_empty() && !u.is_wildcard {
                        let name = u.path.last().unwrap().clone();
                        self.objects.insert(name.clone(), HashMap::new());
                        self.phantom_types.insert(name.clone());
                        // Register a mapping for the compiler
                        if let Ok(mut mappings) = crate::compiler::RUST_MAPPINGS.lock() {
                            mappings.insert(name, format!("::{}", base_path));
                        }
                    } else if !u.items.is_empty() {
                        for item in &u.items {
                            self.objects.insert(item.clone(), HashMap::new());
                            self.phantom_types.insert(item.clone());
                            if let Ok(mut mappings) = crate::compiler::RUST_MAPPINGS.lock() {
                                mappings.insert(item.clone(), format!("::{}::{}", base_path, item));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn analyze_decls(&mut self, decls: &mut [Decl], prefix: &str) -> Result<(), CompilerError> {
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
                    for func in &mut imp.functions { self.analyze_function(func, Some(&full_target))?; }
                }
                Decl::Module(name, inner) => {
                    let new_prefix = if prefix.is_empty() { name.clone() } else { format!("{}::{}", prefix, name) };
                    self.analyze_decls(inner, &new_prefix)?;
                }
                Decl::Use(u) if !u.is_crate => {
                    // Standard use - for now just add the names to symbols if we want them as constants/types
                    // But we don't have a good way to resolve them yet.
                }
                _ => {}
            }
        }
        self.current_prefix = old_prefix;
        Ok(())
    }

    fn analyze_function(&mut self, func: &mut Function, target: Option<&String>) -> Result<(), CompilerError> {
        self.symbols.clear();
        self.current_obj = target.cloned();
        if let Some(t) = target { self.symbols.insert("self".to_string(), (Type::Custom(t.clone(), Vec::new()), false)); }
        for p in &func.params { self.symbols.insert(p.name.clone(), (self.resolve_type(&p.ty), p.is_mutable)); }
        self.analyze_stmt(&mut func.body)?;
        self.current_obj = None;
        Ok(())
    }

    fn analyze_stmt(&mut self, stmt: &mut Stmt) -> Result<(), CompilerError> {
        let span = stmt.span;
        match &mut stmt.kind {
            StmtKind::VarDecl { name, is_mutable, ty, value } => {
                let val_ty = self.analyze_expr(value)?;
                let expected_ty = ty.as_ref().map(|t| self.resolve_type(t));
                let final_ty = if let Some(t) = expected_ty {
                    if !self.types_equal(&val_ty, &t) { 
                        return self.semantic_error(format!("Type mismatch for {}: expected {:?}, found {:?}", name, t, val_ty), span); 
                    }
                    t
                } else { val_ty };
                self.symbols.insert(name.clone(), (final_ty, *is_mutable));
                Ok(())
            }
            StmtKind::Assign { target, value } => {
                let _target_ty = self.analyze_expr(target)?;
                let _val_ty = self.analyze_expr(value)?;
                if let ExprKind::Variable(name) = &target.kind {
                    if let Some((_, mutable)) = self.symbols.get(name) {
                        if !mutable { 
                            return self.semantic_error(format!("Cannot assign to immutable variable '{}'", name), target.span); 
                        }
                    }
                }
                Ok(())
            }
            StmtKind::If { condition, then_branch, else_branch } => {
                let cond_ty = self.analyze_expr(condition)?;
                if !self.types_equal(&cond_ty, &Type::Bool) { 
                    return self.semantic_error(format!("If condition must be bool, found {:?}", cond_ty), condition.span); 
                }
                self.analyze_stmt(then_branch)?;
                if let Some(eb) = else_branch { self.analyze_stmt(eb)?; }
                Ok(())
            }
            StmtKind::While { condition, body } => {
                let cond_ty = self.analyze_expr(condition)?;
                if !self.types_equal(&cond_ty, &Type::Bool) { 
                    return self.semantic_error(format!("While condition must be bool, found {:?}", cond_ty), condition.span); 
                }
                self.analyze_stmt(body)?;
                Ok(())
            }
            StmtKind::Block(stmts) => { for s in stmts { self.analyze_stmt(s)?; } Ok(()) }
            StmtKind::ExprStmt(expr) => { self.analyze_expr(expr)?; Ok(()) }
            StmtKind::Return(expr) => {
                if let Some(e) = expr { self.analyze_expr(e)?; }
                Ok(())
            }
            StmtKind::Match { expr, arms } => {
                let expr_ty = self.analyze_expr(expr)?;
                for arm in arms {
                    let old_symbols = self.symbols.clone();
                    self.analyze_pattern(&mut arm.pattern, &expr_ty, span)?;
                    self.analyze_stmt(&mut arm.body)?;
                    self.symbols = old_symbols;
                }
                Ok(())
            }
        }
    }

    fn analyze_pattern(&mut self, pattern: &mut Pattern, expr_ty: &Type, span: Span) -> Result<(), CompilerError> {
        match pattern {
            Pattern::Variant(enum_name, variant_name, params) => {
                if let Some(variants) = self.enums.get(enum_name) {
                    if let Some(param_types) = variants.get(variant_name) {
                        for (name, ty) in params.iter().zip(param_types) { self.symbols.insert(name.clone(), (ty.clone(), false)); }
                        return Ok(());
                    }
                }
                self.semantic_error(format!("Undeclared variant {}::{}", enum_name, variant_name), span)
            }
            Pattern::Variable(name) => { self.symbols.insert(name.clone(), (expr_ty.clone(), false)); Ok(()) }
            Pattern::Literal(lit) => { self.analyze_expr(lit)?; Ok(()) }
        }
    }

    fn analyze_expr(&mut self, expr: &mut Expr) -> Result<Type, CompilerError> {
        let span = expr.span;
        match &mut expr.kind {
            ExprKind::Unit => Ok(Type::Unit),
            ExprKind::Int(_) => Ok(Type::I32),
            ExprKind::Int64(_) => Ok(Type::I64),
            ExprKind::Float(_) => Ok(Type::F32),
            ExprKind::Float64(_) => Ok(Type::F64),
            ExprKind::Bool(_) => Ok(Type::Bool),
            ExprKind::String(_) => Ok(Type::Str),
            ExprKind::Variable(name) => {
                if let Some((ty, _)) = self.symbols.get(name) { Ok(ty.clone()) }
                else {
                    if name.contains("::") {
                        let parts: Vec<&str> = name.split("::").collect();
                        if self.phantom_types.contains(parts[0]) {
                            return Ok(Type::Any);
                        }
                    } else if self.has_wildcard_phantom {
                        return Ok(Type::Any);
                    }
                    self.semantic_error(format!("Undeclared variable '{}'", name), span)
                }
            }
            ExprKind::Binary(lhs, op, rhs) => {
                let l = self.analyze_expr(lhs)?;
                let r = self.analyze_expr(rhs)?;
                
                if l == Type::Any || r == Type::Any {
                    return Ok(Type::Any);
                }
                
                if *op == BinaryOp::Add {
                    let is_l_string = l == Type::String || l == Type::Str;
                    let is_r_string = r == Type::String || r == Type::Str;
                    
                    if is_l_string || is_r_string {
                        return Ok(Type::String);
                    }
                }

                if !self.types_equal(&l, &r) {
                    return self.semantic_error(format!("Type mismatch in binary operation: {:?} and {:?}", l, r), span);
                }
                match op {
                    BinaryOp::GreaterThan | BinaryOp::LessThan | BinaryOp::Equal => Ok(Type::Bool),
                    BinaryOp::Add => Ok(l),
                    _ => Ok(l),
                }
            }
            ExprKind::MacroCall(name, args) => {
                for arg in args {
                    self.analyze_expr(arg)?;
                }
                if name == "typeof" {
                    Ok(Type::Str)
                } else {
                    Ok(Type::I32) // Macros return i32/Unit effectively for now
                }
            }
            ExprKind::Call(name, args, resolved_name) => {
                if name == "Ok" {
                    let val_ty = self.analyze_expr(&mut args[0])?;
                    return Ok(Type::Result(Box::new(val_ty), Box::new(Type::Generic("E".into()))));
                }
                if name == "Err" {
                    let err_ty = self.analyze_expr(&mut args[0])?;
                    return Ok(Type::Result(Box::new(Type::Generic("T".into())), Box::new(err_ty)));
                }
                if name == "array_init" {
                    let val_ty = self.analyze_expr(&mut args[0])?;
                    return Ok(Type::Array(Box::new(val_ty), 0));
                }
                
                // Try looking up with prefix if it's a simple name
                let name_to_lookup = if !name.contains("::") && !self.current_prefix.is_empty() {
                    format!("{}::{}", self.current_prefix, name)
                } else {
                    name.clone()
                };

                let (found_name, ret_type) = if let Some((_, rt)) = self.functions.get(&name_to_lookup) {
                    (name_to_lookup.clone(), rt.clone())
                } else if let Some((_, rt)) = self.functions.get(name) {
                    (name.clone(), rt.clone())
                } else {
                    if name.contains("::") {
                        let parts: Vec<&str> = name.split("::").collect();
                        if self.phantom_types.contains(parts[0]) {
                            for arg in args { self.analyze_expr(arg)?; }
                            *resolved_name = Some(name.clone());
                            return Ok(Type::Any);
                        }
                    } else if self.has_wildcard_phantom {
                        for arg in args { self.analyze_expr(arg)?; }
                        *resolved_name = Some(name.clone());
                        return Ok(Type::Any);
                    }
                    return self.semantic_error(format!("Undeclared function or variant '{}'", name), span);
                };

                *resolved_name = Some(found_name.clone());

                let mut ret = ret_type.unwrap_or(Type::I32);
                if found_name.contains("::") {
                    let parts: Vec<&str> = found_name.split("::").collect();
                    let obj_name = parts[..parts.len()-1].join("::");
                    let old_obj = self.current_obj.take();
                    self.current_obj = Some(obj_name);
                    ret = self.resolve_type(&ret);
                    self.current_obj = old_obj;
                } else {
                    ret = self.resolve_type(&ret);
                }
                for arg in args {
                    self.analyze_expr(arg)?;
                }
                Ok(ret)
            }
            ExprKind::MethodCall(lhs, name, args, resolved_obj_name) => {
                let mut lhs_ty = self.analyze_expr(lhs)?;
                lhs_ty = self.resolve_type(&lhs_ty);
                if lhs_ty == Type::Any {
                    for arg in args { self.analyze_expr(arg)?; }
                    return Ok(Type::Any);
                }
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
                    Type::Ref(ref inner, _) | Type::BoxPtr(ref inner) | Type::RawPtr(ref inner, _) => match **inner {
                        Type::Custom(ref n, _) => (n.clone(), false),
                        _ => return self.semantic_error(format!("Method call '{}' on non-object type {:?}", name, lhs_ty), span),
                    },
                    _ => return self.semantic_error(format!("Method call '{}' on non-object type {:?}", name, lhs_ty), span),
                };
                
                *resolved_obj_name = Some(obj_name.clone());

                let full_name = format!("{}::{}", obj_name, name);
                
                for arg in args {
                    self.analyze_expr(arg)?;
                }

                if let Some((_, ret_type)) = self.functions.get(&full_name) {
                    let rt = ret_type.clone().unwrap_or(Type::I32);
                    if let Type::Generic(_) = rt {
                        if let Type::Array(inner, _) = lhs_ty { Ok(*inner) }
                        else { Ok(rt) }
                    } else { Ok(rt) }
                } else {
                    self.semantic_error(format!("No method '{}' on {}", name, obj_name), span) 
                }
            }
            ExprKind::StructLiteral { name, fields, resolved_name } => {
                let target_name = if name == "self" || name == "Self" {
                    if let Some((Type::Custom(n, _), _)) = self.symbols.get("self") { n.clone() }
                    else { return self.semantic_error("Cannot use self outside impl".into(), span); }
                } else { name.clone() };
                *resolved_name = Some(target_name.clone());
                for (_, fexpr) in fields {
                    self.analyze_expr(fexpr)?;
                }
                Ok(Type::Custom(target_name, Vec::new()))
            }
            ExprKind::MemberAccess(lhs, name) => {
                let lhs_ty = self.analyze_expr(lhs)?;
                if lhs_ty == Type::Any { return Ok(Type::Any); }
                let actual_ty = match lhs_ty {
                    Type::BoxPtr(inner) => *inner,
                    Type::RawPtr(inner, _) => *inner,
                    Type::Ref(inner, _) => *inner,
                    _ => lhs_ty,
                };
                if let Type::Custom(ref obj_name, _) = actual_ty {
                    if self.phantom_types.contains(obj_name) {
                        return Ok(Type::Any);
                    }
                    if let Some(fields) = self.objects.get(obj_name) {
                        if let Some(ty) = fields.get(name) { return Ok(ty.clone()); }
                    }
                }
                self.semantic_error(format!("No member '{}' on type {:?}", name, actual_ty), span)
            }
            ExprKind::IndexAccess(lhs, index) => {
                let lhs_ty = self.analyze_expr(lhs)?;
                self.analyze_expr(index)?;
                if lhs_ty == Type::Any { return Ok(Type::Any); }
                let actual_ty = match lhs_ty {
                    Type::BoxPtr(inner) => *inner,
                    Type::RawPtr(inner, _) => *inner,
                    Type::Ref(inner, _) => *inner,
                    Type::Array(inner, _) => *inner,
                    _ => return self.semantic_error("Indexing only works on arrays or pointers".into(), span),
                };
                Ok(actual_ty)
            }
            ExprKind::Alloc(inner, kind) => {
                let ty = self.analyze_expr(inner)?;
                match kind {
                    AllocKind::Box => Ok(Type::BoxPtr(Box::new(ty))),
                    AllocKind::RawMut => Ok(Type::RawPtr(Box::new(ty), true)),
                    AllocKind::RawConst => Ok(Type::RawPtr(Box::new(ty), false)),
                }
            }
            ExprKind::Borrow(inner, mutable) => {
                let ty = self.analyze_expr(inner)?;
                Ok(Type::Ref(Box::new(ty), *mutable))
            }
            ExprKind::Negate(inner) => {
                let ty = self.analyze_expr(inner)?;
                if ty == Type::Any { return Ok(Type::Any); }
                match ty {
                    Type::I32 | Type::I64 | Type::F32 | Type::F64 => Ok(ty),
                    _ => self.semantic_error(format!("Cannot negate type {:?}", ty), span),
                }
            }
            ExprKind::Deref(inner) => {
                let ty = self.analyze_expr(inner)?;
                match ty {
                    Type::Ref(inner_ty, _) => Ok(*inner_ty),
                    Type::BoxPtr(inner_ty) => Ok(*inner_ty),
                    Type::RawPtr(inner_ty, _) => Ok(*inner_ty),
                    _ => self.semantic_error(format!("Cannot dereference type {:?}", ty), span),
                }
            }
            ExprKind::Unwrap(inner) => {
                let ty = self.analyze_expr(inner)?;
                match ty {
                    Type::Result(ok, _) => Ok(*ok),
                    _ => Ok(ty), 
                }
            }
            ExprKind::Await(inner) => {
                self.analyze_expr(inner)
            }
            ExprKind::Cast(inner, ty) => {
                self.analyze_expr(inner)?;
                Ok(self.resolve_type(ty))
            }
        }
    }
}
