# Solar (SR) Language Project

## Project Overview
Solar (SR) is a high-performance, indentation-based systems programming language designed for speed and modern ergonomics. It features a Rust-inspired type system, including objects, enums, implementation blocks, and mandatory variable initialization.

The project is a compiler for Solar, written in Rust. It follows a traditional compiler pipeline but uses transpilation as its backend strategy.

### Key Technologies
- **Rust**: The implementation language for the compiler.
- **Chumsky**: Used for combinator-based parsing.
- **Quote & Proc-Macro2**: Used for generating Rust code during the transpilation phase.
- **Cargo**: Used both to build the compiler and to compile the generated Rust code.

### Compiler Architecture
1.  **Lexer (`src/lexer.rs`)**: Handles significant whitespace (indentation/dedentation) to support the language's Python-like syntax.
2.  **Parser (`src/parser/mod.rs`)**: Transforms tokens into an Abstract Syntax Tree (AST), supporting complex features like generics, method calls, and pattern matching.
3.  **Semantic Analyzer (`src/sema/mod.rs`)**: Performs type checking, mutability validation, and symbol resolution across modules.
4.  **Compiler/Backend (`src/compiler/mod.rs`)**: Transpiles the AST into Rust code. It automatically creates a temporary Cargo project in the `solar_out/` directory, compiles it, and produces a final `main_program` executable.

## Building and Running

### Prerequisites
- Rust and Cargo installed.

### Build the Compiler
```bash
cargo build
```

### Compile a Solar Program
To compile a `.sr` file:
```bash
cargo run -- <path_to_file.sr>
```
Example:
```bash
cargo run -- main.sr
```

### Run the Compiled Program
The compiler produces an executable named `main_program` in the root directory:
```bash
./main_program
```

## Development Conventions

### Language Syntax
- **Whitespace**: Uses 4-space indentation for blocks (similar to Python or Mojo).
- **Files**: Solar source files use the `.sr` extension.
- **Variables**: Declared with `var`. Use `var mut` for mutable variables.
- **Data Structures**:
    - `obj`: Struct-like data containers.
    - `enum`: Tagged unions (sum types).
    - `impl`: Blocks for defining methods on objects.
- **Modules**: Supports a hierarchical module system using `use` and directory-based resolution (e.g., `std/vec/mod.sr`).

### Project Structure
- `src/`: Rust source code for the compiler.
- `std/`: The Solar standard library.
- `docs/`: Design specifications and development plans.
- `solar_out/`: Temporary build directory for generated Rust code (can be safely ignored or cleaned).

### Note on Backend Discrepancy
The documentation in `docs/superpowers/specs/` currently describes a C++ transpiler backend as a goal. However, the current stable implementation uses a **Rust transpiler backend**. Adhere to the Rust-based generation patterns found in `src/compiler/mod.rs` when extending the language.
