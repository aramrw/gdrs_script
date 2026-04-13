use crate::ast::*;
use proc_macro2::TokenStream;
use quote::quote;
use std::fs;
use std::process::Command;

fn compile_type(ty: &Type) -> TokenStream {
    match ty {
        Type::I32 => quote!(i32),
        Type::F32 => quote!(f32),
        Type::Bool => quote!(bool),
        Type::Str => quote!(String),
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
            let id = quote::format_ident!("{}", name);
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
            let eid = quote::format_ident!("{}", enm);
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
        Expr::String(v) => quote! { #v.to_string() },
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
            let id = if name.contains("::") {
                let parts: Vec<&str> = name.split("::").collect();
                let obj = quote::format_ident!("{}", parts[0]);
                let meth = quote::format_ident!("{}", parts[1]);
                quote!(#obj::#meth)
            } else {
                let id = quote::format_ident!("{}", name);
                quote!(#id)
            };
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
            let id = if name == "self" {
                let t = target_obj.expect("Cannot use self outside impl");
                quote::format_ident!("{}", t)
            } else {
                quote::format_ident!("{}", name)
            };
            let fields = fields.iter().map(|(n, v)| {
                let fid = quote::format_ident!("{}", n);
                let fval = compile_expr(v, target_obj);
                quote! { #fid: #fval }
            });
            quote! { #id { #( #fields ),* } }
        }
        Expr::MemberAccess(lhs, name) => {
            let lhs_tokens = compile_expr(lhs, target_obj);
            let fid = quote::format_ident!("{}", name);
            quote! { #lhs_tokens.#fid }
        }
        Expr::IndexAccess(lhs, index) => {
            let lhs = compile_expr(lhs, target_obj);
            let idx = compile_expr(index, target_obj);
            quote! { unsafe { *#lhs.add(#idx as usize) } }
        }
        Expr::Alloc(inner, kind) => {
            // SPECIAL CASE: If we are already doing a raw allocation (like array_init), don't wrap it!
            if let Expr::Call(name, _) = &**inner {
                if name == "array_init" { return compile_expr(inner, target_obj); }
            }
            let val = compile_expr(inner, target_obj);
            match kind {
                AllocKind::Box => quote! { Box::new(#val) },
                AllocKind::RawMut => quote! { Box::into_raw(Box::new(#val)) },
                AllocKind::RawConst => quote! { Box::into_raw(Box::new(#val)) as *const _ },
            }
        }
    }
}

fn compile_stmt(stmt: &Stmt, is_last: bool, target_obj: Option<&String>) -> TokenStream {
    match stmt {
        Stmt::VarDecl { name, is_mutable, value, .. } => {
            let id = quote::format_ident!("{}", name);
            let val = compile_expr(value, target_obj);
            let mut_kw = if *is_mutable { quote!(mut) } else { quote!() };
            quote! { let #mut_kw #id = #val; }
        }
        Stmt::Assign { target, value } => {
            let v = compile_expr(value, target_obj);
            if let Expr::IndexAccess(lhs, idx) = target {
                let l = compile_expr(lhs, target_obj);
                let i = compile_expr(idx, target_obj);
                quote! { unsafe { *#l.add(#i as usize) = #v; } }
            } else {
                let t = compile_expr(target, target_obj);
                quote! { #t = #v; }
            }
        }
        Stmt::Print(expr) => {
            let val = compile_expr(expr, target_obj);
            quote! { println!("{:?}", #val); }
        }
        Stmt::If { condition, then_branch, else_branch } => {
            let cond = compile_expr(condition, target_obj);
            let then_tokens = compile_stmt(then_branch, is_last, target_obj);
            let then_block = if matches!(**then_branch, Stmt::Block(_)) { then_tokens } else { quote! { { #then_tokens } } };
            let else_tokens = else_branch.as_ref().map(|b| {
                let t = compile_stmt(b, is_last, target_obj);
                if matches!(**b, Stmt::Block(_)) { t } else { quote! { { #t } } }
            });
            let else_final = if let Some(et) = else_tokens {
                if et.to_string().starts_with("else") { et } else { quote! { else #et } }
            } else { quote! {} };
            quote! { if #cond #then_block #else_final }
        }
        Stmt::While { condition, body } => {
            let cond = compile_expr(condition, target_obj);
            let body_tokens = compile_stmt(body, false, target_obj);
            let body_block = if matches!(**body, Stmt::Block(_)) { body_tokens } else { quote! { { #body_tokens } } };
            quote! { while #cond #body_block }
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
    quote! { fn #name #gens (#( #params ),*) #ret_type #body }
}

pub fn compile(program: Program) {
    let mut tokens = TokenStream::new();
    tokens.extend(quote! { #![allow(unused)] });

    for decl in program.declarations {
        match decl {
            Decl::Function(func) => tokens.extend(compile_function(&func, None)),
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
                let target = quote::format_ident!("{}", imp.target);
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

    fs::write("output.rs", tokens.to_string()).expect("Failed to write Rust code");
    let status = Command::new("rustc").args(&["-C", "opt-level=3", "output.rs", "-o", "main_program"]).status().expect("Failed to invoke rustc.");
    if !status.success() { eprintln!("Rust compilation failed."); }
}
