// src/sema/expressions.rs

use crate::ast::*;
use crate::error::CompilerError;
use crate::lexer::Span;
use miette::SourceSpan;
use std::collections::{HashMap, HashSet}; // Required for context if called standalone

use super::AnalysisInfo;
use crate::sema::types::TypeInfo;

pub struct ExpressionAnalyzer<'a> {
    analysis_info: &'a mut AnalysisInfo,
    type_info: &'a mut TypeInfo,
}

impl<'a> ExpressionAnalyzer<'a> {
    pub fn new(analysis_info: &'a mut AnalysisInfo, type_info: &'a mut TypeInfo) -> Self {
        Self {
            analysis_info,
            type_info,
        }
    }

    pub fn analyze_expr(&mut self, expr: &mut Expr) -> Result<Type, CompilerError> {
        let span = expr.span;
        let ty = match &mut expr.kind {
            ExprKind::Unit => Ok(Type::Unit),
            ExprKind::Int(_) => Ok(Type::I32),
            ExprKind::Int64(_) => Ok(Type::I64),
            ExprKind::Float(_) => Ok(Type::F32),
            ExprKind::Float64(_) => Ok(Type::F64),
            ExprKind::Bool(_) => Ok(Type::Bool),
            ExprKind::String(_) => Ok(Type::Str), // Assuming String literals are of type Str
            ExprKind::Variable(path) => {
                let name = self.analysis_info.path_to_string(path);

                // Check for await promotion
                let await_promoted =
                    if let Some((depth, _)) = self.analysis_info.var_declarations.get(&name) {
                        self.analysis_info
                            .await_points
                            .iter()
                            .any(|&aw_depth| aw_depth >= *depth)
                    } else {
                        false
                    };
                if await_promoted {
                    self.analysis_info.promote_variable(&name, true);
                }

                if let Some((ty, _)) = self.analysis_info.symbols.get(&name) {
                    Ok(ty.clone())
                } else if self.type_info.functions.contains_key(&name) {
                    // Check if it's a function name
                    Ok(Type::Any) // Function call without args is type Any for now
                } else {
                    // Check if it's a phantom type
                    let is_phantom = path.iter().any(|p| self.type_info.is_phantom_type(&p.name));
                    if is_phantom || self.type_info.has_wildcard_phantom {
                        Ok(Type::Any)
                    } else {
                        self.analysis_info
                            .semantic_error(format!("Undeclared variable '{}'", name), span)
                    }
                }
            }
            ExprKind::Binary(lhs, op, rhs) => {
                let l = self.analyze_expr(lhs)?;
                let r = self.analyze_expr(rhs)?;

                if l == Type::Any || r == Type::Any {
                    Ok(Type::Any)
                } else {
                    let l_base = self.analysis_info.deref_type(&l);
                    let r_base = self.analysis_info.deref_type(&r);

                    // Special handling for string concatenation
                    if *op == BinaryOp::Add {
                        let is_l_string = l_base == Type::String || l_base == Type::Str;
                        let is_r_string = r_base == Type::String || r_base == Type::Str;
                        if is_l_string || is_r_string {
                            return Ok(Type::String); // Result of string concat is String
                        }
                    }

                    if !self.analysis_info.types_equal(&l_base, &r_base) {
                        return self.analysis_info.semantic_error(
                            format!("Type mismatch in binary operation: {:?} and {:?}", l, r),
                            span,
                        );
                    }
                    match op {
                        BinaryOp::GreaterThan
                        | BinaryOp::LessThan
                        | BinaryOp::GreaterThanOrEqual
                        | BinaryOp::LessThanOrEqual
                        | BinaryOp::Equal => Ok(Type::Bool),
                        BinaryOp::Modulo => {
                            if l_base != Type::I32 && l_base != Type::I64 {
                                return self.analysis_info.semantic_error(
                                    "Modulo (%) can only be used on integers (for now)".into(),
                                    span,
                                );
                            }
                            Ok(l_base)
                        }
                        _ => Ok(l_base), // For other ops like +, -, *, /, return the base type
                    }
                }
            }
            ExprKind::Tuple(items) => {
                let mut types = Vec::new();
                for item in items {
                    types.push(self.analyze_expr(item)?);
                }
                Ok(Type::Tuple(types))
            }
            ExprKind::MacroCall(name, args) => {
                for arg in args {
                    self.analyze_expr(arg)?;
                }
                // Macro return types are simplified for now
                if name == "typeof" {
                    Ok(Type::Str)
                } else if name == "str" {
                    Ok(Type::String) // str!() returns owned string
                } else {
                    Ok(Type::I32) // Default to I32 or Unit for other macros
                }
            }
            ExprKind::Array(items) => {
                if items.is_empty() {
                    Ok(Type::Array(Box::new(Type::Any), 0))
                } else {
                    let first_ty = self.analyze_expr(&mut items[0])?;
                    for item in items.iter_mut().skip(1) {
                        let ty = self.analyze_expr(item)?;
                        if !self.analysis_info.types_equal(&first_ty, &ty)
                            && first_ty != Type::Any
                            && ty != Type::Any
                        {
                            return self.analysis_info.semantic_error(
                                format!(
                                    "Mismatched types in array literal: {:?} and {:?}",
                                    first_ty, ty
                                ),
                                span,
                            );
                        }
                    }
                    Ok(Type::Array(Box::new(first_ty), items.len()))
                }
            }
            ExprKind::Block(stmts) => {
                use crate::sema::analysis::statements::StatementAnalyzer;
                let mut last_ty = Type::Unit;
                {
                    let mut sa = StatementAnalyzer::new(self.analysis_info, self.type_info);
                    for stmt in stmts {
                        sa.analyze_stmt(stmt)?;
                        if let StmtKind::ExprStmt(ref e) = stmt.kind {
                            last_ty = e.ty.clone().unwrap_or(Type::Unit);
                        } else {
                            last_ty = Type::Unit;
                        }
                    }
                }
                Ok(last_ty)
            }
            ExprKind::Call(path, args, resolved_name, arg_kinds, param_types_field) => {
                let name = self.analysis_info.path_to_string(path);
                //println!("DEBUG analyze Call name={}, current_prefix={}", name, self.analysis_info.current_prefix);

                // Handle special calls like Ok() and Err()
                if name == "Ok" {
                    let val_ty = self.analyze_expr(&mut args[0])?;
                    let ty = Type::Result(Box::new(val_ty), Box::new(Type::Generic("E".into())));
                    expr.ty = Some(ty.clone());
                    return Ok(ty);
                } else if name == "Err" {
                    let err_ty = self.analyze_expr(&mut args[0])?;
                    let ty = Type::Result(Box::new(Type::Generic("T".into())), Box::new(err_ty));
                    expr.ty = Some(ty.clone());
                    return Ok(ty);
                } else if name == "array_init" {
                    let val_ty = self.analyze_expr(&mut args[0])?;
                    let ty = Type::RawPtr(Box::new(val_ty), true);
                    expr.ty = Some(ty.clone());
                    return Ok(ty);
                }

                // Attempt to resolve function signature from type_info
                let mut found_name_and_ret = None;

                let name_to_lookup =
                    if !name.contains("::") && !self.analysis_info.current_prefix.is_empty() {
                        format!("{}::{}", self.analysis_info.current_prefix, name)
                    } else {
                        name.clone()
                    };

                if let Some((param_types, rt)) = self.type_info.functions.get(&name_to_lookup) {
                    found_name_and_ret =
                        Some((name_to_lookup.clone(), param_types.clone(), rt.clone()));
                } else if let Some((param_types, rt)) = self.type_info.functions.get(&name) {
                    found_name_and_ret = Some((name.clone(), param_types.clone(), rt.clone()));
                }

                if found_name_and_ret.is_none() {
                    // Handle phantom types and functions
                    let is_phantom = path.iter().any(|p| self.type_info.is_phantom_type(&p.name));
                    if is_phantom || self.type_info.has_wildcard_phantom {
                        for arg in args.iter_mut() {
                            self.analyze_expr(arg)?;
                        }
                        *resolved_name = Some(name.clone());
                        let ty = Type::Any;
                        expr.ty = Some(ty.clone());
                        return Ok(ty);
                    }

                    return self.analysis_info.semantic_error(
                        format!("Undeclared function or variant '{}'", name),
                        span,
                    );
                }

                let (resolved_func_name, params, ret_type) = found_name_and_ret.unwrap();
                //println!("DEBUG analyze Call resolved_func_name={}", resolved_func_name);
                *resolved_name = Some(resolved_func_name.clone());

                let p_types: Vec<Type> = params.iter().map(|(t, _)| t.clone()).collect();
                let p_kinds: Vec<ArgKind> = params.iter().map(|(_, k)| *k).collect();

                *param_types_field = Some(p_types);
                *arg_kinds = Some(p_kinds);

                let mut arg_types = Vec::new();
                for arg in args.iter_mut() {
                    arg_types.push(self.analyze_expr(arg)?);
                }

                Ok(ret_type.unwrap_or(Type::I32)) // Simplified return type
            }
            ExprKind::MethodCall(
                lhs,
                name,
                args,
                resolved_obj_name,
                arg_kinds,
                param_types_field,
            ) => {
                let mut lhs_ty = self.analyze_expr(lhs)?;
                // println!("DEBUG analyze MethodCall name={}, lhs_ty={:?}", name, lhs_ty);
                let resolved_lhs_ty = self
                    .type_info
                    .resolve_type(&lhs_ty, &self.analysis_info.current_prefix);

                if resolved_lhs_ty == Type::Any {
                    for arg in args.iter_mut() {
                        self.analyze_expr(arg)?;
                    }
                    *resolved_obj_name = Some("any".to_string());
                    let ty = Type::Any;
                    expr.ty = Some(ty.clone());
                    return Ok(ty);
                }

                if name == "clone" {
                    // Special handling for clone
                    expr.ty = Some(resolved_lhs_ty.clone());
                    return Ok(resolved_lhs_ty);
                }

                if name == "into" {
                    // Special handling for into
                    for arg in args.iter_mut() {
                        self.analyze_expr(arg)?;
                    }
                    let ty = Type::Any;
                    expr.ty = Some(ty.clone());
                    return Ok(ty);
                }

                // Dereference to get the actual object type for method lookup
                let actual_ty = self.analysis_info.deref_type(&resolved_lhs_ty);

                let obj_name = match &actual_ty {
                    Type::Str => "str".to_string(),
                    Type::String => "string".to_string(),
                    Type::I32 => "i32".to_string(), // Primitive types may have methods
                    Type::I64 => "i64".to_string(),
                    Type::F32 => "f32".to_string(),
                    Type::F64 => "f64".to_string(),
                    Type::Array(_, _) => "vector".to_string(), // Treat array as vector for method lookup
                    Type::Custom(n, _) => n.clone(),
                    Type::Generic(n) => {
                        // If it's a generic type, we need to look up method in its bounds
                        "generic".to_string() // Placeholder
                    }
                    _ => {
                        return self.analysis_info.semantic_error(
                            format!(
                                "Method call '{}' on non-object/non-primitive type {:?}",
                                name, resolved_lhs_ty
                            ),
                            span,
                        );
                    }
                };
                *resolved_obj_name = Some(obj_name.clone());

                // Check for phantom types
                if self.type_info.is_phantom_type(&obj_name) {
                    for arg in args.iter_mut() {
                        self.analyze_expr(arg)?;
                    }
                    return Ok(Type::Any);
                }

                let full_method_name = format!("{}::{}", obj_name, name);
                //println!("DEBUG analyze MethodCall full_method_name={}", full_method_name);

                // Resolve types of arguments
                let mut arg_types = Vec::new();
                for arg in args.iter_mut() {
                    arg_types.push(self.analyze_expr(arg)?);
                }

                // Lookup the method signature from type_info.functions
                if let Some((params, ret_type)) =
                    self.type_info.functions.get(&full_method_name).cloned()
                {
                    let p_types: Vec<Type> = params.iter().map(|(t, _)| t.clone()).collect();
                    let p_kinds: Vec<ArgKind> = params.iter().map(|(_, k)| *k).collect();

                    *param_types_field = Some(p_types);
                    *arg_kinds = Some(p_kinds);

                    let ty = ret_type.unwrap_or(Type::I32);
                    expr.ty = Some(ty.clone());
                    return Ok(ty);
                } else if self.type_info.has_wildcard_phantom {
                    // Fallback to Any if not found but phantom mode is active
                    return Ok(Type::Any);
                } else {
                    return self.analysis_info.semantic_error(
                        format!("No method '{}' found on type '{}'", name, obj_name),
                        span,
                    );
                }
            }
            ExprKind::StructLiteral {
                path,
                fields,
                resolved_name,
            } => {
                let name = self.analysis_info.path_to_string(path);
                let is_phantom = path.iter().any(|p| self.type_info.is_phantom_type(&p.name));

                // Attempt to resolve struct name
                let mut found = false;
                let name_to_lookup =
                    if !name.contains("::") && !self.analysis_info.current_prefix.is_empty() {
                        format!("{}::{}", self.analysis_info.current_prefix, name)
                    } else {
                        name.clone()
                    };

                if self.type_info.objects.contains_key(&name_to_lookup) {
                    *resolved_name = Some(name_to_lookup.clone());
                    found = true;
                } else if self.type_info.objects.contains_key(&name) {
                    *resolved_name = Some(name.clone());
                    found = true;
                }

                if !found {
                    if is_phantom || self.type_info.has_wildcard_phantom {
                        for (_, fexpr) in fields {
                            self.analyze_expr(fexpr)?;
                        }
                        *resolved_name = Some(name.clone());
                        let ty = Type::Any;
                        expr.ty = Some(ty.clone());
                        return Ok(ty);
                    }

                    return self
                        .analysis_info
                        .semantic_error(format!("Undeclared object type '{}'", name), span);
                }

                let final_name = resolved_name.as_ref().unwrap().clone();

                // Placeholder for detailed struct literal analysis
                for (_, fexpr) in fields {
                    self.analyze_expr(fexpr)?;
                }

                Ok(Type::Custom(final_name, Vec::new())) // Simplified: return type as Custom
            }
            ExprKind::MemberAccess(lhs, name) => {
                let lhs_ty = self.analyze_expr(lhs)?;
                //println!("DEBUG analyze MemberAccess name={}, lhs_ty={:?}", name, lhs_ty);
                if lhs_ty == Type::Any {
                    Ok(Type::Any)
                } else {
                    let resolved_lhs = self
                        .type_info
                        .resolve_type(&lhs_ty, &self.analysis_info.current_prefix);
                    let actual_ty = self.analysis_info.deref_type(&resolved_lhs);

                    match actual_ty {
                        Type::Tuple(ref types) => {
                            if let Ok(idx) = name.parse::<usize>() {
                                if let Some(ty) = types.get(idx) {
                                    Ok(ty.clone())
                                } else {
                                    self.analysis_info.semantic_error(
                                        format!(
                                            "Tuple index {} out of bounds (len: {})",
                                            idx,
                                            types.len()
                                        ),
                                        span,
                                    )
                                }
                            } else {
                                self.analysis_info.semantic_error(
                                    format!(
                                        "Tuple access must be a numeric index, found '{}'",
                                        name
                                    ),
                                    span,
                                )
                            }
                        }
                        Type::Custom(ref obj_name, ref obj_generics) => {
                            if self.type_info.is_phantom_type(obj_name) {
                                return Ok(Type::Any);
                            }
                            // Lookup member in objects map from type_info
                            if let Some((fields, _)) = self.type_info.objects.get(obj_name) {
                                if let Some(field_ty) = fields.get(name) {
                                    Ok(field_ty.clone())
                                } else {
                                    self.analysis_info.semantic_error(
                                        format!("No member '{}' on type '{}'", name, obj_name),
                                        span,
                                    )
                                }
                            } else {
                                self.analysis_info.semantic_error(
                                    format!("Type '{}' not found or has no members", obj_name),
                                    span,
                                )
                            }
                        }
                        Type::Generic(ref generic_name) => {
                            // Accessing member of a generic type. Requires lookup in generic bounds.
                            Ok(Type::Any) // Placeholder
                        }
                        _ => self.analysis_info.semantic_error(
                            format!("No member '{}' on type {:?}", name, actual_ty),
                            span,
                        ),
                    }
                }
            }
            ExprKind::IndexAccess(lhs, index) => {
                let lhs_ty = self.analyze_expr(lhs)?;
                self.analyze_expr(index)?; // Analyze index expression
                if lhs_ty == Type::Any {
                    Ok(Type::Any)
                } else {
                    // Determine the type of the element being indexed.
                    match lhs_ty {
                        Type::BoxPtr(inner)
                        | Type::RawPtr(inner, _)
                        | Type::Ref(inner, _)
                        | Type::Managed(inner)
                        | Type::ThreadSafe(inner)
                        | Type::Array(inner, _) => {
                            Ok(*inner) // Return the inner type
                        }
                        _ => self.analysis_info.semantic_error(
                            "Indexing only supported on array, pointer, or reference types".into(),
                            span,
                        ),
                    }
                }
            }
            ExprKind::Alloc(inner, kind) => {
                let ty = self.analyze_expr(inner)?;
                let base_ty = self.analysis_info.deref_type(&ty);
                match kind {
                    AllocKind::Box => Ok(Type::BoxPtr(Box::new(base_ty))),
                    AllocKind::RawMut => Ok(Type::RawPtr(Box::new(base_ty), true)),
                    AllocKind::RawConst => Ok(Type::RawPtr(Box::new(base_ty), false)),
                    AllocKind::Rc => Ok(Type::Managed(Box::new(base_ty))),
                    AllocKind::Arc => Ok(Type::ThreadSafe(Box::new(base_ty))),
                }
            }
            ExprKind::Borrow(inner, mutable) => {
                let ty = self.analyze_expr(inner)?;
                Ok(Type::Ref(Box::new(ty), *mutable))
            }
            ExprKind::Deref(inner) => {
                let ty = self.analyze_expr(inner)?;
                match ty {
                    Type::Ref(inner_ty, _)
                    | Type::BoxPtr(inner_ty)
                    | Type::RawPtr(inner_ty, _)
                    | Type::Managed(inner_ty)
                    | Type::ThreadSafe(inner_ty) => Ok(*inner_ty),
                    Type::Any => Ok(Type::Any),
                    _ => self
                        .analysis_info
                        .semantic_error(format!("Cannot dereference type {:?}", ty), span),
                }
            }
            ExprKind::Downgrade(inner) => {
                let ty = self.analyze_expr(inner)?;
                match ty {
                    Type::Managed(inner_ty) => Ok(Type::WeakManaged(inner_ty)),
                    Type::ThreadSafe(inner_ty) => Ok(Type::WeakThreadSafe(inner_ty)),
                    _ => self.analysis_info.semantic_error(
                        format!("Cannot downgrade non-managed type {:?}", ty),
                        span,
                    ),
                }
            }
            ExprKind::Negate(inner) => {
                let ty = self.analyze_expr(inner)?;
                match ty {
                    Type::I32 | Type::I64 | Type::F32 | Type::F64 => Ok(ty),
                    Type::Any => Ok(Type::Any),
                    _ => self
                        .analysis_info
                        .semantic_error(format!("Cannot negate type {:?}", ty), span),
                }
            }
            ExprKind::Unwrap(inner) => {
                let ty = self.analyze_expr(inner)?;
                match ty {
                    Type::Result(ok, _) => Ok(*ok),
                    Type::WeakManaged(inner_ty) => Ok(Type::Managed(inner_ty)),
                    Type::WeakThreadSafe(inner_ty) => Ok(Type::ThreadSafe(inner_ty)),
                    Type::Any => Ok(Type::Any),
                    _ => Ok(ty), // If not Result or managed, assume it's already unwrapped
                }
            }
            ExprKind::Try(inner) => {
                let ty = self.analyze_expr(inner)?;
                match ty {
                    Type::Result(ok, _) => Ok(*ok),
                    Type::Custom(ref n, ref g) if n == "Result" && g.len() == 2 => Ok(g[0].clone()),
                    Type::Any => Ok(Type::Any),
                    _ => self.analysis_info.semantic_error(
                        format!("Cannot use '?' on non-result type {:?}", ty),
                        span,
                    ),
                }
            }
            ExprKind::Await(inner) => {
                let ty = self.analyze_expr(inner)?;
                self.analysis_info
                    .await_points
                    .push(self.analysis_info.scope_depth); // Record depth of await
                Ok(ty)
            }
            ExprKind::Cast(inner, ty) => {
                self.analyze_expr(inner)?;
                // Resolve the target type. Requires resolve_type.
                let resolved_ty = self
                    .type_info
                    .resolve_type(ty, &self.analysis_info.current_prefix);
                Ok(resolved_ty)
            }
        }?;
        //println!("DEBUG analyze_expr setting ty for {:?}: {:?}", expr.kind, ty);
        expr.ty = Some(ty.clone());
        Ok(ty)
    }

