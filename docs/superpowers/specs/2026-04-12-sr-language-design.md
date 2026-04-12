# SR Language Design Specification
**Date:** 2026-04-12
**Status:** Draft (Awaiting User Review)

## 1. Overview
SR is a high-performance systems programming language designed for speed, safety, and modern ergonomics. It features a Rust-inspired ownership model with a C++ transpiler backend, allowing it to leverage the power of RAII and industrial-grade C++ optimizations while maintaining a clean, expression-oriented syntax.

## 2. Goals
- **Performance:** Comparable to C and C++.
- **Safety:** Statically typed with a focus on ownership and memory safety.
- **Developer Experience:** Expression-oriented, clear syntax, and helpful compiler errors.
- **Portability:** Transpiles to standard C++, making it compatible with existing ecosystems and toolchains.

## 3. Syntax and Variables
SR uses a strict, static type system with mandatory initialization.

### 3.1 Variable Declaration
- **Immutable:** `var x: i32 = 10;` (Default state, cannot be changed after initialization).
- **Mutable:** `var mut y: i32 = 20;` (Explicitly marked for mutation).
- **Initialization:** All variables must be initialized at declaration.

### 3.2 Basic Types
- `i32`: 32-bit signed integer.
- `f32`: 32-bit floating-point number.
- `bool`: Boolean (`true` or `false`).
- `void`: Empty type for functions with no return value.

## 4. Data Structures

### 4.1 Objects (`obj`)
Replaces the concept of `struct` with a focus on ownership and data encapsulation.
```rust
obj Player {
    health: i32,
    score: i32,
}
```

### 4.2 Enums
Powerful sum types (tagged unions) that can carry data.
```rust
enum Status {
    Alive,
    Dead,
    Respawning(i32),
}
```

## 5. Expressions and Control Flow
SR is expression-oriented. Almost every block of code returns a value.

### 5.1 Implicit Returns
The last line of a block without a trailing semicolon is treated as the return value of that block.
```rust
var result = {
    var a = 5;
    a + 10 // Returns 15
};
```

### 5.2 `if` Expressions
`if` statements return values, allowing them to be used in assignments.
```rust
var status = if x > 10 {
    1
} else {
    0
};
```

## 6. Functions
Functions use the `fn` keyword and explicit typing for parameters and return values. Parameters follow the variable declaration syntax.

```rust
fn calculate(var x: i32, var mut y: i32) -> i32 {
    y = y + x;
    y
}
```

## 7. Compiler Architecture (The C++ Backend)
The SR compiler is written in Rust and follows a multi-stage pipeline:

1.  **Frontend:** Lexing and Parsing using `chumsky` into an Abstract Syntax Tree (AST).
2.  **Middle-end:** Semantic analysis, Type checking, and Ownership/Borrow tracking.
3.  **Backend:** Transpilation of the AST into modern C++ (C++17 or later).
    - Uses C++ classes and destructors for RAII (automatic cleanup).
    - Maps `obj` to C++ structs/classes.
    - Maps `enum` to C++ `std::variant`.
4.  **Compilation:** Invokes a C++ compiler (e.g., `g++` or `clang++`) to produce the final executable.

## 8. Success Criteria
- Valid `.sr` files compile to working executables.
- Compiler catches type mismatches and mutation errors (e.g., trying to change a non-`mut` `var`).
- Performance of generated binaries is within 5% of hand-written C++.
