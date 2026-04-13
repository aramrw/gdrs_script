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
use std::env;
use std::fs;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    let instant = Instant::now();

    if args.len() < 2 {
        eprintln!("Usage: cargo run <file.sr>");
        std::process::exit(1);
    }
    let file_path = &args[1];
    let source_code = fs::read_to_string(file_path).expect("Error reading file");

    // 1. Lexing (Text -> Tokens)
    let tokens = lex(&source_code);
    // println!("Tokens: {:?}", tokens);

    // 2. Parsing (Tokens -> AST)
    match parser().parse(&tokens).into_result() {
        Ok(program) => {
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
        Err(parse_errs) => {
            for err in parse_errs {
                eprintln!("[parse error]: {:?}", err);
            }
            std::process::exit(1);
        }
    }
}
