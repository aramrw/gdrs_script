mod ast;
mod compiler;
mod parser;
mod sema;
mod lexer;

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

fn resolve_module(current_dir: &Path, parts: &[String]) -> Option<PathBuf> {
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

    while let Some(current_file) = queue.pop_front() {
        let source_code = fs::read_to_string(&current_file).expect("Error reading file");
        let tokens = lex(&source_code);

        match parser().parse(&tokens).into_result() {
            Ok(program) => {
                let current_dir = current_file.parent().unwrap();
                let mut deps = Vec::new();
                for decl in &program.declarations {
                    if let Decl::Use(parts) = decl {
                        if let Some(mod_path) = resolve_module(current_dir, parts) {
                            let abs_mod_path = fs::canonicalize(mod_path).unwrap();
                            deps.push((parts.join("::"), abs_mod_path.clone()));
                            if !loaded.contains(&abs_mod_path) {
                                loaded.insert(abs_mod_path.clone());
                                queue.push_back(abs_mod_path);
                            }
                        } else {
                            eprintln!("[module error]: Could not resolve module '{}' from {:?}", parts.join("::"), current_file);
                            std::process::exit(1);
                        }
                    }
                }
                processed.insert(current_file, (program, deps));
            }
            Err(parse_errs) => {
                for err in parse_errs {
                    eprintln!("[parse error in {:?}]: {:?}", current_file, err);
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
            let mut name_to_use = mod_name.clone();
            if name_to_use.starts_with("std::") {
                name_to_use = name_to_use.replace("std::", "");
            }
            let dep_decls = reconstruct(dep_path, processed, visited);
            if !dep_decls.is_empty() {
                decls.push(Decl::Module(name_to_use, dep_decls));
            }
        }
        
        for decl in &program.declarations {
            if !matches!(decl, Decl::Use(_)) {
                decls.push(decl.clone());
            }
        }
        decls
    }

    let all_decls = reconstruct(&root_path, &processed, &mut HashSet::new());
    let program = Program { declarations: all_decls };

    // 3. Semantic Analysis
    let mut sema = SemanticAnalyzer::new();
    if let Err(e) = sema.analyze(&program) {
        eprintln!("[semantic error]: {}", e);
        std::process::exit(1);
    }

    // 4. Compile
    compile(program);
    println!("compiled in {}ms", instant.elapsed().as_millis())
}
