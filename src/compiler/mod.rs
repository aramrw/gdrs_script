pub mod declarations;
pub mod expressions;
pub mod functions;
pub mod paths;
pub mod statements;
pub mod types;

use lazy_static::lazy_static;
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::sync::Mutex;

use crate::ast::*;
use crate::codegen::stdlib::generate_solar_std;
use crate::compiler::declarations::{collect_decl_metadata, compile_decls};
use crate::compiler::expressions::compile_expr;
use crate::compiler::statements::compile_stmt;
use crate::compiler::types::compile_type_ext;
use crate::sema::TypeInfo;

lazy_static! {
    pub(crate) static ref RUST_MAPPINGS: Mutex<HashMap<String, String>> =
        Mutex::new(HashMap::new());
}

pub fn compile_id(name: &str) -> TokenStream {
    compile_id_ext(name, false)
}
fn compile_id_expr(name: &str) -> TokenStream {
    compile_id_ext(name, true)
}

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Compute a hash of the Rust code and Cargo.toml
fn compute_cache_key(rust_code: &str, cargo_toml: &str) -> String {
    let mut hasher = DefaultHasher::new();
    rust_code.hash(&mut hasher);
    cargo_toml.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Check if we have a valid cache and return true if rebuild is needed
fn should_rebuild(project_dir: &str, cache_key: &str) -> bool {
    let cache_file = format!("{}/.solar_cache", project_dir);
    if let Ok(cached_key) = fs::read_to_string(&cache_file) {
        cached_key.trim() != cache_key
    } else {
        true // No cache file = always rebuild first time
    }
}

/// Write the cache key after successful build
fn save_cache(project_dir: &str, cache_key: &str) {
    let cache_file = format!("{}/.solar_cache", project_dir);
    fs::write(&cache_file, cache_key).ok();
}

/// Finds the contents of matching brackets, respecting nested depth.
/// Example:
/// "HashMap<String, Vec<i32>>::iter"
/// -> Some(("HashMap", "String, Vec<i32>", "::iter"))
pub(crate) fn extract_brackets<'a>(
    input: &'a str,
    open: char,
    close: char,
) -> Option<(&'a str, &'a str, &'a str)> {
    let start_idx = input.find(open)?;
    let mut depth = 0;

    for (i, c) in input[start_idx..].char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                let absolute_idx = start_idx + i;
                return Some((
                    // Before the '<'
                    &input[..start_idx],
                    // Inside the '<...>'
                    &input[start_idx + 1..absolute_idx],
                    // After the '>'
                    &input[absolute_idx + 1..],
                ));
            }
        }
    }
    None
}

/// Splits a comma-separated string, but ignores commas inside nested `< >` or `( )`.
/// Example: "String, Vec<i32, f32>" -> ["String", "Vec<i32, f32>"]
pub(crate) fn parse_generic_args(args_str: &str) -> Vec<TokenStream> {
    let mut tokens = Vec::new();
    let mut current_arg = String::new();
    let mut depth = 0;

    for c in args_str.chars() {
        match c {
            '<' | '(' => depth += 1,
            '>' | ')' => depth -= 1,
            ',' if depth == 0 => {
                tokens.push(parse_single_generic(&current_arg));
                current_arg.clear();
                continue;
            }
            _ => {}
        }
        current_arg.push(c);
    }

    if !current_arg.trim().is_empty() {
        tokens.push(parse_single_generic(&current_arg));
    }

    tokens
}

