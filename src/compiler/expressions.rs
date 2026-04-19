use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    ast::{ArgKind, BinaryOp, Expr, ExprKind, Type},
    compiler::{
        compile_id_expr, compile_path, path_to_string, types::compile_type_ext, wrap_expr_for_ref,
    },
};

pub fn compile_expr(expr: &Expr, target_obj: Option<&String>, is_mut: bool) -> TokenStream {
    match &expr.kind {
        ExprKind::Unit => quote! { () },
        ExprKind::Int(v) => quote! { #v },
        ExprKind::Int64(v) => quote! { #v },
        ExprKind::Float(v) => quote! { #v },
        ExprKind::Float64(v) => quote! { #v },
        ExprKind::Bool(v) => quote! { #v },
        ExprKind::String(v) => quote! { #v },
        ExprKind::Variable(path) => {
            let name = path_to_string(path);
            let id = compile_id_expr(&name);
            id
        }
        ExprKind::Binary(lhs, op, rhs) => {
            let l = compile_expr(lhs, target_obj, false);
            let r = compile_expr(rhs, target_obj, false);

            let l_ty = lhs.ty.as_ref();
            let r_ty = rhs.ty.as_ref();

            // Special handling for string concatenation
            if *op == BinaryOp::Add {
                let l_is_str = l_ty.map_or(false, |t| matches!(t, Type::String | Type::Str));
                let r_is_str = r_ty.map_or(false, |t| matches!(t, Type::String | Type::Str));

                if l_is_str || r_is_str {
                    return quote! { (#l).solar_add(&#r) };
                }
            }

            // this seems a little ridiculous
            match op {
                BinaryOp::Add => quote! { ((&#l).as_val() + (&#r).as_val()) },
                BinaryOp::Subtract => quote! { ((&#l).as_val() - (&#r).as_val()) },
                BinaryOp::Multiply => quote! { ((&#l).as_val() * (&#r).as_val()) },
                BinaryOp::Divide => quote! { ((&#l).as_val() / (&#r).as_val()) },
                BinaryOp::GreaterThan => quote! { ((&#l).as_val() > (&#r).as_val()) },
                BinaryOp::LessThan => quote! { ((&#l).as_val() < (&#r).as_val()) },
                BinaryOp::GreaterThanOrEqual => quote! { ((&#l).as_val() >= (&#r).as_val()) },
                BinaryOp::LessThanOrEqual => quote! { ((&#l).as_val() <= (&#r).as_val()) },
                BinaryOp::Equal => quote! { ((&#l).as_val() == (&#r).as_val()) },
                BinaryOp::Modulo => quote! { ((&#l).as_val() % (&#r).as_val()) },
            }
        }
        ExprKind::Tuple(items) => {
            let items = items.iter().map(|i| compile_expr(i, target_obj, false));
            quote! { (#( #items ),*) }
        }
        ExprKind::Array(items) => {
            let items = items.iter().map(|i| {
                let e = compile_expr(i, target_obj, false);
                quote!((&#e).as_val())
            });
            quote! { [#(#items),*] }
        }
        ExprKind::Block(stmts) => {
            use crate::compiler::statements::compile_stmt;
            let len = stmts.len();
            let stmts_compiled = stmts
                .iter()
                .enumerate()
                .map(|(idx, s)| compile_stmt(s, idx == len - 1, target_obj, expr.ty.as_ref()));
            quote! { { #( #stmts_compiled )* } }
        }
        ExprKind::Call(path, args, resolved_name, arg_kinds, param_types) => {
            let is_phantom = matches!(expr.ty, Some(Type::Any));
            let simple_name = path_to_string(path);
            if simple_name.ends_with("::Ok")
                || simple_name.ends_with("::Err")
                || simple_name == "Ok"
                || simple_name == "Err"
            {
                let id = if let Some(resolved) = resolved_name {
                    compile_id_expr(resolved)
                } else {
                    compile_path(path, target_obj)
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
                    let mut v: Vec<#ty> = (0..crate::SolarAsSize::as_size(&#size)).map(|_| (#element)).collect();
                    let p = v.as_mut_ptr();
                    std::mem::forget(v);
                    p
                    } };
            }
            let id = if let Some(resolved) = resolved_name {
                let mut base = compile_id_expr(resolved);
                if resolved.ends_with("<>") {
                    if let Some(last) = path.last() {
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
                compile_path(path, target_obj)
            };
            let args = args
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let kind = arg_kinds
                        .as_ref()
                        .and_then(|ks| ks.get(i))
                        .cloned()
                        .unwrap_or(ArgKind::Value);
                    if is_phantom && matches!(kind, ArgKind::Value) {
                        let e = compile_expr(a, target_obj, false);
                        return quote!((&#e).as_val());
                    }
                    let expected = param_types.as_ref().and_then(|pts| pts.get(i));
                    match kind {
                        ArgKind::Value => wrap_expr_for_ref(a, expected, target_obj, false),
                        ArgKind::Ref => wrap_expr_for_ref(a, expected, target_obj, false),
                        ArgKind::MutRef => wrap_expr_for_ref(a, expected, target_obj, true),
                    }
                })
                .collect::<Vec<_>>();
            quote! { #id(#( #args ),*) }
        }
        ExprKind::MethodCall(lhs, name, args, resolved_obj_name, arg_kinds, param_types) => {
            let is_phantom = matches!(expr.ty, Some(Type::Any));
            println!(
                "DEBUG compile MethodCall name={}, is_phantom={}, ty={:?}",
                name, is_phantom, expr.ty
            );
            let l = compile_expr(lhs, target_obj, is_mut);

            // Special handling for pointer arithmetic methods which expect usize
            if name == "add" || name == "offset" || name == "sub" {
                if let Some(ty) = &lhs.ty {
                    if matches!(ty, Type::RawPtr(_, _) | Type::BoxPtr(_)) {
                        let args = args
                            .iter()
                            .map(|a| {
                                let e = compile_expr(a, target_obj, false);
                                quote!(crate::SolarAsSize::as_size(&#e))
                            })
                            .collect::<Vec<_>>();
                        let name_tokens = if let Ok(idx) = name.parse::<usize>() {
                            let lit = proc_macro2::Literal::usize_unsuffixed(idx);
                            quote!(#lit)
                        } else {
                            let id = quote::format_ident!("{}", name);
                            quote!(#id)
                        };
                        return quote! { (#l).#name_tokens(#( #args ),*) };
                    }
                }
            }

            if name == "into" && !is_phantom {
                return quote! { &((&#l).as_val().solar_into()) };
            }

            let mut name_to_use = name.clone();
            if !is_phantom {
                if let Some(obj) = resolved_obj_name {
                    let obj_low = obj.to_lowercase();
                    if ["string", "str", "vector", "i32", "i64", "f32", "f64"]
                        .contains(&obj_low.as_str())
                        || obj_low.contains("string")
                        || obj_low.contains("str")
                    {
                        name_to_use = format!("solar_{}", name);
                    }
                }
            }

            let args = args
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let kind = arg_kinds
                        .as_ref()
                        .and_then(|ks| ks.get(i))
                        .cloned()
                        .unwrap_or(ArgKind::Value);
                    if is_phantom && matches!(kind, ArgKind::Value) {
                        let e = compile_expr(a, target_obj, false);
                        return quote!((&#e).as_val());
                    }
                    let expected = param_types.as_ref().and_then(|pts| pts.get(i));
                    match kind {
                        ArgKind::Value => wrap_expr_for_ref(a, expected, target_obj, false),
                        ArgKind::Ref => wrap_expr_for_ref(a, expected, target_obj, false),
                        ArgKind::MutRef => wrap_expr_for_ref(a, expected, target_obj, true),
                    }
                })
                .collect::<Vec<_>>();

            let mut id_tokens = if let Ok(idx) = name_to_use.parse::<usize>() {
                let lit = proc_macro2::Literal::usize_unsuffixed(idx);
                quote!(#lit)
            } else {
                let id = quote::format_ident!("{}", name_to_use);
                quote!(#id)
            };

            if !is_phantom && name_to_use.starts_with("solar_") {
                if let Some(obj) = resolved_obj_name {
                    if ["f32", "f64", "i32", "i64"].contains(&obj.to_lowercase().as_str()) {
                        return quote! { (&#l).as_val().#id_tokens(#( #args ),*) };
                    }
                }
            }

            quote! { (#l).#id_tokens(#( #args ),*) }
        }
        ExprKind::MemberAccess(lhs, name) => {
            let l = compile_expr(lhs, target_obj, is_mut);

            if let Some(lhs_ty) = &lhs.ty {
                if matches!(lhs_ty, Type::Tuple(_)) {
                    if let Ok(idx) = name.parse::<usize>() {
                        let idx = syn::Index::from(idx);
                        return quote! { (#l).#idx };
                    }
                }
            }

            let id_tokens = if let Ok(idx) = name.parse::<usize>() {
                let lit = proc_macro2::Literal::usize_unsuffixed(idx);
                quote!(#lit)
            } else {
                let id = quote::format_ident!("{}", name);
                quote!(#id)
            };

            if let Some(ty) = &lhs.ty {
                match ty {
                    Type::Managed(_) => {
                        if is_mut {
                            quote! { (#l.borrow_mut()).#id_tokens }
                        } else {
                            quote! { (#l.borrow()).#id_tokens }
                        }
                    }
                    Type::ThreadSafe(_) => {
                        if is_mut {
                            quote! { (#l.write()).#id_tokens }
                        } else {
                            quote! { (#l.read()).#id_tokens }
                        }
                    }
                    _ => quote! { #l.#id_tokens },
                }
            } else {
                quote! { #l.#id_tokens }
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

            let fields_tokens = fields.iter().map(|(fname, fval)| {
                let id = quote::format_ident!("{}", fname);
                let v = compile_expr(fval, target_obj, false);
                // Primitives and some other types should be passed by value in struct literals
                // unless the field type is explicitly a reference.
                // We'll use as_val() to handle this.
                match &fval.ty {
                    Some(Type::Ref(_, _)) => quote! { #id: #v },
                    Some(Type::RawPtr(_, _)) => quote! { #id: #v },
                    Some(Type::Managed(_)) => quote! { #id: #v },
                    Some(Type::ThreadSafe(_)) => quote! { #id: #v },
                    Some(Type::BoxPtr(_)) => quote! { #id: #v },
                    Some(Type::Array(ty, size)) => quote! { #id: unsafe { std::slice::from_raw_parts(#v, #size as usize).try_into().unwrap() } },
                    _ => quote! { #id: (&#v).as_val() },
                }
            });

            let id = compile_id_expr(&target_name);
            if let Some(p) = target_path {
                if let Some(last) = p.last() {
                    if !last.generics.is_empty() {
                        let gens = last
                            .generics
                            .iter()
                            .map(|g| compile_type_ext(g, target_obj));
                        return quote! { #id :: <#( #gens ),*> { #( #fields_tokens ),* } };
                    }
                }
            }
            quote! { #id { #( #fields_tokens ),* } }
        }
        ExprKind::MacroCall(name, args) => {
            let name_id = quote::format_ident!("{}", name);
            let args_compiled = args
                .iter()
                .map(|a| {
                    let e = compile_expr(a, target_obj, false);
                    quote!(&#e)
                })
                .collect::<Vec<_>>();

            if name == "typeof" {
                let ty = args[0]
                    .ty
                    .as_ref()
                    .map(|t| format!("{:?}", t))
                    .unwrap_or("unknown".into());
                return quote! { crate::SolarCow::Borrowed(#ty) };
            }

            if name == "println"
                || name == "print"
                || name == "log"
                || name == "logln"
                || name == "dbg"
            {
                if args.is_empty() {
                    quote! { #name_id!() }
                } else if name == "dbg" {
                    let all_args = args.iter().map(|a| {
                        let e = compile_expr(a, target_obj, false);
                        if matches!(a.kind, ExprKind::String(_)) {
                            // Ambiguity fix for string literals
                            quote! { (#e).as_str() }
                        } else {
                            quote!((&#e).as_val())
                        }
                    });
                    // Wrap in block and return unit to avoid type mismatch when used as statement
                    return quote! { { dbg!(#( #all_args ),*); } };
                } else {
                    let first_arg = &args[0];
                    if let ExprKind::String(ref s) = first_arg.kind {
                        // If first arg is string literal, check if it has placeholders
                        let placeholder_count = s.matches("{}").count();
                        let rest_args_count = args.len() - 1;

                        if placeholder_count == 0 && rest_args_count > 0 {
                            // If no placeholders but other args exist, append them with spaces
                            let mut format_str = s.clone();
                            for _ in 0..rest_args_count {
                                format_str.push_str(" {}");
                            }
                            let all_rest_args = args[1..].iter().map(|a| {
                                let e = compile_expr(a, target_obj, false);
                                if matches!(a.kind, ExprKind::String(_)) {
                                    quote! { (#e).as_str() }
                                } else {
                                    quote!((&#e).as_val())
                                }
                            });
                            quote! { #name_id!(#format_str, #( #all_rest_args ),*) }
                        } else {
                            // Standard format string or single string
                            let format_str = compile_expr(first_arg, target_obj, false);
                            let rest_args = args[1..].iter().map(|a| {
                                let e = compile_expr(a, target_obj, false);
                                if matches!(a.kind, ExprKind::String(_)) {
                                    quote! { (#e).as_str() }
                                } else {
                                    quote!((&#e).as_val())
                                }
                            });
                            quote! { #name_id!(#format_str, #( #rest_args ),*) }
                        }
                    } else {
                        // No string literal as first arg, generate "{}" for all
                        let format_str = vec!["{}"; args.len()].join(" ");
                        let all_args = args.iter().map(|a| {
                            let e = compile_expr(a, target_obj, false);
                            if matches!(a.kind, ExprKind::String(_)) {
                                quote! { (#e).as_str() }
                            } else {
                                quote!((&#e).as_val())
                            }
                        });
                        quote! { #name_id!(#format_str, #( #all_args ),*) }
                    }
                }
            } else if name == "str" {
                if args.is_empty() {
                    quote! { ::std::string::String::new() }
                } else {
                    let e = compile_expr(&args[0], target_obj, false);
                    quote! { (&#e).as_val() }
                }
            } else {
                quote! { #name_id!(#( #args_compiled ),*) }
            }
        }
        ExprKind::Borrow(inner, mutable) => {
            let e = compile_expr(inner, target_obj, *mutable);
            if *mutable {
                quote!(&mut #e)
            } else {
                quote!(&#e)
            }
        }
        ExprKind::Deref(inner) => {
            let e = compile_expr(inner, target_obj, false);
            quote!(*#e)
        }
        ExprKind::Alloc(inner, kind) => {
            let e = compile_expr(inner, target_obj, false);
            match kind {
                crate::ast::AllocKind::Box => quote!(Box::new((#e).clone())),
                crate::ast::AllocKind::Rc => {
                    quote!(::std::rc::Rc::new(::std::cell::RefCell::new((#e).clone())))
                }
                crate::ast::AllocKind::Arc => {
                    quote!(::std::sync::Arc::new(::parking_lot::RwLock::new((#e).clone())))
                }
                crate::ast::AllocKind::RawMut | crate::ast::AllocKind::RawConst => quote!((#e)),
            }
        }
        ExprKind::Downgrade(inner) => {
            let e = compile_expr(inner, target_obj, false);
            if let Some(Type::ThreadSafe(_)) = &inner.ty {
                quote!(::std::sync::Arc::downgrade(&#e))
            } else {
                quote!(&::std::rc::Rc::downgrade(&#e))
            }
        }
        ExprKind::Negate(inner) => {
            let e = compile_expr(inner, target_obj, false);
            quote!(-#e)
        }
        ExprKind::Unwrap(inner) => {
            let e = compile_expr(inner, target_obj, false);
            quote!(#e.unwrap())
        }
        ExprKind::Await(inner) => {
            let e = compile_expr(inner, target_obj, false);
            quote! { #e.await }
        }
        ExprKind::Try(inner) => {
            let e = compile_expr(inner, target_obj, false);
            quote! { #e? }
        }
        _ => quote!(()), // Fallback for other variants like Downgrade
    }
}
