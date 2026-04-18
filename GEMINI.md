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
The compiler produces an executable named `sr_<filename>` (e.g., `sr_main` for `main.sr`) in the root directory:
```bash
./sr_main
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

# UPDATES
#### this is where updates are put

### `4/13/26 12:57pm` --

 I have fixed the compilation errors in test_io.sr and addressed underlying reference issues in the Solar language.

  Key Fixes:
   1. Consistent Reference Convention: Updated the compiler to generate Rust code where all Solar function parameters
      are passed by reference. This resolves issues with generic types (like Vector<T>) where primitives and objects
      need to be handled uniformly.
   2. Trait-Based Type Normalization:
       * SolarAsArg: Simplified to a generic identity-to-reference trait, ensuring consistent argument passing.
       * SolarAsVal: Introduced to handle cloning or copying when a value is required (e.g., struct fields, arithmetic).
       * SolarAsSize: Introduced specifically for array indexing and initialization.
   3. Module Resolution & Imports:
       * Updated reconstruct in src/main.rs to strip std:: from module names, aligning them with built-in expectations.
       * Ensured all necessary traits (SolarStr, SolarVec, SolarAsArg, etc.) are imported into every generated Rust
         module.
   4. Standard Library Improvements:
       * std/vec/mod.sr: Updated Vector::new to take a default_item parameter, avoiding invalid zero-initialization of
         non-primitive types like str.
       * mem::free: Updated to take references, maintaining consistency with the new parameter convention.
   5. Memory Safety: Confirmed that Solar's destroy methods are correctly mapped to Rust's Drop trait for automatic
      cleanup, and updated tests to avoid double-free errors.


### `4/13/26 13:02pm`:

Thinking...
 │
 │ Resolving the Compile Issue
 │ I've made headway! The initial compile failure, stemming from ambiguous references in `SolarAsVal` during Rust code
 │ generation, is now resolved. I implemented specialized traits for binary operators, including `SolarSub`, `SolarMul`,
 │ `SolarDiv`, `SolarGT`, and `SolarLT`. The program compiles successfully now, and the output is looking good.
 │
 │ Validating Test Coverage
 │ Success! I've cleared the immediate hurdle. The previous compile ambiguity in `SolarAsVal` is history, thanks to
 │ specialized traits and strategic method call syntax. I'm now turning to quality assurance, ensuring I didn't
 │ introduce regressions. I'm preparing to run the suite of tests specified in `GEMINI.md`, including `test_io.sr`,
 │ `test_math.sr`, and more. Rigorous testing is the next priority.

✦ It works! The SolarAsVal ambiguity is resolved by using specialized traits for binary operators and method call syntax
  for seamless reference handling. I've solved the user's immediate problem, but before finishing, I'll run the other
  tests mentioned in GEMINI.md (test_io.sr, test_math.sr, test_string.sr, test_vec_usage.sr) to ensure no regressions.

### `4/13/26 14:16pm`

man im hella tired:

✦ I have successfully addressed the errors in main.sr and improved the Solar compiler to support general macro calls
  while removing the specific output statements.

  Key Changes
   1. Removed Hardcoded Output Statements:
       * Removed print, log, and logln from the AST, parser, semantic analyzer, and compiler.
       * Ensured that the ! suffix is mandatory for using macros in Solar.
   2. General Macro Support:
       * Updated the parser to support name!(args) as a general macro call expression.
       * Added Expr::MacroCall to the AST and semantic analyzer.
       * Modified the compiler to transpile MacroCall to Rust macro calls (name!(...)).
   3. Refined Macro Transpilation:
       * Implemented specialized transpilation for println! and print! macros, ensuring they include the required format
         strings in Rust.
   4. Updated Tests and Programs:
       * Updated main.sr and test_std_rand.sr to use the println! macro for consistent output.

### `4/14/26 12:45pm`

✦ I have implemented an external crate "trust system" in the Solar compiler, enabling the use of any Rust library without manual `extern` declarations.

  Key Improvements:
   1. Phantom Type System:
       * Modified the `SemanticAnalyzer` to treat `use crate::<crate_name>` as a registration of a "phantom type".
       * Any variable access or function call prefixed with a phantom type (e.g., `prelude::WHITE`, `prelude::next_frame()`) is now treated as `Type::Any`, bypassing strict type checking.
   2. Enhanced Lexer & Attribute Support:
       * Updated the lexer to prevent automatic semicolon insertion after `Token::Attribute`, allowing `#[macroquad::main(...)]` to be used correctly before functions.
       * Added `Token::Any` to the lexer's `to_string` mapping to fix non-exhaustive match errors.
   3. Flexible Control Flow:
       * Updated `if` and `while` statement analysis to use `types_equal` for condition checks. This allows `Type::Any` (returned by phantom function calls like `is_key_down`) to satisfy the requirement for a boolean condition.
   4. Verified macroquad Integration:
       * Successfully compiled and ran a `test_macroquad.sr` program that uses `macroquad` for graphics and input, demonstrating seamless integration with external Rust crates.
