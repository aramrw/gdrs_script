use proc_macro2::TokenStream;
use quote::quote;

use crate::{ast::{ExprKind, Stmt, StmtKind, Type}, compiler::{compile_expr, compile_pattern, types::compile_type_ext}};

pub fn compile_stmt(
    stmt: &Stmt,
    is_last: bool,
    target_obj: Option<&String>,
    expected_ret: Option<&Type>,
) -> TokenStream {
    match &stmt.kind {
        StmtKind::VarDecl {
            name,
            is_mutable,
            ty: expected_ty,
            value,
        } => {
            let id = quote::format_ident!("{}", name);
            let mut_kw = if *is_mutable { quote!(mut) } else { quote!() };

            let val_raw = compile_expr(value, target_obj, false);

            // Check if variable was promoted (by looking at the type stored in the value expression)
            // Prioritize expected_ty if it exists and is different from value.ty (e.g. for pointers)
            let final_ty = expected_ty.as_ref().or(value.ty.as_ref());

            let (ty_tokens, final_val) = if let Some(pt) = final_ty {
                let ct = compile_type_ext(pt, target_obj);
                let val_managed = match pt {
                    Type::Managed(_) if !matches!(value.ty, Some(Type::Managed(_))) => {
                        quote! { ::std::rc::Rc::new(::std::cell::RefCell::new(#val_raw)) }
                    }
                    Type::ThreadSafe(_) if !matches!(value.ty, Some(Type::ThreadSafe(_))) => {
                        quote! { ::std::sync::Arc::new(::parking_lot::RwLock::new(#val_raw)) }
                    }
                    Type::BoxPtr(_) if !matches!(value.ty, Some(Type::BoxPtr(_))) => {
                        quote! { Box::new(#val_raw) }
                    }
                    _ => val_raw,
                };
                (quote!(: #ct), val_managed)
            } else {
                (quote!(), val_raw)
            };

            quote! { let #mut_kw #id #ty_tokens = #final_val; }
        }
        StmtKind::Assign { target, value } => {
            let v = compile_expr(value, target_obj, false);
            if let ExprKind::IndexAccess(lhs, index) = &target.kind {
                let l = compile_expr(lhs, target_obj, true);
                let i = compile_expr(index, target_obj, false);
                quote! { unsafe { *#l.add(crate::SolarAsSize::as_size(&#i)) = (&#v).as_val(); } }
            } else {
                let t = compile_expr(target, target_obj, true);
                quote! { #t = (&#v).as_val(); }
            }
        }
        StmtKind::Block(stmts) => {
            let mut inner = TokenStream::new();
            for (i, s) in stmts.iter().enumerate() {
                inner.extend(compile_stmt(
                    s,
                    i == stmts.len() - 1,
                    target_obj,
                    expected_ret,
                ));
            }
            quote! { { #inner } }
        }
        StmtKind::UnsafeBlock(stmts) => {
            let mut inner = TokenStream::new();
            for (i, s) in stmts.iter().enumerate() {
                inner.extend(compile_stmt(
                    s,
                    i == stmts.len() - 1,
                    target_obj,
                    expected_ret,
                ));
            }
            quote! { unsafe { #inner } }
        }
        StmtKind::ExprStmt(expr) => {
            let e = compile_expr(expr, target_obj, false);
            if is_last {
                match expected_ret {
                    Some(Type::Unit) | None => quote!(#e;),
                    _ => quote!(#e),
                }
            } else {
                quote!(#e;)
            }
        }
        StmtKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let cond = compile_expr(condition, target_obj, false);
            let then = compile_stmt(then_branch, is_last, target_obj, expected_ret);
            if let Some(else_b) = else_branch {
                let els = compile_stmt(else_b, is_last, target_obj, expected_ret);
                quote! { if #cond #then else #els }
            } else {
                quote! { if #cond #then }
            }
        }
        StmtKind::While { condition, body } => {
            let cond = compile_expr(condition, target_obj, false);
            let b = compile_stmt(body, false, target_obj, expected_ret);
            quote! { while #cond #b }
        }
        StmtKind::Loop { body } => {
            let b = compile_stmt(body, false, target_obj, expected_ret);
            quote! { loop #b }
        }
        StmtKind::Break(expr) => {
            let e = expr.as_ref().map(|e| compile_expr(e, target_obj, false));
            if let Some(tokens) = e {
                quote! { break #tokens; }
            } else {
                quote! { break; }
            }
        }
        StmtKind::Return(expr) => {
            let e = expr.as_ref().map(|e| compile_expr(e, target_obj, false));
            if let Some(tokens) = e {
                quote! { return #tokens; }
            } else {
                quote! { return; }
            }
        }
        StmtKind::Match { expr, arms } => {
            let e = compile_expr(expr, target_obj, false);
            let arm_tokens = arms.iter().map(|arm| {
                let pat = compile_pattern(&arm.pattern);
                let body = compile_stmt(&arm.body, is_last, target_obj, expected_ret);
                quote! { #pat => { #body } }
            });
            quote! { match #e { #( #arm_tokens ),* } }
        }
    }
}
