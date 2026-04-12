use chumsky::prelude::*;
use std::env; // Added for command line arguments
use std::fs;
use std::process::Command;
use crate::ast::*;

// ==========================================
// 3. The Compiler (Fixed C Generation)
// ==========================================
pub fn compile(func: Function) {
    let mut c_code = String::new();
    c_code.push_str("#include <stdio.h>\n#include <stdint.h>\n\n");

    // 1. Give the user's function a safe name so it never conflicts with C's real 'main'
    let safe_name = format!("user_{}", func.name);
    c_code.push_str(&format!("int {}() {{\n", safe_name));

    for stmt in func.body {
        match stmt {
            Stmt::Print(Expr::Int(val)) => {
                // 2. Append 'LL' to the integer so G++ knows it is a 64-bit integer
                c_code.push_str(&format!("    printf(\"%lld\\n\", {}LL);\n", val));
            }
        }
    }

    c_code.push_str("    return 0;\n}\n");

    // 3. If they wrote 'fn main()', automatically generate the C entry point to call it
    if func.name == "main" {
        c_code.push_str(&format!("\nint main() {{\n    return {}();\n}}\n", safe_name));
    }

    fs::write("output.cpp", &c_code).expect("Failed to write C++ code");

    let status = Command::new("g++")
        .args(&["-O3", "output.cpp", "-o", "main_program"])
        .status()
        .expect("Failed to invoke g++. Is it installed?");

    if status.success() {
        println!("[compiled]");
    } else {
        eprintln!("compilation failed.");
    }
}
