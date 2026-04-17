pub mod types;
pub mod paths;
pub mod declarations;
pub mod statements;
pub mod expressions;
pub mod functions;

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

pub(crate) fn compile_id_ext(name: &str, is_expr: bool) -> TokenStream {
    let name = name.trim();
    if name.is_empty() {
        return quote!();
    }

    // 1. Resolve nested generics safely
    if let Some((prefix, gens_str, suffix)) = extract_brackets(name, '<', '>') {
        let id = compile_id_ext(prefix, is_expr);
        let gens_tokens = parse_generic_args(gens_str);

        let sep = if is_expr && !gens_tokens.is_empty() {
            quote!(::)
        } else {
            quote!()
        };

        let mut res = if gens_tokens.is_empty() {
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
    let root_modules = ["mem", "sr_fs", "sr_io", "sr_math", "util", "vec", "option"];

    // 4. Resolve namespaces (::)
    if name.contains("::") {
        let parts: Vec<&str> = name.split("::").collect();
        let first = parts[0];
        let rest_tokens = crate::compiler::paths::format_path_parts(&parts[1..]);

        match first {
            // ZERO-COST ABSTRACTIONS: Direct access to Rust's std library!
            "std" | "rust" => quote!(::std::#( #rest_tokens )::*),

            // External crates via `crate::`
            "crate" => quote!(::#( #rest_tokens )::*),

            // Solar standard library mapped directly to the active crate
            "solar" => quote!(crate::#( #rest_tokens )::*),

            // If it hits one of Solar's root modules, enforce the `crate::` prefix
            _ if root_modules.contains(&first) => {
                let first_id = quote::format_ident!("{}", first);
                quote!(crate::#first_id::#( #rest_tokens )::*)
            }

            // Standard namespace resolution
            _ => {
                let first_id = quote::format_ident!("{}", first);
                quote!(#first_id::#( #rest_tokens )::*)
            }
        }
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

fn compile_path(path: &[PathPart], target_obj: Option<&String>) -> TokenStream {
    let parts: Vec<_> = path
        .iter()
        .filter(|p| !p.name.is_empty())
        .map(|p| {
            let name = quote::format_ident!("{}", p.name);
            if p.generics.is_empty() {
                quote!(#name)
            } else {
                let gens = p.generics.iter().map(|g| compile_type_ext(g, target_obj));
                quote!(#name::<#( #gens ),*>)
            }
        })
        .collect();
    quote!(#( #parts )::*)
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
                if mutable {
                    quote! { &mut *#tokens.borrow_mut() }
                } else {
                    quote! { &*#tokens.borrow() }
                }
            }
            Type::ThreadSafe(_) => {
                if mutable {
                    quote! { &mut *#tokens.write() }
                } else {
                    quote! { &*#tokens.read() }
                }
            }
            Type::Ref(_, _) => tokens, // Already a ref
            _ => {
                if mutable {
                    quote! { &mut #tokens }
                } else {
                    quote! { &#tokens }
                }
            }
        }
    } else {
        if mutable {
            quote! { &mut #tokens }
        } else {
            quote! { &#tokens }
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



pub fn compile(program: Program, output_name: &str) {
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
    tokens.extend(generate_solar_std());

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

    println!("+=[Cargo]");
    let mut args = vec!["build", "--message-format=short"];
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
        let src_binary = format!("{}/target/debug/solar_out", project_dir);
        let dst_binary = output_name;
        if let Err(e) = fs::copy(&src_binary, dst_binary) {
            eprintln!("Failed to copy binary to {}: {}", dst_binary, e);
        }
    } else {
        eprintln!("Cargo compilation failed.");
    }
}

