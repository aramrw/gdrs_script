use crate::ast::*;
use proc_macro2::TokenStream;
use quote::quote;
use std::fs;
use std::process::Command;

fn compile_id(name: &str) -> TokenStream {
    if name.contains("::") {
        let mut parts: Vec<&str> = name.split("::").collect();
        
        // Strip 'std' from our known internal std modules.
        if parts.get(0) == Some(&"std") && parts.len() > 1 {
            let second = parts[1];
            if second == "util" || second == "vec" {
                parts.remove(0);
            }
        }

        // If it's a root-level module we defined (mem, fs, math, util, vec), 
        // prepend crate:: so it's accessible from anywhere.
        let first = parts[0];
        let root_modules = ["mem", "fs", "math", "util", "vec"];
        if root_modules.contains(&first) {
            let tokens: Vec<TokenStream> = parts.iter().map(|p| {
                let id = quote::format_ident!("{}", p);
                quote!(#id)
            }).collect();
            return quote!(crate::#( #tokens )::*);
        }

        let tokens: Vec<TokenStream> = parts.iter().map(|p| {
            let id = quote::format_ident!("{}", p);
            quote!(#id)
        }).collect();
        quote!(#( #tokens )::*)
    } else {
        // If it's a known root-level module, also prepend crate::
        let root_modules = ["mem", "fs", "math", "util", "vec"];
        if root_modules.contains(&name) {
            let id = quote::format_ident!("{}", name);
            return quote!(crate::#id);
        }
        let id = quote::format_ident!("{}", name);
        quote!(#id)
    }
}

fn compile_type(ty: &Type) -> TokenStream {
    match ty {
        Type::I32 => quote!(i32),
        Type::F32 => quote!(f32),
        Type::Bool => quote!(bool),
        Type::Str => quote!(&str),
        Type::String => quote!(String),
        Type::File => quote!(std::fs::File),
        Type::BoxPtr(inner) => {
            let t = compile_type(inner);
            quote!(Box<#t>)
        }
        Type::RawPtr(inner, mutable) => {
            let t = compile_type(inner);
            if *mutable { quote!(*mut #t) } else { quote!(*const #t) }
        }
        Type::Array(inner, _) => {
            let t = compile_type(inner);
            quote!(Vec<#t>)
        }
        Type::Generic(name) => {
            let id = quote::format_ident!("{}", name);
            quote!(#id)
        }
        Type::Custom(name, generics) => {
            let id = compile_id(name);
            if generics.is_empty() { quote!(#id) }
            else {
                let gens = generics.iter().map(compile_type);
                quote!(#id<#( #gens ),*>)
            }
        }
        Type::SelfType => quote!(Self),
    }
}

fn compile_pattern(pat: &Pattern) -> TokenStream {
    match pat {
        Pattern::Variant(enm, var, params) => {
            let eid = compile_id(enm);
            let vid = quote::format_ident!("{}", var);
            let pids = params.iter().map(|p| quote::format_ident!("{}", p));
            if params.is_empty() { quote! { #eid::#vid } }
            else { quote! { #eid::#vid(#( #pids ),*) } }
        }
        Pattern::Variable(n) => { let id = quote::format_ident!("{}", n); quote! { #id } }
        Pattern::Literal(e) => compile_expr(e, None),
    }
}

fn compile_expr(expr: &Expr, target_obj: Option<&String>) -> TokenStream {
    match expr {
        Expr::Int(v) => quote! { #v },
        Expr::Float(v) => quote! { #v as f32 },
        Expr::Bool(v) => quote! { #v },
        Expr::String(v) => quote! { #v },
        Expr::Variable(n) => {
            let id = if n == "self" { quote!(self) } else { let id = quote::format_ident!("{}", n); quote!(#id) };
            quote! { #id }
        }
        Expr::Binary(lhs, op, rhs) => {
            let lhs = compile_expr(lhs, target_obj);
            let rhs = compile_expr(rhs, target_obj);
            match op {
                BinaryOp::Add => quote! { (#lhs + #rhs) },
                BinaryOp::Subtract => quote! { (#lhs - #rhs) },
                BinaryOp::Multiply => quote! { (#lhs * #rhs) },
                BinaryOp::Divide => quote! { (#lhs / #rhs) },
                BinaryOp::GreaterThan => quote! { (#lhs > #rhs) },
                BinaryOp::LessThan => quote! { (#lhs < #rhs) },
            }
        }
        Expr::Call(name, args) => {
            if name == "array_init" {
                let size = compile_expr(&args[1], target_obj);
                return quote! { unsafe {
                    let mut v: Vec<_> = (0..#size).map(|_| std::mem::zeroed()).collect();
                    let p = v.as_mut_ptr();
                    std::mem::forget(v);
                    p
                } };
            }
            let id = compile_id(name);
            let args = args.iter().map(|a| compile_expr(a, target_obj));
            quote! { #id(#( #args ),*) }
        }
        Expr::MethodCall(lhs, name, args) => {
            let l = compile_expr(lhs, target_obj);
            let id = quote::format_ident!("{}", name);
            let args = args.iter().map(|a| compile_expr(a, target_obj));
            quote! { #l.#id(#( #args ),*) }
        }
        Expr::StructLiteral { name, fields } => {
            let target_name = if name == "self" {
                if let Some(t) = target_obj { t.clone() }
                else { "Self".to_string() }
            } else { name.clone() };
            let id = compile_id(&target_name);
            let fields = fields.iter().map(|(n, v)| {
                let fname = quote::format_ident!("{}", n);
                let fval = compile_expr(v, target_obj);
                quote! { #fname: #fval }
            });
            quote! { #id { #( #fields ),* } }
        }
        Expr::MemberAccess(lhs, name) => {
            let l = compile_expr(lhs, target_obj);
            let id = quote::format_ident!("{}", name);
            quote! { #l.#id }
        }
        Expr::IndexAccess(lhs, index) => {
            let l = compile_expr(lhs, target_obj);
            let i = compile_expr(index, target_obj);
            quote! { (unsafe { *#l.add(#i as usize) }) }
        }
        Expr::Alloc(inner, kind) => {
            let e = compile_expr(inner, target_obj);
            match kind {
                AllocKind::Box => quote! { Box::new(#e) },
                AllocKind::RawMut => quote! { #e },
                AllocKind::RawConst => quote! { #e },
            }
        }
    }
}

fn compile_stmt(stmt: &Stmt, is_last: bool, target_obj: Option<&String>) -> TokenStream {
    match stmt {
        Stmt::VarDecl { name, is_mutable, ty, value } => {
            let id = quote::format_ident!("{}", name);
            let mut_kw = if *is_mutable { quote!(mut) } else { quote!() };
            let val = compile_expr(value, target_obj);
            let ty_tokens = match ty {
                Some(t) => { let ct = compile_type(t); quote!(: #ct) }
                None => quote!(),
            };
            quote! { let #mut_kw #id #ty_tokens = #val; }
        }
        Stmt::Assign { target, value } => {
            let v = compile_expr(value, target_obj);
            if let Expr::IndexAccess(lhs, index) = target {
                let l = compile_expr(lhs, target_obj);
                let i = compile_expr(index, target_obj);
                quote! { unsafe { *#l.add(#i as usize) = #v; } }
            } else {
                let t = compile_expr(target, target_obj);
                quote! { #t = #v; }
            }
        }
        Stmt::Print(expr) => {
            let e = compile_expr(expr, target_obj);
            quote! { println!("{:?}", #e); }
        }
        Stmt::If { condition, then_branch, else_branch } => {
            let cond = compile_expr(condition, target_obj);
            let then = compile_stmt(then_branch, is_last, target_obj);
            match else_branch {
                Some(eb) => {
                    let eb_tokens = compile_stmt(eb, is_last, target_obj);
                    quote! { if #cond #then else #eb_tokens }
                }
                None => quote! { if #cond #then },
            }
        }
        Stmt::While { condition, body } => {
            let cond = compile_expr(condition, target_obj);
            let b = compile_stmt(body, false, target_obj);
            quote! { while #cond #b }
        }
        Stmt::Block(stmts) => {
            let len = stmts.len();
            let tokens = stmts.iter().enumerate().map(|(i, s)| compile_stmt(s, is_last && i == len - 1, target_obj));
            quote! { { #( #tokens )* } }
        }
        Stmt::ExprStmt(expr) => {
            let e = compile_expr(expr, target_obj);
            if is_last { quote! { return #e; } } else { quote! { #e; } }
        }
        Stmt::Return(expr) => {
            let e = expr.as_ref().map(|e| compile_expr(e, target_obj));
            if let Some(tokens) = e { quote! { return #tokens; } }
            else { quote! { return; } }
        }
        Stmt::Match { expr, arms } => {
            let e = compile_expr(expr, target_obj);
            let arm_tokens = arms.iter().map(|arm| {
                let pat = compile_pattern(&arm.pattern);
                let body = compile_stmt(&arm.body, is_last, target_obj);
                quote! { #pat => { #body } }
            });
            quote! { match #e { #( #arm_tokens ),* } }
        }
    }
}

fn compile_function(func: &Function, target_obj: Option<&String>) -> TokenStream {
    let name = quote::format_ident!("{}", func.name);
    let gens = if func.generics.is_empty() { quote!() } else {
        let gids = func.generics.iter().map(|g| quote::format_ident!("{}", g));
        quote!(<#( #gids: Clone + Copy ),*>)
    };
    let params = func.params.iter().map(|p| {
        let p_name = quote::format_ident!("{}", p.name);
        let p_ty = compile_type(&p.ty);
        if p.name == "self" {
            if p.is_mutable { quote!(&mut self) } else { quote!(&self) }
        } else {
            let mut_kw = if p.is_mutable { quote!(mut) } else { quote!() };
            quote! { #mut_kw #p_name: #p_ty }
        }
    });
    let ret_type = match &func.return_type {
        Some(ty) => { let t = compile_type(ty); quote!(-> #t) }
        None => quote!(),
    };
    let body = compile_stmt(&func.body, true, target_obj);
    quote! { pub fn #name #gens (#( #params ),*) #ret_type #body }
}

fn compile_extern_function(_func: &Function) -> TokenStream {
    quote!()
}

pub fn compile(program: Program) {
    let mut tokens = TokenStream::new();
    tokens.extend(quote! {
        #![allow(unused)]
        use std::io::{Read, Write};

        pub trait SolarStr {
            fn to_owned_string(&self) -> String;
            fn len(&self) -> i32;
        }
        impl SolarStr for str {
            fn to_owned_string(&self) -> String { self.to_owned() }
            fn len(&self) -> i32 { self.len() as i32 }
        }

        pub trait SolarString {
            fn append(&mut self, s: &str);
            fn len(&self) -> i32;
        }
        impl SolarString for String {
            fn append(&mut self, s: &str) { self.push_str(s); }
            fn len(&self) -> i32 { self.len() as i32 }
        }

        pub mod mem {
            pub fn free<T>(p: *mut T, size: i32) {
                unsafe {
                    let _ = Vec::from_raw_parts(p, 0, size as usize);
                }
            }
        }

        pub mod fs {
            use std::io::{Read, Write};

            pub fn create(path: &str) -> std::fs::File {
                std::fs::File::create(path).expect("Failed to create file")
            }

            pub fn open(path: &str) -> std::fs::File {
                std::fs::File::open(path).expect("Failed to open file")
            }

            pub fn read(mut f: std::fs::File) -> String {
                let mut s = String::new();
                f.read_to_string(&mut s).expect("Failed to read file");
                s
            }

            pub fn write(mut f: std::fs::File, content: &str) {
                f.write_all(content.as_bytes()).expect("Failed to write file");
            }

            pub fn write_string(mut f: std::fs::File, content: String) {
                f.write_all(content.as_bytes()).expect("Failed to write file");
            }
        }
    });

    compile_decls(&program.declarations, &mut tokens);

    fs::write("output.rs", tokens.to_string()).expect("Failed to write Rust code");
    
    // Create a temporary cargo project
    let project_dir = "solar_out";
    fs::create_dir_all(format!("{}/src", project_dir)).ok();
    
    let cargo_toml = r#"
[package]
name = "solar_out"
version = "0.1.0"
edition = "2021"

[dependencies]
# Dependencies will be added here
"#;
    
    fs::write(format!("{}/Cargo.toml", project_dir), cargo_toml).expect("Failed to write Cargo.toml");
    fs::write(format!("{}/src/main.rs", project_dir), tokens.to_string()).expect("Failed to write src/main.rs");

    println!("Compiling via Cargo...");
    let status = Command::new("cargo")
        .args(&["build"])
        .current_dir(project_dir)
        .status()
        .expect("Failed to invoke cargo.");

    if status.success() {
        // Copy the binary back to the root
        let binary_name = if cfg!(windows) { "solar_out.exe" } else { "solar_out" };
        fs::copy(format!("{}/target/debug/{}", project_dir, binary_name), "main_program").ok();
    } else {
        eprintln!("Cargo compilation failed.");
    }
}

fn compile_decls(decls: &[Decl], tokens: &mut TokenStream) {
    for decl in decls {
        match decl {
            Decl::Function(func) => tokens.extend(compile_function(&func, None)),
            Decl::ExternFunction(func) => tokens.extend(compile_extern_function(&func)),
            Decl::ExternObject(_) => {}, 
            Decl::ExternEnum(_) => {},
            Decl::ExternImpl(_) => {},
            Decl::Module(name, inner) => {
                let mut inner_tokens = TokenStream::new();
                compile_decls(inner, &mut inner_tokens);
                
                let mut mod_tokens = inner_tokens;
                let mut parts: Vec<&str> = name.split("::").collect();
                
                // If it starts with 'std', remove it.
                if parts.get(0) == Some(&"std") {
                    parts.remove(0);
                }
                
                for part in parts.into_iter().rev() {
                    let id = quote::format_ident!("{}", part);
                    mod_tokens = quote! { pub mod #id { #mod_tokens } };
                }
                tokens.extend(mod_tokens);
            }
            Decl::Use(_) => {}, 
            Decl::Object(obj) => {
                let name = quote::format_ident!("{}", obj.name);
                let gens = if obj.generics.is_empty() { quote!() } else {
                    let gids = obj.generics.iter().map(|g| quote::format_ident!("{}", g));
                    quote!(<#( #gids: Clone + Copy ),*>)
                };
                let fields = obj.fields.iter().map(|f| {
                    let fname = quote::format_ident!("{}", f.name);
                    let fty = compile_type(&f.ty);
                    quote! { pub #fname: #fty }
                });
                tokens.extend(quote! { #[derive(Clone, Copy, Debug)] pub struct #name #gens { #( #fields ),* } });
            }
            Decl::Enum(enm) => {
                let name = quote::format_ident!("{}", enm.name);
                let gens = if enm.generics.is_empty() { quote!() } else {
                    let gids = enm.generics.iter().map(|g| quote::format_ident!("{}", g));
                    quote!(<#( #gids: Clone + Copy ),*>)
                };
                let variants = enm.variants.iter().map(|v| {
                    let vname = quote::format_ident!("{}", v.name);
                    let vtypes = v.types.iter().map(compile_type);
                    if v.types.is_empty() { quote! { #vname } }
                    else { quote! { #vname(#( #vtypes ),*) } }
                });
                tokens.extend(quote! { #[derive(Clone, Copy, Debug, PartialEq)] pub enum #name #gens { #( #variants ),* } });
            }
            Decl::Impl(imp) => {
                let target = compile_id(&imp.target);
                let gids = imp.generics.iter().map(|g| quote::format_ident!("{}", g));
                let gparams = if imp.generics.is_empty() { quote!() } else { quote!(<#( #gids: Clone + Copy ),*>) };
                let gtarget = if imp.generics.is_empty() { quote!() } else {
                    let gids2 = imp.generics.iter().map(|g| quote::format_ident!("{}", g));
                    quote!(<#( #gids2 ),*>)
                };
                let functions = imp.functions.iter().map(|f| compile_function(f, Some(&imp.target)));
                tokens.extend(quote! { impl #gparams #target #gtarget { #( #functions )* } });
            }
        }
    }
}
