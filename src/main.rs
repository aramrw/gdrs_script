mod ast;
mod compiler;
mod parser;
mod sema;
mod lexer;
mod error;
mod codegen;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use chumsky::prelude::*;
use compiler::compile;
use parser::parser;
use sema::SemanticAnalyzer;
use lexer::lex;
use ast::{Decl, Program};
use std::env;
use std::fs;
use std::time::Instant;
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

fn resolve_module(current_dir: &Path, std_path: Option<&Path>, parts: &[String]) -> Option<PathBuf> {
    if let Some(std) = std_path {
        if parts.first().map(|s| s.as_str()) == Some("std") {
            let mut path = std.to_path_buf();
            for part in &parts[1..] {
                path.push(part);
            }
            let sr_path = path.with_extension("sr");
            if sr_path.exists() { return Some(sr_path); }
            let mod_sr_path = path.join("mod.sr");
            if mod_sr_path.exists() { return Some(mod_sr_path); }
            return None;
        }
    }

    let mut path = current_dir.to_path_buf();
    for part in parts {
        path.push(part);
    }
    
    let sr_path = path.with_extension("sr");
    if sr_path.exists() {
        return Some(sr_path);
    }
    
    let mod_sr_path = path.join("mod.sr");
    if mod_sr_path.exists() {
        return Some(mod_sr_path);
    }
    
    None
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let instant = Instant::now();

    if args.len() < 2 {
        eprintln!("Usage: cargo run <file.sr>");
        std::process::exit(1);
    }
    
    let file_path = &args[1];
    let mut loaded = HashSet::new();
    let mut queue = VecDeque::new();
    let mut processed = std::collections::HashMap::new();
    
    let root_path = fs::canonicalize(file_path).expect("Invalid file path");
    queue.push_back(root_path.clone());
    loaded.insert(root_path.clone());

    let cwd = env::current_dir().unwrap();
    let std_path = cwd.join("std");
    let std_path = if std_path.exists() { Some(std_path) } else { None };

    while let Some(current_file) = queue.pop_front() {
        let source_code = fs::read_to_string(&current_file).expect("Error reading file");
        let tokens = match lex(&source_code) {
            Ok(t) => t,
            Err(e) => {
                use miette::Report;
                let report = Report::from(e).with_source_code(source_code);
                eprintln!("{:?}", report);
                std::process::exit(1);
            }
        };

        let end_span = SimpleSpan::new((), source_code.len()..source_code.len());
        let input = tokens.as_slice().split_token_span(end_span);

        match parser().parse(input).into_result() {
            Ok(program) => {
                let current_dir = current_file.parent().unwrap();
                let mut deps = Vec::new();
                for decl in &program.declarations {
                    if let Decl::Use(u) = decl {
                        if u.is_crate {
                            continue;
                        }
                        if let Some(mod_path) = resolve_module(current_dir, std_path.as_deref(), &u.path) {
                            let abs_mod_path = fs::canonicalize(mod_path).unwrap();
                            let mod_name = u.path.join("::");
                            deps.push((mod_name, abs_mod_path.clone()));
                            if !loaded.contains(&abs_mod_path) {
                                loaded.insert(abs_mod_path.clone());
                                queue.push_back(abs_mod_path);
                            }
                        } else {
                            let mod_name = u.path.join("::");
                            eprintln!("[module error]: Could not resolve module '{}' from {:?}", mod_name, current_file);
                            std::process::exit(1);
                        }
                    }
                }
                processed.insert(current_file, (program, deps));
            }
            Err(parse_errs) => {
                use miette::Report;
                use crate::error::CompilerError;
                for err in parse_errs {
                    let report = Report::from(CompilerError::from_rich(err)).with_source_code(source_code.clone());
                    eprintln!("{:?}", report);
                }
                std::process::exit(1);
            }
        }
    }

    // Reconstruction: Start from root, and wrap each dependency in Decl::Module
    fn reconstruct(
        path: &PathBuf, 
        processed: &std::collections::HashMap<PathBuf, (Program, Vec<(String, PathBuf)>)>,
        visited: &mut HashSet<PathBuf>
    ) -> Vec<Decl> {
        if visited.contains(path) { return Vec::new(); }
        visited.insert(path.clone());

        let (program, deps) = processed.get(path).unwrap();
        let mut decls = Vec::new();
        
        for (mod_name, dep_path) in deps {
            let name_to_use = mod_name.clone();
            let dep_decls = reconstruct(dep_path, processed, visited);
            if !dep_decls.is_empty() {
                // Handle nested modules like std::vec -> Module("std", [Module("vec", ...)])
                let parts: Vec<&str> = name_to_use.split("::").collect();
                let mut current_module_decls = dep_decls;
                for i in (0..parts.len()).rev() {
                    current_module_decls = vec![Decl::Module(parts[i].to_string(), current_module_decls)];
                }
                decls.extend(current_module_decls);
            }
        }
        
        for decl in &program.declarations {
            if let Decl::Use(u) = decl {
                if u.is_crate {
                    decls.push(decl.clone());
                }
            } else if !matches!(decl, Decl::Module(_, _)) {
                decls.push(decl.clone());
            }
        }
        decls
    }

    let all_decls = reconstruct(&root_path, &processed, &mut HashSet::new());
    let mut program = Program { declarations: all_decls };

    // 3. Semantic Analysis
    let mut sema = SemanticAnalyzer::new();
    if let Err(e) = sema.analyze(&mut program) {
        use miette::Report;
        let report = Report::from(e);
        eprintln!("{:?}", report);
        std::process::exit(1);
    }

    // 4. Compile
    let output_name = format!("sr_{}", root_path.file_stem().unwrap().to_str().unwrap());
    compile(program, &output_name);
    println!("compiled in {}ms", instant.elapsed().as_millis())
}
