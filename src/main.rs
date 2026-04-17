use lang::run_compiler;
use std::env;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: cargo run <file.sr>");
        std::process::exit(1);
    }

    let file_path = &args[1];
    if let Err(e) = run_compiler(file_path) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
