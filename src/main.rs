mod ast;
mod parser;
mod compiler;

use chumsky::prelude::*;
use std::env; 
use std::fs;
use parser::parser;
use compiler::compile;

fn main() {
    let args: Vec<String> = env::args().collect();

    // 2. Ensure the user passed a file
    if args.len() < 2 {
        eprintln!("Usage: cargo run <file.sr>");
        std::process::exit(1);
    }
    let file_path = &args[1];
    if !file_path.ends_with(".sr") {
        eprintln!("Error: Compiler only accepts files with the '.sr' extension.");
        std::process::exit(1);
    }

    let source_code = match fs::read_to_string(file_path) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            std::process::exit(1);
        }
    };
    println!("[read] {}", file_path);


    // 5. Parse and compile
    match parser().parse(source_code.as_str()).into_result() {
        Ok(ast) => {
            println!("[parsed]");
            compile(ast);
        }
        Err(parse_errs) => {
            for err in parse_errs {
                eprintln!("[parse error]: {}", err);
            }
        }
    }

}
