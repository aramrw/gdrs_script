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
    // Check if it's already a complex Rust path (generics, pointers, etc.)
    if name.contains('<') || name.contains('(') || name.contains('[') || name.contains('*') {
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
                .map(|p| {
                    let id = quote::format_ident!("{}", p);
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
                .map(|p| {
                    let id = quote::format_ident!("{}", p);
                    quote!(#id)
                })
                .collect();
            return quote!(crate::#( #tokens )::*);
        }

        let tokens: Vec<TokenStream> = parts
            .iter()
            .map(|p| {
                let id = quote::format_ident!("{}", p);
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

    let id = quote::format_ident!("{}", name);
    quote!(#id)
}

fn compile_type(ty: &Type) -> TokenStream {
    match ty {
        Type::Unit => quote!(()),
        Type::I32 => quote!(i32),
        Type::I64 => quote!(i64),
        Type::F32 => quote!(f32),
        Type::F64 => quote!(f64),
        Type::Bool => quote!(bool),
        Type::Str => quote!(&str),
        Type::String => quote!(String),
        Type::File => quote!(::std::fs::File),
        Type::BoxPtr(inner) => {
            let t = compile_type(inner);
            quote!(Box<#t>)
        }
        Type::Array(inner, size) => {
            let t = compile_type(inner);
            quote!([#t; #size])
        }
        Type::RawPtr(inner, mutable) => {
            let t = compile_type(inner);
            if *mutable {
                quote!(*mut #t)
            } else {
                quote!(*const #t)
            }
        }
        Type::Ref(inner, mutable) => {
            let t = compile_type(inner);
            if *mutable {
                quote!(&mut #t)
            } else {
                quote!(&#t)
            }
        }
        Type::Generic(name) => {
            let id = quote::format_ident!("{}", name);
            quote!(#id)
        }
        Type::Custom(name, generics) => {
            let id = compile_id(name);
            if generics.is_empty() {
                quote!(#id)
            } else {
                let gens = generics.iter().map(compile_type);
                quote!(#id<#( #gens ),*>)
            }
        }
        Type::SelfType => quote!(Self),
        Type::Result(ok, err) => {
            let o = compile_type(ok);
            let e = compile_type(err);
            quote!(Result<#o, #e>)
        }
        Type::Any => quote!(_),
        Type::Error => quote!(Box<dyn ::std::error::Error>),
    }
}

fn compile_pattern(pat: &Pattern) -> TokenStream {
    match pat {
        Pattern::Variant(enm, var, params) => {
            let eid = compile_id(enm);
            let vid = quote::format_ident!("{}", var);
            let pids = params.iter().map(|p| quote::format_ident!("{}", p));
            if params.is_empty() {
                quote! { #eid::#vid }
            } else {
                quote! { #eid::#vid(#( #pids ),*) }
            }
        }
        Pattern::Variable(n) => {
            let id = quote::format_ident!("{}", n);
            quote!(#id)
        }
        Pattern::Literal(lit) => compile_expr(lit, None),
    }
}

fn compile_expr(expr: &Expr, target_obj: Option<&String>) -> TokenStream {
    match &expr.kind {
        ExprKind::Unit => quote!(()),
        ExprKind::Int(v) => quote!(#v),
        ExprKind::Int64(v) => quote! { #v },
        ExprKind::Float(v) => quote! { (#v as f32) },
        ExprKind::Float64(v) => quote! { (#v as f64) },
        ExprKind::Bool(v) => quote! { #v },
        ExprKind::String(v) => quote! { #v },
        ExprKind::Variable(n) => {
            if n == "self" {
                quote!(self)
            } else {
                compile_id(n)
            }
        }
        ExprKind::Binary(lhs, op, rhs) => {
            let l = compile_expr(lhs, target_obj);
            let r = compile_expr(rhs, target_obj);
            match op {
                BinaryOp::Add => {
                    quote! { (#l).solar_add(&#r) }
                }
                BinaryOp::Subtract => quote! { (#l).solar_sub(&#r) },
                BinaryOp::Multiply => quote! { (#l).solar_mul(&#r) },
                BinaryOp::Divide => quote! { (#l).solar_div(&#r) },
                BinaryOp::GreaterThan => quote! { (#l).solar_gt(&#r) },
                BinaryOp::LessThan => quote! { (#l).solar_lt(&#r) },
                BinaryOp::Equal => quote! { (#l).solar_eq(&#r) },
            }
        }
        ExprKind::MacroCall(name, args) => {
            let name_id = compile_id(name);
            let args_compiled: Vec<_> = args.iter().map(|a| compile_expr(a, target_obj)).collect();
            if name == "println" || name == "print" {
                let format_str = vec!["{:?}"; args_compiled.len()].join(" ");
                quote! { #name_id!(#format_str, #( &#args_compiled ),*) }
            } else if name == "typeof" {
                let arg = &args_compiled[0];
                quote! { std::any::type_name_of_val(&#arg) }
            } else {
                quote! { #name_id!(#( #args_compiled ),*) }
            }
        }
        ExprKind::Call(name, args, resolved_name) => {
            if name == "Box" && args.len() == 1 {
                let inner = compile_expr(&args[0], target_obj);
                return quote! { Box::new(#inner) };
            }
            if name == "Ok" || name == "Err" {
                let id = compile_id(name);
                let args = args.iter().map(|a| {
                    let e = compile_expr(a, target_obj);
                    quote!(crate::SolarAsVal::as_val(&#e))
                });
                return quote! { #id(#( #args ),*) };
            }
            if name == "array_init" {
                let element = compile_expr(&args[0], target_obj);
                let size = compile_expr(&args[1], target_obj);
                return quote! { unsafe {
                    let mut v: Vec<_> = (0..crate::SolarAsSize::as_size(&#size)).map(|_| crate::SolarAsVal::as_val(&#element)).collect();
                    let p = v.as_mut_ptr();
                    std::mem::forget(v);
                    p
                } };
            }
            let id = if let Some(resolved) = resolved_name {
                compile_id(resolved)
            } else {
                compile_id(name)
            };
            let args = args.iter().map(|a| compile_expr(a, target_obj));
            quote! { #id(#( #args ),*) }
        }
        ExprKind::MethodCall(lhs, name, args, _resolved_obj_name) => {
            let l = compile_expr(lhs, target_obj);
            let id = quote::format_ident!("{}", name);
            let args = args.iter().map(|a| compile_expr(a, target_obj));
            quote! { (#l).#id(#( #args ),*) }
        }
        ExprKind::MemberAccess(lhs, name) => {
            let l = compile_expr(lhs, target_obj);
            let id = quote::format_ident!("{}", name);
            quote! { #l.#id }
        }
        ExprKind::IndexAccess(lhs, index) => {
            let l = compile_expr(lhs, target_obj);
            let i = compile_expr(index, target_obj);
            quote! { (unsafe { &*#l.add(crate::SolarAsSize::as_size(&#i)) }) }
        }
        ExprKind::Cast(inner, ty) => {
            let e = compile_expr(inner, target_obj);
            let t = compile_type(ty);
            quote! { (crate::SolarAsVal::as_val(&#e) as #t) }
        }
        ExprKind::StructLiteral {
            name,
            fields,
            resolved_name,
        } => {
            let target_name = if let Some(resolved) = resolved_name {
                resolved.clone()
            } else if name == "self" {
                if let Some(t) = target_obj {
                    t.clone()
                } else {
                    "Self".to_string()
                }
            } else {
                name.clone()
            };
            let id = compile_id(&target_name);
            let fields = fields.iter().map(|(n, v)| {
                let fname = quote::format_ident!("{}", n);
                let fval = compile_expr(v, target_obj);
                quote! { #fname: crate::SolarAsVal::as_val(&#fval) }
            });
            quote! { #id { #( #fields ),* } }
        }
        ExprKind::Alloc(inner, kind) => {
            let e = compile_expr(inner, target_obj);
            match kind {
                AllocKind::Box => quote! { Box::new(#e) },
                AllocKind::RawMut => quote! { #e },
                AllocKind::RawConst => quote! { #e },
            }
        }
        ExprKind::Borrow(inner, mutable) => {
            let e = compile_expr(inner, target_obj);
            if *mutable {
                quote! { &mut #e }
            } else {
                quote! { &#e }
            }
        }
        ExprKind::Negate(inner) => {
            let e = compile_expr(inner, target_obj);
            quote! { (-#e) }
        }
        ExprKind::Deref(inner) => {
            let e = compile_expr(inner, target_obj);
            quote! { (*#e) }
        }
        ExprKind::Unwrap(inner) => {
            let e = compile_expr(inner, target_obj);
            quote! { #e? }
        }
        ExprKind::Await(inner) => {
            let e = compile_expr(inner, target_obj);
            quote! { #e.await }
        }
    }
}

fn compile_stmt(stmt: &Stmt, is_last: bool, target_obj: Option<&String>, expected_ret: Option<&Type>) -> TokenStream {
    match &stmt.kind {
        StmtKind::VarDecl {
            name,
            is_mutable,
            ty,
            value,
        } => {
            let id = quote::format_ident!("{}", name);
            let mut_kw = if *is_mutable { quote!(mut) } else { quote!() };
            let val = compile_expr(value, target_obj);
            let ty_tokens = match ty {
                Some(t) => {
                    let ct = compile_type(t);
                    quote!(: #ct)
                }
                None => quote!(),
            };
            quote! { let #mut_kw #id #ty_tokens = #val; }
        }
        StmtKind::Assign { target, value } => {
            let v = compile_expr(value, target_obj);
            if let ExprKind::IndexAccess(lhs, index) = &target.kind {
                let l = compile_expr(lhs, target_obj);
                let i = compile_expr(index, target_obj);
                quote! { unsafe { *#l.add(crate::SolarAsSize::as_size(&#i)) = crate::SolarAsVal::as_val(&#v); } }
            } else {
                let t = compile_expr(target, target_obj);
                quote! { #t = crate::SolarAsVal::as_val(&#v); }
            }
        }
        StmtKind::Block(stmts) => {
            let mut inner = TokenStream::new();
            for (i, s) in stmts.iter().enumerate() {
                inner.extend(compile_stmt(s, i == stmts.len() - 1, target_obj, expected_ret));
            }
            quote! { { #inner } }
        }
        StmtKind::ExprStmt(expr) => {
            let e = compile_expr(expr, target_obj);
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
            let cond = compile_expr(condition, target_obj);
            let then = compile_stmt(then_branch, is_last, target_obj, expected_ret);
            if let Some(else_b) = else_branch {
                let els = compile_stmt(else_b, is_last, target_obj, expected_ret);
                quote! { if #cond #then else #els }
            } else {
                quote! { if #cond #then }
            }
        }
        StmtKind::While { condition, body } => {
            let cond = compile_expr(condition, target_obj);
            let b = compile_stmt(body, false, target_obj, expected_ret);
            quote! { while #cond #b }
        }
        StmtKind::Return(expr) => {
            let e = expr.as_ref().map(|e| compile_expr(e, target_obj));
            if let Some(tokens) = e {
                quote! { return #tokens; }
            } else {
                quote! { return; }
            }
        }
        StmtKind::Match { expr, arms } => {
            let e = compile_expr(expr, target_obj);
            let arm_tokens = arms.iter().map(|arm| {
                let pat = compile_pattern(&arm.pattern);
                let body = compile_stmt(&arm.body, is_last, target_obj, expected_ret);
                quote! { #pat => { #body } }
            });
            quote! { match #e { #( #arm_tokens ),* } }
        }
    }
}

fn compile_function(func: &Function, target_obj: Option<&String>) -> TokenStream {
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

    let gens = if func.generics.is_empty() {
        quote!()
    } else {
        let gids = func.generics.iter().map(|g| quote::format_ident!("{}", g));
        quote!(<#( #gids: Clone + Default ),*>)
    };
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
            let p_ty = compile_type(&p.ty);
            let mut_kw = if p.is_mutable { quote!(mut) } else { quote!() };
            // If it's already a reference in Solar, compile_type will handle it.
            // If it's NOT a reference, we used to force it. Let's stop forcing it.
            quote! { #mut_kw #p_name: #p_ty }
        }
    });
    let is_macroquad = func.attributes.iter().any(|a| a.contains("macroquad::main"));
    let ret_type = if func.name == "main" && !is_macroquad {
        quote!(-> Result<(), Box<dyn ::std::error::Error>>)
    } else {
        match &func.return_type {
            Some(ty) => {
                let t = compile_type(ty);
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
    quote! { #main_attr #( #attrs )* pub #async_kw fn #name #gens (#( #params ),*) #ret_type #body }
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

    tokens.extend(quote! {
        #![allow(unused)]
        use std::io::{Read, Write};

        pub trait SolarStr {
            fn to_owned_string(&self) -> String;
            fn solar_to_string(&self) -> String;
            fn solar_len(&self) -> i32;
            fn solar_contains(&self, s: impl AsRef<str>) -> bool;
            fn solar_split(&self, s: impl AsRef<str>) -> Vec<String>;
        }

        impl SolarStr for str {
            fn to_owned_string(&self) -> String { self.to_owned() }
            fn solar_to_string(&self) -> String { self.to_owned() }
            fn solar_len(&self) -> i32 { self.len() as i32 }
            fn solar_contains(&self, s: impl AsRef<str>) -> bool { self.contains(s.as_ref()) }
            fn solar_split(&self, s: impl AsRef<str>) -> Vec<String> { self.split(s.as_ref()).map(|x| x.to_owned()).collect() }
        }

        pub trait SolarString {
            fn solar_append(&mut self, s: impl AsRef<str>);
            fn solar_len(&self) -> i32;
            fn solar_contains(&self, s: impl AsRef<str>) -> bool;
            fn as_str(&self) -> &str;
            fn solar_lines(&self) -> Vec<String>;
            fn solar_split(&self, s: impl AsRef<str>) -> Vec<String>;
        }

        impl SolarString for String {
            fn solar_append(&mut self, s: impl AsRef<str>) { self.push_str(s.as_ref()); }
            fn solar_len(&self) -> i32 { self.len() as i32 }
            fn solar_contains(&self, s: impl AsRef<str>) -> bool { self.contains(s.as_ref()) }
            fn as_str(&self) -> &str { self.as_str() }
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

        impl SolarAdd<&str> for String {
            type Output = String;
            fn solar_add(&self, rhs: &&str) -> String { format!("{}{}", self, rhs) }
        }

        impl SolarAdd<String> for String {
            type Output = String;
            fn solar_add(&self, rhs: &String) -> String { format!("{}{}", self, rhs) }
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

        impl<T: Clone> SolarAsVal<T> for T {
            fn as_val(&self) -> T { self.clone() }
        }

        impl<'a, T: Clone> SolarAsVal<T> for &'a T {
            fn as_val(&self) -> T { (*self).clone() }
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
                println!("{:?}", content.as_ref());
            }

            pub fn exit(code: i32) {
                std::process::exit(code);
            }

            pub fn args() -> Vec < String > {
                std::env::args().collect()
            }
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

    fs::write(format!("{}/Cargo.toml", project_dir), cargo_toml)
        .expect("Failed to write Cargo.toml");
    fs::write(format!("{}/src/main.rs", project_dir), tokens.to_string())
        .expect("Failed to write src/main.rs");

    println!("Compiling via Cargo...");
    let status = Command::new("cargo")
        .args(&["build"])
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
                tokens.extend(compile_function(func, None));
            }
            Decl::Impl(imp) => {
                let target = compile_id(&imp.target);
                let gids: Vec<_> = imp
                    .generics
                    .iter()
                    .map(|g| quote::format_ident!("{}", g))
                    .collect();
                let gparams = if imp.generics.is_empty() {
                    quote!()
                } else {
                    quote!(<#( #gids: Clone + Default ),*>)
                };
                let gtarget = if imp.generics.is_empty() {
                    quote!()
                } else {
                    quote!(<#( #gids ),*>)
                };
                let functions = imp
                    .functions
                    .iter()
                    .map(|f| compile_function(f, Some(&imp.target)));

                tokens.extend(quote! { impl #gparams #target #gtarget { #( #functions )* } });

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
                let gens = if obj.generics.is_empty() {
                    quote!()
                } else {
                    let gids = obj.generics.iter().map(|g| quote::format_ident!("{}", g));
                    quote!(<#( #gids: Clone + Default ),*>)
                };
                let fields = obj.fields.iter().map(|f| {
                    let fname = quote::format_ident!("{}", f.name);
                    let fty = compile_type(&f.ty);
                    quote! { pub #fname: #fty }
                });
                let attrs = obj.attributes.iter().map(|a| {
                    let attr = a.parse::<TokenStream>().expect("Failed to parse attribute");
                    quote! { #[#attr] }
                });
                tokens.extend(
                    quote! { #( #attrs )* #[derive(Clone, Debug, Default, PartialEq)] pub struct #name #gens { #( #fields ),* } },
                );
            }
            Decl::Enum(enm) => {
                let name = quote::format_ident!("{}", enm.name);
                let gens = if enm.generics.is_empty() {
                    quote!()
                } else {
                    let gids = enm.generics.iter().map(|g| quote::format_ident!("{}", g));
                    quote!(<#( #gids: Clone + Default ),*>)
                };
                let variants = enm.variants.iter().map(|v| {
                    let vname = quote::format_ident!("{}", v.name);
                    let vtypes = v.types.iter().map(compile_type);
                    if v.types.is_empty() {
                        quote! { #vname }
                    } else {
                        quote! { #vname(#( #vtypes ),*) }
                    }
                });
                let attrs = enm.attributes.iter().map(|a| {
                    let attr = a.parse::<TokenStream>().expect("Failed to parse attribute");
                    quote! { #[#attr] }
                });
                tokens.extend(quote! { #( #attrs )* #[derive(Clone, Debug, PartialEq)] pub enum #name #gens { #( #variants ),* } });
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
                    let gids = obj.generics.iter().map(|g| quote::format_ident!("{}", g));
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
                    let gids = enm.generics.iter().map(|g| quote::format_ident!("{}", g));
                    quote!(<#( #gids ),*>)
                };
                tokens.extend(quote! {
                    pub type #name #gens = #target_path #gens;
                });
            }
            Decl::ExternImpl(_) | Decl::RustDependency(_, _) => {}
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
                    mod_tokens = quote! { pub mod #id { use std; use crate::{SolarStr, SolarString, SolarVec, SolarAdd, SolarSub, SolarMul, SolarDiv, SolarGT, SolarLT, SolarEq, SolarI32, SolarI64, SolarF32, SolarF64, SolarAsArg, SolarAsVal, SolarAsSize, sr_math, sr_io, sr_fs}; #mod_tokens } };
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
