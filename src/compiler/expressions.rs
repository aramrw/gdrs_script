use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    ast::{ArgKind, BinaryOp, Expr, ExprKind, Type},
    compiler::{
        compile_id, compile_id_expr, compile_id_ext, compile_path, has_clone, path_to_string,
        types::compile_type_ext, wrap_expr_for_ref,
    },
    sema::TypeInfo,
};

pub fn compile_expr(
    expr: &Expr,
    target_obj: Option<&String>,
    is_mut: bool,
    type_info: &TypeInfo,
) -> TokenStream {
    match &expr.kind {
        ExprKind::Unit => quote! { () },
        ExprKind::Int(v) => quote! { #v },
        ExprKind::Int64(v) => quote! { #v },
        ExprKind::Float(v) => quote! { #v },
        ExprKind::Float64(v) => quote! { #v },
        ExprKind::Bool(v) => quote! { #v },
        ExprKind::String(v) => quote! { #v },
        ExprKind::Variable(path) => {
            if path.len() > 1 {
                return compile_path(path, target_obj, true);
            }
            compile_id_expr(&path[0].name)
        }
        ExprKind::Binary(lhs, op, rhs) => {
            let l = compile_expr(lhs, target_obj, false, type_info);
            let r = compile_expr(rhs, target_obj, false, type_info);

            let l_ty = lhs.ty.as_ref();
            let r_ty = rhs.ty.as_ref();

            // Special handling for string concatenation
            if *op == BinaryOp::Add {
                let l_is_str = l_ty.map_or(false, |t| matches!(t, Type::Str));
                let r_is_str = r_ty.map_or(false, |t| matches!(t, Type::Str));

                if l_is_str || r_is_str {
                    return quote! { format!("{}{}", #l, #r) };
                }
            }

            match op {
                BinaryOp::Add => {
                    let l_ct = compile_type_ext(l_ty.unwrap_or(&Type::Any), target_obj);
                    let r_ct = compile_type_ext(r_ty.unwrap_or(&Type::Any), target_obj);
                    quote! { (<_ as crate::SolarAsVal<#l_ct>>::as_val(&#l) + <_ as crate::SolarAsVal<#r_ct>>::as_val(&#r)) }
                }
                BinaryOp::Subtract => {
                    let l_ct = compile_type_ext(l_ty.unwrap_or(&Type::Any), target_obj);
                    let r_ct = compile_type_ext(r_ty.unwrap_or(&Type::Any), target_obj);
                    quote! { (<_ as crate::SolarAsVal<#l_ct>>::as_val(&#l) - <_ as crate::SolarAsVal<#r_ct>>::as_val(&#r)) }
                }
                BinaryOp::Multiply => {
                    let l_ct = compile_type_ext(l_ty.unwrap_or(&Type::Any), target_obj);
                    let r_ct = compile_type_ext(r_ty.unwrap_or(&Type::Any), target_obj);
                    quote! { (<_ as crate::SolarAsVal<#l_ct>>::as_val(&#l) * <_ as crate::SolarAsVal<#r_ct>>::as_val(&#r)) }
                }
                BinaryOp::Divide => {
                    let l_ct = compile_type_ext(l_ty.unwrap_or(&Type::Any), target_obj);
                    let r_ct = compile_type_ext(r_ty.unwrap_or(&Type::Any), target_obj);
                    quote! { (<_ as crate::SolarAsVal<#l_ct>>::as_val(&#l) / <_ as crate::SolarAsVal<#r_ct>>::as_val(&#r)) }
                }
                BinaryOp::GreaterThan => {
                    let l_ct = compile_type_ext(l_ty.unwrap_or(&Type::Any), target_obj);
                    let r_ct = compile_type_ext(r_ty.unwrap_or(&Type::Any), target_obj);
                    quote! { (<_ as crate::SolarAsVal<#l_ct>>::as_val(&#l) > <_ as crate::SolarAsVal<#r_ct>>::as_val(&#r)) }
                }
                BinaryOp::LessThan => {
                    let l_ct = compile_type_ext(l_ty.unwrap_or(&Type::Any), target_obj);
                    let r_ct = compile_type_ext(r_ty.unwrap_or(&Type::Any), target_obj);
                    quote! { (<_ as crate::SolarAsVal<#l_ct>>::as_val(&#l) < <_ as crate::SolarAsVal<#r_ct>>::as_val(&#r)) }
                }
                BinaryOp::GreaterThanOrEqual => {
                    let l_ct = compile_type_ext(l_ty.unwrap_or(&Type::Any), target_obj);
                    let r_ct = compile_type_ext(r_ty.unwrap_or(&Type::Any), target_obj);
                    quote! { (<_ as crate::SolarAsVal<#l_ct>>::as_val(&#l) >= <_ as crate::SolarAsVal<#r_ct>>::as_val(&#r)) }
                }
                BinaryOp::LessThanOrEqual => {
                    let l_ct = compile_type_ext(l_ty.unwrap_or(&Type::Any), target_obj);
                    let r_ct = compile_type_ext(r_ty.unwrap_or(&Type::Any), target_obj);
                    quote! { (<_ as crate::SolarAsVal<#l_ct>>::as_val(&#l) <= <_ as crate::SolarAsVal<#r_ct>>::as_val(&#r)) }
                }
                BinaryOp::Equal => {
                    let l_ct = compile_type_ext(l_ty.unwrap_or(&Type::Any), target_obj);
                    let r_ct = compile_type_ext(r_ty.unwrap_or(&Type::Any), target_obj);
                    quote! { (<_ as crate::SolarAsVal<#l_ct>>::as_val(&#l) == <_ as crate::SolarAsVal<#r_ct>>::as_val(&#r)) }
                }
                BinaryOp::Modulo => {
                    let l_ct = compile_type_ext(l_ty.unwrap_or(&Type::Any), target_obj);
                    let r_ct = compile_type_ext(r_ty.unwrap_or(&Type::Any), target_obj);
                    quote! { (<_ as crate::SolarAsVal<#l_ct>>::as_val(&#l) % <_ as crate::SolarAsVal<#r_ct>>::as_val(&#r)) }
                }
                BinaryOp::AddAssign => {
                    let r_ct = compile_type_ext(r_ty.unwrap_or(&Type::Any), target_obj);
                    quote! { (#l += <_ as crate::SolarAsVal<#r_ct>>::as_val(&#r)) }
                }
                BinaryOp::SubAssign => {
                    let r_ct = compile_type_ext(r_ty.unwrap_or(&Type::Any), target_obj);
                    quote! { (#l -= <_ as crate::SolarAsVal<#r_ct>>::as_val(&#r)) }
                }
                BinaryOp::MulAssign => {
                    let r_ct = compile_type_ext(r_ty.unwrap_or(&Type::Any), target_obj);
                    quote! { (#l *= <_ as crate::SolarAsVal<#r_ct>>::as_val(&#r)) }
                }
                BinaryOp::DivAssign => {
                    let r_ct = compile_type_ext(r_ty.unwrap_or(&Type::Any), target_obj);
                    quote! { (#l /= <_ as crate::SolarAsVal<#r_ct>>::as_val(&#r)) }
                }
                BinaryOp::Or => {
                    let r_ct = compile_type_ext(r_ty.unwrap_or(&Type::Any), target_obj);
                    quote! { (#l || <_ as crate::SolarAsVal<#r_ct>>::as_val(&#r)) }
                }
            }
        }
        ExprKind::Tuple(items) => {
            let items = items
                .iter()
                .map(|i| compile_expr(i, target_obj, false, type_info));
            quote! { (#( #items ),*) }
        }
        ExprKind::Array(items) => {
            let items = items.iter().map(|i| {
                let e = compile_expr(i, target_obj, false, type_info);
                if let Some(ty) = &i.ty {
                    if has_clone(ty, type_info) {
                        return quote!((&#e).as_val());
                    }
                }
                quote!(#e)
            });
            quote! { [#(#items),*] }
        }
        ExprKind::Block(stmts) => {
            use crate::compiler::statements::compile_stmt;
            let len = stmts.len();
            let stmts_compiled = stmts.iter().enumerate().map(|(idx, s)| {
                compile_stmt(s, idx == len - 1, target_obj, expr.ty.as_ref(), type_info)
            });
            quote! { { #( #stmts_compiled )* } }
        }
        ExprKind::Call(path, args, resolved_name, arg_kinds, param_types) => {
            let is_phantom = matches!(expr.ty, Some(Type::Any));
            let name = path_to_string(path);

            // Special handling for standard variants/macros
            if name == "Some"
                || name == "None"
                || name == "Ok"
                || name == "Err"
                || name.ends_with("::Some")
                || name.ends_with("::None")
                || name.ends_with("::Ok")
                || name.ends_with("::Err")
            {
                let id = if let Some(resolved) = resolved_name {
                    if let Some(Type::Custom(_, generics)) = &expr.ty {
                        if !generics.is_empty() && !generics.iter().any(|g| matches!(g, Type::Any))
                        {
                            let gens = generics.iter().map(|g| compile_type_ext(g, target_obj));
                            let base_id = compile_id_ext(resolved.trim_end_matches("<>"), true);
                            quote!(#base_id::<#( #gens ),*>)
                        } else {
                            compile_id_expr(resolved)
                        }
                    } else {
                        compile_id_expr(resolved)
                    }
                } else {
                    compile_path(path, target_obj, true)
                };
                let args_compiled = args.iter().map(|a| {
                    let e = compile_expr(a, target_obj, false, type_info);
                    quote!((&#e).as_val())
                });
                return quote! { #id(#( #args_compiled ),*) };
            }

            if name == "array_init" {
                let element = compile_expr(&args[0], target_obj, false, type_info);
                let size = compile_expr(&args[1], target_obj, false, type_info);
                let ty = compile_type_ext(args[0].ty.as_ref().unwrap_or(&Type::I32), target_obj);

                if let ExprKind::Int(v) = &args[1].kind {
                    let _s = *v as usize;
                    if _s == 0 {
                        return quote! { Vec::<#ty>::new() };
                    }
                    return quote! { ::std::array::from_fn::<#ty, #_s, _>(|_| (#element)) };
                } else {
                    return quote! { (0..crate::SolarAsSize::as_size(&#size)).map(|_| (#element)).collect::<Vec<#ty>>() };
                }
            }

            if name == "new" || name.ends_with("::new") {
                if let Some(part) = path.get(path.len().saturating_sub(2)) {
                    if part.name == "Vector" || part.name == "Vec" {
                        let ty_src = if let Some(Type::Custom(_, gens)) = &expr.ty {
                            if !gens.is_empty() && !matches!(gens[0], Type::Any) {
                                compile_type_ext(&gens[0], target_obj)
                            } else {
                                quote!(_)
                            }
                        } else {
                            quote!(_)
                        };
                        return quote! { Vec::<#ty_src>::new() };
                    }
                }
            }

            let id = if let Some(resolved) = resolved_name {
                if let Some(Type::Custom(_, generics)) = &expr.ty {
                    if !generics.is_empty() && !generics.iter().any(|g| matches!(g, Type::Any)) {
                        let gens = generics.iter().map(|g| compile_type_ext(g, target_obj));
                        let base_id = compile_id_ext(resolved.trim_end_matches("<>"), true);
                        quote!(#base_id::<#( #gens ),*>)
                    } else {
                        compile_id_expr(resolved)
                    }
                } else {
                    compile_id_expr(resolved)
                }
            } else {
                compile_path(path, target_obj, true)
            };

            let args_compiled = args
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let kind = arg_kinds
                        .as_ref()
                        .and_then(|ks| ks.get(i))
                        .cloned()
                        .unwrap_or(ArgKind::Value);
                    let mutable_expr_for_wrap = matches!(kind, ArgKind::MutRef);
                    if is_phantom && matches!(kind, ArgKind::Value) {
                        return wrap_expr_for_ref(
                            a,
                            Some(&Type::Any),
                            target_obj,
                            mutable_expr_for_wrap,
                            kind,
                            type_info,
                        );
                    }
                    let expected = param_types.as_ref().and_then(|pts| pts.get(i));
                    wrap_expr_for_ref(
                        a,
                        expected,
                        target_obj,
                        mutable_expr_for_wrap,
                        kind,
                        type_info,
                    )
                })
                .collect::<Vec<_>>();
            quote! { #id(#( #args_compiled ),*) }
        }
        ExprKind::MethodCall(lhs, name, args, resolved_obj_name, arg_kinds, param_types) => {
            let is_phantom = matches!(expr.ty, Some(Type::Any));
            let l = compile_expr(lhs, target_obj, is_mut, type_info);

            if name == "add" || name == "offset" || name == "sub" {
                if let Some(ty) = &lhs.ty {
                    if matches!(ty, Type::RawPtr(_, _) | Type::BoxPtr(_)) {
                        let args_c = args
                            .iter()
                            .map(|a| {
                                let e = compile_expr(a, target_obj, false, type_info);
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
                        return quote! { (#l).#name_tokens(#( #args_c ),*) };
                    }
                }
            }

            if name == "into" && !is_phantom {
                return quote! { &((&#l).as_val().solar_into()) };
            }

            let mut name_to_use = name.clone();
            if let Some(obj) = resolved_obj_name {
                let mut name_to_use = name.clone();
                if let Some(obj) = resolved_obj_name {
                    let obj_low = obj.to_lowercase();

                    // Check for exact built-in / standard library types ONLY
                    let is_builtin = matches!(
                        obj_low.as_str(),
                        "string"
                            | "str"
                            | "vector"
                            | "vec"
                            | "i32"
                            | "i64"
                            | "f32"
                            | "f64"
                            | "array"
                            | "std::string::string"
                            | "std::vec::vector"
                            | "std::vec::vec"
                    );

                    if !name.starts_with("solar_") && is_builtin {
                        name_to_use = format!("solar_{}", name);
                    }
                }
            }

            let args_c = args
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let kind = arg_kinds
                        .as_ref()
                        .and_then(|ks| ks.get(i))
                        .cloned()
                        .unwrap_or(ArgKind::Value);
                    let mutable_expr_for_wrap = matches!(kind, ArgKind::MutRef);
                    if is_phantom && matches!(kind, ArgKind::Value) {
                        return wrap_expr_for_ref(
                            a,
                            Some(&Type::Any),
                            target_obj,
                            mutable_expr_for_wrap,
                            kind,
                            type_info,
                        );
                    }
                    let expected = param_types.as_ref().and_then(|pts| pts.get(i));
                    wrap_expr_for_ref(
                        a,
                        expected,
                        target_obj,
                        mutable_expr_for_wrap,
                        kind,
                        type_info,
                    )
                })
                .collect::<Vec<_>>();

            let id_tokens = if let Ok(idx) = name_to_use.parse::<usize>() {
                let lit = proc_macro2::Literal::usize_unsuffixed(idx);
                quote!(#lit)
            } else {
                let id = quote::format_ident!("{}", name_to_use);
                quote!(#id)
            };

            if !is_phantom && name_to_use.starts_with("solar_") {
                if let Some(obj) = resolved_obj_name {
                    if ["f32", "f64", "i32", "i64"].contains(&obj.to_lowercase().as_str()) {
                        return quote! { (&#l).as_val().#id_tokens(#( #args_c ),*) };
                    }
                }
            }

            quote! { (#l).#id_tokens(#( #args_c ),*) }
        }
        ExprKind::MemberAccess(lhs, name) => {
            let l = compile_expr(lhs, target_obj, is_mut, type_info);

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
            let l = compile_expr(lhs, target_obj, is_mut, type_info);
            let i = compile_expr(index, target_obj, false, type_info);
            quote! { (#l)[crate::SolarAsSize::as_size(&(#i))] }
        }
        ExprKind::Cast(inner, ty) => {
            let e = compile_expr(inner, target_obj, false, type_info);
            let t = compile_type_ext(ty, target_obj);
            if has_clone(ty, type_info) {
                quote! { ((&#e).as_val() as #t) }
            } else {
                quote! { (#e as #t) }
            }
        }
        ExprKind::StructLiteral {
            path,
            fields,
            resolved_name,
        } => {
            let simple_name = path_to_string(path);
            let (target_name, target_path) = if simple_name == "self" || simple_name == "Self" {
                if let Some(t) = target_obj {
                    (t.clone(), None)
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
                let v = compile_expr(fval, target_obj, false, type_info);
                let has_v_clone = if let Some(ty) = &fval.ty {
                    has_clone(ty, type_info)
                } else {
                    false
                };
                match &fval.ty {
                    Some(Type::Ref(_, _))
                    | Some(Type::RawPtr(_, _))
                    | Some(Type::Managed(_))
                    | Some(Type::ThreadSafe(_))
                    | Some(Type::BoxPtr(_))
                    | Some(Type::Array(_, _)) => quote! { #id: #v },
                    _ if has_v_clone => quote! { #id: (&#v).as_val() },
                    _ => quote! { #id: #v },
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
            if name == "typeof" {
                let e = compile_expr(&args[0], target_obj, false, type_info);
                return quote! { crate::SolarCow::Borrowed(crate::solar_typeof(&#e)) };
            }

            if name == "println" || name == "log" || name == "logln" || name == "dbg" {
                if args.is_empty() {
                    quote! { #name_id!() }
                } else if name == "dbg" {
                    let all_args = args.iter().map(|a| {
                        let e = compile_expr(a, target_obj, false, type_info);
                        quote!(&#e)
                    });
                    return quote! { { dbg!(#( #all_args ),*); } };
                } else {
                    let first_arg = &args[0];
                    if let ExprKind::String(ref s) = first_arg.kind {
                        let placeholder_count = s.matches("{}").count();
                        let rest_args_count = args.len() - 1;

                        if placeholder_count == 0 && rest_args_count > 0 {
                            let mut format_str = s.clone();
                            for _ in 0..rest_args_count {
                                format_str.push_str(" {:?}");
                            }
                            let all_rest_args = args[1..].iter().map(|a| {
                                let e = compile_expr(a, target_obj, false, type_info);
                                quote!(&#e)
                            });
                            quote! { #name_id!(#format_str, #( #all_rest_args ),*) }
                        } else {
                            // If placeholders exist, replace them with {:?} to avoid Display errors
                            let format_str = s.replace("{}", "{:?}");
                            let rest_args = args.iter().skip(1).map(|a| {
                                let e = compile_expr(a, target_obj, false, type_info);
                                quote!(&#e)
                            });
                            quote! { #name_id!(#format_str, #( #rest_args ),*) }
                        }
                    } else {
                        let format_str = vec!["{:?}"; args.len()].join(" ");
                        let all_args = args.iter().map(|a| {
                            let e = compile_expr(a, target_obj, false, type_info);
                            quote!(&#e)
                        });
                        quote! { #name_id!(#format_str, #( #all_args ),*) }
                    }
                }
            } else if name == "str" {
                if args.is_empty() {
                    quote! { ::std::string::String::new() }
                } else {
                    let e = compile_expr(&args[0], target_obj, false, type_info);
                    quote! { (&#e).as_val() }
                }
            } else {
                let args_compiled = args
                    .iter()
                    .map(|a| {
                        let e = compile_expr(a, target_obj, false, type_info);
                        quote!(&#e)
                    })
                    .collect::<Vec<_>>();
                quote! { #name_id!(#( #args_compiled ),*) }
            }
        }
        ExprKind::Borrow(inner, mutable) => {
            let e = compile_expr(inner, target_obj, *mutable, type_info);
            if *mutable {
                quote!(&mut #e)
            } else {
                quote!(&#e)
            }
        }
        ExprKind::Deref(inner) => {
            let e = compile_expr(inner, target_obj, false, type_info);
            quote!(*#e)
        }
        ExprKind::Alloc(inner, kind) => {
            let e = compile_expr(inner, target_obj, false, type_info);
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
            let e = compile_expr(inner, target_obj, false, type_info);
            if let Some(Type::ThreadSafe(_)) = &inner.ty {
                quote!(::std::sync::Arc::downgrade(&#e))
            } else {
                quote!(&::std::rc::Rc::downgrade(&#e))
            }
        }
        ExprKind::Negate(inner) => {
            let e = compile_expr(inner, target_obj, false, type_info);
            quote!(-#e)
        }
        ExprKind::Unwrap(inner) => {
            let e = compile_expr(inner, target_obj, false, type_info);
            quote!(#e.unwrap())
        }
        ExprKind::Await(inner) => {
            let e = compile_expr(inner, target_obj, false, type_info);
            quote! { #e.await }
        }
        ExprKind::Try(inner) => {
            let e = compile_expr(inner, target_obj, false, type_info);
            quote! { #e? }
        }
        _ => quote!(()),
    }
}