fn parse_single_generic(s: &str) -> TokenStream {
    let s = s.trim();
    if s.is_empty() {
        return quote!();
    }
    if s.chars().all(|c| c.is_alphanumeric() || c == '_') {
        let gid = quote::format_ident!("{}", s);
        quote!(#gid)
    } else {
        s.parse::<TokenStream>().unwrap_or_else(|_| {
            let gid = quote::format_ident!("{}", s);
            quote!(#gid)
        })
    }
}

pub fn has_clone(ty: &Type, type_info: &TypeInfo) -> bool {
    match ty {
        Type::I32 | Type::I64 | Type::F32 | Type::F64 | Type::Bool => true,
        Type::Str => true,
        Type::Custom(name, _) => {
            if let Some((_, _, attrs)) = type_info.objects.get(name) {
                attrs.iter().any(|a| a.contains("Clone"))
            } else if let Some((_, _, attrs)) = type_info.enums.get(name) {
                attrs.iter().any(|a| a.contains("Clone"))
            } else {
                // If we don't know, we assume it's NOT cloneable for safety,
                // UNLESS it's a known cloneable type like Vec
                if name.contains("Vector") || name.contains("Vec") {
                    return true;
                }
                false
            }
        }
        Type::Generic(_) => true, // Generics are required to be Clone in Solar
        Type::Result(ok, err) => has_clone(ok, type_info) && has_clone(err, type_info),
        Type::Tuple(types) => types.iter().all(|t| has_clone(t, type_info)),
        Type::Array(inner, _) => has_clone(inner, type_info),
        _ => false,
    }
}

pub(crate) fn compile_id_ext(name: &str, is_expr: bool) -> TokenStream {
    let name = name.trim();
    if name.is_empty() {
        return quote!();
    }

    // 1. Resolve nested generics safely
    if let Some((prefix, gens_str, suffix)) = extract_brackets(name, '<', '>') {
        let id = compile_id_ext(prefix, is_expr);
        let gens_tokens = parse_generic_args(gens_str);

        let has_any = gens_str.contains("Any");

        let sep = if is_expr && !gens_tokens.is_empty() && !has_any {
            quote!(::)
        } else {
            quote!()
        };

        let mut res = if gens_tokens.is_empty() || has_any {
            quote!(#id)
        } else {
            quote!(#id #sep <#( #gens_tokens ),*>)
        };

        if !suffix.trim().is_empty() {
            let suffix_clean = suffix.trim_start_matches("::");
            let parsed_suffix = compile_id_ext(suffix_clean, is_expr);
            res = quote!(#res::#parsed_suffix);
        }
        return res;
    }

    // 2. Fallback for raw rust types (arrays, references, etc.)
    if name.contains('(') || name.contains('[') || name.contains('*') {
        return name
            .parse()
            .expect("Failed to parse complex rust path as TokenStream");
    }

    // 3. Check explicit RUST_MAPPINGS
    if let Ok(mappings) = RUST_MAPPINGS.lock() {
        if name.contains("pi") {
            println!(
                "DEBUG compile_id_ext name={}, mappings has std::math::pi={:?}",
                name,
                mappings.get("std::math::pi")
            );
        }
        // adjust this import if needed
        // Exact Match
        if let Some(rust_path) = mappings.get(name) {
            return rust_path
                .parse()
                .expect("Failed to parse rust mapping as TokenStream");
        }

        // Prefix Match (module::Type::Variant -> module::Type matched, append Variant)
        let parts: Vec<&str> = name.split("::").collect();
        for i in (1..parts.len()).rev() {
            let prefix = parts[..i].join("::");
            if let Some(rust_prefix) = mappings.get(&prefix) {
                if rust_prefix == &prefix {
                    continue;
                }
                let rest_tokens = crate::compiler::paths::format_path_parts(&parts[i..]);
                let prefix_tokens: TokenStream = rust_prefix.parse().unwrap();
                return quote!(#prefix_tokens::#( #rest_tokens )::*);
            }
        }
    }

    // Known Solar standard library root modules
    let root_modules = [
        "mem",
        "sr_fs",
        "sr_io",
        "sr_math",
        "sr_string",
        "string",
        "util",
        "vec",
        "option",
    ];

    // 4. Resolve namespaces (::)
    if name.contains("::") {
        let is_absolute = name.starts_with("::");
        let name_trimmed = name.trim_start_matches("::");
        let parts: Vec<&str> = name_trimmed.split("::").collect();
        let first = parts[0];
        let rest_tokens = crate::compiler::paths::format_path_parts(&parts[1..]);

        let mut res = match first {
            // ZERO-COST ABSTRACTIONS: Direct access to Rust's libraries!
            "rust" => quote!(::#( #rest_tokens )::*),

            // Solar standard library extensions
            "stext" => quote!(crate::#( #rest_tokens )::*),

            // Solar standard library mapped directly to the active crate
            "solar" => quote!(crate::#( #rest_tokens )::*),

            // If it hits one of Solar's root modules, enforce the `crate::` prefix
            _ if root_modules.contains(&first) => {
                let mapped_first = match first {
                    "string" => "sr_string",
                    "fs" => "sr_fs",
                    "io" => "sr_io",
                    "math" => "sr_math",
                    _ => first,
                };
                let first_id = quote::format_ident!("{}", mapped_first);
                quote!(crate::#first_id::#( #rest_tokens )::*)
            }

            // Standard namespace resolution
            _ => {
                let first_id = quote::format_ident!("{}", first);
                quote!(#first_id::#( #rest_tokens )::*)
            }
        };

        if is_absolute && !res.to_string().starts_with("::") {
            res = quote!(::#res);
        }
        return res;
    } else {
        // 5. Single identifiers (no `::`)
        let clean_name = name.trim_end_matches("<>");
        if clean_name.is_empty() {
            return quote!();
        }

        let id = quote::format_ident!("{}", clean_name);

        if root_modules.contains(&clean_name) {
            quote!(crate::#id)
        } else {
            quote!(#id)
        }
    }
}

fn path_to_string(path: &[PathPart]) -> String {
    path.iter()
        .map(|p| p.name.clone())
        .collect::<Vec<_>>()
        .join("::")
}

pub fn compile_path(path: &[PathPart], target_obj: Option<&String>, is_expr: bool) -> TokenStream {
    let mut parts: Vec<TokenStream> = Vec::new();
    let mut start_idx = 0;

    if let Some(first) = path.first() {
        if first.name == "stext" {
            parts.push(quote!(crate));
            start_idx = 1;
        } else if first.name == "rust" {
            // "rust::" prefix means use native rust path directly, optionally keeping "std::"
            // We just skip the "rust" part. The rest will be formatted as a normal rust path.
            start_idx = 1;
        }
    }

    for (i, p) in path.iter().enumerate().skip(start_idx) {
        if p.name.is_empty() {
            continue;
        }

        let name_to_use = match p.name.as_str() {
            "string" => "sr_string",
            "fs" => "sr_fs",
            "io" => "sr_io",
            "math" => "sr_math",
            _ => &p.name,
        };
        let name = quote::format_ident!("{}", name_to_use);
        if p.generics.is_empty() || p.generics.iter().any(|g| matches!(g, Type::Any)) {
            parts.push(quote!(#name));
        } else {
            let gens = p.generics.iter().map(|g| compile_type_ext(g, target_obj));
            if is_expr && i == path.len() - 1 {
                parts.push(quote!(#name::<#( #gens ),*>));
            } else {
                parts.push(quote!(#name<#( #gens ),*>));
            }
        }
    }

    quote!(#( #parts )::*)
}

fn compile_type(ty: &Type) -> TokenStream {
    compile_type_ext(ty, None)
}

fn compile_pattern(pat: &Pattern, type_info: &TypeInfo) -> TokenStream {
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
        Pattern::Literal(lit) => compile_expr(lit, None, false, type_info),
    }
}

fn wrap_expr_for_ref(
    expr: &Expr,
    expected_ty: Option<&Type>,
    target_obj: Option<&String>,
    mutable_expr: bool,
    arg_kind: ArgKind,
    type_info: &TypeInfo,
) -> TokenStream {
    let tokens = compile_expr(expr, target_obj, mutable_expr, type_info); // Use mutable_expr here

    if let Some(ty) = &expr.ty {
        match ty {
            Type::Managed(_) | Type::ThreadSafe(_) | Type::BoxPtr(_) => {
                // If we have a pointer, but the function expects the inner type, auto-deref.
                let is_ptr_expected = expected_ty.map_or(false, |et| {
                    matches!(et, Type::Managed(_) | Type::ThreadSafe(_) | Type::BoxPtr(_))
                });

                if !is_ptr_expected {
                    match ty {
                        Type::Managed(_) => {
                            if arg_kind == ArgKind::MutRef {
                                // Use arg_kind here
                                quote! { &mut *#tokens.borrow_mut() }
                            } else {
                                quote! { &*#tokens.borrow() }
                            }
                        }
                        Type::ThreadSafe(_) => {
                            if arg_kind == ArgKind::MutRef {
                                // Use arg_kind here
                                quote! { &mut *#tokens.write() }
                            } else {
                                quote! { &*#tokens.read() }
                            }
                        }
                        Type::BoxPtr(_) => {
                            if arg_kind == ArgKind::MutRef {
                                // Use arg_kind here
                                quote! { &mut **#tokens }
                            } else {
                                quote! { &**#tokens }
                            }
                        }
                        _ => {
                            // For other pointer types or just to generate a reference to the content
                            if arg_kind == ArgKind::MutRef {
                                quote! { &mut #tokens }
                            } else {
                                quote! { &#tokens }
                            }
                        }
                    }
                } else {
                    // Function explicitly expects the pointer type, so pass a reference to it.
                    if arg_kind == ArgKind::MutRef {
                        // Use arg_kind here
                        quote! { &mut #tokens }
                    } else {
                        quote! { &#tokens }
                    }
                }
            }
            Type::Ref(_, _) => tokens, // Already a ref
            _ => {
                // This is the case for non-pointer, non-reference types
                match arg_kind {
                    ArgKind::Value => {
                        let ty = expr.ty.as_ref().unwrap_or(&Type::Any);
                        let ct = compile_type_ext(ty, target_obj);
                        match ty {
                            Type::Str => {
                                // If it's a Solar string literal and a Solar String type is expected (by value)
                                if expected_ty.map_or(false, |et| matches!(et, Type::Str)) {
                                    quote! { (&#tokens).solar_to_string() }
                                } else {
                                    // If a Str type is expected by value (e.g., &str), use solar_to_str
                                    quote! { (&#tokens).solar_to_str() }
                                }
                            }
                            Type::Custom(n, _) if n.contains("Option") || n.contains("Result") => {
                                quote! { (#tokens) }
                            }
                            Type::Result(_, _) => quote! { (#tokens) },
                            _ if has_clone(ty, type_info) => {
                                quote! { <_ as crate::SolarAsVal<#ct>>::as_val(&#tokens) }
                            }
                            _ => quote! { (#tokens) },
                        }
                    }
                    ArgKind::Ref => quote! { &#tokens },
                    ArgKind::MutRef => quote! { &mut #tokens },
                }
            }
        }
    } else {
        // If expr.ty is None, meaning the type is unknown
        match arg_kind {
            ArgKind::Value => quote! { (#tokens) }, // Pass by value
            ArgKind::Ref => quote! { &#tokens },
            ArgKind::MutRef => quote! { &mut #tokens },
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

pub fn compile(program: Program, output_name: &str, type_info: &TypeInfo) {
    let mut tokens = TokenStream::new();
    let mut dependencies = Vec::new();
    let mut use_tokio = false;

    // Reset and collect mappings
    if let Ok(mut mappings) = RUST_MAPPINGS.lock() {
        mappings.clear();
        mappings.insert("string".to_string(), "crate::sr_string".to_string());
        mappings.insert("String".to_string(), "crate::sr_string".to_string());
        collect_decl_metadata(
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
    tokens.extend(generate_solar_std(has_macroquad));

    compile_decls(&program.declarations, &mut tokens, type_info);

    fs::write("output.rs", tokens.to_string()).expect("Failed to write Rust code");

    // Create a temporary cargo project
    let project_dir = "solar_out";
    fs::create_dir_all(format!("{}/src", project_dir)).ok();

    let mut cargo_toml = String::from(
        r#"cargo-features = ["codegen-backend"]

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
    // Add profile optimizations for faster compilation
    cargo_toml.push_str(
        r#"
[profile.dev]
codegen-backend = "cranelift"
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

    let rust_code = tokens.to_string();

    // ===== CACHE CHECK =====
    let cache_key = compute_cache_key(&rust_code, &cargo_toml);
    let rebuild_needed = should_rebuild(project_dir, &cache_key);
    // ===== END CACHE CHECK =====

    fs::write(format!("{}/Cargo.toml", project_dir), &cargo_toml)
        .expect("Failed to write Cargo.toml");

    fs::write(format!("{}/src/main.rs", project_dir), &rust_code)
        .expect("Failed to write src/main.rs");

    // Format the generated code for better error messages
    Command::new("cargo")
        .args(&["+nightly", "fmt"])
        .current_dir(project_dir)
        .status()
        .ok();

    // Only run cargo if code or dependencies changed
    if rebuild_needed {
        build_args(output_name, has_macroquad, project_dir, &cache_key);
    } else {
        println!("+=[Cache Hit] Skipping cargo build");
        // Binary already exists from previous build, just verify it
        let dst_binary = output_name;
        if !std::path::Path::new(dst_binary).exists() {
            // If binary doesn't exist but cache says it's valid, force rebuild
            println!("+=[Cargo] Binary missing, rebuilding...");
            build_args(output_name, has_macroquad, project_dir, &cache_key);
        }
    }
}

fn build_args<'a>(output_name: &str, has_macroquad: bool, project_dir: &str, cache_key: &str) {
    println!("+=[Cargo]");
    let mut args = vec!["+nightly", "build", "-Z", "codegen-backend"];
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
        save_cache(project_dir, &cache_key);

        let src_binary = format!("{}/target/debug/solar_out", project_dir);
        let dst_binary = output_name.to_owned();
        if let Err(e) = fs::copy(&src_binary, &dst_binary) {
            panic!("Failed to copy binary to {}: {}", &dst_binary, e);
        }
    } else {
        panic!("Cargo compilation failed.");
    }
}
