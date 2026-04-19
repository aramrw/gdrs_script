use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    ast::{ExprKind, Stmt, StmtKind, Type},
    compiler::{compile_expr, compile_pattern, types::compile_type_ext},
};

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

            // If the expression returns a reference in Rust (Alloc, Downgrade),
            // but for a variable declaration we want the owned value.
            let returns_ref = match &value.kind {
                ExprKind::Alloc(_, _) | ExprKind::Downgrade(_) => true,
                ExprKind::MethodCall(_, name, _, _, _, _) if name == "into" => true,
                _ => false,
            };

            // Check if variable was promoted (by looking at the type stored in the value expression)
            // Prioritize expected_ty if it exists and is different from value.ty (e.g. for pointers)
            let final_ty = expected_ty.as_ref().or(value.ty.as_ref());

            let (ty_tokens, final_val) = if let Some(pt) = final_ty {
                let ct = if matches!(pt, Type::Str) {
                    quote!(&str)
                } else {
                    compile_type_ext(pt, target_obj)
                };

                let mut val_src = if returns_ref {
                    quote! { (#val_raw).clone() }
                } else {
                    val_raw.clone()
                };

                let val_managed = match pt {
                    Type::Managed(_) if !matches!(value.ty, Some(Type::Managed(_))) => {
                        quote! { ::std::rc::Rc::new(::std::cell::RefCell::new((&#val_src).as_val())) }
                    }
                    Type::ThreadSafe(_) if !matches!(value.ty, Some(Type::ThreadSafe(_))) => {
                        quote! { ::std::sync::Arc::new(::parking_lot::RwLock::new((&#val_src).as_val())) }
                    }
                    Type::BoxPtr(_) if !matches!(value.ty, Some(Type::BoxPtr(_))) => {
                        quote! { Box::new((&#val_src).as_val()) }
                    }
                    _ => val_src,
                };
                (quote!(: #ct), val_managed)
            } else {
                let final_val = if returns_ref {
                    quote! { (#val_raw).clone() }
                } else {
                    val_raw
                };
                (quote!(), final_val)
            };

            quote! { let #mut_kw #id #ty_tokens = #final_val; }
        }
        StmtKind::Assign { target, value } => {
            let v = compile_expr(value, target_obj, false);
            if let ExprKind::IndexAccess(lhs, index) = &target.kind {
                let l = compile_expr(lhs, target_obj, true);
                let i = compile_expr(index, target_obj, false);
                quote! {
                    {
                        let __val = (&#v).as_val();
                        #l.solar_index_mut(#i, __val);
                    }
                }
            } else {
                let t = compile_expr(target, target_obj, true);
                quote! {
                    {
                        let __val = (&#v).as_val();
                        #t = __val;
                    }
                }
            }
        }
        StmtKind::ExprStmt(expr) => {
            let e = compile_expr(expr, target_obj, false);
            if is_last {
                if let Some(Type::Result(_, _)) = expected_ret {
                    quote! { Ok(#e) }
                } else if matches!(expected_ret, Some(Type::Unit)) {
                    quote! { { #e; () } }
                } else {
                    quote! { #e }
                }
            } else {
                quote! { let _ = #e; }
            }
        }
        StmtKind::Return(expr) => {
            if let Some(e) = expr {
                let e_compiled = compile_expr(e, target_obj, false);
                if let Some(Type::Result(_, _)) = expected_ret {
                    quote! { return Ok(#e_compiled); }
                } else {
                    quote! { return #e_compiled; }
                }
            } else {
                if let Some(Type::Result(_, _)) = expected_ret {
                    quote! { return Ok(()); }
                } else {
                    quote! { return; }
                }
            }
        }
        StmtKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let cond = compile_expr(condition, target_obj, false);
            let then_tokens = compile_stmt(then_branch, is_last, target_obj, expected_ret);
            if let Some(else_stmt) = else_branch {
                let else_tokens = compile_stmt(else_stmt, is_last, target_obj, expected_ret);
                quote! { if #cond { #then_tokens } else { #else_tokens } }
            } else {
                quote! { if #cond { #then_tokens } }
            }
        }
        StmtKind::While { condition, body } => {
            let cond = compile_expr(condition, target_obj, false);
            let body_tokens = compile_stmt(body, false, target_obj, expected_ret);
            quote! { while #cond { #body_tokens } }
        }
        StmtKind::Loop { body } => {
            let body_tokens = compile_stmt(body, false, target_obj, expected_ret);
            quote! { loop { #body_tokens } }
        }
        StmtKind::Block(stmts) => {
            let mut tokens = TokenStream::new();
            for (i, s) in stmts.iter().enumerate() {
                let is_last_in_block = i == stmts.len() - 1;
                tokens.extend(compile_stmt(
                    s,
                    is_last && is_last_in_block,
                    target_obj,
                    expected_ret,
                ));
            }
            quote! { { #tokens } }
        }
        StmtKind::UnsafeBlock(stmts) => {
            let mut tokens = TokenStream::new();
            for (i, s) in stmts.iter().enumerate() {
                let is_last_in_block = i == stmts.len() - 1;
                tokens.extend(compile_stmt(
                    s,
                    is_last && is_last_in_block,
                    target_obj,
                    expected_ret,
                ));
            }
            quote! { unsafe { #tokens } }
        }
        StmtKind::Break(expr) => {
            if let Some(e) = expr {
                let e_compiled = compile_expr(e, target_obj, false);
                quote!(break #e_compiled;)
            } else {
                quote!(break;)
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