    pub fn refine_expr(&mut self, expr: &mut Expr) -> Result<(), CompilerError> {
        match &mut expr.kind {
            ExprKind::Binary(lhs, _, rhs) => {
                self.refine_expr(lhs)?;
                self.refine_expr(rhs)?;
            }
            ExprKind::Call(_, args, _, _, _)
            | ExprKind::MacroCall(_, args)
            | ExprKind::Array(args) => {
                for arg in args {
                    self.refine_expr(arg)?;
                }
            }
            ExprKind::Block(stmts) => {
                use crate::sema::analysis::statements::StatementAnalyzer;
                let mut sa = StatementAnalyzer::new(self.analysis_info, self.type_info);
                for stmt in stmts {
                    sa.refine_stmt(stmt)?;
                }
            }
            ExprKind::MethodCall(lhs, _, args, _, _, _) => {
                self.refine_expr(lhs)?;
                for arg in args {
                    self.refine_expr(arg)?;
                }
            }
            ExprKind::StructLiteral { fields, .. } => {
                for (_, fexpr) in fields {
                    self.refine_expr(fexpr)?;
                }
            }
            ExprKind::MemberAccess(lhs, _)
            | ExprKind::IndexAccess(lhs, _)
            | ExprKind::Alloc(lhs, _)
            | ExprKind::Borrow(lhs, _)
            | ExprKind::Deref(lhs)
            | ExprKind::Downgrade(lhs)
            | ExprKind::Negate(lhs)
            | ExprKind::Unwrap(lhs)
            | ExprKind::Await(lhs)
            | ExprKind::Try(lhs)
            | ExprKind::Cast(lhs, _) => {
                self.refine_expr(lhs)?;
            }
            _ => {} // Do nothing for other expression kinds
        }
        Ok(())
    }
}
