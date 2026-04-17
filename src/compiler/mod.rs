use lazy_static::lazy_static;
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::sync::Mutex;

use crate::ast::*;

lazy_static! {
    pub(crate) static ref RUST_MAPPINGS: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

fn compile_id(name: &str) -> TokenStream {
    compile_id_ext(name, false)
}

fn compile_id_expr(name: &str) -> TokenStream {
    compile_id_ext(name, true)
}

fn compile_id_ext(name: &str, is_expr: bool) -> TokenStream {
    if name.is_empty() { return quote!(); }
    // Check if it's already a complex Rust path (generics, pointers, etc.)
    if name.contains('<') {
        let parts: Vec<_> = name.splitn(2, '<').collect();
        let id = compile_id_ext(parts[0], is_expr);
        let rest = parts[1];
        if let Some(gt_pos) = rest.find('>') {
            let gens_str = &rest[..gt_pos];
            let after_gens = &rest[gt_pos + 1..];
            
            let gens_tokens: Vec<TokenStream> = gens_str.split(',')
                .filter(|s| !s.trim().is_empty())
                .map(|s| {
                    let s = s.trim();
                    // If it's a generic parameter like "T", format it as an ident
                    if s.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        let gid = quote::format_ident!("{}", s);
                        quote!(#gid)
                    } else {
                        // Otherwise try to parse it (it might be a complex type)
                        s.parse::<TokenStream>().unwrap_or_else(|_| {
                            let gid = quote::format_ident!("{}", s);
                            quote!(#gid)
                        })
                    }
                }).collect();
            
            let sep = if is_expr && !gens_tokens.is_empty() { quote!(::) } else { quote!() };
            let mut res = if gens_tokens.is_empty() {
                quote!(#id)
            } else {
                quote!(#id #sep <#( #gens_tokens ),*>)
            };
            
            if !after_gens.is_empty() {
                let suffix = compile_id_ext(after_gens.trim_start_matches("::"), is_expr);
                res = quote!(#res::#suffix);
            }
            return res;
        }
    }
    if name.contains('(') || name.contains('[') || name.contains('*') {
        return name
            .parse()
            .expect("Failed to parse complex rust path as TokenStream");
    }

    // Check for explicit rust mapping
    if let Ok(mappings) = RUST_MAPPINGS.lock() {
        if let Some(rust_path) = mappings.get(name) {
            return rust_path
                .parse()
                .expect("Failed to parse rust mapping as TokenStream");
        }

        // Try matching prefixes for paths like module::Type::Variant
        let parts: Vec<&str> = name.split("::").collect();
        for i in (1..parts.len()).rev() {
            let prefix = parts[..i].join("::");
            if let Some(rust_prefix) = mappings.get(&prefix) {
                if rust_prefix == &prefix {
                    continue;
                }
                let rest = &parts[i..];
                let rest_tokens: Vec<TokenStream> = rest
                    .iter()
                    .map(|s| {
                        let id = quote::format_ident!("{}", s);
                        quote!(#id)
                    })
                    .collect();
                let prefix_tokens: TokenStream = rust_prefix.parse().unwrap();
                return quote!(#prefix_tokens::#( #rest_tokens )::*);
            }
        }
    }

    if name.contains("::") {
        let parts: Vec<&str> = name.split("::").collect();
        let first = parts[0];

        // rust:: is the magic prefix for real Rust std
        if first == "rust" {
            let rest = &parts[1..];
            let tokens: Vec<TokenStream> = rest
                .iter()
                .map(|p| {
                    let id = quote::format_ident!("{}", p);
                    quote!(#id)
                })
                .collect();
            return quote!(::std::#( #tokens )::*);
        }

        // crate:: is for external rust crates in the generated project
        if first == "crate" {
            let rest = &parts[1..];
            let tokens: Vec<TokenStream> = rest
                .iter()
                .map(|p| {
                    let id = quote::format_ident!("{}", p);
                    quote!(#id)
                })
                .collect();
            return quote!(::#( #tokens )::*);
        }

        // std:: is Solar's standard library, which we transpile into our own crate
        if first == "std" {
            let rest = &parts[1..];
            let tokens: Vec<TokenStream> = rest
                .iter()
                .filter(|s| !s.is_empty())
                .map(|p| {
                    let mut s = p.to_string();
                    if s.ends_with("<>") { s = s.replace("<>", ""); }
                    let id = quote::format_ident!("{}", s);
                    quote!(#id)
                })
                .collect();
            return quote!(crate::#( #tokens )::*);
        }

        // For everything else namespaced, check if it's a known root module
        let root_modules = ["mem", "sr_fs", "sr_io", "sr_math", "util", "vec", "option"];
        if root_modules.contains(&first) {
            let tokens: Vec<TokenStream> = parts
                .iter()
                .filter(|s| !s.is_empty())
                .map(|p| {
                    let mut s = p.to_string();
                    if s.ends_with("<>") { s = s.replace("<>", ""); }
                    let id = quote::format_ident!("{}", s);
                    quote!(#id)
                })
                .collect();
            return quote!(crate::#( #tokens )::*);
        }

        let tokens: Vec<TokenStream> = parts
            .iter()
            .filter(|s| !s.is_empty())
            .map(|p| {
                let mut s = p.to_string();
                if s.ends_with("<>") { s = s.replace("<>", ""); }
                let id = quote::format_ident!("{}", s);
                quote!(#id)
            })
            .collect();
        return quote!(#( #tokens )::*);
    }

    // If it's a known root-level module, also prepend crate::
    let root_modules = ["mem", "sr_fs", "sr_io", "sr_math", "util", "vec", "option"];

    if root_modules.contains(&name) {
        let id = quote::format_ident!("{}", name);
        return quote!(crate::#id);
    }

    let mut final_name = name.to_string();
    if final_name.ends_with("<>") { final_name = final_name.replace("<>", ""); }
    if final_name.is_empty() { return quote!(); }
    let id = quote::format_ident!("{}", final_name);
    quote!(#id)
}

fn path_to_string(path: &[PathPart]) -> String {
    path.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join("::")
}

fn compile_path(path: &[PathPart], target_obj: Option<&String>) -> TokenStream {
    let parts: Vec<_> = path.iter().filter(|p| !p.name.is_empty()).map(|p| {
        let name = quote::format_ident!("{}", p.name);
        if p.generics.is_empty() {
            quote!(#name)
        } else {
            let gens = p.generics.iter().map(|g| compile_type_ext(g, target_obj));
            quote!(#name::<#( #gens ),*>)
        }
    }).collect();
    quote!(#( #parts )::*)
}

fn compile_type_ext(ty: &Type, target_obj: Option<&String>) -> TokenStream {
    match ty {
        Type::Unit => quote!(()),
        Type::I32 => quote!(i32),
        Type::I64 => quote!(i64),
        Type::F32 => quote!(f32),
        Type::F64 => quote!(f64),
        Type::Bool => quote!(bool),
        Type::Str => quote!(&str),
        Type::String => quote!(::std::string::String),
        Type::File => quote!(::std::fs::File),
        Type::BoxPtr(inner) => {
            let t = compile_type_ext(inner, target_obj);
            quote!(Box<#t>)
        }
        Type::Array(inner, size) => {
            let t = compile_type_ext(inner, target_obj);
            if *size == 0 {
                quote!(Vec<#t>)
            } else {
                quote!([#t; #size])
            }
        }
        Type::RawPtr(inner, mutable) => {
            let t = compile_type_ext(inner, target_obj);
            if *mutable {
                quote!(*mut #t)
            } else {
                quote!(*const #t)
            }
        }
        Type::Ref(inner, mutable) => {
            let t = compile_type_ext(inner, target_obj);
            if *mutable {
                quote!(&mut #t)
            } else {
                quote!(&#t)
            }
        }
        Type::Managed(inner) => {
            let t = compile_type_ext(inner, target_obj);
            quote!(::std::rc::Rc<::std::cell::RefCell<#t>>)
        }
        Type::ThreadSafe(inner) => {
            let t = compile_type_ext(inner, target_obj);
            quote!(::std::sync::Arc<::parking_lot::RwLock<#t>>)
        }
        Type::WeakManaged(inner) => {
            let t = compile_type_ext(inner, target_obj);
            quote!(::std::rc::Weak<::std::cell::RefCell<#t>>)
        }
        Type::WeakThreadSafe(inner) => {
            let t = compile_type_ext(inner, target_obj);
            quote!(::std::sync::Weak<::parking_lot::RwLock<#t>>)
        }
        Type::Generic(name) => {
            let id = quote::format_ident!("{}", name);
            quote!(#id)
        }
        Type::Custom(name, generics) => {
            if name == "String" || name == "string" || name == "sr_string" {
                quote!(::std::string::String)
            } else if name == "File" {
                quote!(::std::fs::File)
            } else {
                let id = compile_id(name);
                if generics.is_empty() {
                    quote!(#id)
                } else {
                    let gens = generics.iter().map(|g| compile_type_ext(g, target_obj));
                    quote!(#id<#( #gens ),*>)
                }
            }
        }
        Type::SelfType => {
            if let Some(obj) = target_obj {
                // Parse "Vector<T>" back into tokens properly
                if obj.contains('<') {
                    let parts: Vec<_> = obj.split('<').collect();
                    let id = compile_id(parts[0]);
                    let gens_str = parts[1].trim_end_matches('>');
                    let gens_tokens: Vec<_> = gens_str.split(',').map(|s| {
                        let s = s.trim();
                        let gid = quote::format_ident!("{}", s);
                        quote!(#gid)
                    }).collect();
                    quote!(#id<#( #gens_tokens ),*>)
                } else {
                    let id = compile_id(obj);
                    quote!(#id)
                }
            } else {
                quote!(Self)
            }
        }
        Type::Result(ok, err) => {
            let o = compile_type_ext(ok, target_obj);
            let e = compile_type_ext(err, target_obj);
            quote!(Result<#o, #e>)
        }
        Type::Any => quote!(_),
        Type::Error => quote!(Box<dyn ::std::error::Error>),
    }
}

fn compile_type(ty: &Type) -> TokenStream {
    compile_type_ext(ty, None)
}

fn compile_pattern(pat: &Pattern) -> TokenStream {
    match pat {
        Pattern::Variant(enm, var, params) => {
            let vid = quote::format_ident!("{}", var);
            let pids = params.iter().map(|p| quote::format_ident!("{}", p));
            if enm == "Result" {
                if params.is_empty() {
                    quote! { #vid }
                } else {
                    quote! { #vid(#( #pids ),*) }
                }
            } else {
                let eid = compile_id(enm);
                if params.is_empty() {
                    quote! { #eid::#vid }
                } else {
                    quote! { #eid::#vid(#( #pids ),*) }
                }
            }
        }
        Pattern::Variable(n) => {
            let id = quote::format_ident!("{}", n);
            quote!(#id)
        }
        Pattern::Literal(lit) => compile_expr(lit, None, false),
    }
}

fn wrap_expr_for_ref(expr: &Expr, target_obj: Option<&String>, mutable: bool) -> TokenStream {
    let tokens = compile_expr(expr, target_obj, mutable);
    
    if let Some(ty) = &expr.ty {
        match ty {
            Type::Managed(_) => {
                if mutable { quote! { &mut *#tokens.borrow_mut() } }
                else { quote! { &*#tokens.borrow() } }
            }
            Type::ThreadSafe(_) => {
                if mutable { quote! { &mut *#tokens.write() } }
                else { quote! { &*#tokens.read() } }
            }
            Type::Ref(_, _) => tokens, // Already a ref
            _ => {
                if mutable { quote! { &mut #tokens } }
                else { quote! { &#tokens } }
            }
        }
    } else {
        if mutable { quote! { &mut #tokens } }
        else { quote! { &#tokens } }
    }
}

fn compile_expr(expr: &Expr, target_obj: Option<&String>, is_mut: bool) -> TokenStream {
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
            let args_compiled: Vec<_> = args.iter().map(|a| wrap_expr_for_ref(a, target_obj, false)).collect();
            if name == "println" || name == "print" {
                if let Some(Expr { kind: ExprKind::String(fmt), .. }) = args.get(0) {
                     let rest_compiled = &args_compiled[1..];
                     quote! { #name_id!(#fmt, #( #rest_compiled ),*) }
                } else {
                     let format_str = vec!["{}"; args_compiled.len()].join(" ");
                     quote! { #name_id!(#format_str, #( #args_compiled ),*) }
                }
            } else if name == "typeof" {
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
                let args_raw: Vec<_> = args.iter().map(|a| compile_expr(a, target_obj, false)).collect();
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
            if simple_name.ends_with("::Ok") || simple_name.ends_with("::Err") || simple_name == "Ok" || simple_name == "Err" {
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
                            let gens = last.generics.iter().map(|g| compile_type_ext(g, target_obj));
                            base = quote!(#base :: <#( #gens ),*>);
                        }
                    }
                }
                base
            } else {
                compile_path(name, target_obj)
            };
            let args = args.iter().enumerate().map(|(i, a)| {
                let kind = arg_kinds.as_ref().and_then(|ks| ks.get(i)).cloned().unwrap_or(ArgKind::Value);
                if is_phantom && matches!(kind, ArgKind::Value) {
                    let e = compile_expr(a, target_obj, false);
                    return quote!((&#e).as_val());
                }                match kind {
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
                    if ["string", "str", "vector", "i32", "i64", "f32", "f64"].contains(&obj.as_str()) {
                        name_to_use = format!("solar_{}", name);
                    }
                }
            }

            let id = quote::format_ident!("{}", name_to_use);
            let args = args.iter().enumerate().map(|(i, a)| {
                let kind = arg_kinds.as_ref().and_then(|ks| ks.get(i)).cloned().unwrap_or(ArgKind::Value);
                if is_phantom && matches!(kind, ArgKind::Value) {
                    let e = compile_expr(a, target_obj, false);
                    return quote!((&#e).as_val());
                }                match kind {
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
                if is_mut { quote! { (#l.borrow_mut()).#id } }
                else { quote! { (#l.borrow()).#id } }
            } else if let Some(Type::ThreadSafe(_)) = &lhs.ty {
                if is_mut { quote! { (#l.write()).#id } }
                else { quote! { (#l.read()).#id } }
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
                (resolved_name.as_ref().unwrap_or(&simple_name).clone(), Some(path))
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
                    ExprKind::Call(_, _, _, _) | ExprKind::MethodCall(_, _, _, _, _) | 
                    ExprKind::Alloc(_, _) | ExprKind::StructLiteral { .. } |
                    ExprKind::Int(_) | ExprKind::Int64(_) | ExprKind::Float(_) | 
                    ExprKind::Bool(_) | ExprKind::String(_) | ExprKind::Unit |
                    ExprKind::Try(_) | ExprKind::Unwrap(_) | ExprKind::Await(_) => {
                        quote! { #fname: #fval }
                    }
                    _ => quote! { #fname: (&#fval).as_val() }
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

fn compile_stmt(stmt: &Stmt, is_last: bool, target_obj: Option<&String>, expected_ret: Option<&Type>) -> TokenStream {
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
                    Type::Managed(_) if !matches!(value.ty, Some(Type::Managed(_))) => quote! { ::std::rc::Rc::new(::std::cell::RefCell::new(#val_raw)) },
                    Type::ThreadSafe(_) if !matches!(value.ty, Some(Type::ThreadSafe(_))) => quote! { ::std::sync::Arc::new(::parking_lot::RwLock::new(#val_raw)) },
                    Type::BoxPtr(_) if !matches!(value.ty, Some(Type::BoxPtr(_))) => quote! { Box::new(#val_raw) },
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
                inner.extend(compile_stmt(s, i == stmts.len() - 1, target_obj, expected_ret));
            }
            quote! { { #inner } }
        }
        StmtKind::UnsafeBlock(stmts) => {
            let mut inner = TokenStream::new();
            for (i, s) in stmts.iter().enumerate() {
                inner.extend(compile_stmt(s, i == stmts.len() - 1, target_obj, expected_ret));
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

fn compile_generics(generics: &[(String, Vec<String>)]) -> TokenStream {
    if generics.is_empty() {
        quote!()
    } else {
        let gids = generics.iter().map(|(g, bounds)| {
            let id = quote::format_ident!("{}", g);
            if bounds.is_empty() {
                quote!(#id: Clone)
            } else {
                let b_ids = bounds.iter().map(|b| compile_id(b));
                quote!(#id: Clone + #( #b_ids )+*)
            }
        });
        quote!(<#( #gids ),*>)
    }
}

fn compile_function(func: &Function, target_obj: Option<&String>, is_trait_impl: bool) -> TokenStream {
    let name_to_use = &func.name;
    let name = quote::format_ident!("{}", name_to_use);
    let async_kw = if func.is_async {
        quote!(async)
    } else {
        quote!()
    };
    let attrs = func.attributes.iter().map(|a| {
        let attr = a.parse::<TokenStream>().expect("Failed to parse attribute");
        quote! { #[#attr] }
    });
    let main_attr = if func.name == "main" && func.is_async {
        if func.attributes.is_empty() {
            quote!(#[tokio::main])
        } else {
            quote!()
        }
    } else {
        quote!()
    };

    let gens = compile_generics(&func.generics);
    let params = func.params.iter().map(|p| {
        let p_name = quote::format_ident!("{}", p.name);
        if p.name == "self" {
            match &p.ty {
                Type::Ref(inner, mutable) if matches!(**inner, Type::SelfType) => {
                    if *mutable {
                        quote!(&mut self)
                    } else {
                        quote!(&self)
                    }
                }
                Type::SelfType => {
                    if p.is_mutable {
                        quote!(mut self)
                    } else {
                        quote!(self)
                    }
                }
                _ => {
                    // Fallback for when self is not explicitly a reference but should be
                    // according to previous conventions or just as a default.
                    // But here we'll try to be more strict if the user wants Rust-like.
                    if p.is_mutable {
                        quote!(&mut self)
                    } else {
                        quote!(&self)
                    }
                }
            }
        } else if p.ty == Type::Str {
            quote! { #p_name: &str }
        } else {
            let p_ty = compile_type_ext(&p.ty, target_obj);
            let mut_kw = if p.is_mutable { quote!(mut) } else { quote!() };
            // ALL Solar function parameters are passed by reference in the generated Rust
            if p.is_mutable {
                quote!(#p_name: &mut #p_ty)
            } else {
                quote!(#p_name: &#p_ty)
            }
        }
        }).collect::<Vec<_>>();

    let is_macroquad = func.attributes.iter().any(|a| a.contains("macroquad::main"));
    let ret_type = if func.name == "main" && !is_macroquad {
        quote!(-> Result<(), Box<dyn ::std::error::Error>>)
    } else {
        match &func.return_type {
            Some(ty) => {
                let t = compile_type_ext(ty, target_obj);
                quote!(-> #t)
            }
            None => quote!(),
        }
    };
    let unit_ty = Type::Unit;
    let main_ret_ty = Type::Result(Box::new(Type::Unit), Box::new(Type::Error));

    let body = if func.name == "main" && !is_macroquad {
        let b = compile_stmt(&func.body, true, target_obj, Some(&main_ret_ty));
        quote! { { #b Ok(()) } }
    } else {
        compile_stmt(&func.body, true, target_obj, func.return_type.as_ref().or(Some(&unit_ty)))
    };
    let vis = if is_trait_impl { quote!() } else { quote!(pub) };
    quote! { #main_attr #( #attrs )* #vis #async_kw fn #name #gens (#( #params ),*) #ret_type #body }
}

fn collect_metadata(
    decls: &[Decl],
    prefix: &str,
    mappings: &mut HashMap<String, String>,
    dependencies: &mut Vec<(String, String)>,
    use_tokio: &mut bool,
) {
    for decl in decls {
        match decl {
            Decl::ExternFunction(f) => {
                if let Some(p) = &f.rust_path {
                    let name = if prefix.is_empty() {
                        f.name.clone()
                    } else {
                        format!("{}::{}", prefix, f.name)
                    };
                    mappings.insert(name, p.clone());
                }
            }
            Decl::ExternObject(o) => {
                if let Some(p) = &o.rust_path {
                    let name = if prefix.is_empty() {
                        o.name.clone()
                    } else {
                        format!("{}::{}", prefix, o.name)
                    };
                    mappings.insert(name, p.clone());
                }
            }
            Decl::ExternEnum(e) => {
                if let Some(p) = &e.rust_path {
                    let name = if prefix.is_empty() {
                        e.name.clone()
                    } else {
                        format!("{}::{}", prefix, e.name)
                    };
                    mappings.insert(name, p.clone());
                }
            }
            Decl::RustDependency(name, version) => {
                dependencies.push((name.clone(), version.clone()));
            }
            Decl::Function(f) if f.name == "main" && f.is_async && f.attributes.is_empty() && prefix.is_empty() => {
                *use_tokio = true;
            }
            Decl::Module(name, inner_decls) => {
                let new_prefix = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}::{}", prefix, name)
                };
                collect_metadata(inner_decls, &new_prefix, mappings, dependencies, use_tokio);
            }
            _ => {}
        }
    }
}

pub fn compile(program: Program, output_name: &str) {
    let mut tokens = TokenStream::new();
    let mut dependencies = Vec::new();
    let mut use_tokio = false;

    // Reset and collect mappings
    if let Ok(mut mappings) = RUST_MAPPINGS.lock() {
        mappings.clear();
        mappings.insert("string".to_string(), "crate::sr_string".to_string());
        mappings.insert("String".to_string(), "crate::sr_string".to_string());
        collect_metadata(
            &program.declarations,
            "",
            &mut mappings,
            &mut dependencies,
            &mut use_tokio,
        );
    }

    if use_tokio {
        dependencies.push(("tokio".to_string(), "1.0".to_string()));
    }

    let has_macroquad = dependencies.iter().any(|(n, _)| n == "macroquad");
    let cargo_toml_features = if has_macroquad {
        quote! { [features]
macroquad = [] }
    } else {
        quote!()
    };

    tokens.extend(quote! {
        #![allow(unused)]
        #[global_allocator]
        static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

        use std::io::{Read, Write};
        use std::borrow::Cow;

        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum SolarCow<'a> {
            Borrowed(&'a str),
            Owned(String),
        }

        impl<'a> SolarCow<'a> {
            pub fn as_str(&self) -> &str {
                match self {
                    SolarCow::Borrowed(s) => s,
                    SolarCow::Owned(s) => s.as_str(),
                }
            }
        }

        impl<'a> std::ops::Deref for SolarCow<'a> {
            type Target = str;
            fn deref(&self) -> &str { self.as_str() }
        }

        impl<'a> AsRef<str> for SolarCow<'a> {
            fn as_ref(&self) -> &str { self.as_str() }
        }

        impl<'a> std::fmt::Display for SolarCow<'a> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.as_str())
            }
        }

        impl<'a> From<&'a str> for SolarCow<'a> {
            fn from(s: &'a str) -> Self { SolarCow::Borrowed(s) }
        }

        impl<'a> From<String> for SolarCow<'a> {
            fn from(s: String) -> Self { SolarCow::Owned(s) }
        }

        pub trait SolarStr {
            fn to_owned_string(&self) -> String;
            fn solar_to_string(&self) -> String;
            fn solar_to_str(&self) -> &str;
            fn solar_len(&self) -> i32;
            fn solar_contains(&self, s: impl AsRef<str>) -> bool;
            fn solar_split(&self, s: impl AsRef<str>) -> Vec<String>;
        }

        impl SolarStr for str {
            fn to_owned_string(&self) -> String { self.to_owned() }
            fn solar_to_string(&self) -> String { self.to_owned() }
            fn solar_to_str(&self) -> &str { self }
            fn solar_len(&self) -> i32 { self.len() as i32 }
            fn solar_contains(&self, s: impl AsRef<str>) -> bool { self.contains(s.as_ref()) }
            fn solar_split(&self, s: impl AsRef<str>) -> Vec<String> { self.split(s.as_ref()).map(|x| x.to_owned()).collect() }
        }

        pub trait SolarString {
            fn solar_append(&mut self, s: impl AsRef<str>);
            fn solar_len(&self) -> i32;
            fn solar_contains(&self, s: impl AsRef<str>) -> bool;
            fn solar_as_str(&self) -> &str;
            fn solar_to_str(&self) -> &str;
            fn solar_to_string(&self) -> String;
            fn solar_lines(&self) -> Vec<String>;
            fn solar_split(&self, s: impl AsRef<str>) -> Vec<String>;
        }

        impl SolarString for String {
            fn solar_append(&mut self, s: impl AsRef<str>) { self.push_str(s.as_ref()); }
            fn solar_len(&self) -> i32 { self.len() as i32 }
            fn solar_contains(&self, s: impl AsRef<str>) -> bool { self.contains(s.as_ref()) }
            fn solar_as_str(&self) -> &str { self.as_str() }
            fn solar_to_str(&self) -> &str { self.as_str() }
            fn solar_to_string(&self) -> String { self.clone() }
            fn solar_lines(&self) -> Vec<String> { self.lines().map(|x| x.to_owned()).collect() }
            fn solar_split(&self, s: impl AsRef<str>) -> Vec<String> { self.split(s.as_ref()).map(|x| x.to_owned()).collect() }
        }

        pub trait SolarVec<T> {
            fn solar_len(&self) -> i32;
            fn solar_get(&self, i: &i32) -> T;
        }

        impl<T: Clone> SolarVec<T> for Vec<T> {
            fn solar_len(&self) -> i32 { self.len() as i32 }
            fn solar_get(&self, i: &i32) -> T { self[*i as usize].clone() }
        }

        pub trait SolarIndex<Idx> {
            type Output;
            fn solar_index(&self, i: Idx) -> &Self::Output;
        }

        impl<T> SolarIndex<i32> for Vec<T> {
            type Output = T;
            fn solar_index(&self, i: i32) -> &T { &self[i as usize] }
        }

        impl<T> SolarIndex<i32> for *mut T {
            type Output = T;
            fn solar_index(&self, i: i32) -> &T { unsafe { &*self.add(i as usize) } }
        }

        impl<T> SolarIndex<i32> for *const T {
            type Output = T;
            fn solar_index(&self, i: i32) -> &T { unsafe { &*self.add(i as usize) } }
        }

        pub trait SolarAdd<Rhs = Self> {
            type Output;
            fn solar_add(&self, rhs: &Rhs) -> Self::Output;
        }

        impl SolarAdd for i32 {
            type Output = i32;
            fn solar_add(&self, rhs: &i32) -> i32 { *self + *rhs }
        }

        impl SolarAdd for i64 {
            type Output = i64;
            fn solar_add(&self, rhs: &i64) -> i64 { *self + *rhs }
        }

        impl SolarAdd for f32 {
            type Output = f32;
            fn solar_add(&self, rhs: &f32) -> f32 { *self + *rhs }
        }

        impl SolarAdd for f64 {
            type Output = f64;
            fn solar_add(&self, rhs: &f64) -> f64 { *self + *rhs }
        }

        impl SolarAdd<&str> for &str {
            type Output = String;
            fn solar_add(&self, rhs: &&str) -> String { format!("{}{}", self, rhs) }
        }

        impl SolarAdd<String> for &str {
            type Output = String;
            fn solar_add(&self, rhs: &String) -> String { format!("{}{}", self, rhs) }
        }

        impl<'a> SolarAdd<&'a String> for &str {
            type Output = String;
            fn solar_add(&self, rhs: &&'a String) -> String { format!("{}{}", self, *rhs) }
        }

        impl SolarAdd<&str> for String {
            type Output = String;
            fn solar_add(&self, rhs: &&str) -> String { format!("{}{}", self, rhs) }
        }

        impl SolarAdd<String> for String {
            type Output = String;
            fn solar_add(&self, rhs: &String) -> String { format!("{}{}", self, rhs) }
        }

        impl<'a> SolarAdd<&'a String> for String {
            type Output = String;
            fn solar_add(&self, rhs: &&'a String) -> String { format!("{}{}", self, *rhs) }
        }

        impl SolarAdd<i32> for &str {
            type Output = String;
            fn solar_add(&self, rhs: &i32) -> String { format!("{}{}", self, rhs) }
        }

        impl SolarAdd<i64> for &str {
            type Output = String;
            fn solar_add(&self, rhs: &i64) -> String { format!("{}{}", self, rhs) }
        }

        impl SolarAdd<f32> for &str {
            type Output = String;
            fn solar_add(&self, rhs: &f32) -> String { format!("{}{}", self, rhs) }
        }

        impl SolarAdd<f64> for &str {
            type Output = String;
            fn solar_add(&self, rhs: &f64) -> String { format!("{}{}", self, rhs) }
        }

        impl SolarAdd<i32> for String {
            type Output = String;
            fn solar_add(&self, rhs: &i32) -> String { format!("{}{}", self, rhs) }
        }

        impl SolarAdd<i64> for String {
            type Output = String;
            fn solar_add(&self, rhs: &i64) -> String { format!("{}{}", self, rhs) }
        }

        impl SolarAdd<f32> for String {
            type Output = String;
            fn solar_add(&self, rhs: &f32) -> String { format!("{}{}", self, rhs) }
        }

        impl SolarAdd<f64> for String {
            type Output = String;
            fn solar_add(&self, rhs: &f64) -> String { format!("{}{}", self, rhs) }
        }

        pub trait SolarSub<Rhs = Self> {
            type Output;
            fn solar_sub(&self, rhs: &Rhs) -> Self::Output;
        }

        impl SolarSub for i32 {
            type Output = i32;
            fn solar_sub(&self, rhs: &i32) -> i32 { *self - *rhs }
        }

        impl SolarSub for i64 {
            type Output = i64;
            fn solar_sub(&self, rhs: &i64) -> i64 { *self - *rhs }
        }

        impl SolarSub for f32 {
            type Output = f32;
            fn solar_sub(&self, rhs: &f32) -> f32 { *self - *rhs }
        }

        impl SolarSub for f64 {
            type Output = f64;
            fn solar_sub(&self, rhs: &f64) -> f64 { *self - *rhs }
        }

        pub trait SolarMul<Rhs = Self> {
            type Output;
            fn solar_mul(&self, rhs: &Rhs) -> Self::Output;
        }

        impl SolarMul for i32 {
            type Output = i32;
            fn solar_mul(&self, rhs: &i32) -> i32 { *self * *rhs }
        }

        impl SolarMul for i64 {
            type Output = i64;
            fn solar_mul(&self, rhs: &i64) -> i64 { *self * *rhs }
        }

        impl SolarMul for f32 {
            type Output = f32;
            fn solar_mul(&self, rhs: &f32) -> f32 { *self * *rhs }
        }

        impl SolarMul for f64 {
            type Output = f64;
            fn solar_mul(&self, rhs: &f64) -> f64 { *self * *rhs }
        }

        pub trait SolarDiv<Rhs = Self> {
            type Output;
            fn solar_div(&self, rhs: &Rhs) -> Self::Output;
        }

        impl SolarDiv for i32 {
            type Output = i32;
            fn solar_div(&self, rhs: &i32) -> i32 { *self / *rhs }
        }

        impl SolarDiv for i64 {
            type Output = i64;
            fn solar_div(&self, rhs: &i64) -> i64 { *self / *rhs }
        }

        impl SolarDiv for f32 {
            type Output = f32;
            fn solar_div(&self, rhs: &f32) -> f32 { *self / *rhs }
        }

        impl SolarDiv for f64 {
            type Output = f64;
            fn solar_div(&self, rhs: &f64) -> f64 { *self / *rhs }
        }

        pub trait SolarLT<Rhs = Self> {
            fn solar_lt(&self, rhs: &Rhs) -> bool;
        }

        impl SolarLT for i32 {
            fn solar_lt(&self, rhs: &i32) -> bool { *self < *rhs }
        }

        impl SolarLT for i64 {
            fn solar_lt(&self, rhs: &i64) -> bool { *self < *rhs }
        }

        impl SolarLT for f32 {
            fn solar_lt(&self, rhs: &f32) -> bool { *self < *rhs }
        }

        impl SolarLT for f64 {
            fn solar_lt(&self, rhs: &f64) -> bool { *self < *rhs }
        }

        pub trait SolarGT<Rhs = Self> {
            fn solar_gt(&self, rhs: &Rhs) -> bool;
        }

        impl SolarGT for i32 {
            fn solar_gt(&self, rhs: &i32) -> bool { *self > *rhs }
        }

        impl SolarGT for i64 {
            fn solar_gt(&self, rhs: &i64) -> bool { *self > *rhs }
        }

        impl SolarGT for f32 {
            fn solar_gt(&self, rhs: &f32) -> bool { *self > *rhs }
        }

        impl SolarGT for f64 {
            fn solar_gt(&self, rhs: &f64) -> bool { *self > *rhs }
        }

        pub trait SolarLE<Rhs = Self> {
            fn solar_le(&self, rhs: &Rhs) -> bool;
        }

        impl SolarLE for i32 {
            fn solar_le(&self, rhs: &i32) -> bool { *self <= *rhs }
        }

        impl SolarLE for i64 {
            fn solar_le(&self, rhs: &i64) -> bool { *self <= *rhs }
        }

        impl SolarLE for f32 {
            fn solar_le(&self, rhs: &f32) -> bool { *self <= *rhs }
        }

        impl SolarLE for f64 {
            fn solar_le(&self, rhs: &f64) -> bool { *self <= *rhs }
        }

        pub trait SolarGE<Rhs = Self> {
            fn solar_ge(&self, rhs: &Rhs) -> bool;
        }

        impl SolarGE for i32 {
            fn solar_ge(&self, rhs: &i32) -> bool { *self >= *rhs }
        }

        impl SolarGE for i64 {
            fn solar_ge(&self, rhs: &i64) -> bool { *self >= *rhs }
        }

        impl SolarGE for f32 {
            fn solar_ge(&self, rhs: &f32) -> bool { *self >= *rhs }
        }

        impl SolarGE for f64 {
            fn solar_ge(&self, rhs: &f64) -> bool { *self >= *rhs }
        }

        pub trait SolarEq<Rhs = Self> {
            fn solar_eq(&self, rhs: &Rhs) -> bool;
        }

        impl SolarEq for i32 {
            fn solar_eq(&self, rhs: &i32) -> bool { *self == *rhs }
        }

        impl SolarEq for i64 {
            fn solar_eq(&self, rhs: &i64) -> bool { *self == *rhs }
        }

        impl SolarEq for f32 {
            fn solar_eq(&self, rhs: &f32) -> bool { *self == *rhs }
        }

        impl SolarEq for f64 {
            fn solar_eq(&self, rhs: &f64) -> bool { *self == *rhs }
        }

        impl SolarEq for bool {
            fn solar_eq(&self, rhs: &bool) -> bool { *self == *rhs }
        }

        impl SolarEq for String {
            fn solar_eq(&self, rhs: &String) -> bool { self == rhs }
        }

        impl SolarEq<&str> for &str {
            fn solar_eq(&self, rhs: &&str) -> bool { self == rhs }
        }

        pub trait SolarI32 {
            fn solar_to_string(&self) -> String;
        }

        impl SolarI32 for i32 {
            fn solar_to_string(&self) -> String { self.to_string() }
        }

        pub trait SolarI64 {
            fn solar_to_string(&self) -> String;
        }

        impl SolarI64 for i64 {
            fn solar_to_string(&self) -> String { self.to_string() }
        }

        pub trait SolarF32 {
            fn solar_to_string(&self) -> String;
        }

        impl SolarF32 for f32 {
            fn solar_to_string(&self) -> String { self.to_string() }
        }

        pub trait SolarF64 {
            fn solar_to_string(&self) -> String;
        }

        impl SolarF64 for f64 {
            fn solar_to_string(&self) -> String { self.to_string() }
        }

        pub trait SolarAsArg<'a> {
            type Out;
            fn as_arg(&'a self) -> Self::Out;
        }

        impl<'a, T: 'a> SolarAsArg<'a> for T {
            type Out = &'a T;
            fn as_arg(&'a self) -> &'a T { self }
        }

        pub trait SolarAsVal<T> {
            fn as_val(&self) -> T;
        }

        impl<'a> crate::SolarAsVal<&'a str> for String {
            fn as_val(&self) -> &'a str { unsafe { std::mem::transmute(self.as_str()) } }
        }

        impl<'a, 'b> crate::SolarAsVal<&'a str> for &'b String {
            fn as_val(&self) -> &'a str { unsafe { std::mem::transmute(self.as_str()) } }
        }

        impl<'a> crate::SolarAsVal<&'a str> for &'a str {
            fn as_val(&self) -> &'a str { self }
        }

        impl<'a, 'b> crate::SolarAsVal<&'a str> for &'b &'a str {
            fn as_val(&self) -> &'a str { *self }
        }

        impl crate::SolarAsVal<String> for &str {
            fn as_val(&self) -> String { self.to_string() }
        }

        impl crate::SolarAsVal<String> for &&str {
            fn as_val(&self) -> String { self.to_string() }
        }

        impl crate::SolarAsVal<String> for String {
            fn as_val(&self) -> String { self.clone() }
        }

        impl<'a> crate::SolarAsVal<String> for &'a String {
            fn as_val(&self) -> String { (*self).clone() }
        }

        // Only implement for non-specialized types to avoid conflicts
        // This is a hack, but without negative trait bounds or better specialization it's hard.
        // We'll just implement for primitive-ish types that need it.
        impl crate::SolarAsVal<i32> for i32 { fn as_val(&self) -> i32 { *self } }
        impl crate::SolarAsVal<i32> for &i32 { fn as_val(&self) -> i32 { **self } }
        impl crate::SolarAsVal<f32> for f32 { fn as_val(&self) -> f32 { *self } }
        impl crate::SolarAsVal<f32> for &f32 { fn as_val(&self) -> f32 { **self } }
        impl crate::SolarAsVal<f64> for f64 { fn as_val(&self) -> f64 { *self } }
        impl crate::SolarAsVal<f64> for &f64 { fn as_val(&self) -> f64 { **self } }
        impl crate::SolarAsVal<bool> for bool { fn as_val(&self) -> bool { *self } }
        impl crate::SolarAsVal<bool> for &bool { fn as_val(&self) -> bool { **self } }

        #[cfg(feature = "macroquad")]
        impl crate::SolarAsVal<::macroquad::math::Vec2> for ::macroquad::math::Vec2 { fn as_val(&self) -> ::macroquad::math::Vec2 { *self } }
        #[cfg(feature = "macroquad")]
        impl crate::SolarAsVal<::macroquad::math::Vec2> for &::macroquad::math::Vec2 { fn as_val(&self) -> ::macroquad::math::Vec2 { **self } }
        #[cfg(feature = "macroquad")]
        impl crate::SolarAsVal<::macroquad::color::Color> for ::macroquad::color::Color { fn as_val(&self) -> ::macroquad::color::Color { *self } }
        #[cfg(feature = "macroquad")]
        impl crate::SolarAsVal<::macroquad::color::Color> for &::macroquad::color::Color { fn as_val(&self) -> ::macroquad::color::Color { **self } }


        // We can't have a blanket impl T: Clone because it conflicts with specialized impls.
        // For custom Solar objects, we can either generate the impl or use a macro.
        // For now, let's add a few more common ones.
        impl<T: Clone> SolarAsVal<Vec<T>> for Vec<T> { fn as_val(&self) -> Vec<T> { self.clone() } }
        impl<T: Clone> SolarAsVal<Vec<T>> for &Vec<T> { fn as_val(&self) -> Vec<T> { (*self).clone() } }


        impl<T: Clone> SolarAsVal<T> for ::std::rc::Rc<::std::cell::RefCell<T>> {
            fn as_val(&self) -> T { self.borrow().clone() }
        }

        impl<T: Clone> SolarAsVal<T> for ::std::sync::Arc<::parking_lot::RwLock<T>> {
            fn as_val(&self) -> T { self.read().clone() }
        }

        pub trait SolarAsSize {
            fn as_size(&self) -> usize;
        }

        impl SolarAsSize for i32 {
            fn as_size(&self) -> usize { *self as usize }
        }

        impl<'a> SolarAsSize for &'a i32 {
            fn as_size(&self) -> usize { **self as usize }
        }

        pub mod mem {
            pub fn alloc<T>(size: &i32) -> *mut T {
                let mut v = Vec::with_capacity(*size as usize);
                let p = v.as_mut_ptr();
                std::mem::forget(v);
                p
            }
            pub fn free<T>(p: &*mut T, size: &i32) {
                unsafe {
                    let _ = Vec::from_raw_parts(*p, 0, *size as usize);
                }
            }
        }

        pub mod sr_fs {
            use std::io::{Read, Write};
            use std::fs::OpenOptions;

            pub fn create(path: impl AsRef<str>) -> std::fs::File {
                std::fs::File::create(path.as_ref()).expect("Failed to create file")
            }

            pub fn open(path: impl AsRef<str>) -> std::fs::File {
                std::fs::File::open(path.as_ref()).expect("Failed to open file")
            }

            pub fn append(path: impl AsRef<str>) -> std::fs::File {
                OpenOptions::new().append(true).create(true).open(path.as_ref()).expect("Failed to open file for append")
            }

            pub fn read(f: &mut std::fs::File) -> String {
                let mut s = String::new();
                f.read_to_string(&mut s).expect("Failed to read file");
                s
            }

            pub fn read_to_string(path: impl AsRef<str>) -> String {
                std::fs::read_to_string(path.as_ref()).expect("Failed to read file to string")
            }

            pub fn write(f: &mut std::fs::File, content: impl AsRef<str>) {
                f.write_all(content.as_ref().as_bytes()).expect("Failed to write file");
            }

            pub fn write_string(f: &mut std::fs::File, content: String) {
                f.write_all(content.as_bytes()).expect("Failed to write file");
            }
        }

        pub mod sr_io {
            use std::io::{self, Write};

            pub fn readline() -> String {
                let mut s = String::new();
                io::stdin().read_line(&mut s).expect("Failed to read line");
                s.trim().to_string()
            }

            pub fn write(content: impl AsRef<str>) {
                io::stdout().write_all(content.as_ref().as_bytes()).expect("Failed to write to stdout");
                io::stdout().flush().expect("Failed to flush stdout");
            }

            pub fn println(content: impl AsRef<str>) {
                println!("{}", content.as_ref());
            }

            pub fn exit(code: i32) {
                std::process::exit(code);
            }

            pub fn args() -> Vec < String > {
                std::env::args().collect()
            }
        }

        pub mod sr_string {
            pub fn new() -> String { String::new() }
        }

            pub mod sr_math {
            pub fn pi() -> f32 { std::f32::consts::PI }
            pub fn e() -> f32 { std::f32::consts::E }
            pub fn tau() -> f32 { std::f32::consts::TAU }
            pub fn pi64() -> f64 { std::f64::consts::PI }
            pub fn e64() -> f64 { std::f64::consts::E }
            pub fn tau64() -> f64 { std::f64::consts::TAU }
            }
            });

    compile_decls(&program.declarations, &mut tokens);

    fs::write("output.rs", tokens.to_string()).expect("Failed to write Rust code");

    // Create a temporary cargo project
    let project_dir = "solar_out";
    fs::create_dir_all(format!("{}/src", project_dir)).ok();

    let mut cargo_toml = String::from(
        r#"
[package]
name = "solar_out"
version = "0.1.0"
edition = "2021"

[dependencies]
mimalloc = "0.1"
parking_lot = "0.12"
thiserror = "1.0"
"#,
    );

    if use_tokio {
        cargo_toml.push_str("tokio = { version = \"1.0\", features = [\"full\"] }\n");
    }

    for (name, version) in dependencies {
        if name != "tokio" {
            if name == "serde" {
                let v = if version.starts_with('"') {
                    version.trim_matches('"').to_string()
                } else {
                    version.clone()
                };
                cargo_toml.push_str(&format!(
                    "serde = {{ version = \"{}\", features = [\"derive\"] }}\n",
                    v
                ));
            } else if version.starts_with('{') || version.starts_with('"') {
                cargo_toml.push_str(&format!("{} = {}\n", name, version));
            } else {
                cargo_toml.push_str(&format!("{} = \"{}\"\n", name, version));
            }
        }
    }

    if has_macroquad {
        cargo_toml.push_str("\n[features]\nmacroquad = []\n");
    }

    // Add profile optimizations for faster compilation
    cargo_toml.push_str(
        r#"
[profile.dev]
opt-level = 0
debug = true
split-debuginfo = "unpacked"
incremental = true
codegen-units = 256

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
panic = "abort"
"#,
    );

    fs::write(format!("{}/Cargo.toml", project_dir), cargo_toml)
        .expect("Failed to write Cargo.toml");

    fs::write(format!("{}/src/main.rs", project_dir), tokens.to_string())
        .expect("Failed to write src/main.rs");

    println!("Compiling via Cargo...");
    let mut args = vec!["build"];
    if has_macroquad {
        args.push("--features");
        args.push("macroquad");
    }
    let status = Command::new("cargo")
        .args(&args)
        .current_dir(project_dir)
        .status()
        .expect("Failed to run cargo build");

    if status.success() {
        println!("Compilation successful!");
        let src_binary = format!("{}/target/debug/solar_out", project_dir);
        let dst_binary = output_name;
        if let Err(e) = fs::copy(&src_binary, dst_binary) {
            eprintln!("Failed to copy binary to {}: {}", dst_binary, e);
        }
    } else {
        eprintln!("Cargo compilation failed.");
    }
}

fn compile_decls(decls: &[Decl], tokens: &mut TokenStream) {
    for decl in decls {
        match decl {
            Decl::Function(func) => {
                tokens.extend(compile_function(func, None, false));
            }
            Decl::Trait(tr) => {
                let name = quote::format_ident!("{}", tr.name);
                let gens = compile_generics(&tr.generics);
                let bounds = if tr.bounds.is_empty() {
                    quote!()
                } else {
                    let b_tokens = tr.bounds.iter().map(|b| {
                        let id = compile_id(b);
                        quote!(#id)
                    });
                    quote!(: #( #b_tokens )+*)
                };
                let functions = tr.functions.iter().map(|f| {
                    let fname = quote::format_ident!("{}", f.name);
                    let params = f.params.iter().map(|p| {
                        let p_name = quote::format_ident!("{}", p.name);
                        if p.name == "self" {
                            match &p.ty {
                                Type::Ref(_, mutable) => {
                                    if *mutable { quote!(&mut self) }
                                    else { quote!(&self) }
                                }
                                _ => quote!(&self),
                            }
                        } else {
                            let p_ty = compile_type(&p.ty);
                            quote!(#p_name: #p_ty)
                        }
                    });
                    let ret = if let Some(rt) = &f.return_type {
                        let rty = compile_type(rt);
                        quote!(-> #rty)
                    } else {
                        quote!()
                    };
                    quote!(fn #fname(#( #params ),*) #ret;)
                });
                tokens.extend(quote! { pub trait #name #gens #bounds { #( #functions )* } });
            }
            Decl::Impl(imp) => {
                let target = compile_id(&imp.target);
                let gparams = compile_generics(&imp.generics);
                let gtarget = if imp.generics.is_empty() {
                    quote!()
                } else {
                    let gids = imp.generics.iter().map(|(g, _)| quote::format_ident!("{}", g));
                    quote!(<#( #gids ),*>)
                };
                let is_trait_impl = imp.trait_name.is_some();
                let full_target = if imp.generics.is_empty() {
                    imp.target.clone()
                } else {
                    let gids: Vec<_> = imp.generics.iter().map(|(g, _)| g.clone()).collect();
                    format!("{}<{}>", imp.target, gids.join(", "))
                };
                let functions = imp
                    .functions
                    .iter()
                    .map(|f| compile_function(f, Some(&full_target), is_trait_impl));

                if let Some(trait_name) = &imp.trait_name {
                    let tr_name = compile_id(trait_name);
                    tokens.extend(quote! { impl #gparams #tr_name #gtarget for #target #gtarget { #( #functions )* } });
                } else {
                    tokens.extend(quote! { impl #gparams #target #gtarget { #( #functions )* } });
                }

                // If it has a destroy method, implement Drop
                if imp.functions.iter().any(|f| f.name == "destroy") {
                    tokens.extend(quote! {
                        impl #gparams Drop for #target #gtarget {
                            fn drop(&mut self) {
                                self.destroy();
                            }
                        }
                    });
                }
            }
            Decl::Object(obj) => {
                let name = quote::format_ident!("{}", obj.name);
                let gens = compile_generics(&obj.generics);
                let fields = obj.fields.iter().map(|f| {
                    let fname = quote::format_ident!("{}", f.name);
                    let fty = compile_type(&f.ty);
                    let fattrs = f.attributes.iter().map(|a| {
                        if a.starts_with("e(") {
                            let content = &a[2..a.len()-1];
                            quote!(#[error(#content)])
                        } else if a == "from" {
                            quote!(#[from])
                        } else {
                            let attr = a.parse::<TokenStream>().expect("Failed to parse attribute");
                            quote!(#[#attr])
                        }
                    });
                    quote! { #( #fattrs )* pub #fname: #fty }
                });
                let is_error = obj.attributes.iter().any(|a| a == "error");
                let attrs = obj.attributes.iter().filter(|a| *a != "error").map(|a| {
                    let attr = a.parse::<TokenStream>().expect("Failed to parse attribute");
                    quote! { #[#attr] }
                });
                let derive_error = if is_error {
                    quote!(#[derive(thiserror::Error, Debug, Clone)])
                } else {
                    if obj.generics.is_empty() {
                        quote!(#[derive(Clone, Debug, Default)])
                    } else {
                        quote!(#[derive(Clone, Debug)])
                    }
                };
                let gens_short = if obj.generics.is_empty() {
                    quote!()
                } else {
                    let gids = obj.generics.iter().map(|(g, _)| quote::format_ident!("{}", g));
                    quote!(<#( #gids ),*>)
                };
                tokens.extend(
                    quote! { 
                        #( #attrs )* #derive_error pub struct #name #gens { #( #fields ),* } 
                        impl #gens crate::SolarAsVal<#name #gens_short> for #name #gens_short {
                            fn as_val(&self) -> #name #gens_short { self.clone() }
                        }
                        impl #gens crate::SolarAsVal<#name #gens_short> for &#name #gens_short {
                            fn as_val(&self) -> #name #gens_short { (*self).clone() }
                        }
                    },
                );
            }
            Decl::Enum(enm) => {
                let name = quote::format_ident!("{}", enm.name);
                let gens = compile_generics(&enm.generics);
                let is_error = enm.attributes.iter().any(|a| a == "error");
                let variants = enm.variants.iter().map(|v| {
                    let vname = quote::format_ident!("{}", v.name);
                    let has_from = v.attributes.iter().any(|a| a == "from");
                    let vattrs = v.attributes.iter().filter(|a| *a != "from").map(|a| {
                        if a.starts_with("e(") {
                            let content = &a[2..a.len()-1];
                            quote!(#[error(#content)])
                        } else {
                            let attr = a.parse::<TokenStream>().expect("Failed to parse attribute");
                            quote!(#[#attr])
                        }
                    });
                    
                    let from_attr = if has_from { quote!(#[from]) } else { quote!() };

                    if v.types.is_empty() {
                        quote! { #( #vattrs )* #vname }
                    } else {
                        // Place #[from] on the first field if requested
                        let mut field_tokens = Vec::new();
                        for (i, t) in v.types.iter().enumerate() {
                            let ty = compile_type(t);
                            if i == 0 {
                                field_tokens.push(quote!(#from_attr #ty));
                            } else {
                                field_tokens.push(quote!(#ty));
                            }
                        }
                        quote! { #( #vattrs )* #vname(#( #field_tokens ),*) }
                    }
                });
                let attrs = enm.attributes.iter().filter(|a| *a != "error").map(|a| {
                    let attr = a.parse::<TokenStream>().expect("Failed to parse attribute");
                    quote! { #[#attr] }
                });
                let derive_error = if is_error {
                    quote!(#[derive(thiserror::Error, Debug, Clone)])
                } else {
                    quote!(#[derive(Clone, Debug)])
                };
                let gens_short = if enm.generics.is_empty() {
                    quote!()
                } else {
                    let gids = enm.generics.iter().map(|(g, _)| quote::format_ident!("{}", g));
                    quote!(<#( #gids ),*>)
                };
                tokens.extend(quote! { 
                    #( #attrs )* #derive_error pub enum #name #gens { #( #variants ),* } 
                    impl #gens crate::SolarAsVal<#name #gens_short> for #name #gens_short {
                        fn as_val(&self) -> #name #gens_short { self.clone() }
                    }
                    impl #gens crate::SolarAsVal<#name #gens_short> for &#name #gens_short {
                        fn as_val(&self) -> #name #gens_short { (*self).clone() }
                    }
                });
            }
            Decl::ExternFunction(_) => {}
            Decl::ExternObject(obj) => {
                let name = quote::format_ident!("{}", obj.name);
                let rust_path = obj
                    .rust_path
                    .as_ref()
                    .expect("Extern object must have a rust_path");
                let target_path = compile_id(rust_path);
                let gens = if obj.generics.is_empty() {
                    quote!()
                } else {
                    let gids = obj.generics.iter().map(|(g, _)| quote::format_ident!("{}", g));
                    quote!(<#( #gids ),*>)
                };
                tokens.extend(quote! {
                    pub type #name #gens = #target_path #gens;
                });
            }
            Decl::ExternEnum(enm) => {
                let name = quote::format_ident!("{}", enm.name);
                let rust_path = enm
                    .rust_path
                    .as_ref()
                    .expect("Extern enum must have a rust_path");
                let target_path = compile_id(rust_path);
                let gens = if enm.generics.is_empty() {
                    quote!()
                } else {
                    let gids = enm.generics.iter().map(|(g, _)| quote::format_ident!("{}", g));
                    quote!(<#( #gids ),*>)
                };
                tokens.extend(quote! {
                    pub type #name #gens = #target_path #gens;
                });
            }
            Decl::ExternTrait(_) | Decl::ExternImpl(_) | Decl::RustDependency(_, _) => {}
            Decl::RustBlock(code) => {
                let comment = format!("// Injected Rust Block\n{}", code);
                tokens.extend(quote!(#comment));
            }
            Decl::Module(name, inner) => {
                let mut inner_tokens = TokenStream::new();
                compile_decls(inner, &mut inner_tokens);

                let mut mod_tokens = inner_tokens;
                let mut parts: Vec<&str> = name.split("::").collect();

                if parts.get(0) == Some(&"std") {
                    parts.remove(0);
                }

                for part in parts.into_iter().rev() {
                    let id = quote::format_ident!("{}", part);
                    mod_tokens = quote! { pub mod #id { use std; use crate::{SolarStr, SolarString, SolarVec, SolarIndex, SolarAdd, SolarSub, SolarMul, SolarDiv, SolarGT, SolarLT, SolarLE, SolarGE, SolarEq, SolarI32, SolarI64, SolarF32, SolarF64, SolarAsArg, SolarAsVal, SolarAsSize, sr_math, sr_io, sr_fs}; #mod_tokens } };
                }
                tokens.extend(mod_tokens);
            }
            Decl::Use(u) => {
                let mut path_tokens = Vec::new();
                for (i, part) in u.path.iter().enumerate() {
                    let id = quote::format_ident!("{}", part);
                    if i == 0 && u.is_crate {
                        path_tokens.push(quote!(::#id));
                    } else {
                        path_tokens.push(quote!(#id));
                    }
                }
                
                let path = quote!(#( #path_tokens )::*);

                if u.is_wildcard {
                    tokens.extend(quote!(pub use #path::*;));
                } else if u.items.is_empty() {
                    tokens.extend(quote!(pub use #path;));
                } else {
                    let items = u.items.iter().map(|i| quote::format_ident!("{}", i));
                    tokens.extend(quote!(pub use #path::{#( #items ),*};));
                }            }
        }
    }
}
