use proc_macro2::TokenStream;
use quote::quote;

use crate::{ast::{AllocKind, ArgKind, BinaryOp, Expr, ExprKind, Type}, compiler::{compile_id, compile_id_expr, compile_path, path_to_string, types::compile_type_ext, wrap_expr_for_ref}};

pub fn compile_expr(expr: &Expr, target_obj: Option<&String>, is_mut: bool) -> TokenStream {
    match &expr.kind {
        ExprKind::Unit => quote!(()),
        ExprKind::Int(v) => quote!(#v),
        ExprKind::Int64(v) => quote! { #v },
        ExprKind::Float(v) => quote! { (#v as f32) },
        ExprKind::Float64(v) => quote! { (#v as f64) },
        ExprKind::Bool(v) => quote! { #v },
        ExprKind::String(v) => quote! { #v },
        ExprKind::Variable(n) => {
            let name = path_to_string(n);
            if name == "self" {
                quote!(self)
            } else {
                compile_path(n, target_obj)
            }
        }
        ExprKind::Binary(lhs, op, rhs) => {
            let l = compile_expr(lhs, target_obj, is_mut);
            let r = wrap_expr_for_ref(rhs, target_obj, false);
            match op {
                BinaryOp::Add => {
                    quote! { (#l).solar_add(#r) }
                }
                BinaryOp::Subtract => quote! { (#l).solar_sub(#r) },
                BinaryOp::Multiply => quote! { (#l).solar_mul(#r) },
                BinaryOp::Divide => quote! { (#l).solar_div(#r) },
                BinaryOp::GreaterThan => quote! { (#l).solar_gt(#r) },
                BinaryOp::LessThan => quote! { (#l).solar_lt(#r) },
                BinaryOp::GreaterThanOrEqual => quote! { (#l).solar_ge(#r) },
                BinaryOp::LessThanOrEqual => quote! { (#l).solar_le(#r) },
                BinaryOp::Equal => quote! { (#l).solar_eq(#r) },
            }
        }
        ExprKind::MacroCall(name, args) => {
            let name_id = compile_id(name);
            let args_compiled: Vec<_> = args
                .iter()
                .map(|a| wrap_expr_for_ref(a, target_obj, false))
                .collect();
            if name == "println" || name == "print" || name == "log" || name == "logln" {
                let actual_name = match name.as_str() {
                    "log" => "print",
                    "logln" => "println",
                    _ => name,
                };
                let name_id = quote::format_ident!("{}", actual_name);
                if let Some(Expr {
                    kind: ExprKind::String(fmt),
                    ..
                }) = args.get(0)
                {
                    let mut format_str = fmt.clone();
                    let rest_compiled = &args_compiled[1..];

                    // Count existing placeholders
                    let placeholders = format_str.matches("{}").count();
                    for _ in placeholders..rest_compiled.len() {
                        if !format_str.is_empty() && !format_str.ends_with(' ') {
                            format_str.push(' ');
                        }
                        format_str.push_str("{}");
                    }

                    quote! { #name_id!(#format_str, #( #rest_compiled ),*) }
                } else {
                    let format_str = vec!["{}"; args_compiled.len()].join(" ");
                    quote! { #name_id!(#format_str, #( #args_compiled ),*) }
                }
            }
 else if name == "typeof" {
                let arg = &args_compiled[0];
                quote! { std::any::type_name_of_val(#arg) }
            } else if name == "str" {
                if args.is_empty() {
                    quote! { ::std::string::String::new() }
                } else {
                    let arg = compile_expr(&args[0], target_obj, false);
                    quote! { (&#arg).as_val() }
                }
            } else {
                let args_raw: Vec<_> = args
                    .iter()
                    .map(|a| compile_expr(a, target_obj, false))
                    .collect();
                quote! { #name_id!(#( #args_raw ),*) }
            }
        }
        ExprKind::Call(name, args, resolved_name, arg_kinds) => {
            let is_phantom = matches!(expr.ty, Some(Type::Any));
            let simple_name = path_to_string(name);
            if simple_name == "Box" && args.len() == 1 {
                let inner = compile_expr(&args[0], target_obj, false);
                return quote! { Box::new(#inner) };
            }
            if simple_name.ends_with("::Ok")
                || simple_name.ends_with("::Err")
                || simple_name == "Ok"
                || simple_name == "Err"
            {
                let id = if let Some(resolved) = resolved_name {
                    compile_id_expr(resolved)
                } else {
                    compile_path(name, target_obj)
                };
                let args = args.iter().map(|a| {
                    let e = compile_expr(a, target_obj, false);
                    quote!((&#e).as_val())
                });
                return quote! { #id(#( #args ),*) };
            }
            if simple_name == "array_init" {
                let element = compile_expr(&args[0], target_obj, false);
                let size = compile_expr(&args[1], target_obj, false);
                // The element's type in Solar is T, so we want *mut T in Rust
                let ty = compile_type_ext(args[0].ty.as_ref().unwrap_or(&Type::I32), target_obj);
                return quote! { unsafe {
                    let mut v: Vec<#ty> = (0..crate::SolarAsSize::as_size(&#size)).map(|_| (&#element).as_val()).collect();
                    let p = v.as_mut_ptr();
                    std::mem::forget(v);
                    p
                } };
            }
            let id = if let Some(resolved) = resolved_name {
                let mut base = compile_id_expr(resolved);
                if resolved.ends_with("<>") {
                    if let Some(last) = name.last() {
                        if !last.generics.is_empty() {
                            let gens = last
                                .generics
                                .iter()
                                .map(|g| compile_type_ext(g, target_obj));
                            base = quote!(#base :: <#( #gens ),*>);
                        }
                    }
                }
                base
            } else {
                compile_path(name, target_obj)
            };
            let args = args.iter().enumerate().map(|(i, a)| {
                let kind = arg_kinds
                    .as_ref()
                    .and_then(|ks| ks.get(i))
                    .cloned()
                    .unwrap_or(ArgKind::Value);
                if is_phantom && matches!(kind, ArgKind::Value) {
                    let e = compile_expr(a, target_obj, false);
                    return quote!((&#e).as_val());
                }
                match kind {
                    ArgKind::Value => wrap_expr_for_ref(a, target_obj, false),
                    ArgKind::Ref => wrap_expr_for_ref(a, target_obj, false),
                    ArgKind::MutRef => wrap_expr_for_ref(a, target_obj, true),
                }
            });
            quote! { #id(#( #args ),*) }
        }
        ExprKind::MethodCall(lhs, name, args, resolved_obj_name, arg_kinds) => {
            let is_phantom = matches!(expr.ty, Some(Type::Any));
            let l = compile_expr(lhs, target_obj, is_mut);

            // Special handling for pointer arithmetic methods which expect usize
            if name == "add" || name == "offset" || name == "sub" {
                if let Some(ty) = &lhs.ty {
                    if matches!(ty, Type::RawPtr(_, _) | Type::BoxPtr(_)) {
                        let args = args.iter().map(|a| {
                            let e = compile_expr(a, target_obj, false);
                            quote!(crate::SolarAsSize::as_size(&#e))
                        });
                        let name_id = quote::format_ident!("{}", name);
                        return quote! { (#l).#name_id(#( #args ),*) };
                    }
                }
            }

            let mut name_to_use = name.clone();
            if !is_phantom {
                if let Some(obj) = resolved_obj_name {
                    if ["string", "str", "vector", "i32", "i64", "f32", "f64"]
                        .contains(&obj.as_str())
                    {
                        name_to_use = format!("solar_{}", name);
                    }
                }
            }

            let id = quote::format_ident!("{}", name_to_use);
            let args = args.iter().enumerate().map(|(i, a)| {
                let kind = arg_kinds
                    .as_ref()
                    .and_then(|ks| ks.get(i))
                    .cloned()
                    .unwrap_or(ArgKind::Value);
                if is_phantom && matches!(kind, ArgKind::Value) {
                    let e = compile_expr(a, target_obj, false);
                    return quote!((&#e).as_val());
                }
                match kind {
                    ArgKind::Value => wrap_expr_for_ref(a, target_obj, false),
                    ArgKind::Ref => wrap_expr_for_ref(a, target_obj, false),
                    ArgKind::MutRef => wrap_expr_for_ref(a, target_obj, true),
                }
            });
            quote! { (#l).#id(#( #args ),*) }
        }
        ExprKind::MemberAccess(lhs, name) => {
            let l = compile_expr(lhs, target_obj, is_mut);
            let id = quote::format_ident!("{}", name);

            if let Some(Type::Managed(_)) = &lhs.ty {
                if is_mut {
                    quote! { (#l.borrow_mut()).#id }
                } else {
                    quote! { (#l.borrow()).#id }
                }
            } else if let Some(Type::ThreadSafe(_)) = &lhs.ty {
                if is_mut {
                    quote! { (#l.write()).#id }
                } else {
                    quote! { (#l.read()).#id }
                }
            } else {
                quote! { #l.#id }
            }
        }
        ExprKind::IndexAccess(lhs, index) => {
            let l = compile_expr(lhs, target_obj, is_mut);
            let i = compile_expr(index, target_obj, false);
            quote! { (#l).solar_index((&#i).as_val()) }
        }
        ExprKind::Cast(inner, ty) => {
            let e = compile_expr(inner, target_obj, false);
            let t = compile_type_ext(ty, target_obj);
            quote! { ((&#e).as_val() as #t) }
        }
        ExprKind::StructLiteral {
            path,
            fields,
            resolved_name,
        } => {
            let simple_name = path_to_string(path);
            let (target_name, target_path) = if simple_name == "self" || simple_name == "Self" {
                if let Some(t) = target_obj {
                    (t.clone(), None) // The target_obj name already has generics resolved
                } else {
                    ("Self".to_string(), None)
                }
            } else {
                (
                    resolved_name.as_ref().unwrap_or(&simple_name).clone(),
                    Some(path),
                )
            };

            let final_id = if let Some(p) = target_path {
                compile_path(p, target_obj)
            } else {
                compile_id_expr(&target_name)
            };

            let fields = fields.iter().map(|(n, v)| {
                let fname = quote::format_ident!("{}", n);
                let fval = compile_expr(v, target_obj, false);
                match &v.kind {
                    ExprKind::Call(_, _, _, _)
                    | ExprKind::MethodCall(_, _, _, _, _)
                    | ExprKind::Alloc(_, _)
                    | ExprKind::StructLiteral { .. }
                    | ExprKind::Int(_)
                    | ExprKind::Int64(_)
                    | ExprKind::Float(_)
                    | ExprKind::Bool(_)
                    | ExprKind::String(_)
                    | ExprKind::Unit
                    | ExprKind::Try(_)
                    | ExprKind::Unwrap(_)
                    | ExprKind::Await(_) => {
                        quote! { #fname: #fval }
                    }
                    _ => quote! { #fname: (&#fval).as_val() },
                }
            });
            quote! { #final_id { #( #fields ),* } }
        }
        ExprKind::Alloc(inner, kind) => {
            let e = compile_expr(inner, target_obj, false);
            match kind {
                AllocKind::Box => quote! { Box::new(#e) },
                AllocKind::RawMut => quote! { #e },
                AllocKind::RawConst => quote! { #e },
                AllocKind::Rc => quote! { ::std::rc::Rc::new(::std::cell::RefCell::new(#e)) },
                AllocKind::Arc => quote! { ::std::sync::Arc::new(::parking_lot::RwLock::new(#e)) },
            }
        }
        ExprKind::Borrow(inner, mutable) => {
            let e = compile_expr(inner, target_obj, *mutable);
            if *mutable {
                quote! { &mut #e }
            } else {
                quote! { &#e }
            }
        }
        ExprKind::Negate(inner) => {
            let e = compile_expr(inner, target_obj, false);
            quote! { (-#e) }
        }
        ExprKind::Deref(inner) => {
            let e = compile_expr(inner, target_obj, is_mut);
            quote! { (*#e) }
        }
        ExprKind::Downgrade(inner) => {
            let e = compile_expr(inner, target_obj, false);
            if let Some(Type::Managed(_)) = &inner.ty {
                quote! { ::std::rc::Rc::downgrade(&#e) }
            } else {
                quote! { ::std::sync::Arc::downgrade(&#e) }
            }
        }
        ExprKind::Unwrap(inner) => {
            let e = compile_expr(inner, target_obj, false);
            if let Some(Type::WeakManaged(_)) | Some(Type::WeakThreadSafe(_)) = &inner.ty {
                quote! { #e.upgrade() }
            } else {
                quote! { #e? }
            }
        }
        ExprKind::Await(inner) => {
            let e = compile_expr(inner, target_obj, false);
            quote! { #e.await }
        }
        ExprKind::Try(inner) => {
            let e = compile_expr(inner, target_obj, false);
            quote! { #e? }
        }
    }
}
